use std::ops::{Div, Mul, Sub};

use super::{bid::Bid, BidPoolError, SumSnapshot};
use crate::decimal_scaled::DecimalScaled;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal256, Fraction as _, StdResult, Uint256};

/// Pooled bidding to distribute tokens between bidders at a specific exchange rate,
/// consuming their bids, in O(1) time
///
/// CRITICAL: When loading a Bid, you **must** call sync_bid to update the Bid with the latest pool state
#[cw_serde]
#[derive(Copy)]
pub struct Pool {
    pub(crate) total: Uint256,
    pub(crate) sum: DecimalScaled,
    pub(crate) product: DecimalScaled,
    pub(crate) epoch: u32,
}

impl Default for Pool {
    fn default() -> Self {
        Self {
            total: Uint256::zero(),
            sum: DecimalScaled::zero(),
            product: DecimalScaled::one(),
            epoch: 0,
        }
    }
}

impl Pool {
    pub fn new_bid(&mut self, amount: Uint256) -> Bid {
        self.total += amount;

        Bid {
            amount,
            filled: Uint256::zero(),
            product_snapshot: self.product,
            sum_snapshot: self.sum,
            epoch_snapshot: self.epoch,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.total.is_zero()
    }

    /// Distribute the given amount at the given rate, consuming the total amount of the pool,
    pub fn distribute(
        &mut self,
        offer: Uint256,
        rate: &Decimal256,
    ) -> Result<DistributionResult, BidPoolError> {
        if offer.is_zero() || self.total.is_zero() {
            return Ok(DistributionResult::default());
        }
        let bids_value = offer.mul_floor(*rate);

        // Switch between entirely consuming either the pool or the offer.
        // This effectively carries the rounding truncation from the distribute_full,
        // until we get to distribute_partial, where it's fully consumed and we have
        // symmetry via the fair distro algo

        // To account for rounding issues, if a bids_value is short by 1 unit, we treat it as fully consumed.
        if bids_value + Uint256::one() >= self.total {
            // Offer is worth more than pool. Empty the pool an increase the epoch
            return self.distribute_full(rate);
        }
        // Pool is worth more than the offer. Consume the rest of the offer
        self.distribute_partial(bids_value, offer)
    }

    pub fn total(&self) -> Uint256 {
        self.total
    }

    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    fn distribute_full(&mut self, rate: &Decimal256) -> Result<DistributionResult, BidPoolError> {
        let consumed_offer = self
            .total
            .multiply_ratio(rate.denominator(), rate.numerator());
        let consumed_bids = self.total;
        let offer_per_bid = DecimalScaled::from_ratio(consumed_offer, consumed_bids);

        let sum = self.sum + self.product.mul(offer_per_bid);

        self.sum = sum;
        let mut snapshots = vec![SumSnapshot::from(*self)];
        self.increment_epoch();
        snapshots.push(SumSnapshot::from(*self));

        Ok(DistributionResult {
            consumed_offer,
            consumed_bids,
            snapshots,
        })
    }

    fn distribute_partial(
        &mut self,
        bids_value: Uint256,
        offer: Uint256,
    ) -> Result<DistributionResult, BidPoolError> {
        // S + E / D * P
        let offer_per_bid = DecimalScaled::from_ratio(offer, self.total);

        // If sum is not incremented, bids are not filled.
        // Eg     sum = 0.99999999999999999999999999999999999999999999999999992559979936021098395430438
        //    product = 0.0000000000000000000000000000000000000000000000000000000000000000000000000000000006226962958147167932332866989055998293039199447359426092824598426728536322091
        // And so the scale of the product is too small to increment the sum. At this point the pool
        // has scaled too much and we can't execute a distribution
        let sum = self.sum + self.product * offer_per_bid;
        if sum == self.sum {
            return Err(BidPoolError::DistributionError {});
        }

        // 1 - Q / D
        let ratio = DecimalScaled::one() - DecimalScaled::from_ratio(bids_value, self.total);
        let product = self.product * ratio;

        self.product = product;
        self.sum = sum;

        if self.product == DecimalScaled::zero() {
            return Err(BidPoolError::DistributionError {});
        }

        self.total -= bids_value;
        let snapshots = vec![SumSnapshot::from(*self)];

        Ok(DistributionResult {
            consumed_offer: offer,
            consumed_bids: bids_value,
            snapshots,
        })
    }

    pub fn sync_bid(&self, bid: &mut Bid, sum_snapshot: Option<DecimalScaled>) -> StdResult<()> {
        bid.filled += self.bid_filled_amount(bid, sum_snapshot)?;
        bid.amount = self.bid_remaining_amount(bid)?;
        bid.product_snapshot = self.product;
        bid.sum_snapshot = self.sum;
        bid.epoch_snapshot = self.epoch;

        Ok(())
    }

    fn bid_filled_amount(
        &self,
        bid: &Bid,
        sum_snapshot: Option<DecimalScaled>,
    ) -> StdResult<Uint256> {
        if bid.product_snapshot.is_zero() {
            return Ok(Uint256::zero());
        }
        if bid.amount.is_zero() {
            return Ok(Uint256::zero());
        }

        let reference_ss = sum_snapshot.unwrap_or(bid.sum_snapshot);
        let res = reference_ss
            .sub(bid.sum_snapshot)
            .mul(bid.amount)
            .div(bid.product_snapshot)
            .to_uint_floor();

        Ok(res)
    }

    fn bid_remaining_amount(&self, bid: &mut Bid) -> StdResult<Uint256> {
        if bid.product_snapshot.is_zero() {
            return Ok(bid.amount);
        }
        if bid.amount.is_zero() {
            return Ok(Uint256::zero());
        }

        let epoch_diff: Uint256 =
            Uint256::from(self.epoch).checked_sub(Uint256::from(bid.epoch_snapshot))?;

        if !epoch_diff.is_zero() {
            return Ok(Uint256::zero());
        }

        let ratio = self.product / bid.product_snapshot;
        let amount = ratio * bid.amount;

        Ok(amount.to_uint_floor())
    }

    fn increment_epoch(&mut self) {
        self.total = Uint256::zero();
        self.sum = DecimalScaled::zero();
        self.product = DecimalScaled::one();
        self.epoch += 1;
    }
}

#[derive(Default, Debug)]
pub struct DistributionResult {
    pub consumed_offer: Uint256,
    pub consumed_bids: Uint256,
    pub snapshots: Vec<SumSnapshot>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_distribution() {
        let mut pool = Pool::default();
        let mut bid_a = pool.new_bid(Uint256::from(100u128));
        let mut bid_b = pool.new_bid(Uint256::from(200u128));
        let mut bid_c = pool.new_bid(Uint256::from(250u128));
        let mut bid_d = pool.new_bid(Uint256::from(1000u128));

        assert_eq!(pool.total(), Uint256::from(1550u128));
        assert_eq!(pool.sum, DecimalScaled::zero());
        assert_eq!(pool.epoch, 0);
        assert_eq!(pool.product, DecimalScaled::one());

        let res = pool
            .distribute(
                Uint256::from(7750u128),
                &Decimal256::from_str("0.05").unwrap(),
            )
            .unwrap();

        assert_eq!(res.consumed_offer, Uint256::from(7750u128));
        assert_eq!(res.consumed_bids, Uint256::from(387u128));
        assert_eq!(pool.total(), Uint256::from(1163u128));
        assert_eq!(pool.epoch, 0);

        let s = res.snapshots.first().map(|s| s.sum);

        pool.sync_bid(&mut bid_a, s).unwrap();

        pool.sync_bid(&mut bid_b, s).unwrap();
        pool.sync_bid(&mut bid_c, s).unwrap();
        pool.sync_bid(&mut bid_d, s).unwrap();

        // 7750 * 0.05 = 387, 25% fills
        assert_eq!(bid_a.amount, Uint256::from(75u128));
        assert_eq!(bid_a.filled, Uint256::from(500u128));
        assert_eq!(bid_b.amount, Uint256::from(150u128));
        assert_eq!(bid_b.filled, Uint256::from(1000u128));
        assert_eq!(bid_c.amount, Uint256::from(187u128));
        assert_eq!(bid_c.filled, Uint256::from(1250u128));
        assert_eq!(bid_d.amount, Uint256::from(750u128));
        assert_eq!(bid_d.filled, Uint256::from(5000u128));

        assert_eq!(bid_a.claim_filled(), Uint256::from(500u128));
        assert_eq!(bid_b.claim_filled(), Uint256::from(1000u128));

        let mut bid_e = pool.new_bid(Uint256::from(450u128));
        let mut bid_f = pool.new_bid(Uint256::from(950u128));

        assert_eq!(pool.total(), Uint256::from(2563u128));
        pool.sync_bid(&mut bid_a, s).unwrap();
        pool.sync_bid(&mut bid_b, s).unwrap();
        pool.sync_bid(&mut bid_c, s).unwrap();
        pool.sync_bid(&mut bid_d, s).unwrap();
        pool.sync_bid(&mut bid_e, s).unwrap();
        pool.sync_bid(&mut bid_f, s).unwrap();

        assert_eq!(bid_a.amount, Uint256::from(75u128));
        assert_eq!(bid_a.filled, Uint256::zero());
        assert_eq!(bid_b.amount, Uint256::from(150u128));
        assert_eq!(bid_b.filled, Uint256::zero());
        assert_eq!(bid_c.amount, Uint256::from(187u128));
        assert_eq!(bid_c.filled, Uint256::from(1250u128));
        assert_eq!(bid_d.amount, Uint256::from(750u128));
        assert_eq!(bid_d.filled, Uint256::from(5000u128));
        assert_eq!(bid_e.amount, Uint256::from(450u128));
        assert_eq!(bid_e.filled, Uint256::zero());
        assert_eq!(bid_f.amount, Uint256::from(950u128));
        assert_eq!(bid_f.filled, Uint256::zero());

        let res = pool
            .distribute(
                Uint256::from(10000u128),
                &Decimal256::from_str("0.05").unwrap(),
            )
            .unwrap();

        assert_eq!(res.consumed_offer, Uint256::from(10000u128));
        assert_eq!(res.consumed_bids, Uint256::from(500u128));
        assert_eq!(pool.total(), Uint256::from(2063u128));
        assert_eq!(pool.epoch, 0);

        let s = res.snapshots.first().map(|s| s.sum);

        pool.sync_bid(&mut bid_a, s).unwrap();
        pool.sync_bid(&mut bid_b, s).unwrap();
        pool.sync_bid(&mut bid_c, s).unwrap();
        pool.sync_bid(&mut bid_d, s).unwrap();
        pool.sync_bid(&mut bid_e, s).unwrap();
        pool.sync_bid(&mut bid_f, s).unwrap();

        assert_eq!(bid_a.amount, Uint256::from(60u128));
        assert_eq!(bid_a.filled, Uint256::from(292u128));
        assert_eq!(bid_b.amount, Uint256::from(120u128));
        assert_eq!(bid_b.filled, Uint256::from(585u128));
        assert_eq!(bid_c.amount, Uint256::from(150u128));
        assert_eq!(bid_c.filled, Uint256::from(1979u128));
        assert_eq!(bid_d.amount, Uint256::from(603u128));
        assert_eq!(bid_d.filled, Uint256::from(7926u128));
        assert_eq!(bid_e.amount, Uint256::from(362u128));
        assert_eq!(bid_e.filled, Uint256::from(1755u128));
        assert_eq!(bid_f.amount, Uint256::from(764u128));
        assert_eq!(bid_f.filled, Uint256::from(3706u128));
    }

