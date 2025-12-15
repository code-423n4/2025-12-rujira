use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Attribute, Decimal, Event, Storage, Uint128};
use std::ops::Mul;

use crate::fin::SwapRequest;

use super::{commitment::Commitment, error::SwapError, Swappable};

/// Executes a swap over an Iterator<Swappable>, consuming the offer and returning the returned amount
#[cw_serde]
pub struct Swapper<T> {
    event_prefix: String,
    events: Vec<Event>,
    fee: Decimal,
    req: SwapRequest,
    consumed_offer: Uint128,
    remaining_offer: Uint128,
    returned: Uint128,
    pending: Vec<T>,
}

impl<T: Swappable> Swapper<T> {
    pub fn new(event_prefix: &str, offer: Uint128, req: SwapRequest, fee: Decimal) -> Self {
        Self {
            event_prefix: event_prefix.to_string(),
            events: vec![],
            fee,
            req,
            consumed_offer: Uint128::zero(),
            remaining_offer: offer,
            returned: Uint128::zero(),
            pending: vec![],
        }
    }

    pub fn swap(&mut self, iter: &mut dyn Iterator<Item = T>) -> Result<SwapResult, SwapError>
    where
        T: std::fmt::Debug,
    {
        for mut v in iter {
            let (offer, bids) = v.swap(self.remaining_offer)?;

            // If we've breached reached a SwapRequest::Limit, don't commit this step and break
            if let SwapRequest::Limit { price: limit, .. } = self.req {
                if !bids.is_zero() {
                    let achieved = Decimal::from_ratio(offer, bids);
                    if achieved > limit {
                        break;
                    }
                }
            }

            let attrs = v.attributes();
            self.events
                .push(event(&v, &self.event_prefix, offer, bids, &attrs));
            self.pending.push(v);
            self.consumed_offer += offer;
            self.remaining_offer -= offer;
            self.returned += bids;
            if self.remaining_offer.is_zero() {
                break;
            }
        }

        let fee = Decimal::from_ratio(self.returned, 1u128)
            .mul(self.fee)
            .to_uint_ceil();

        self.returned -= fee;

        match self.req {
            SwapRequest::Min { min_return, .. } => {
                if self.returned < min_return {
                    return Err(SwapError::InsufficientReturn {
                        expected: min_return,
                        returned: self.returned,
                    });
                }
            }
            SwapRequest::Exact { exact_return, .. } => {
                if self.returned != exact_return {
                    return Err(SwapError::InsufficientReturn {
                        expected: exact_return,
                        returned: self.returned,
                    });
                }
            }
            _ => {}
        }

        Ok(SwapResult {
            events: self.events.clone(),
            fee_amount: fee,
            return_amount: self.returned,
            consumed_offer: self.consumed_offer,
            remaining_offer: self.remaining_offer,
        })
    }

    pub fn commit(&self, storage: &mut dyn Storage) -> Result<Commitment, SwapError> {
        let mut res = Commitment::default();
        for pool in self.pending.iter() {
            res += pool.commit(storage)?;
        }

        Ok(res)
    }
}

pub fn event<T: Swappable>(
    s: &T,
    prefix: &String,
    offer: Uint128,
    bid: Uint128,
    attributes: &[Attribute],
) -> Event {
    Event::new(format!("{prefix}/trade"))
        .add_attribute("rate", s.rate().to_string())
        .add_attribute("offer", offer.to_string())
        .add_attribute("bid", bid.to_string())
        .add_attributes(attributes.to_owned())
}

#[derive(Debug)]
pub struct SwapResult {
    pub events: Vec<Event>,
    pub fee_amount: Uint128,
    pub return_amount: Uint128,
    pub consumed_offer: Uint128,
    pub remaining_offer: Uint128,
}

#[cfg(test)]

mod tests {

    use cosmwasm_std::Fraction;

