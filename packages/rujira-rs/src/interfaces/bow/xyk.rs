use crate::bow::error::StrategyError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coin, ensure, from_json, to_json_binary, Coin, Decimal, Decimal256, Deps, DepsMut, Env,
    Fraction, Isqrt, StdResult, Uint128, Uint256,
};
use cw_storage_plus::Item;
use cw_utils::NativeBalance;
use std::{
    cmp::min,
    ops::{Add, Div, Mul, Sub},
};

use super::{strategy::Strategy, QuoteRequest, QuoteResponse};

static STORE: Item<(Uint128, Uint128, Uint128)> = Item::new("strategy-xyk");

#[cw_serde]
pub struct Xyk {
    x: String,
    y: String,
    // The % deployed in each request
    step: Decimal,
    // The minimum number that X and Y must meet in order to quote a price
    min_quote: Uint128,
    // The fee that's charged on each quote and required to be paid
    // in `validate` function
    fee: Decimal,
}

impl Xyk {
    pub const MIN_FEE: u64 = 0u64;
    pub const MAX_FEE: u64 = 1000u64;
    pub const MIN_STEP: u64 = 1u64;
    pub const MAX_STEP: u64 = 1000u64;
    pub const MIN_MIN_QUOTE: u64 = 1000u64;

    pub fn new(x: String, y: String, step: Decimal, min_quote: Uint128, fee: Decimal) -> Self {
        Self {
            x,
            y,
            step,
            min_quote,
            fee,
        }
    }
}

#[cw_serde]
pub struct XykState {
    x: Uint128,
    y: Uint128,
    k: Uint256,
    pub(crate) shares: Uint128,
}

impl Default for XykState {
    fn default() -> Self {
        Self::new()
    }
}

impl XykState {
    pub fn new() -> Self {
        Self {
            x: Uint128::zero(),
            y: Uint128::zero(),
            k: Uint256::zero(),
            shares: Uint128::zero(),
        }
    }

    pub fn value(&self) -> Uint256 {
        self.k
    }

    /// Swaps an offer_size of X and returns (Amount, Price) of Y
    pub fn swap(&mut self, amount: &Uint128) -> Result<Uint128, StrategyError> {
        if self.x.is_zero() {
            return Ok(Uint128::zero());
        }
        let k = self.k;
        self.x = self.x.add(amount);
        let y_cur = self.y;
        let y_new: Uint128 = Decimal256::from_ratio(self.k, self.x)
            .to_uint_ceil()
            .try_into()?;
        // We used to deduct fees here, however:
        // The way the fees compund makes the pool smaller in each iteration,
        // reducing the fees collected in subseuqent actions
        // We should swap at vanilla XYK, and take fees at the edges
        let return_amount = y_cur.sub(y_new);
        self.y = y_cur.sub(return_amount);
        self.k = Uint256::from(self.x) * Uint256::from(self.y);
        ensure!(self.k >= k, StrategyError::Underflow {});
        Ok(return_amount)
    }

    pub fn price(&self) -> Decimal {
        if self.x.is_zero() {
            return Decimal::zero();
        }
        Decimal::from_ratio(self.y, self.x)
    }

    pub fn invert(&mut self) {
        let cloned = self.clone();
        self.x = cloned.y;
        self.y = cloned.x;
    }

    pub fn set(&mut self, x: Uint128, y: Uint128) {
        self.x = x;
        self.y = y;
        self.k = Uint256::from(self.x) * Uint256::from(self.y);
    }
}

impl From<(Uint128, Uint128, Uint128)> for XykState {
    fn from((x, y, shares): (Uint128, Uint128, Uint128)) -> Self {
        XykState {
            x,
            y,
            k: Uint256::from(x) * Uint256::from(y),
            shares,
        }
    }
}

impl Strategy<XykState> for Xyk {
    fn validate(&self) -> Result<(), StrategyError> {
        if self.step > Decimal::bps(Self::MAX_STEP) || self.step < Decimal::bps(Self::MIN_STEP) {
            return Err(StrategyError::InvalidConfig("step".into()));
        }
        if self.fee > Decimal::bps(Self::MAX_FEE) || self.fee < Decimal::bps(Self::MIN_FEE) {
            return Err(StrategyError::InvalidConfig("fee".into()));
        }
        if self.min_quote < Uint128::from(Self::MIN_MIN_QUOTE) {
            return Err(StrategyError::InvalidConfig("min_quote".into()));
        }
        Ok(())
    }

