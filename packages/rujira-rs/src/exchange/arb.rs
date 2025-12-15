use std::fmt::Debug;

use cosmwasm_std::{Storage, Uint128};

use super::{Commitment, SwapError, Swappable};

/// Utility to capture arbitrage from overlapping orders
#[derive(Debug)]
pub struct Arber<T> {
    pending_base: Vec<T>,
    pending_quote: Vec<T>,
}

impl<T> Default for Arber<T> {
    fn default() -> Self {
        Self {
            pending_base: vec![],
            pending_quote: vec![],
        }
    }
}

impl<T: Arbitrage + Clone> Arber<T> {
    pub fn run(
        &mut self,
        mut base_iter: impl Iterator<Item = T>,
        mut quote_iter: impl Iterator<Item = T>,
    ) -> Result<ArbitrageResult, SwapError> {
        let mut base = base_iter.next();
        let mut quote = quote_iter.next();
        let mut profit_base = Uint128::zero();
        let mut profit_quote = Uint128::zero();
        while let (Some(b), Some(q)) = (&mut base, &mut quote) {
            match b.arbitrage(q)? {
                Some((base_profit, quote_profit)) => {
                    profit_base += base_profit;
                    profit_quote += quote_profit;
                    self.pending_quote.push(q.clone());
                    self.pending_base.push(b.clone());

                    if b.total().is_zero() {
                        base = base_iter.next()
                    }

                    if q.total().is_zero() {
                        quote = quote_iter.next()
                    }
                }
                None => break,
            }
        }
        Ok(ArbitrageResult {
            profit_base,
            profit_quote,
        })
    }

    pub fn commit(
        &mut self,
        storage: &mut dyn Storage,
    ) -> Result<(Commitment, Commitment), SwapError> {
        let mut base = Commitment::default();
        for x in self.pending_base.iter_mut() {
            base += x.commit(storage)?;
        }

        let mut quote = Commitment::default();
        for x in self.pending_quote.iter_mut() {
            quote += x.commit(storage)?;
        }

        Ok((base, quote))
    }
}

#[derive(Debug)]
pub struct ArbitrageResult {
    pub profit_base: Uint128,
    pub profit_quote: Uint128,
}

pub trait Arbitrage: Swappable + Sized {
    /// Must be called as bid token sell.arbitrage(buy) with respect to Swappable::rate()
    /// Returns arbitrage profit in the (`self`, `other`) bid tokens
    fn arbitrage(&mut self, other: &mut Self) -> Result<Option<(Uint128, Uint128)>, SwapError>;
}

impl<T> Arbitrage for T
where
    T: Swappable + Clone + Debug,
{
    fn arbitrage(&mut self, other: &mut Self) -> Result<Option<(Uint128, Uint128)>, SwapError> {
        let float = self.total();
        let a = other.swap(float)?;
        let b = self.swap(a.1)?;
        match (b.1.checked_sub(a.0), a.1.checked_sub(b.0)) {
            (Ok(a), Ok(b)) => Ok(Some((a, b))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use cosmwasm_std::{Decimal, Fraction, Uint128};

    use crate::exchange::testing::{TestItem, TestIter};

    use super::{Arber, Arbitrage};

    // TODO: Test when rounding of b

    #[test]

    fn test_aribtrage() {
        let mut a = TestItem::new("0.625", 1000u128, true);
        let mut b = TestItem::new("0.8", 3000u128, false);

        let res = a.arbitrage(&mut b).unwrap().unwrap();
        assert_eq!(res, (Uint128::zero(), Uint128::from(175u128)));

        let mut a = TestItem::new("1.6", 1000u128, false);
        let mut b = TestItem::new("1.25", 3000u128, true);

        let res = b.arbitrage(&mut a).unwrap().unwrap();
        assert_eq!(res, (Uint128::from(175u128), Uint128::zero()));

        // Test when profit token is on the opposite side
        let mut a = TestItem::new("0.625", 5000u128, true);
        let mut b = TestItem::new("0.8", 1000u128, false);

        let res = a.arbitrage(&mut b).unwrap().unwrap();
        assert_eq!(res, (Uint128::from(350u128), Uint128::zero()));

        let mut a = TestItem::new("1.6", 5000u128, false);
        let mut b = TestItem::new("1.25", 1000u128, true);

        let res = b.arbitrage(&mut a).unwrap().unwrap();
        assert_eq!(res, (Uint128::zero(), Uint128::from(350u128)));

        // Test no opportunity
        let mut a = TestItem::new("0.5", 1000u128, false);
        let mut b = TestItem::new("1", 3000u128, true);

        assert_eq!(b.arbitrage(&mut a).unwrap(), None);

        // And when profit token is on the opposite side
        let mut a = TestItem::new("2", 5000u128, true);
        let mut b = TestItem::new("1", 1000u128, false);

        assert_eq!(b.arbitrage(&mut a).unwrap(), None);
    }

    #[test]
    fn test_arber() {
        let base_iter = TestIter::new(vec![
            (
                Decimal::from_str("0.9").unwrap().inv().unwrap(),
                Uint128::from(1000u128),
            ),
            (
                Decimal::from_str("0.8").unwrap().inv().unwrap(),
                Uint128::from(1000u128),
            ),
            (
                Decimal::from_str("0.75").unwrap().inv().unwrap(),
                Uint128::from(1000u128),
            ),
        ]);

        let quote_iter = TestIter::new(vec![
            (Decimal::from_str("1.1").unwrap(), Uint128::from(1000u128)),
            (Decimal::from_str("0.9").unwrap(), Uint128::from(1000u128)),
            (Decimal::from_str("0.7").unwrap(), Uint128::from(1000u128)),
            (Decimal::from_str("0.6").unwrap(), Uint128::from(1000u128)),
        ]);

        let mut arb = Arber::default();
        let res = arb.run(base_iter, quote_iter).unwrap();
        assert_eq!(res.profit_base, Uint128::from(114u128));
        assert_eq!(res.profit_quote, Uint128::from(200u128));
        assert_eq!(
            arb.pending_base,
            vec![
                TestItem {
                    price: Decimal::from_str("0.9").unwrap().inv().unwrap(),
                    amount: Uint128::zero(),
                    commitment: (Uint128::from(900u128), Uint128::from(1000u128))
                },
                TestItem {
                    price: Decimal::from_str("0.8").unwrap().inv().unwrap(),
                    amount: Uint128::zero(),
                    commitment: (Uint128::from(800u128), Uint128::from(1000u128))
                },
                TestItem {
                    price: Decimal::from_str("0.75").unwrap().inv().unwrap(),
                    amount: Uint128::from(866u128),
                    commitment: (Uint128::from(100u128), Uint128::from(134u128))
                }
            ]
        );

        assert_eq!(
            arb.pending_quote,
            vec![
                TestItem {
                    price: Decimal::from_str("1.1").unwrap(),
                    amount: Uint128::zero(),
                    commitment: (Uint128::from(909u128), Uint128::from(1000u128))
                },
                // The 0.9 buy pool was used to fill the 0.8 and 0.75 sells.
                // Two individual commitments required, with a correctly decrementing amount between the two
                TestItem {
                    price: Decimal::from_str("0.9").unwrap(),
                    amount: Uint128::from(100u128),
                    commitment: (Uint128::from(1000u128), Uint128::from(900u128))
                },
                TestItem {
                    price: Decimal::from_str("0.9").unwrap(),
                    amount: Uint128::zero(),
                    commitment: (Uint128::from(111u128), Uint128::from(100u128))
                },
            ]
        );
    }
}
