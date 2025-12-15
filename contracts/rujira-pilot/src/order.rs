use std::cmp::min;

use crate::{error::ContractError, pool::Pool};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, StdResult, Storage, Timestamp, Uint128};
use cw_storage_plus::Map;
use rujira_rs::bid_pool;

pub const ORDERS: Map<(Addr, u8), (Timestamp, Uint128, bid_pool::Bid)> = Map::new("orders");
const MAX_LIMIT: u8 = 31;
const DEFAULT_LIMIT: u8 = 10;

#[cw_serde]
pub struct Order {
    pub owner: Addr,
    pub updated_at: Timestamp,
    /// Original offer amount, as it was at `updated_at` time
    pub offer: Uint128,
    pub bid: bid_pool::Bid,
}

impl Order {
    pub fn load(storage: &dyn Storage, owner: &Addr, premium: &u8) -> Result<Self, ContractError> {
        let (updated_at, offer, bid) = ORDERS
            .load(storage, (owner.clone(), *premium))
            .map_err(|_| ContractError::NotFound {})?;
        Ok(Self {
            owner: owner.clone(),
            updated_at,
            offer,
            bid,
        })
    }

    pub fn by_owner(
        storage: &dyn Storage,
        owner: &Addr,
        offset: Option<u8>,
        limit: Option<u8>,
    ) -> StdResult<Vec<(u8, Self)>> {
        let limit = min(limit.unwrap_or(DEFAULT_LIMIT), MAX_LIMIT) as usize;
        let offset = offset.unwrap_or(0) as usize;
        ORDERS
            .prefix(owner.clone())
            .range(storage, None, None, cosmwasm_std::Order::Ascending)
            .skip(offset)
            .take(limit)
            .map(|x| {
                x.map(|(k, (updated_at, offer, bid))| {
                    (
                        k,
                        Self {
                            owner: owner.clone(),
                            updated_at,
                            offer,
                            bid,
                        },
                    )
                })
            })
            .collect()
    }

    pub fn amount(&self) -> Uint128 {
        self.bid.amount().try_into().unwrap()
    }

    pub fn save(&self, storage: &mut dyn Storage, pool: &Pool) -> StdResult<()> {
        if self.bid.is_empty() {
            self.remove(storage, pool);
            return Ok(());
        }
        ORDERS.save(
            storage,
            (self.owner.clone(), pool.premium),
            &(self.updated_at, self.offer, self.bid.clone()),
        )?;
        Ok(())
    }

    fn remove(&self, storage: &mut dyn Storage, pool: &Pool) {
        ORDERS.remove(storage, (self.owner.clone(), pool.premium))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use cosmwasm_std::{testing::MockStorage, Addr, Decimal, Timestamp, Uint128};
    use rujira_rs::exchange::Swappable;

    use crate::pool::Pool;

    #[test]
    fn query_order() {
        let mut store = MockStorage::new();
        let timestamp = Timestamp::default();
        let owner = Addr::unchecked("owner");
        let offer = Uint128::from(100u128);
        let mut pool = Pool::load(&store, &0, &Decimal::one());
        pool.create_order(&mut store, &timestamp, &owner, offer)
            .unwrap();

        pool.commit(&mut store).unwrap();

        let order = Order::load(&store, &owner, &0).unwrap();
        assert_eq!(order.owner, owner);
        assert_eq!(order.offer, offer);
    }

    #[test]
    fn query_orders_by_owner() {
        let mut store = MockStorage::new();
        let timestamp = Timestamp::default();
        let owner = Addr::unchecked("owner");
        let owner2 = Addr::unchecked("owner2");
        let offer = Uint128::from(100u128);
        let oracle = Decimal::one();
        let mut pool1 = Pool::load(&store, &0, &oracle);
        let mut pool2 = Pool::load(&store, &1, &oracle);
        let mut pool3 = Pool::load(&store, &2, &oracle);
        let mut pool4 = Pool::load(&store, &10, &oracle);
        let mut pool5 = Pool::load(&store, &11, &oracle);
        let mut pool6 = Pool::load(&store, &12, &oracle);

        pool1
            .create_order(&mut store, &timestamp, &owner, offer)
            .unwrap();
        pool2
            .create_order(&mut store, &timestamp, &owner, offer)
            .unwrap();
        pool3
            .create_order(&mut store, &timestamp, &owner, offer)
            .unwrap();
        pool4
            .create_order(&mut store, &timestamp, &owner, offer)
            .unwrap();
        pool5
            .create_order(&mut store, &timestamp, &owner, offer)
            .unwrap();
        pool6
            .create_order(&mut store, &timestamp, &owner, offer)
            .unwrap();

        pool1
            .create_order(&mut store, &timestamp, &owner2, offer)
            .unwrap();

        pool1.commit(&mut store).unwrap();

        let orders = Order::by_owner(&store, &owner, None, None).unwrap();
        assert_eq!(orders.len(), 6);
        assert_eq!(orders[0].1.owner, owner);
        assert_eq!(orders[0].1.offer, offer);
    }
}