    #[test]
    fn test_scaling() {
        // With the change in `distribute_full` to use `to_unit_floor` instead of `to_unit_ceil`,
        // and the safe `+ Uint256::one()` in the `distribute` function, this test will now always
        // distribute the entire amount, leaving no remainder.

        let mut pool = Pool::default();
        let mut bids: Vec<Bid> = vec![];

        for n in 0..13 {
            let bid = pool.new_bid(Uint256::from(10000000u128));

            let res = pool
                .distribute(Uint256::from(9999999u128), &Decimal256::one())
                .unwrap();
            // TODO: bail earlier. the overlap between sum and product is the precision with which bids are filled
            println!("      {}", pool.sum);
            println!("      {}", pool.product);

            let s = res.snapshots.first().map(|s| s.sum);
            bids.push(bid);
            for bid in bids.iter_mut() {
                pool.sync_bid(bid, s).unwrap();
                // Only the most recent bid has any amount remaining
                assert!(bid.amount <= pool.total);
                assert!(bid.amount + bid.filled <= Uint256::from(10000000u128));
                println!("{n} amount {} filled {}", bid.amount, bid.filled);
            }
        }
        pool.new_bid(Uint256::from(10000000u128));
        pool.distribute(Uint256::from(9999999u128), &Decimal256::one())
            .unwrap();
    }