    fn load_state(&self, deps: Deps, _env: Env) -> StdResult<XykState> {
        let stored = STORE.load(deps.storage).unwrap_or_default();
        Ok(XykState::from(stored))
    }

    fn commit_state(&self, deps: DepsMut, state: &XykState) -> StdResult<()> {
        STORE.save(deps.storage, &(state.x, state.y, state.shares))
    }

    fn denom(&self) -> String {
        format!("bow-xyk-{}-{}", self.x, self.y)
    }

    fn validate_swap(
        &self,
        state: &mut XykState,
        offer: Coin,
        ask: Coin,
    ) -> Result<(Coin, Coin), StrategyError> {
        // The state passed here will be the result of load_state.
        // Orientate it for the quote
        if offer.denom == self.y {
            state.invert()
        }

        let return_amount_total = state.swap(&offer.amount)?;
        let fee_amount =
            return_amount_total.multiply_ratio(self.fee.numerator(), self.fee.denominator());
        let return_amount = return_amount_total.sub(fee_amount);
        ensure!(
            return_amount >= ask.amount,
            StrategyError::InsufficientReturn {
                expected: ask.clone(),
                returned: coin(return_amount.u128(), ask.denom),
            }
        );

        // Switch back before commitment if needed
        if offer.denom == self.y {
            state.invert()
        }

        Ok((
            // Fee amount
            coin(fee_amount.u128(), ask.denom.clone()),
            // Surplus retained
            coin(
                return_amount_total.sub(ask.amount).sub(fee_amount).u128(),
                ask.denom,
            ),
        ))
    }

    fn quote(
        &self,
        state: &XykState,
        req: QuoteRequest,
    ) -> Result<Option<QuoteResponse>, StrategyError> {
        // Determine which state to work with
        let mut working_state = if let Some(data) = req.data {
            // Attempt to deserialize state from the data blob
            from_json::<XykState>(&data)?
        } else {
            // Clone and possibly invert the current state
            let mut s = state.clone();
            if req.offer_denom == self.y {
                s.invert();
            }
            s
        };

        let amount = working_state
            .x
            .multiply_ratio(self.step.numerator(), self.step.denominator());

        let current_price = working_state.price();
        let ask_size_total = working_state.swap(&amount)?;

        if amount.lt(&self.min_quote) || ask_size_total.lt(&self.min_quote) {
            return Ok(None);
        }

        let fee_amount = ask_size_total
            .multiply_ratio(self.fee.numerator(), self.fee.denominator())
            // See note in `XykState::swap` regards fees
            // Truncation here can cause insufficient return expectations from multiple quotes
            // Eg ask_size_total of 1046 and 1021 with fee amount of 102 bps = 10 + 10
            // Where a single 102bps fee on 2047 = 21
            // Therefore we artificially ceil this value to ensure the quotes are within xyk + fees
            .add(Uint128::one());
        let ask_size = ask_size_total.sub(fee_amount);
        let price = Decimal::from_ratio(ask_size, amount);
        if price.ge(&current_price) || price.is_zero() {
            return Ok(None);
        }

        Ok(Some(QuoteResponse {
            price,
            size: ask_size,
            data: Some(to_json_binary(&working_state)?),
        }))
    }

    fn deposit(
        &self,
        state: &mut XykState,
        funds: NativeBalance,
    ) -> Result<Uint128, StrategyError> {
        let x = balance_of(&funds, &self.x);
        let y = balance_of(&funds, &self.y);

        let minted = if state.shares.is_zero() {
            if x.is_zero() && y.is_zero() {
                return Err(StrategyError::InvalidDeposit {});
            }
            Uint256::from(x).mul(Uint256::from(y)).isqrt()
        } else {
            // Issue shares based on the change in sqrt(k),
            // and charge a swap fee on the excess above a balanced deposit
            let balanced_shares = min(
                Uint256::from(x).multiply_ratio(state.shares, state.x),
                Uint256::from(y).multiply_ratio(state.shares, state.y),
            );

            let x = Uint256::from(x);
            let y = Uint256::from(y);
            let sqrt_k = state.k.isqrt();
            let delta_sqrt_k = x
                .checked_add(state.x.into())?
                .checked_mul(y.checked_add(state.y.into())?)?
                .isqrt()
                .checked_sub(sqrt_k)?;

            let shares = delta_sqrt_k.multiply_ratio(state.shares, sqrt_k);
            let fee = if shares.gt(&balanced_shares) {
                shares
                    .sub(balanced_shares)
                    .multiply_ratio(self.fee.numerator(), self.fee.denominator())
                    .div(Uint256::from(2u128))
            } else {
                Uint256::zero()
            };

            shares.sub(fee)
        }
        .try_into()?;

        state.set(state.x.add(x), state.y.add(y));
        state.shares += minted;
        Ok(minted)
    }

