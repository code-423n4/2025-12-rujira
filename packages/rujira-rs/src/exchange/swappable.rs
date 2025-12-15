use cosmwasm_std::{Attribute, Decimal, Storage, Uint128};
use itertools::EitherOrBoth;
use std::ops::Add;

use super::{commitment::Commitment, error::SwapError};

/// Swappable trait allows any struct or container to be used in an Iterator and consumed by Swapper
/// E.g. EitherOrBoth<T, V> is implemented in order to support merged, ordered iterators for solving
/// swaps from multiple liquidity providers
pub trait Swappable {
    /// The rate that the swap should be executed at
    fn rate(&self) -> Decimal;

    /// Extra attributes to append to trade events
    fn attributes(&self) -> Vec<Attribute>;

    /// Total amount of bids available for swapping
    fn total(&self) -> Uint128;

    /// Returns the (offer_consumed, bids_returned) amounts
    fn swap(&mut self, offer: Uint128) -> Result<(Uint128, Uint128), SwapError>;

    /// Commits the result of the Swap.
    /// Storage is provided to commit local state
    /// SwapCommit is returned for commitments that require inter-contract communication
    fn commit(&self, storage: &mut dyn Storage) -> Result<Commitment, SwapError>;
}

impl<T, V> Swappable for EitherOrBoth<T, V>
where
    T: Swappable + Clone,
    V: Swappable + Clone,
{
    fn rate(&self) -> Decimal {
        match self {
            EitherOrBoth::Both(a, _) => a.rate(),
            EitherOrBoth::Left(x) => x.rate(),
            EitherOrBoth::Right(x) => x.rate(),
        }
    }

    fn attributes(&self) -> Vec<Attribute> {
        match self {
            EitherOrBoth::Both(a, b) => {
                let mut res = a.attributes();
                res.append(&mut b.attributes());
                res
            }
            EitherOrBoth::Left(x) => x.attributes(),
            EitherOrBoth::Right(x) => x.attributes(),
        }
    }

    fn total(&self) -> Uint128 {
        match self {
            EitherOrBoth::Both(a, b) => a.total().add(b.total()),
            EitherOrBoth::Left(a) => a.total(),
            EitherOrBoth::Right(a) => a.total(),
        }
    }

    fn swap(&mut self, amount: Uint128) -> Result<(Uint128, Uint128), SwapError> {
        match self {
            EitherOrBoth::Both(a, b) => {
                let offer_a = amount.checked_multiply_ratio(a.total(), a.total().add(b.total()))?;
                let offer_b = amount - offer_a;
                let (consumed_offer_a, consumed_bids_a) = a.swap(offer_a)?;
                let (consumed_offer_b, consumed_bids_b) = b.swap(offer_b)?;

                Ok((
                    consumed_offer_a + consumed_offer_b,
                    consumed_bids_a + consumed_bids_b,
                ))
            }
            EitherOrBoth::Left(x) => x.swap(amount),
            EitherOrBoth::Right(x) => x.swap(amount),
        }
    }

    fn commit(&self, storage: &mut dyn Storage) -> Result<Commitment, SwapError> {
        match self {
            EitherOrBoth::Both(a, b) => Ok(a.commit(storage)? + b.commit(storage)?),
            EitherOrBoth::Left(a) => a.commit(storage),
            EitherOrBoth::Right(a) => a.commit(storage),
        }
    }
}

impl<T> Swappable for Vec<T>
where
    T: Swappable + Clone,
{
    fn rate(&self) -> Decimal {
        self.first().map(|x| x.rate()).unwrap_or(Decimal::zero())
    }

    fn attributes(&self) -> Vec<Attribute> {
        self.iter().flat_map(|x| x.attributes()).collect()
    }

    fn total(&self) -> Uint128 {
        self.iter().fold(Uint128::zero(), |acc, x| acc + x.total())
    }

    fn swap(&mut self, amount: Uint128) -> Result<(Uint128, Uint128), SwapError> {
        let total = self.total();
        let mut remaining = amount;
        let mut consumed_offer = Uint128::zero();
        let mut consumed_bids = Uint128::zero();
        for x in self.iter_mut() {
            if remaining.is_zero() {
                break;
            }
            let offer = amount.multiply_ratio(x.total(), total).min(remaining);
            let (c_offer, c_bids) = x.swap(offer)?;
            consumed_offer += c_offer;
            consumed_bids += c_bids;
            remaining -= c_offer;
        }
        Ok((consumed_offer, consumed_bids))
    }

    fn commit(&self, storage: &mut dyn Storage) -> Result<Commitment, SwapError> {
        self.iter()
            .try_fold(Commitment::default(), |acc, x| Ok(acc + x.commit(storage)?))
    }
}

