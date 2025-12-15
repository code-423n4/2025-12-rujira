use std::cmp::Ordering;
use std::fmt::Write;
use std::ops::{Add, Div, Mul};
use std::{fmt, ops::Sub};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal256, Fraction, OverflowError, Uint256};

/// A Decimal value that stores a scale for high precision arithmetic at low values
/// Useful for storing and multiplying very large numbers by very small fractions, where the output
/// is in a sensible range
#[cw_serde]
#[derive(Copy, Default)]
pub struct DecimalScaled((Uint256, i64));

impl DecimalScaled {
    const DECIMAL_FRACTIONAL: Uint256 = // 1*10**18
        Uint256::from_u128(1_000_000_000_000_000_000_000_000);
    pub const DECIMAL_PLACES: i64 = 24;

    /// Create a 1.0 Decimal
    #[inline]
    pub const fn one() -> Self {
        Self((Uint256::one(), 0))
    }

    /// Create a 0.0 Decimal
    #[inline]
    pub const fn zero() -> Self {
        Self((Uint256::zero(), 0))
    }

    pub const fn is_zero(&self) -> bool {
        self.0 .0.is_zero()
    }

    fn normalize(&mut self) {
        if self.0 .0.is_zero() {
            self.0 .1 = 0;
            return;
        }
        while self.0 .0 % Uint256::from(10u64) == Uint256::zero() && !self.is_zero() {
            self.0 .0 /= Uint256::from(10u64);
            self.0 .1 -= 1;
        }
    }

    pub fn to_uint_floor(&self) -> Uint256 {
        match Uint256::from(10u128).checked_pow(self.0 .1.unsigned_abs() as u32) {
            Ok(v) => {
                if self.0 .1 < 0 {
                    self.0 .0.mul(v)
                } else {
                    self.0 .0.div(v)
                }
            }
            Err(_) => self.truncate(1).to_uint_floor(),
        }
    }

    pub fn from_ratio(numerator: impl Into<Uint256>, denominator: impl Into<Uint256>) -> Self {
        let v = numerator
            .into()
            .checked_multiply_ratio(Self::DECIMAL_FRACTIONAL, denominator.into())
            .unwrap();

        let mut res = Self((v, Self::DECIMAL_PLACES));
        res.normalize();
        res
    }

    pub fn pow(&self, exp: u32) -> Self {
        Self((self.0 .0, self.0 .1 + exp as i64))
    }

    pub fn truncate(&self, scale: u32) -> Self {
        Self((
            self.0 .0.div(Uint256::from(10u128).pow(scale)),
            self.0 .1 - scale as i64,
        ))
    }

    fn map_equalised(
        &self,
        other: Self,
        f: impl Fn(Uint256, Uint256, i64) -> Self,
        g: impl Fn(Self, Self) -> Self,
    ) -> Self {
        let mut res = match self.0 .1.cmp(&other.0 .1) {
            Ordering::Less => {
                let pow = other.0 .1.sub(self.0 .1) as u32;

                match Uint256::from(10u128)
                    .checked_pow(pow)
                    .and_then(|x| self.0 .0.checked_mul(x))
                {
                    Ok(v) => f(v, other.0 .0, other.0 .1),
                    Err(OverflowError { .. }) => {
                        // Shave an order of precision off and try again
                        g(*self, other.truncate(1))
                    }
                }
            }
            Ordering::Equal => f(self.0 .0, other.0 .0, self.0 .1),
            Ordering::Greater => {
                let pow = self.0 .1.sub(other.0 .1) as u32;
                match Uint256::from(10u128)
                    .checked_pow(pow)
                    .and_then(|x| other.0 .0.checked_mul(x))
                {
                    Ok(v) => f(self.0 .0, v, self.0 .1),
                    Err(OverflowError { .. }) => {
                        // Shave an order of precision off and try again
                        g(self.truncate(1), other)
                    }
                }
            }
        };

        res.normalize();
        res
    }

    /// Split into (integer_part, fractional_part) where:
    /// - integer_part is floor(self) as Uint256
    /// - fractional_part = self - integer_part, as DecimalScaled, with 0 <= fractional_part < 1
    pub fn decompose(&self) -> (Uint256, DecimalScaled) {
        let i = self.to_uint_floor();
        // Build a DecimalScaled for the integer part (scale 0) and subtract to get the fraction.
        let f = *self - DecimalScaled((i, 0));
        (i, f)
    }
}