    use crate::exchange::testing::TestIter;

    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_swap_execution() {
        let fee = Decimal::from_str("0.001").unwrap();
        let mut iter = TestIter::new(vec![
            (Decimal::from_str("1.0").unwrap(), Uint128::from(1000u128)),
            (Decimal::from_str("0.95").unwrap(), Uint128::from(1000u128)),
            (Decimal::from_str("0.9").unwrap(), Uint128::from(1000u128)),
            (Decimal::from_str("0.85").unwrap(), Uint128::from(1000u128)),
            (Decimal::from_str("0.8").unwrap(), Uint128::from(1000u128)),
            (Decimal::from_str("0.7").unwrap(), Uint128::from(1000u128)),
            (Decimal::from_str("0.6").unwrap(), Uint128::from(1000u128)),
        ]);

        let mut s = Swapper::new(
            "some-prefix",
            Uint128::from(7500u128),
            SwapRequest::Yolo {
                to: None,
                callback: None,
            },
            fee,
        );
        let res = s.swap(&mut iter).unwrap();
        assert_eq!(res.return_amount, Uint128::from(6283u128));
        assert_eq!(res.fee_amount, Uint128::from(7u128));
        assert_eq!(res.remaining_offer, Uint128::zero());

        let event = res.events[0].clone();
        assert_eq!(event.ty, "some-prefix/trade");
        assert_eq!(event.attributes[0].key, "rate");
        assert_eq!(event.attributes[0].value, "1");
        assert_eq!(event.attributes[1].key, "offer");
        assert_eq!(event.attributes[1].value, "1000");
        assert_eq!(event.attributes[2].key, "bid");
        assert_eq!(event.attributes[2].value, "1000");
        assert_eq!(event.attributes[3].key, "test");
        assert_eq!(event.attributes[3].value, "attr");

        let event = res.events[1].clone();
        assert_eq!(event.ty, "some-prefix/trade");
        assert_eq!(event.attributes[0].key, "rate");
        assert_eq!(event.attributes[0].value, "0.95");
        assert_eq!(event.attributes[1].key, "offer");
        assert_eq!(event.attributes[1].value, "1052");
        assert_eq!(event.attributes[2].key, "bid");
        assert_eq!(event.attributes[2].value, "1000");
        assert_eq!(event.attributes[3].key, "test");
        assert_eq!(event.attributes[3].value, "attr");

        let event = res.events[2].clone();
        assert_eq!(event.ty, "some-prefix/trade");
        assert_eq!(event.attributes[0].key, "rate");
        assert_eq!(event.attributes[0].value, "0.9");
        assert_eq!(event.attributes[1].key, "offer");
        assert_eq!(event.attributes[1].value, "1111");
        assert_eq!(event.attributes[2].key, "bid");
        assert_eq!(event.attributes[2].value, "1000");
        assert_eq!(event.attributes[3].key, "test");
        assert_eq!(event.attributes[3].value, "attr");

        let event = res.events[3].clone();
        assert_eq!(event.ty, "some-prefix/trade");
        assert_eq!(event.attributes[0].key, "rate");
        assert_eq!(event.attributes[0].value, "0.85");
        assert_eq!(event.attributes[1].key, "offer");
        assert_eq!(event.attributes[1].value, "1176");
        assert_eq!(event.attributes[2].key, "bid");
        assert_eq!(event.attributes[2].value, "1000");
        assert_eq!(event.attributes[3].key, "test");
        assert_eq!(event.attributes[3].value, "attr");

        let event = res.events[4].clone();
        assert_eq!(event.ty, "some-prefix/trade");
        assert_eq!(event.attributes[0].key, "rate");
        assert_eq!(event.attributes[0].value, "0.8");
        assert_eq!(event.attributes[1].key, "offer");
        assert_eq!(event.attributes[1].value, "1250");
        assert_eq!(event.attributes[2].key, "bid");
        assert_eq!(event.attributes[2].value, "1000");
        assert_eq!(event.attributes[3].key, "test");
        assert_eq!(event.attributes[3].value, "attr");

        let event = res.events[5].clone();
        assert_eq!(event.ty, "some-prefix/trade");
        assert_eq!(event.attributes[0].key, "rate");
        assert_eq!(event.attributes[0].value, "0.7");
        assert_eq!(event.attributes[1].key, "offer");
        assert_eq!(event.attributes[1].value, "1428");
        assert_eq!(event.attributes[2].key, "bid");
        assert_eq!(event.attributes[2].value, "1000");
        assert_eq!(event.attributes[3].key, "test");
        assert_eq!(event.attributes[3].value, "attr");

        let event = res.events[6].clone();
        assert_eq!(event.ty, "some-prefix/trade");
        assert_eq!(event.attributes[0].key, "rate");
        assert_eq!(event.attributes[0].value, "0.6");
        assert_eq!(event.attributes[1].key, "offer");
        assert_eq!(event.attributes[1].value, "483");
        assert_eq!(event.attributes[2].key, "bid");
        assert_eq!(event.attributes[2].value, "290");
        assert_eq!(event.attributes[3].key, "test");
        assert_eq!(event.attributes[3].value, "attr");
    }

