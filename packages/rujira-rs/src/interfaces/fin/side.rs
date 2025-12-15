use std::fmt::Display;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::StdError;
use cw_storage_plus::{Key, KeyDeserialize, Prefixer, PrimaryKey};

#[cw_serde]
pub enum Side {
    Base,
    Quote,
}

impl Side {
    pub fn other(&self) -> Self {
        match self {
            Side::Base => Side::Quote,
            Side::Quote => Side::Base,
        }
    }
}

impl Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Base => write!(f, "base"),
            Side::Quote => write!(f, "quote"),
        }
    }
}

impl PrimaryKey<'_> for Side {
    type Prefix = ();
    type SubPrefix = ();
    type Suffix = ();
    type SuperSuffix = ();

    fn key(&self) -> std::vec::Vec<Key<'_>> {
        match self {
            Side::Base => vec![Key::Val8([0])],
            Side::Quote => vec![Key::Val8([1])],
        }
    }
}

impl<'a> Prefixer<'a> for Side {
    fn prefix(&self) -> Vec<Key> {
        self.key()
    }
}

impl KeyDeserialize for Side {
    type Output = Self;
    const KEY_ELEMS: u16 = 1;

    fn from_vec(value: Vec<u8>) -> cosmwasm_std::StdResult<Self::Output> {
        match value.first() {
            Some(0u8) => Ok(Self::Base),
            Some(1u8) => Ok(Self::Quote),
            _ => Err(StdError::generic_err("invalid Side key")),
        }
    }
}
