use cosmwasm_std::{coin, Addr, Decimal, Event, Storage, Timestamp, Uint128};
use cw_utils::NativeBalance;
use rujira_rs::pilot::Denoms;
use std::cmp::Ordering;
use std::ops::{Mul, Sub};

use crate::{
    events::{event_create_order, event_increase_order, event_retract_order, event_withdraw_order},
    order::Order,
    pool::Pool,
    ContractError,
};

pub struct OrderManager {
    denoms: Denoms,
    fee: Decimal,
    owner: Addr,
    timestamp: Timestamp,
    oracle: Decimal,
    max_premium: u8,
    // NativeBalance can't be negative. Store in and out separately and we'll validate
    // no negative balances at the end
    // What we receive from the user and withdrawn and retracted orders
    receive: NativeBalance,
    // What we spend creating and increasing orders
    send: NativeBalance,
    fees: NativeBalance,
    events: Vec<Event>,
}

impl OrderManager {
    pub fn new(
        denoms: Denoms,
        fee: Decimal,
        max_premium: u8,
        owner: Addr,
        timestamp: Timestamp,
        oracle: Decimal,
        funds: NativeBalance,
    ) -> Self {
        Self {
            denoms,
            fee,
            max_premium,
            owner,
            timestamp,
            oracle,
            receive: funds,
            send: NativeBalance::default(),
            fees: NativeBalance::default(),
            events: vec![],
        }
    }

    pub fn execute_orders(
        &mut self,
        storage: &mut dyn Storage,
        o: Vec<(u8, Uint128)>,
    ) -> Result<ExecutionResult, ContractError> {
        for (premium, target) in o {
            if premium > self.max_premium {
                return Err(ContractError::InvalidPremium { premium });
            }
            let mut pool = Pool::load(storage, &premium, &self.oracle);
            match pool.load_order(storage, &self.owner) {
                Ok(mut order) => {
                    self.execute_existing_order(storage, &mut pool, &mut order, target)?
                }
                Err(ContractError::NotFound {}) => {
                    self.execute_new_order(storage, &mut pool, target)?
                }
                Err(err) => return Err(err),
            }
        }

        for x in self.send.clone().into_vec() {
            self.receive = (self.receive.clone() - x)?;
        }

        Ok(self.into())
    }

    fn execute_existing_order(
        &mut self,
        storage: &mut dyn Storage,
        pool: &mut Pool,
        order: &mut Order,
        target: Uint128,
    ) -> Result<(), ContractError> {
        self.maybe_withdraw(storage, pool, order)?;
        let amount = Uint128::try_from(order.bid.amount()).unwrap();
        match amount.cmp(&target) {
            Ordering::Less => {
                let diff = target - amount;

                let amount = pool.increase_order(storage, order, &self.timestamp, diff)?;
                let coins = coin(amount.u128(), self.denoms.bid());
                self.send += coins;
                self.events.push(event_increase_order(pool, order, &diff));
            }
            Ordering::Greater => {
                let diff = amount - target;
                let amount = pool.retract_order(storage, order, &self.timestamp, Some(diff))?;
                let coins = coin(amount.u128(), self.denoms.bid());
                self.receive += coins;
                self.events.push(event_retract_order(pool, order, &diff));
            }
            Ordering::Equal => {}
        }
        Ok(())
    }

    fn execute_new_order(
        &mut self,
        storage: &mut dyn Storage,
        pool: &mut Pool,
        target: Uint128,
    ) -> Result<(), ContractError> {
        let order = pool.create_order(storage, &self.timestamp, &self.owner, target)?;
        let coins = coin(order.amount().u128(), self.denoms.bid());
        self.send += coins;
        self.events.push(event_create_order(pool, &order));
        Ok(())
    }