    #[test]
    fn test_swap_variants() {
        let fee = Decimal::from_str("0.001").unwrap();
        for (offer, req, result) in vec![
            (
                Uint128::from(900u128),
                SwapRequest::Min {
                    min_return: Uint128::from(1000u128),
                    to: None,
                    callback: None,
                },
                None,
            ),
            (
                Uint128::from(1100u128),
                SwapRequest::Min {
                    min_return: Uint128::from(1000u128),
                    to: None,
                    callback: None,
                },
                Some((
                    Uint128::from(1093u128),
                    Uint128::from(2u128),
                    Uint128::zero(),
                )),
            ),
            (
                Uint128::from(1100u128),
                SwapRequest::Exact {
                    exact_return: Uint128::from(1000u128),
                    to: None,
                    callback: None,
                },
                None,
            ),
            (
                Uint128::from(900u128),
                SwapRequest::Exact {
                    // Less fees
                    exact_return: Uint128::from(899u128),
                    to: None,
                    callback: None,
                },
                Some((
                    Uint128::from(899u128),
                    Uint128::from(1u128),
                    Uint128::zero(),
                )),
            ),
            (
                Uint128::from(1100u128),
                SwapRequest::Limit {
                    price: Decimal::one(),
                    to: None,
                    callback: None,
                },
                Some((
                    Uint128::from(999u128),
                    Uint128::from(1u128),
                    Uint128::from(100u128),
                )),
            ),
            (
                Uint128::from(10000u128),
                SwapRequest::Limit {
                    // Price is inverted from the bid price in the iterator
                    price: Decimal::from_str("0.85").unwrap().inv().unwrap(),
                    to: None,
                    callback: None,
                },
                Some((
                    // Should get up to 0.85 and halt
                    // Receive 4 x 1000 - fee
                    Uint128::from(3996u128),
                    Uint128::from(4u128),
                    Uint128::from(5661u128),
                )),
            ),
        ] {
            let mut iter = TestIter::new(vec![
                (Decimal::from_str("1.0").unwrap(), Uint128::from(1000u128)),
                (Decimal::from_str("0.95").unwrap(), Uint128::from(1000u128)),
                (Decimal::from_str("0.9").unwrap(), Uint128::from(1000u128)),
                (Decimal::from_str("0.85").unwrap(), Uint128::from(1000u128)),
                (Decimal::from_str("0.8").unwrap(), Uint128::from(1000u128)),
                (Decimal::from_str("0.7").unwrap(), Uint128::from(1000u128)),
                (Decimal::from_str("0.6").unwrap(), Uint128::from(1000u128)),
            ]);

            let mut s = Swapper::new("some-prefix", offer, req, fee);
            let res = s.swap(&mut iter);
            match result {
                Some((returned, fee, remaining)) => {
                    let res = res.unwrap();
                    assert_eq!(res.return_amount, returned);
                    assert_eq!(res.fee_amount, fee);
                    assert_eq!(res.remaining_offer, remaining);
                }
                None => {
                    res.unwrap_err();
                }
            }
        }
    }
}
