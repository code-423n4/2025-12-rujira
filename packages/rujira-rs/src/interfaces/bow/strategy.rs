use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, Deps, DepsMut, Env, StdResult, Uint128};
use cw_utils::NativeBalance;

use super::{
    error::StrategyError,
    xyk::{Xyk, XykState},
    QuoteRequest, QuoteResponse,
};

pub trait Strategy<T> {
    fn validate(&self) -> Result<(), StrategyError>;

    /// Manages the strategy's state
    fn load_state(&self, deps: Deps, env: Env) -> StdResult<T>;
    fn commit_state(&self, deps: DepsMut, state: &T) -> StdResult<()>;

    /// The receipt token denom string for the strategy
    fn denom(&self) -> String;
    /// Validates a swap size against the strategy
    /// Offer is the amount offered _to_ the strategy (ie increase in local balance)
    /// and Ask is amount requested _from_ the strategy (decrease)
    /// Returns the fee charged for the swap, and any surplus retained
    fn validate_swap(
        &self,
        state: &mut T,
        offer: Coin,
        ask: Coin,
    ) -> Result<(Coin, Coin), StrategyError>;

    /// Quotes for a FIN market maker request
    fn quote(&self, state: &T, req: QuoteRequest) -> Result<Option<QuoteResponse>, StrategyError>;

    /// Deposits the funds in message.info to the strategy, returning
    /// the amount of shares that it has earned
    fn deposit(&self, state: &mut T, funds: NativeBalance) -> Result<Uint128, StrategyError>;

    /// Withdraws the `amount` of shares from the strategy, returning
    /// the amount of underlying assets to be repaid
    fn withdraw(&self, state: &mut T, amount: Uint128) -> Result<NativeBalance, StrategyError>;
}

#[cw_serde]
pub enum Strategies {
    Xyk(Xyk),
}

#[cw_serde]
pub enum StrategyState {
    Xyk(XykState),
}

impl Strategy<StrategyState> for Strategies {
    fn validate(&self) -> Result<(), StrategyError> {
        match self {
            Strategies::Xyk(x) => x.validate(),
        }
    }

    fn denom(&self) -> String {
        match self {
            Strategies::Xyk(x) => x.denom(),
        }
    }

    fn load_state(&self, deps: Deps, env: Env) -> StdResult<StrategyState> {
        match self {
            Strategies::Xyk(x) => x.load_state(deps, env).map(StrategyState::Xyk),
        }
    }

    fn commit_state(&self, deps: DepsMut, state: &StrategyState) -> StdResult<()> {
        match (self, state) {
            (Strategies::Xyk(x), StrategyState::Xyk(s)) => Ok(x.commit_state(deps, s)?),
        }
    }

    fn validate_swap(
        &self,
        state: &mut StrategyState,
        offer: Coin,
        ask: Coin,
    ) -> Result<(Coin, Coin), StrategyError> {
        match (self, state) {
            (Strategies::Xyk(x), StrategyState::Xyk(ref mut s)) => x.validate_swap(s, offer, ask),
            // _ => Err(StrategyError::InvalidStrategyState {}),
        }
    }

    fn quote(
        &self,
        state: &StrategyState,
        req: QuoteRequest,
    ) -> Result<Option<QuoteResponse>, StrategyError> {
        match (self, state) {
            (Strategies::Xyk(x), StrategyState::Xyk(s)) => x.quote(s, req),
            // _ => Err(StrategyError::InvalidStrategyState {}),
        }
    }

    fn deposit(
        &self,
        state: &mut StrategyState,
        funds: NativeBalance,
    ) -> Result<Uint128, StrategyError> {
        match (self, state) {
            (Strategies::Xyk(x), StrategyState::Xyk(ref mut s)) => x.deposit(s, funds),
            // _ => Err(StrategyError::InvalidStrategyState {}),
        }
    }

    fn withdraw(
        &self,
        state: &mut StrategyState,
        amount: Uint128,
    ) -> Result<NativeBalance, StrategyError> {
        match (self, state) {
            (Strategies::Xyk(x), StrategyState::Xyk(ref mut s)) => x.withdraw(s, amount),
            // _ => Err(StrategyError::InvalidStrategyState {}),
        }
    }
}
