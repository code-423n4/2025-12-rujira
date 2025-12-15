use cosmwasm_std::{Coin, Decimal, OverflowError, QuerierWrapper, Uint128};
use cw_utils::NativeBalance;
use std::ops::Add;
use thiserror::Error;

use crate::{
    query::{
        network::{Network, TryFromNetworkError},
        oracle_price::{OraclePrice, OraclePriceError},
        pool::{Pool, PoolError},
    },
    Layer1Asset, SecuredAsset, SecuredAssetError,
};

pub trait Oracle {
    fn tor_price(&self, q: QuerierWrapper) -> Result<Decimal, OracleError>;
    fn oracle_price(&self, q: QuerierWrapper) -> Result<Decimal, OracleError>;
}

pub trait OracleValue {
    fn value_usd(&self, q: QuerierWrapper) -> Result<Decimal, OracleError>;
}

impl Oracle for Layer1Asset {
    fn tor_price(&self, q: QuerierWrapper) -> Result<Decimal, OracleError> {
        if self.is_rune() {
            Ok(Network::load(q)?.rune_price_in_tor)
        } else {
            Ok(Pool::load(q, self)?.asset_tor_price)
        }
    }
    fn oracle_price(&self, q: QuerierWrapper) -> Result<Decimal, OracleError> {
        self.ticker().oracle_price(q)
    }
}

impl Oracle for String {
    fn tor_price(&self, _q: QuerierWrapper) -> Result<Decimal, OracleError> {
        Err(OracleError::Unavailable {})
    }
    fn oracle_price(&self, q: QuerierWrapper) -> Result<Decimal, OracleError> {
        Ok(OraclePrice::load(q, self)?.price)
    }
}

impl<T: Oracle> Oracle for [T; 2] {
    fn tor_price(&self, q: QuerierWrapper) -> Result<Decimal, OracleError> {
        Ok(self[0].tor_price(q)? / self[1].tor_price(q)?)
    }
    fn oracle_price(&self, q: QuerierWrapper) -> Result<Decimal, OracleError> {
        Ok(self[0].oracle_price(q)? / self[1].oracle_price(q)?)
    }
}

impl OracleValue for Coin {
    fn value_usd(&self, q: QuerierWrapper) -> Result<Decimal, OracleError> {
        Ok(SecuredAsset::from_denom(&self.denom)?
            .to_layer_1()
            .oracle_price(q)?
            .checked_mul(Decimal::from_ratio(self.amount, Uint128::one()))?)
    }
}

impl OracleValue for NativeBalance {
    fn value_usd(&self, q: QuerierWrapper) -> Result<Decimal, OracleError> {
        self.clone()
            .into_vec()
            .iter()
            .try_fold(Decimal::zero(), |agg, v| Ok(v.value_usd(q)?.add(agg)))
    }
}

#[derive(Error, Debug)]
pub enum OracleError {
    #[error("{0}")]
    SecuredAsset(#[from] SecuredAssetError),
    #[error("{0}")]
    Pool(#[from] PoolError),
    #[error("{0}")]
    TryFromNetwork(#[from] TryFromNetworkError),
    #[error("{0}")]
    OraclePrice(#[from] OraclePriceError),
    #[error("{0}")]
    Overflow(#[from] OverflowError),
    #[error("Unavailable")]
    Unavailable {},
}
