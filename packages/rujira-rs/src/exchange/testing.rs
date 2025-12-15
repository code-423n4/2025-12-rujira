use super::{Commitment, SwapError, Swappable};
use cosmwasm_std::{Attribute, Decimal, Fraction, Storage, Uint128};
use std::cmp::Ordering;
use std::ops::{Div, Mul};
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestItem {
    pub price: Decimal,
    pub amount: Uint128,
    pub commitment: (Uint128, Uint128),
}

impl TestItem {
    pub fn new(price: &str, amount: u128, is_sell: bool) -> Self {
        let mut price = Decimal::from_str(price).unwrap();
        if is_sell {
            price = price.inv().unwrap();
        }
        Self {
            price,
            amount: Uint128::from(amount),
            commitment: Default::default(),
        }
    }
}

impl Swappable for TestItem {
    fn rate(&self) -> Decimal {
        self.price
    }

    fn attributes(&self) -> Vec<Attribute> {
        vec![Attribute::new("test", "attr")]
    }

    fn total(&self) -> Uint128 {
        self.amount
    }

    fn swap(&mut self, offer: Uint128) -> Result<(Uint128, Uint128), SwapError> {
        let offer_value = Decimal::from_ratio(offer, 1u128)
            .mul(self.price)
            .to_uint_ceil();
        let pool_value = Decimal::from_ratio(self.amount, 1u128)
            .div(self.price)
            .to_uint_floor();
        let res = match self.amount.cmp(&offer_value) {
            // Partial fill
            Ordering::Greater => {
                self.amount -= offer_value;
                (offer, offer_value)
            }
            // Complete fill
            _ => {
                let size = self.amount;
                self.amount = Uint128::zero();
                (pool_value, size)
            }
        };

        // Increment commitment as the same offer may be used multiple times, particularly during arbitrage
        self.commitment = res;
        Ok(res)
    }

    fn commit(&self, _storage: &mut dyn Storage) -> Result<Commitment, SwapError> {
        Ok(Commitment::default())
    }
}

pub struct TestIter {
    pub items: Vec<(Decimal, Uint128)>,
}

impl TestIter {
    pub fn new(items: Vec<(Decimal, Uint128)>) -> Self {
        let mut items = items;
        items.reverse();
        Self { items }
    }
}

impl Iterator for TestIter {
    type Item = TestItem;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.pop().map(|(price, amount)| TestItem {
            price,
            amount,
            commitment: Default::default(),
        })
    }
}
