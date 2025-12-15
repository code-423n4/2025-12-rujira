use super::grpc::{QueryError, Queryable};
use crate::{
    asset::Layer1AssetError,
    proto::{
        self,
        types::{QueryPoolRequest, QueryPoolResponse},
    },
    Asset, Layer1Asset,
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, QuerierWrapper, StdError, StdResult, Uint128};
use std::{
    num::{ParseIntError, TryFromIntError},
    ops::Div,
    str::FromStr,
};
use thiserror::Error;

#[cw_serde]
pub enum PoolStatus {
    Unknown,
    Available,
    Staged,
    Suspended,
}

impl TryFrom<String> for PoolStatus {
    type Error = StdError;
    fn try_from(value: String) -> StdResult<Self> {
        value.as_str().try_into()
    }
}

impl TryFrom<&String> for PoolStatus {
    type Error = StdError;
    fn try_from(value: &String) -> StdResult<Self> {
        value.as_str().try_into()
    }
}

impl TryFrom<&str> for PoolStatus {
    type Error = StdError;
    fn try_from(value: &str) -> StdResult<Self> {
        match proto::types::PoolStatus::from_str_name(value) {
            Some(proto::types::PoolStatus::Available) => Ok(PoolStatus::Available),
            Some(proto::types::PoolStatus::Staged) => Ok(PoolStatus::Staged),
            Some(proto::types::PoolStatus::UnknownPoolStatus) => Ok(PoolStatus::Unknown),
            Some(proto::types::PoolStatus::Suspended) => Ok(PoolStatus::Suspended),
            None => Err(StdError::generic_err(format!(
                "Invalid PoolStatus {}",
                value
            ))),
        }
    }
}

impl FromStr for PoolStatus {
    type Err = StdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.try_into()
    }
}

#[cw_serde]
pub struct Pool {
    pub asset: Asset,
    pub short_code: String,
    pub status: PoolStatus,
    pub decimals: u8,
    pub pending_inbound_asset: Uint128,
    pub pending_inbound_rune: Uint128,
    pub balance_asset: Uint128,
    pub balance_rune: Uint128,
    /// the USD (TOR) price of the asset in 1e8
    pub asset_tor_price: Decimal,
    /// the total pool units, this is the sum of LP and synth units
    pub pool_units: Uint128,
    /// the total pool liquidity provider units
    pub lp_units: Uint128,
    /// the total synth units in the pool
    pub synth_units: Uint128,
    /// the total supply of synths for the asset
    pub synth_supply: Uint128,
    /// the balance of L1 asset deposited into the Savers Vault
    pub savers_depth: Uint128,
    /// the number of units owned by Savers
    pub savers_units: Uint128,
    /// the filled savers capacity in basis points, 4500/10000 = 45%
    pub savers_fill_bps: u32,
    /// amount of remaining capacity in asset
    pub savers_capacity_remaining: Uint128,
    /// whether additional synths cannot be minted
    pub synth_mint_paused: bool,
    /// the amount of synth supply remaining before the current max supply is reached
    pub synth_supply_remaining: Uint128,
    /// the depth of the derived virtual pool relative to L1 pool (in basis points)
    pub derived_depth_bps: u32,
    pub trading_halted: bool,
}

impl TryFrom<QueryPoolResponse> for Pool {
    type Error = TryFromPoolError;
    fn try_from(v: QueryPoolResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            asset: Asset::Layer1(Layer1Asset::try_from(v.asset)?),
            short_code: v.short_code,
            status: PoolStatus::try_from(v.status)?,
            decimals: u8::try_from(v.decimals)?,
            pending_inbound_asset: Uint128::from_str(v.pending_inbound_asset.as_str())?,
            pending_inbound_rune: Uint128::from_str(v.pending_inbound_rune.as_str())?,
            balance_asset: Uint128::from_str(v.balance_asset.as_str())?,
            balance_rune: Uint128::from_str(v.balance_rune.as_str())?,
            asset_tor_price: Decimal::from_str(v.asset_tor_price.as_str())?
                .div(Uint128::from(10u128).pow(8)),
            pool_units: Uint128::from_str(v.pool_units.as_str())?,
            lp_units: Uint128::from_str(v.lp_units.as_str())?,
            synth_units: Uint128::from_str(v.synth_units.as_str())?,
            synth_supply: Uint128::from_str(v.synth_supply.as_str())?,
            savers_depth: Uint128::from_str(v.savers_depth.as_str())?,
            savers_units: Uint128::from_str(v.savers_units.as_str())?,
            savers_fill_bps: u32::from_str(&v.savers_fill_bps)?,
            savers_capacity_remaining: Uint128::from_str(v.savers_capacity_remaining.as_str())?,
            synth_mint_paused: v.synth_mint_paused,
            synth_supply_remaining: Uint128::from_str(v.synth_supply_remaining.as_str())?,
            derived_depth_bps: u32::from_str(v.derived_depth_bps.as_str())?,
            trading_halted: v.trading_halted,
        })
    }
}

#[derive(Error, Debug)]
pub enum TryFromPoolError {
    #[error("{0}")]
    Std(#[from] StdError),
    #[error("{0}")]
    TryFromInt(#[from] TryFromIntError),
    #[error("{0}")]
    ParseInt(#[from] ParseIntError),
    #[error("{0}")]
    Layer1Asset(#[from] Layer1AssetError),
}

impl Pool {
    pub fn load(q: QuerierWrapper, asset: &Layer1Asset) -> Result<Self, PoolError> {
        let req = QueryPoolRequest {
            asset: asset.to_string(),
            height: "0".to_string(),
        };
        let res = QueryPoolResponse::get(q, req)?;
        Ok(Pool::try_from(res)?)
    }
}

#[derive(Error, Debug)]
pub enum PoolError {
    #[error("{0}")]
    TryFrom(#[from] TryFromPoolError),
    #[error("{0}")]
    Query(#[from] QueryError),
}