    fn withdraw(
        &self,
        state: &mut XykState,
        amount: Uint128,
    ) -> Result<NativeBalance, StrategyError> {
        if state.shares.is_zero() || amount.is_zero() {
            return Ok(NativeBalance::default());
        }
        if state.shares == amount {
            let mut balances = NativeBalance(vec![
                coin(state.x.u128(), self.x.as_str()),
                coin(state.y.u128(), self.y.as_str()),
            ]);
            balances.normalize();
            return Ok(balances);
        }

        let x = state.x.multiply_ratio(amount, state.shares);
        let y = state.y.multiply_ratio(amount, state.shares);
        let mut balances = NativeBalance(vec![
            coin(x.u128(), self.x.as_str()),
            coin(y.u128(), self.y.as_str()),
        ]);
        state.set(state.x - x, state.y - y);
        state.shares -= amount;
        balances.normalize();

        Ok(balances)
    }
}

fn balance_of(balance: &NativeBalance, denom: &String) -> Uint128 {
    balance
        .clone()
        .into_vec()
        .iter()
        .enumerate()
        .find(|(_i, c)| c.denom == *denom)
        .map(|x| x.1.amount)
        .unwrap_or_default()
}

#[cfg(test)]
mod test {
    use cosmwasm_std::Binary;
    use proptest::prelude::*;
    use std::str::FromStr;

    use crate::bow::Strategy;

    use super::*;

    #[test]
    fn test_validate() {
        let xyk = Xyk {
            x: "x".to_string(),
            y: "y".to_string(),
            min_quote: Uint128::zero(),
            step: Decimal::zero(),
            fee: Decimal::from_str("0.2").unwrap(),
        };
        let mut state = XykState::new();
        xyk.deposit(
            &mut state,
            NativeBalance(vec![coin(1000, "x"), coin(2000, "y")]),
        )
        .unwrap();
        let fee = xyk
            .validate_swap(&mut state, coin(50, "x"), coin(76, "y"))
            .unwrap();
        assert_eq!(fee, (coin(19, "y"), coin(0, "y")));

        let mut state = XykState::new();
        xyk.deposit(
            &mut state,
            NativeBalance(vec![coin(1000, "x"), coin(2000, "y")]),
        )
        .unwrap();
        xyk.validate_swap(&mut state, coin(50, "x"), coin(96, "y"))
            .unwrap_err();

        let mut state = XykState::new();
        xyk.deposit(
            &mut state,
            NativeBalance(vec![coin(1000, "x"), coin(2000, "y")]),
        )
        .unwrap();
        let fee = xyk
            .validate_swap(&mut state, coin(100, "y"), coin(35, "x"))
            .unwrap();
        assert_eq!(fee, (coin(9, "x"), coin(3, "x")));

        let mut state = XykState::new();
        xyk.deposit(
            &mut state,
            NativeBalance(vec![coin(1000, "x"), coin(2000, "y")]),
        )
        .unwrap();
        xyk.validate_swap(&mut state, coin(100, "y"), coin(48, "x"))
            .unwrap_err();
    }