#[cfg(test)]
mod tests {
    use crate::exchange::testing::TestItem;

    use super::*;
    use cosmwasm_std::Uint128;

    // price and is_sell must be the same for all items
    // Swappable for vec assumes that all the quotes are identical but different amounts
    fn item(amount: u128) -> TestItem {
        TestItem::new("1", amount, false)
    }

    #[test]
    fn vec_swappable_proportional_even_split_same_price() {
        // Totals: 50 + 50; Offer: 100 -> expect 50/50
        let mut v = vec![item(50), item(50)];
        let (consumed_offer, consumed_bids) = v.swap(Uint128::new(100)).unwrap();

        assert_eq!(consumed_offer, Uint128::new(100));
        assert_eq!(consumed_bids, Uint128::new(100));
        assert_eq!(v[0].amount, Uint128::new(0));
        assert_eq!(v[1].amount, Uint128::new(0));
    }

    #[test]
    fn vec_swappable_proportional_weighted_same_price() {
        // Totals: 30 + 70; Offer: 100 -> expect 30/70
        let mut v = vec![item(30), item(70)];
        let (consumed_offer, consumed_bids) = v.swap(Uint128::new(100)).unwrap();

        assert_eq!(consumed_offer, Uint128::new(100));
        assert_eq!(consumed_bids, Uint128::new(100));
        assert_eq!(v[0].amount, Uint128::new(0)); // 30 consumed
        assert_eq!(v[1].amount, Uint128::new(0)); // 70 consumed
    }

    #[test]
    fn vec_swappable_zero_liquidity_is_ignored_same_price() {
        // Totals: 0 + 100; Offer: 25 -> only second contributes
        let mut v = vec![item(0), item(100)];
        let (consumed_offer, consumed_bids) = v.swap(Uint128::new(25)).unwrap();

        assert_eq!(consumed_offer, Uint128::new(25));
        assert_eq!(consumed_bids, Uint128::new(25));
        assert_eq!(v[0].amount, Uint128::new(0));
        assert_eq!(v[1].amount, Uint128::new(75)); // 25 consumed
    }

    #[test]
    fn vec_swappable_capacity_cap_same_price() {
        // Totals: 10 + 90; Offer: 200 -> can only consume 10 + 90 = 100 total
        let mut v = vec![item(10), item(90)];
        let (consumed_offer, consumed_bids) = v.swap(Uint128::new(200)).unwrap();

        assert_eq!(consumed_offer, Uint128::new(100));
        assert_eq!(consumed_bids, Uint128::new(100));
        assert_eq!(v[0].amount, Uint128::new(0));
        assert_eq!(v[1].amount, Uint128::new(0));
    }

    #[test]
    fn vec_swappable_three_pools_clean_division_same_price() {
        // Totals: 20 + 30 + 50; Offer: 100 -> expect 20/30/50
        let mut v = vec![item(20), item(30), item(50)];
        let (consumed_offer, consumed_bids) = v.swap(Uint128::new(100)).unwrap();

        assert_eq!(consumed_offer, Uint128::new(100));
        assert_eq!(consumed_bids, Uint128::new(100));
        assert_eq!(v[0].amount, Uint128::new(0));
        assert_eq!(v[1].amount, Uint128::new(0));
        assert_eq!(v[2].amount, Uint128::new(0));
    }

    #[test]
    fn vec_swappable_rounding_behaviour_same_price() {
        // Totals: 1 + 1 + 1; Offer: 2 -> each target share is 2/3;
        // total consumed = 0 because multiply_ratio always floors
        let mut v = vec![item(1), item(1), item(1)];
        let (consumed_offer, consumed_bids) = v.swap(Uint128::new(2)).unwrap();

        assert_eq!(consumed_offer, Uint128::new(0));
        assert_eq!(consumed_bids, Uint128::new(0));

        let remaining: u128 = v.iter().map(|x| x.amount.u128()).sum();
        assert_eq!(remaining, 3);
    }
}
