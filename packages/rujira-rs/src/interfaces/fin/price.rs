use std::fmt::Display;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, Fraction, StdError, Uint128};
use cw_storage_plus::{IntKey, Key, KeyDeserialize, PrimaryKey};

use crate::Premiumable;

#[cw_serde]
pub enum Price {
    Fixed(Decimal),
    Oracle(i16),
}

impl Display for Price {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Price::Fixed(fixed) => write!(f, "fixed:{}", fixed),
            Price::Oracle(deviation) => write!(f, "oracle:{}", deviation),
        }
    }
}

impl Price {
    pub fn to_rate(&self, oracle: &impl Premiumable) -> Decimal {
        match self {
            Price::Fixed(fixed) => *fixed,
            Price::Oracle(bps) => oracle.adjust(bps),
        }
    }
}

impl PrimaryKey<'_> for Price {
    type Prefix = ();
    type SubPrefix = ();
    type Suffix = ();
    type SuperSuffix = ();

    fn key(&self) -> std::vec::Vec<Key<'_>> {
        match self {
            Price::Fixed(fixed) => vec![Key::Val128(fixed.numerator().to_be_bytes())],
            Price::Oracle(deviation) => vec![Key::Val16(deviation.to_cw_bytes())],
        }
    }
}

impl KeyDeserialize for Price {
    type Output = Self;
    const KEY_ELEMS: u16 = 1;

    fn from_vec(value: Vec<u8>) -> cosmwasm_std::StdResult<Self::Output> {
        match value.len() {
            16 => Ok(Self::Fixed(Decimal::new(Uint128::from(
                u128::from_be_bytes(value.try_into().unwrap()),
            )))),
            2 => Ok(Self::Oracle(i16::from_cw_bytes(value.try_into().unwrap()))),
            _ => Err(StdError::generic_err("invalid Price key")),
        }
    }
}