    fn maybe_withdraw(
        &mut self,
        storage: &mut dyn Storage,
        pool: &mut Pool,
        order: &mut Order,
    ) -> Result<(), ContractError> {
        if order.bid.filled().is_zero() {
            return Ok(());
        }
        let amount = pool.claim_order(storage, order)?;
        let fees = Decimal::from_ratio(amount, 1u128)
            .mul(self.fee)
            .to_uint_ceil();

        let receive = coin(amount.sub(fees).u128(), self.denoms.ask());
        let fees = coin(fees.u128(), self.denoms.ask());

        self.receive += receive;
        self.fees += fees;
        self.events.push(event_withdraw_order(pool, order, &amount));
        Ok(())
    }
}

impl From<&mut OrderManager> for ExecutionResult {
    fn from(e: &mut OrderManager) -> Self {
        e.fees.normalize();
        e.receive.normalize();
        Self {
            withdraw: e.receive.clone(),
            fees: e.fees.clone(),
            events: e.events.clone(),
        }
    }
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub withdraw: NativeBalance,
    pub fees: NativeBalance,
    pub events: Vec<Event>,
}

#[cfg(test)]

mod tests {
    use super::*;
    use cosmwasm_std::{
        coins,
        testing::{message_info, mock_dependencies, mock_env},
    };
    use std::str::FromStr;