    #[test]
    fn test_deposit() {
        let xyk = Xyk {
            x: "x".to_string(),
            y: "y".to_string(),
            min_quote: Uint128::zero(),
            step: Decimal::zero(),
            fee: Decimal::from_ratio(1u128, 10u128),
        };

        // Initial deposit. Share = sqrt(k)
        let mut state = XykState::new();
        let amount = xyk
            .deposit(
                &mut state,
                NativeBalance(vec![coin(1000, "x"), coin(2000, "y")]),
            )
            .unwrap();

        assert_eq!(amount, Uint128::from(1414u128));
        assert_eq!(state.x, Uint128::from(1000u128));
        assert_eq!(state.y, Uint128::from(2000u128));

        let amount = xyk
            .deposit(
                &mut state,
                NativeBalance(vec![coin(500, "x"), coin(1000, "y")]),
            )
            .unwrap();
        assert_eq!(amount, Uint128::from(707u128));
        assert_eq!(state.x, Uint128::from(1500u128));
        assert_eq!(state.y, Uint128::from(3000u128));

        // Adding more than 50% on one side
        let mut state = XykState::new();
        let amount = xyk
            .deposit(
                &mut state,
                NativeBalance(vec![coin(500, "x"), coin(2000, "y")]),
            )
            .unwrap();
        assert_eq!(amount, Uint128::from(1000u128));
        assert_eq!(state.x, Uint128::from(500u128));
        assert_eq!(state.y, Uint128::from(2000u128));

        // Change in sqrt(k) = 1000 -> 2738 = 1738;
        // Min even allocation (250:1000) = 500
        // Chargeable increase = 1738 - 500 = 1238
        // 10% Fee "taken from 1 side" = 61
        // New shares = 1677
        let amount = xyk
            .deposit(
                &mut state,
                NativeBalance(vec![coin(2000, "x"), coin(1000, "y")]),
            )
            .unwrap();
        assert_eq!(amount, Uint128::from(1677u128));
        assert_eq!(state.x, Uint128::from(2500u128));
        assert_eq!(state.y, Uint128::from(3000u128));

        // Single sided deposit
        let mut state = XykState::new();
        let amount = xyk
            .deposit(
                &mut state,
                NativeBalance(vec![coin(500, "x"), coin(2000, "y")]),
            )
            .unwrap();
        assert_eq!(amount, Uint128::from(1000u128));
        assert_eq!(state.x, Uint128::from(500u128));
        assert_eq!(state.y, Uint128::from(2000u128));

        let amount = xyk
            .deposit(&mut state, NativeBalance(vec![coin(1000, "y")]))
            .unwrap();
        assert_eq!(amount, Uint128::from(213u128));
        assert_eq!(state.x, Uint128::from(500u128));
        assert_eq!(state.y, Uint128::from(3000u128));
    }

    #[test]
    fn test_withdraw() {
        let xyk = Xyk {
            x: "x".to_string(),
            y: "y".to_string(),
            min_quote: Uint128::zero(),
            step: Decimal::zero(),
            fee: Decimal::zero(),
        };

        // Initial deposit. Share = sqrt(k)
        let mut state = XykState::new();

        let amount = xyk.withdraw(&mut state, Uint128::zero()).unwrap();
        assert_eq!(amount, NativeBalance::default());

        let amount = xyk.withdraw(&mut state, Uint128::from(1000u128)).unwrap();
        assert_eq!(amount, NativeBalance::default());

        let shares = xyk
            .deposit(
                &mut state,
                NativeBalance(vec![coin(250, "x"), coin(500, "y")]),
            )
            .unwrap();
        assert_eq!(shares, Uint128::from(353u128));
        let amount = xyk.withdraw(&mut state, Uint128::from(176u128)).unwrap();
        assert_eq!(amount, NativeBalance(vec![coin(124, "x"), coin(249, "y")]));
        let amount = xyk.withdraw(&mut state, Uint128::from(177u128)).unwrap();
        assert_eq!(amount, NativeBalance(vec![coin(126, "x"), coin(251, "y")]));
    }

    #[test]
    fn pool_state_swaps() {
        let xyk = Xyk {
            x: "x".to_string(),
            y: "y".to_string(),
            min_quote: Uint128::zero(),
            step: Decimal::zero(),
            fee: Decimal::zero(),
        };

        let mut state = XykState::new();
        xyk.deposit(
            &mut state,
            NativeBalance(vec![coin(10000, "x"), coin(20000, "y")]),
        )
        .unwrap();

        let returned = state.swap(&Uint128::from(100u128)).unwrap();
        assert_eq!(returned, Uint128::from(198u128));
        assert_eq!(state.x, Uint128::from(10_100u128));
        assert_eq!(state.y, Uint128::from(19_802u128));
        assert_eq!(state.k, Uint256::from(200_000_200u128));

        let mut state = XykState::new();
        xyk.deposit(
            &mut state,
            NativeBalance(vec![coin(20000, "x"), coin(10000, "y")]),
        )
        .unwrap();

        let returned = state.swap(&Uint128::from(100u128)).unwrap();
        assert_eq!(returned, Uint128::from(49u128));
        assert_eq!(state.x, Uint128::from(20_100u128));
        assert_eq!(state.y, Uint128::from(9_951u128));
        assert_eq!(state.k, Uint256::from(200_015_100u128));
    }

