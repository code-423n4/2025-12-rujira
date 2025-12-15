use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    Addr, Attribute, Decimal, Decimal256, StdResult, Storage, Timestamp, Uint128, Uint256,
};
use cw_storage_plus::Map;
use rujira_rs::{
    bid_pool::{self, SumSnapshot},
    exchange::{Commitment, SwapError, Swappable},
    DecimalScaled,
};

use crate::{order::Order, premium::Premium, ContractError};
const SNAPSHOTS: Map<(u8, bid_pool::SumSnapshotKey), DecimalScaled> = Map::new("snapshots");
// The POOLS Map is used simply as an indicator that there is a non-zero BidPool at this key
// The BID_POOLS Map is used to store the BidPool itself, and the Key is used to populate the Pool values
const POOLS: Map<u8, ()> = Map::new("pools");
const BID_POOLS: Map<u8, bid_pool::Pool> = Map::new("bid-pools");

/// A wrapper around a BidPool to provide a side & price, used for keying orders and
/// storing pools for iterating during execution
#[cw_serde]
pub struct Pool {
    pub premium: u8,
    pub rate: Decimal,
    pub pool: bid_pool::Pool,
    #[serde(skip)]
    pending_sum_snapshots: Vec<SumSnapshot>,
}

impl Pool {
    pub fn iter<'a>(
        storage: &'a dyn Storage,
        oracle: &'a Decimal,
    ) -> Box<dyn Iterator<Item = Self> + 'a> {
        let iter = POOLS
            .range(storage, None, None, cosmwasm_std::Order::Ascending)
            .filter_map(|x: StdResult<(u8, ())>| -> Option<Self> {
                match x {
                    Ok((premium, _)) => Some(Self {
                        premium,
                        rate: premium.to_rate(oracle),
                        // The presence of the key indicates a BidPool should be present,
                        // so we should panic if this is incorrect
                        pool: BID_POOLS.load(storage, premium).unwrap(),
                        pending_sum_snapshots: vec![],
                    }),
                    Err(_) => None,
                }
            });

        Box::new(iter)
    }

    pub fn load(storage: &dyn Storage, premium: &u8, oracle: &Decimal) -> Self {
        Self {
            premium: *premium,
            rate: premium.to_rate(oracle),
            pool: BID_POOLS.load(storage, *premium).unwrap_or_default(),
            pending_sum_snapshots: vec![],
        }
    }

    pub fn create_order(
        &mut self,
        storage: &mut dyn Storage,
        timestamp: &Timestamp,
        owner: &Addr,
        offer: Uint128,
    ) -> Result<Order, ContractError> {
        let order = Order {
            owner: owner.clone(),
            offer,
            updated_at: *timestamp,
            bid: self.pool.new_bid(offer.into()),
        };
        self.commit(storage)?;
        order.save(storage, self)?;
        Ok(order)
    }

    pub fn load_order(&self, storage: &dyn Storage, owner: &Addr) -> Result<Order, ContractError> {
        let mut order = Order::load(storage, owner, &self.premium)?;
        self.sync_order(storage, &mut order)?;
        Ok(order)
    }

    pub fn increase_order(
        &mut self,
        storage: &mut dyn Storage,
        order: &mut Order,
        timestamp: &Timestamp,
        amount: Uint128,
    ) -> Result<Uint128, ContractError> {
        order.bid.increase(&mut self.pool, amount.into())?;
        order.offer = order.amount();
        order.updated_at = *timestamp;
        order.save(storage, self)?;
        self.commit(storage)?;
        Ok(amount)
    }

    pub fn retract_order(
        &mut self,
        storage: &mut dyn Storage,
        order: &mut Order,
        timestamp: &Timestamp,
        amount: Option<Uint128>,
    ) -> Result<Uint128, ContractError> {
        let amount256 = amount.map(Uint256::from);
        let refund_amount = order.bid.retract(&mut self.pool, amount256)?;
        order.offer = order.amount();
        order.updated_at = *timestamp;
        order.save(storage, self)?;
        self.commit(storage)?;
        Ok(Uint128::try_from(refund_amount)?)
    }

    pub fn claim_order(
        &mut self,
        storage: &mut dyn Storage,
        order: &mut Order,
    ) -> Result<Uint128, ContractError> {
        let claimed = order.bid.claim_filled();
        order.save(storage, self)?;
        Ok(Uint128::try_from(claimed)?)
    }

    pub fn sync_order(
        &self,
        storage: &dyn Storage,
        order: &mut Order,
    ) -> Result<(), ContractError> {
        let sum_snapshot = self.sum_snapshot(storage, &order.bid).ok();
        Ok(self.pool.sync_bid(&mut order.bid, sum_snapshot)?)
    }

    fn sum_snapshot(&self, storage: &dyn Storage, bid: &bid_pool::Bid) -> StdResult<DecimalScaled> {
        let key = (self.premium, bid.sum_snapshot_key());
        SNAPSHOTS.load(storage, key)
    }
}

impl Swappable for Pool {
    fn swap(&mut self, offer: Uint128) -> Result<(Uint128, Uint128), SwapError> {
        let res = self
            .pool
            .distribute(offer.into(), &Decimal256::from(self.rate))?;

        self.pending_sum_snapshots = res.snapshots;

        Ok((
            res.consumed_offer.try_into()?,
            res.consumed_bids.try_into()?,
        ))
    }

    fn commit(&self, storage: &mut dyn Storage) -> Result<Commitment, SwapError> {
        for s in self.pending_sum_snapshots.clone() {
            SNAPSHOTS.save(storage, (self.premium, s.key()), &s.sum)?;
        }

        BID_POOLS.save(storage, self.premium, &self.pool)?;
        // Clear empty pools so they're not iterated over during a swap
        if self.pool.is_zero() {
            POOLS.remove(storage, self.premium);
            return Ok(Commitment::default());
        }

        POOLS.save(storage, self.premium, &())?;
        Ok(Commitment::default())
    }

    fn attributes(&self) -> Vec<Attribute> {
        vec![Attribute::new("premium", self.premium.to_string())]
    }

    fn rate(&self) -> Decimal {
        self.rate
    }

    fn total(&self) -> Uint128 {
        self.pool.total().try_into().unwrap()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use cosmwasm_std::{testing::MockStorage, Decimal};

    #[test]
    // Verify that when a Pool is removed from storage, the BidPool is retained and the correct values used for syncing bids
    fn pool_bid_pool_replacement() {
        let mut store = MockStorage::new();
        let timestamp = Timestamp::default();
        let owner = Addr::unchecked("owner");
        let offer = Uint128::from(100u128);
        let premium = 0;
        let oracle = Decimal::one();
        let mut pool = Pool::load(&store, &premium, &oracle);
        pool.create_order(&mut store, &timestamp, &owner, offer)
            .unwrap();

        pool.commit(&mut store).unwrap();
        // Check both the pool and bid pool are stored
        POOLS.load(&store, premium).unwrap();
        BID_POOLS.load(&store, premium).unwrap();

        pool.swap(Uint128::from(100u128)).unwrap();

        // Bid Pool should have been emptied, so the container Pool shold be cleared, but the BidPool should remain
        pool.commit(&mut store).unwrap();
        POOLS.load(&store, premium).unwrap_err();
        let bp = BID_POOLS.load(&store, premium).unwrap();
        // Check it's different from the default
        assert_ne!(bp, bid_pool::Pool::default());

        // Now check it's restored to the pool correctly
        let pool = Pool::load(&store, &premium, &oracle);
        assert_eq!(pool.pool, bp);
    }
}