    #[test]
    fn test_simple_success() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&Addr::unchecked("addr0000"), &[]);
        let oracle = Decimal::from_str("1.0").unwrap();
        let mut funds = NativeBalance::default();
        funds += coin(1000, "usdc");
        let fee = Decimal::from_str("0.001").unwrap();

        let mut e = OrderManager::new(
            Denoms::new("ruji", "usdc"),
            fee,
            30,
            info.sender,
            env.block.time,
            oracle,
            funds,
        );

        let res = e
            .execute_orders(&mut deps.storage, vec![(1, Uint128::from(1000u128))])
            .unwrap();

        assert_eq!(res.withdraw, NativeBalance::default());
        let event = res.events[0].clone();
        assert_eq!(event.ty, "rujira-orca/order.create");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "1");
        assert_eq!(event.attributes[2].key, "offer");
        assert_eq!(event.attributes[2].value, "1000");
    }

    #[test]
    fn test_multiple_orders() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&Addr::unchecked("addr0000"), &[]);
        let fee = Decimal::from_str("0.001").unwrap();

        let oracle = Decimal::from_str("1.0").unwrap();
        let mut funds = NativeBalance::default();
        funds += coin(10000, "usdc");
        funds += coin(10000, "ruji");

        let mut e = OrderManager::new(
            Denoms::new("ruji", "usdc"),
            fee,
            30,
            info.sender,
            env.block.time,
            oracle,
            funds,
        );

        let res = e
            .execute_orders(
                &mut deps.storage,
                vec![
                    (0, Uint128::from(2000u128)),
                    (1, Uint128::from(1000u128)),
                    (2, Uint128::from(1200u128)),
                    (14, Uint128::from(1300u128)),
                ],
            )
            .unwrap();
        let returned = NativeBalance(vec![coin(10000, "ruji"), coin(4500, "usdc")]);
        assert_eq!(res.withdraw, returned);
        let event = res.events[0].clone();
        assert_eq!(event.ty, "rujira-orca/order.create");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "0");
        assert_eq!(event.attributes[2].key, "offer");
        assert_eq!(event.attributes[2].value, "2000");

        let event = res.events[1].clone();
        assert_eq!(event.ty, "rujira-orca/order.create");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "1");
        assert_eq!(event.attributes[2].key, "offer");
        assert_eq!(event.attributes[2].value, "1000");

        let event = res.events[2].clone();
        assert_eq!(event.ty, "rujira-orca/order.create");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "2");
        assert_eq!(event.attributes[2].key, "offer");
        assert_eq!(event.attributes[2].value, "1200");

        let event = res.events[3].clone();
        assert_eq!(event.ty, "rujira-orca/order.create");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "14");
        assert_eq!(event.attributes[2].key, "offer");
        assert_eq!(event.attributes[2].value, "1300");
    }

    #[test]
    fn test_out_of_funds() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&Addr::unchecked("addr0000"), &[]);
        let fee = Decimal::from_str("0.001").unwrap();

        let oracle = Decimal::from_str("1.0").unwrap();
        let funds = NativeBalance::default();
        let mut e = OrderManager::new(
            Denoms::new("ruji", "usdc"),
            fee,
            30,
            info.sender,
            env.block.time,
            oracle,
            funds,
        );

        e.execute_orders(&mut deps.storage, vec![(0, Uint128::from(1000u128))])
            .unwrap_err();
    }

    #[test]
    fn test_moving_orders() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&Addr::unchecked("addr0000"), &[]);
        let fee = Decimal::from_str("0.001").unwrap();

        let oracle = Decimal::from_str("1.0").unwrap();
        let mut funds = NativeBalance::default();
        funds += coin(10000, "usdc");
        funds += coin(10000, "ruji");
        let mut e = OrderManager::new(
            Denoms::new("ruji", "usdc"),
            fee,
            30,
            info.sender.clone(),
            env.block.time,
            oracle,
            funds,
        );

        // Same as above
        e.execute_orders(
            &mut deps.storage,
            vec![
                (0, Uint128::from(1000u128)),
                (1, Uint128::from(2000u128)),
                (2, Uint128::from(1200u128)),
                (10, Uint128::from(1300u128)),
            ],
        )
        .unwrap();

        let mut e = OrderManager::new(
            Denoms::new("ruji", "usdc"),
            fee,
            30,
            info.sender.clone(),
            env.block.time,
            oracle,
            NativeBalance::default(),
        );

        let res = e
            .execute_orders(
                &mut deps.storage,
                vec![
                    (0, Uint128::from(1000u128)),
                    // Split 1200 ito 2 x 600
                    (2, Uint128::from(600u128)),
                    (3, Uint128::from(600u128)),
                    (9, Uint128::from(1300u128)),
                    (10, Uint128::zero()),
                ],
            )
            .unwrap();

        let returned = NativeBalance::default();
        assert_eq!(res.withdraw, returned);
        assert_eq!(res.events.len(), 4);

        let event = res.events[0].clone();
        assert_eq!(event.ty, "rujira-orca/order.retract");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "2");
        assert_eq!(event.attributes[2].key, "amount");
        assert_eq!(event.attributes[2].value, "600");

        let event = res.events[1].clone();
        assert_eq!(event.ty, "rujira-orca/order.create");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "3");
        assert_eq!(event.attributes[2].key, "offer");
        assert_eq!(event.attributes[2].value, "600");

        let event = res.events[2].clone();
        assert_eq!(event.ty, "rujira-orca/order.create");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "9");
        assert_eq!(event.attributes[2].key, "offer");
        assert_eq!(event.attributes[2].value, "1300");

        let event = res.events[3].clone();
        assert_eq!(event.ty, "rujira-orca/order.retract");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "10");
        assert_eq!(event.attributes[2].key, "amount");
        assert_eq!(event.attributes[2].value, "1300");

        let mut e = OrderManager::new(
            Denoms::new("ruji", "usdc"),
            fee,
            30,
            info.sender.clone(),
            env.block.time,
            oracle,
            NativeBalance(coins(300, "usdc")),
        );

        let res = e
            .execute_orders(
                &mut deps.storage,
                vec![(1, Uint128::from(300u128)), (10, Uint128::from(2000u128))],
            )
            .unwrap();

        let returned = NativeBalance::default();
        assert_eq!(res.withdraw, returned);
        assert_eq!(res.events.len(), 2);

        let event = res.events[0].clone();
        assert_eq!(event.ty, "rujira-orca/order.retract");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "1");
        assert_eq!(event.attributes[2].key, "amount");
        assert_eq!(event.attributes[2].value, "1700");

        let event = res.events[1].clone();
        assert_eq!(event.ty, "rujira-orca/order.create");
        assert_eq!(event.attributes[0].key, "owner");
        assert_eq!(event.attributes[0].value, "addr0000");
        assert_eq!(event.attributes[1].key, "premium");
        assert_eq!(event.attributes[1].value, "10");
        assert_eq!(event.attributes[2].key, "offer");
        assert_eq!(event.attributes[2].value, "2000");
    }
}