    #[test]
    fn test_quote() {
        let xyk = Xyk {
            x: "x".to_string(),
            y: "y".to_string(),
            min_quote: Uint128::from(10_000u128),
            step: Decimal::from_ratio(1u128, 1000u128),
            fee: Decimal::zero(),
        };

        let mut state = XykState::new();

        // Check no quote returned when pool is empty
        let quote = xyk
            .quote(
                &state,
                QuoteRequest {
                    min_price: None,
                    offer_denom: "x".to_string(),
                    ask_denom: "y".to_string(),
                    data: None,
                },
            )
            .unwrap();
        assert_eq!(quote, None);

        xyk.deposit(
            &mut state,
            NativeBalance(vec![coin(1000, "x"), coin(2000, "y")]),
        )
        .unwrap();

        // Test with low balances
        let quote = xyk
            .quote(
                &state,
                QuoteRequest {
                    min_price: None,
                    offer_denom: "x".to_string(),
                    ask_denom: "y".to_string(),
                    data: None,
                },
            )
            .unwrap();
        assert_eq!(quote, None);

        xyk.deposit(
            &mut state,
            NativeBalance(vec![coin(1_000_000_000, "x"), coin(2_000_000_000, "y")]),
        )
        .unwrap();

        // Test initial price
        let quote = xyk
            .quote(
                &state,
                QuoteRequest {
                    min_price: None,
                    offer_denom: "x".to_string(),
                    ask_denom: "y".to_string(),
                    data: None,
                },
            )
            .unwrap()
            .unwrap();
        // Request is to sell X for Y. Price should be lower than base price (2/1)
        assert_eq!(quote.price, Decimal::from_str("1.998000001999998").unwrap());
        assert_eq!(quote.size, Uint128::from(1_998_002u128));
        let data: XykState = from_json(quote.data.clone().unwrap()).unwrap();
        assert_eq!(data.x, Uint128::from(1_001_001_001u128));
        assert_eq!(data.y, Uint128::from(1_998_003_997u128));
        assert_eq!(data.k, Uint256::from(2_000_004_000_999_000_997u128));

        // Test subsequent quote
        let quote = xyk
            .quote(
                &state,
                QuoteRequest {
                    // Request next tick
                    min_price: Some(Decimal::from_str("1.9979").unwrap()),
                    offer_denom: "x".to_string(),
                    ask_denom: "y".to_string(),
                    data: quote.data,
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            quote.price,
            Decimal::from_str("1.994009995994009995").unwrap()
        );
        assert_eq!(quote.size, Uint128::from(1_996_006u128));
        let data: XykState = from_json(quote.data.unwrap()).unwrap();
        assert_eq!(data.x, Uint128::from(1_002_002_002u128));
        assert_eq!(data.y, Uint128::from(1_996_007_990u128));
        assert_eq!(data.k, Uint256::from(2_000_004_001_987_995_980u128));

        // Test inverse
        let mut state = XykState::new();
        xyk.deposit(
            &mut state,
            NativeBalance(vec![coin(1_000_000_000, "x"), coin(2_000_000_000, "y")]),
        )
        .unwrap();

        let quote = xyk
            .quote(
                &state,
                QuoteRequest {
                    min_price: None,
                    offer_denom: "y".to_string(),
                    ask_denom: "x".to_string(),
                    data: None,
                },
            )
            .unwrap()
            .unwrap();
        // Check direction, poolstate tested above
        assert_eq!(quote.price, Decimal::from_str("0.4994995").unwrap());
        assert_eq!(quote.size, Uint128::from(998_999u128));

        // Test subsequent quote
        let quote = xyk
            .quote(
                &state,
                QuoteRequest {
                    // Request next tick
                    min_price: Some(Decimal::from_str("0.4994").unwrap()),
                    offer_denom: "y".to_string(),
                    ask_denom: "x".to_string(),
                    data: quote.data,
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            quote.price,
            Decimal::from_str("0.498501998001998001").unwrap()
        );
        assert_eq!(quote.size, Uint128::from(998_001u128));
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 10000,
            ..Default::default()
        })]
        #[test]
        fn test_k_always_increases(
            deposit_x in 1u128..100_000,
            deposit_y in 1u128..100_000,
            step in 1u64..10_000,
            fee in 0u64..1000
        ) {
            let mut state = XykState::new();
            let pool = Xyk::new("x".into(), "y".into(), Decimal::permille(step), Uint128::zero(), Decimal::permille(fee));

            let deposit = NativeBalance(vec![
                coin(deposit_x, "x"),
                coin(deposit_y, "y")
            ]);

            let result = pool.deposit(&mut state, deposit.clone());

            prop_assert!(result.is_ok());
            let new_k = Uint256::from(state.x) * Uint256::from(state.y);
            prop_assert!(new_k >= state.k);
        }