    #[test]
    fn test_resizing_increase_full() {
        // Ensure that bids are reset when increased after a full consimption
        let mut pool = Pool::default();
        let mut bid = pool.new_bid(Uint256::from(500u128));

        let res = pool
            .distribute(
                Uint256::from(500_000u128),
                &Decimal256::from_str("0.001").unwrap(),
            )
            .unwrap();
        assert_eq!(bid.epoch_snapshot, 0);

        assert_eq!(res.consumed_offer, Uint256::from(500_000u128));
        assert_eq!(res.consumed_bids, Uint256::from(500u128));
        let snapshot = res.snapshots[0].clone();
        assert_eq!(snapshot.epoch, bid.epoch_snapshot);

        pool.sync_bid(&mut bid, Some(snapshot.sum)).unwrap();
        assert_eq!(bid.amount, Uint256::zero());
        assert_eq!(bid.filled, Uint256::from(500_000u128));

        bid.increase(&mut pool, Uint256::from(800u128)).unwrap();
        assert_eq!(bid.product_snapshot, DecimalScaled::one());
        assert_eq!(bid.sum_snapshot, DecimalScaled::zero());
        assert_eq!(bid.epoch_snapshot, 1);
        let snapshot = res.snapshots[1].clone();
        assert_eq!(snapshot.epoch, bid.epoch_snapshot);
        pool.sync_bid(&mut bid, Some(snapshot.sum)).unwrap();
        assert_eq!(bid.filled, Uint256::from(500_000u128));
        assert_eq!(bid.amount, Uint256::from(800u128));
    }

