use crate::decimal_scaled::DecimalScaled;

use super::pool::Pool;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::{Key, KeyDeserialize, Prefixer, PrimaryKey};

#[cw_serde]
pub struct SumSnapshot {
    pub epoch: u32,
    pub sum: DecimalScaled,
}

impl SumSnapshot {
    pub fn key(&self) -> SumSnapshotKey {
        SumSnapshotKey(self.epoch)
    }
}

impl From<Pool> for SumSnapshot {
    fn from(value: Pool) -> Self {
        Self {
            epoch: value.epoch,
            sum: value.sum,
        }
    }
}

#[derive(Clone)]
pub struct SumSnapshotKey(u32);

impl SumSnapshotKey {
    pub fn new(epoch: u32) -> Self {
        Self(epoch)
    }
}

impl PrimaryKey<'_> for SumSnapshotKey {
    type Prefix = ();
    type SubPrefix = ();
    type Suffix = ();
    type SuperSuffix = ();
    fn key(&self) -> Vec<cw_storage_plus::Key> {
        self.0.key()
    }
}
impl<'a> Prefixer<'a> for SumSnapshotKey {
    fn prefix(&self) -> Vec<Key> {
        self.key()
    }
}

impl KeyDeserialize for SumSnapshotKey {
    type Output = Self;
    const KEY_ELEMS: u16 = 6;

    fn from_vec(value: Vec<u8>) -> cosmwasm_std::StdResult<Self::Output> {
        let epoch = u32::from_be_bytes(value[..4].try_into().unwrap());
        Ok(Self(epoch))
    }
}