        #[test]
        fn test_fee_is_captured_in_k_growth(
            x in 1u128..1_000_000,
            y in 1u128..1_000_000,
            swap_amount in 1u128..10_000,
            fee in 1u64..1000
        ) {
            let mut state = XykState::new();

            let pool = Xyk::new("x".into(), "y".into(), Decimal::percent(10), Uint128::zero(), Decimal::permille(fee));
            pool.deposit(&mut state, NativeBalance(vec![     coin(x, "x"),coin(y, "y")
            ])).unwrap();
            let v1 = state.value();
            state.swap(&swap_amount.into()).unwrap();
            let v2 = state.value();

            prop_assert!(v2 >= v1, "value should increase after a swap");
        }

        #[test]
        fn test_total_share_issuance_equals_deposit_return(
            x1 in 1u128..1_000_000,
            y1 in 1u128..1_000_000,
            x2 in 1u128..100_000,
            y2 in 1u128..100_000
        ) {
            let mut state = XykState::new();

            let pool = Xyk::new("x".into(), "y".into(), Decimal::percent(1), Uint128::zero(), Decimal::zero());
            let deposit = NativeBalance(vec![
                coin(x1, "x"),
                coin(y1, "y")
            ]);
            let s1 = pool.deposit(&mut state, deposit.clone()).unwrap();


            let deposit = NativeBalance(vec![
                coin(x2, "x"),
                coin(y2, "y")
            ]);

            let s2 = pool.deposit(&mut state, deposit.clone()).unwrap();
            prop_assert_eq!(s1.add(s2), state.shares);
        }

        #[test]
        fn test_quote_size_at_price_matches_swap_validity(
            x in 1u128..1_000_000,
            y in 1u128..1_000_000,
            fee in Xyk::MIN_FEE..Xyk::MAX_FEE,
            step in Xyk::MIN_STEP..Xyk::MAX_STEP,
            quotes in 1usize..1000
        ) {
            let mut state = XykState {
                x: Uint128::new(x),
                y: Uint128::new(y),
                k: Uint256::from(x) * Uint256::from(y),
                shares: Uint128::zero(),
            };

            let pool = Xyk::new(
                "x".into(),
                "y".into(),
                Decimal::bps(step),
                Uint128::from(Xyk::MIN_MIN_QUOTE),
                Decimal::bps(fee)
            );

            let mut total_offer = Uint128::zero();
            let mut total_ask = Uint128::zero();

            let mut price = None;
            let mut data: Option<Binary> = None;

            for _ in 0..quotes {
                let req = QuoteRequest {
                    min_price: price,
                    offer_denom: "x".to_string(),
                    ask_denom: "y".to_string(),
                    data,
                };

                if let Ok(Some(res)) = pool.quote(&state, req.clone()) {
                    total_offer += res
                        .size
                        .multiply_ratio(res.price.denominator(), res.price.numerator());
                    total_ask += res.size;
                    price = Some(res.price);
                    data = res.data;
                } else {
                    break;
                }
            }

            let offer = Coin::new(total_offer.u128(), "x");
            let ask = Coin::new(total_ask.u128(), "y");
            let valid = pool.validate_swap(&mut state, offer.clone(), ask.clone());

            prop_assert!(valid.is_ok(), "total quote should represent a valid swap");
        }
    }
}