    #[test]
    fn test_resizing_increase_partial() {
        // Ensure that bids are reset when increased after a partial consumption
        let mut pool = Pool::default();
        let mut bid = pool.new_bid(Uint256::from(1000u128));

        let res = pool
            .distribute(
                Uint256::from(500_000u128),
                &Decimal256::from_str("0.001").unwrap(),
            )
            .unwrap();
        assert_eq!(bid.epoch_snapshot, 0);

        assert_eq!(res.consumed_offer, Uint256::from(500_000u128));
        assert_eq!(res.consumed_bids, Uint256::from(500u128));
        let snapshot = res.snapshots[0].clone();
        assert_eq!(snapshot.epoch, bid.epoch_snapshot);

        pool.sync_bid(&mut bid, Some(snapshot.sum)).unwrap();
        assert_eq!(bid.amount, Uint256::from(500u128));
        assert_eq!(bid.filled, Uint256::from(500_000u128));

        bid.increase(&mut pool, Uint256::from(800u128)).unwrap();
        assert_eq!(bid.epoch_snapshot, 0);
        let snapshot = res.snapshots[0].clone();
        assert_eq!(snapshot.epoch, bid.epoch_snapshot);
        pool.sync_bid(&mut bid, Some(snapshot.sum)).unwrap();
        assert_eq!(bid.filled, Uint256::from(500_000u128));
        assert_eq!(bid.amount, Uint256::from(1300u128));
    }

    #[test]
    fn test_resizing_decrease() {
        // Ensure that bids are reset when retracted
        let mut pool = Pool::default();
        let mut bid = pool.new_bid(Uint256::from(500u128));

        let res = pool
            .distribute(
                Uint256::from(250_000u128),
                &Decimal256::from_str("0.001").unwrap(),
            )
            .unwrap();

        assert_eq!(res.consumed_offer, Uint256::from(250_000u128));
        assert_eq!(res.consumed_bids, Uint256::from(250u128));

        pool.sync_bid(&mut bid, Some(res.snapshots[0].sum)).unwrap();
        assert_eq!(bid.amount, Uint256::from(250u128));
        assert_eq!(bid.filled, Uint256::from(250_000u128));

        bid.retract(&mut pool, Some(Uint256::from(125u128)))
            .unwrap();
        pool.sync_bid(&mut bid, Some(res.snapshots[0].sum)).unwrap();

        assert_eq!(bid.filled, Uint256::from(250_000u128));
        assert_eq!(bid.amount, Uint256::from(125u128));
    }
}
