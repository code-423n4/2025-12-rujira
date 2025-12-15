use cosmwasm_std::{Coin, Fraction, Uint128};
use cw_utils::NativeBalance;

/// Convenience trait for defining operations on Vec<Coin> and NativeBalance
pub trait Coins {
    fn mul_coins<T, F>(&self, f: F) -> Self
    where
        F: Fraction<T> + Copy,
        T: Into<Uint128>;
}

impl Coins for Vec<Coin> {
    fn mul_coins<T, F>(&self, ratio: F) -> Self
    where
        F: Fraction<T> + Copy,
        T: Into<Uint128>,
    {
        self.iter()
            .map(|x| Coin {
                amount: x.amount.mul_floor(ratio),
                ..x.clone()
            })
            .collect()
    }
}

impl Coins for NativeBalance {
    fn mul_coins<T, F>(&self, ratio: F) -> Self
    where
        F: Fraction<T> + Copy,
        T: Into<Uint128>,
    {
        NativeBalance(self.clone().into_vec().mul_coins(ratio))
    }
}