impl From<Decimal256> for DecimalScaled {
    fn from(value: Decimal256) -> Self {
        if value.is_zero() {
            return Self((Uint256::zero(), 0));
        }

        let mut val = value.numerator();
        let mut decimals = Decimal256::DECIMAL_PLACES as i64;

        while val % Uint256::from(10u64) == Uint256::zero() {
            val /= Uint256::from(10u64);
            decimals -= 1;
        }

        Self((val, decimals))
    }
}

impl From<DecimalScaled> for Decimal256 {
    fn from(value: DecimalScaled) -> Self {
        Decimal256::from_ratio(value.numerator(), value.denominator())
    }
}

impl Mul for DecimalScaled {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        if self.is_zero() || rhs.is_zero() {
            return Self::zero();
        }

        let mut res = match self.0 .0.checked_mul(rhs.0 .0) {
            Ok(v) => Self((v, self.0 .1 + rhs.0 .1)),
            Err(_) => self.truncate(1) * rhs.truncate(1),
        };
        res.normalize();
        res
    }
}

impl Div for DecimalScaled {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        if self.is_zero() {
            return Self::zero();
        }
        if self == rhs {
            return Self::one();
        }

        self.map_equalised(
            rhs,
            |a, b, _| {
                Self((
                    Self::DECIMAL_FRACTIONAL
                        .checked_multiply_ratio(a, b)
                        .unwrap(),
                    Self::DECIMAL_PLACES,
                ))
            },
            Div::div,
        )
    }
}

impl Add for DecimalScaled {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.map_equalised(
            rhs,
            |a, b, i| match a.checked_add(b) {
                Ok(v) => Self((v, i)),
                Err(_) => self.truncate(1).add(rhs.truncate(1)),
            },
            Add::add,
        )
    }
}

impl Sub for DecimalScaled {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self.map_equalised(rhs, |a, b, i| Self((a.sub(b), i)), Sub::sub)
    }
}

impl Fraction<Uint256> for DecimalScaled {
    fn numerator(&self) -> Uint256 {
        if self.0 .1 < 0 {
            self.0
                 .0
                .mul(Uint256::from(10u128).pow(self.0 .1.unsigned_abs() as u32))
        } else {
            self.0 .0
        }
    }

    fn denominator(&self) -> Uint256 {
        if self.0 .1 <= 0 {
            Uint256::one()
        } else {
            Uint256::from(10u128).pow(self.0 .1.unsigned_abs() as u32)
        }
    }

    fn inv(&self) -> Option<Self> {
        if self.numerator().is_zero() {
            return None;
        }
        let v = self
            .denominator()
            .checked_multiply_ratio(Self::DECIMAL_FRACTIONAL, self.numerator())
            .unwrap();

        let mut res = Self((v, Self::DECIMAL_PLACES));
        res.normalize();
        Some(res)
    }
}

impl Mul<Uint256> for DecimalScaled {
    type Output = DecimalScaled;

    fn mul(self, rhs: Uint256) -> Self::Output {
        if self.is_zero() || rhs.is_zero() {
            return Self::zero();
        }

        let mut res = match self.0 .0.checked_mul(rhs) {
            Ok(val) => Self((val, self.0 .1)),
            Err(_) => self.truncate(1).mul(rhs),
        };

        res.normalize();
        res
    }
}

impl fmt::Display for DecimalScaled {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0 .1 <= 0 {
            self.0 .0.fmt(f)?;
            for _ in 0..self.0 .1.abs() {
                f.write_char('0')?;
            }

            Ok(())
        } else {
            write!(f, "0.{:0>width$}", self.0 .0, width = self.0 .1 as usize)
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use cosmwasm_std::Uint256;

    #[test]
    fn to_uint_floor() {
        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 1)).to_uint_floor(),
            Uint256::zero()
        );

        assert_eq!(
            DecimalScaled((Uint256::from(512u128), 3)).to_uint_floor(),
            Uint256::zero()
        );
        assert_eq!(
            DecimalScaled((Uint256::from(512u128), 2)).to_uint_floor(),
            Uint256::from(5u128)
        );

        assert_eq!(
            DecimalScaled((Uint256::from(512u128), -1)).to_uint_floor(),
            Uint256::from(5120u128)
        );
    }

    #[test]
    fn from() {
        assert_eq!(
            DecimalScaled::from(Decimal256::zero()),
            DecimalScaled((Uint256::zero(), 0))
        );

        assert_eq!(
            DecimalScaled::from(Decimal256::from_ratio(1u128, 2u128)),
            DecimalScaled((Uint256::from(5u128), 1))
        );

        assert_eq!(
            DecimalScaled::from(Decimal256::from_ratio(1u128, 8u128)),
            DecimalScaled((Uint256::from(125u128), 3))
        );

        assert_eq!(
            DecimalScaled::from(Decimal256::from_ratio(30u128, 1u128)),
            DecimalScaled((Uint256::from(3u128), -1))
        );

        assert_eq!(
            DecimalScaled::from(Decimal256::from_ratio(12u128, 8u128)),
            DecimalScaled((Uint256::from(15u128), 1))
        );

        assert_eq!(
            DecimalScaled::from(Decimal256::from_ratio(12345u128, 3u128)),
            DecimalScaled((Uint256::from(4115u128), 0))
        );
    }

    #[test]
    fn extreme_values() {
        // Check normal function
        assert_eq!(DecimalScaled((Uint256::from(5u128), 1)).to_string(), "0.5");
        assert_eq!(DecimalScaled((Uint256::from(5u128), -2)).to_string(), "500");
        assert_eq!(
            DecimalScaled((Uint256::from(512u128), 4)).to_string(),
            "0.0512"
        );
        assert_eq!(DecimalScaled((Uint256::from(5u128), 0)).to_string(), "5");

        assert_eq!(
            DecimalScaled((Uint256::from(1u128), 50)).to_string(),
            // 50 leading zeros
            "0.00000000000000000000000000000000000000000000000001"
        );
        assert_eq!(
            DecimalScaled((Uint256::from(1u128), 100)).to_string(),
            // 100 leading zeros
            "0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001"
        );
        assert_eq!(
            DecimalScaled((Uint256::from(1u128), -100)).to_string(),
            // 100 leading zeros
            "10000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn multiplication() {
        assert_eq!(
            DecimalScaled::zero() * DecimalScaled::zero(),
            DecimalScaled::zero()
        );

        assert_eq!(
            DecimalScaled::one() * DecimalScaled::zero(),
            DecimalScaled::zero()
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 1)) * DecimalScaled((Uint256::from(8u128), 0)),
            DecimalScaled((Uint256::from(4u128), 0))
        );

        assert_eq!(
            DecimalScaled((Uint256::from(2u128), 10)) * DecimalScaled((Uint256::from(3u128), 10)),
            DecimalScaled((Uint256::from(6u128), 20))
        );
    }

    #[test]
    fn fraction() {
        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 0)).numerator(),
            Uint256::from(5u128)
        );
        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 0)).denominator(),
            Uint256::one()
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5u128), -3)).numerator(),
            Uint256::from(5000u128)
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5u128), -3)).denominator(),
            Uint256::one()
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 3)).numerator(),
            Uint256::from(5u128)
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 3)).denominator(),
            Uint256::from(1000u128)
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 0)).inv().unwrap(),
            DecimalScaled((Uint256::from(2u128), 1)),
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5u128), -3)).inv().unwrap(),
            DecimalScaled((Uint256::from(2u128), 4)),
        );
        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 3)).inv().unwrap(),
            DecimalScaled((Uint256::from(2u128), -2)),
        );
    }

    #[test]
    fn add() {
        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 0)) + DecimalScaled((Uint256::from(3u128), 0)),
            DecimalScaled((Uint256::from(8u128), 0))
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5125u128), 3)) + DecimalScaled((Uint256::from(3u128), 0)),
            DecimalScaled((Uint256::from(8125u128), 3))
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5u128), -2)) + DecimalScaled((Uint256::from(3u128), 0)),
            DecimalScaled((Uint256::from(503u128), 0))
        );

        assert_eq!(
            DecimalScaled((Uint256::one(), 80)) + DecimalScaled((Uint256::one(), 100)),
            DecimalScaled((Uint256::from(100000000000000000001u128), 100))
        );

        assert_eq!(
            DecimalScaled((Uint256::MAX, 50)) + DecimalScaled((Uint256::MAX, 60)),
            DecimalScaled((
                Uint256::MAX
                    // We need to take the scale down one to have space to add
                    .div(Uint256::from(10u128))
                    // Divide rhs by 1e10, plus the extra one from above
                    .add(Uint256::MAX.div(Uint256::from(100000000000u128))),
                49
            ))
        );
    }
    #[test]
    fn sub() {
        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 0)) - DecimalScaled((Uint256::from(3u128), 0)),
            DecimalScaled((Uint256::from(2u128), 0))
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5125u128), 3)) - DecimalScaled((Uint256::from(3u128), 0)),
            DecimalScaled((Uint256::from(2125u128), 3))
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5u128), -2)) - DecimalScaled((Uint256::from(3u128), 0)),
            DecimalScaled((Uint256::from(497u128), 0))
        );
    }

    #[test]
    fn div() {
        assert_eq!(
            DecimalScaled((Uint256::from(5u128), 0)) / DecimalScaled((Uint256::from(3u128), 0)),
            DecimalScaled((Uint256::from(1666666666666666666666666u128), 24))
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5125u128), 3)) / DecimalScaled((Uint256::from(3u128), 0)),
            DecimalScaled((Uint256::from(1708333333333333333333333u128), 24))
        );

        assert_eq!(
            DecimalScaled((Uint256::from(5u128), -2)) / DecimalScaled((Uint256::from(3u128), 0)),
            DecimalScaled((Uint256::from(166666666666666666666666666u128), 24))
        );

        assert_eq!(
            DecimalScaled((Uint256::from(6u128), -4)) / DecimalScaled((Uint256::from(3u128), 0)),
            DecimalScaled((Uint256::from(2u128), -4))
        );
        assert_eq!(
            // 101
            DecimalScaled((Uint256::from(101u128), 0))
            // 100.0000000000000000000000001
                / DecimalScaled((Uint256::from(1_000_000_000_000_000_000_000_000_001u128), 25)),
            DecimalScaled((Uint256::from(1009999999999999999999999u128), 24))
        );
    }

    #[test]
    fn truncate() {
        assert_eq!(
            // 166666666.6666666666
            DecimalScaled((Uint256::from(1666666666666666666u128), 10)).truncate(1),
            // 166666666.666666666
            DecimalScaled((Uint256::from(166666666666666666u128), 9)),
        );
    }

    #[test]
    fn handle_overflow() {
        let v = DecimalScaled((Uint256::MAX, 75));

        assert_eq!(
            v.mul(Uint256::from(10u128)),
            DecimalScaled((Uint256::MAX.div(Uint256::from(10u128)), 73))
        );
    }

    #[test]
    fn decompose() {
        // 0.5
        let v = DecimalScaled((Uint256::from(5u128), 1));
        let (i, f) = v.decompose();
        assert_eq!(i, Uint256::zero()); // floor(0.5) = 0
        assert_eq!(f, DecimalScaled((Uint256::from(5u128), 1))); // 0.5

        // 1.23456789
        let v = DecimalScaled((Uint256::from(123456789u128), 8));
        let (i, f) = v.decompose();
        assert_eq!(i, Uint256::from(1u128)); // floor = 1
        assert_eq!(f, DecimalScaled((Uint256::from(23456789u128), 8))); // 0.23456789

        // 12.0 exactly
        let v = DecimalScaled((Uint256::from(12u128), 0));
        let (i, f) = v.decompose();
        assert_eq!(i, Uint256::from(12u128)); // floor = 12
        assert_eq!(f, DecimalScaled::zero()); // no fractional part

        // 1234.567
        let v = DecimalScaled((Uint256::from(1234567u128), 3));
        let (i, f) = v.decompose();
        assert_eq!(i, Uint256::from(1234u128)); // floor = 1234
        assert_eq!(f, DecimalScaled((Uint256::from(567u128), 3))); // 0.567
    }
}
