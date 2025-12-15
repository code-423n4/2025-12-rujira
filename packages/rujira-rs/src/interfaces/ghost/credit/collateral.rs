use cosmwasm_schema::cw_serde;
use cosmwasm_std::{coin, Coin, Decimal, Deps};
use cw_utils::NativeBalance;
use std::ops::{Add, Mul};
use std::{collections::BTreeMap, fmt::Display};
use thiserror::Error;

use crate::{OracleError, OracleValue, SecuredAsset, SecuredAssetError};

#[cw_serde]
pub enum Collateral {
    Coin(Coin),
}

impl Collateral {
    pub fn value_adjusted(
        &self,
        deps: Deps,
        ratios: &BTreeMap<String, Decimal>,
    ) -> Result<Decimal, CollateralError> {
        self.balance()
            .into_vec()
            .iter()
            .try_fold(Decimal::zero(), |agg, v| {
                Ok(v.value_usd(deps.querier)?
                    .mul(ratios.get(&v.denom).copied().unwrap_or_default())
                    .add(agg))
            })
    }

    pub fn balance(&self) -> NativeBalance {
        match self {
            Collateral::Coin(coin) => NativeBalance(vec![coin.clone()]),
        }
    }
}

impl OracleValue for Collateral {
    fn value_usd(&self, q: cosmwasm_std::QuerierWrapper) -> Result<Decimal, OracleError> {
        match self {
            Collateral::Coin(coin) => Ok(coin.value_usd(q)?),
        }
    }
}

impl Display for Collateral {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Collateral::Coin(coin) => write!(f, "coin:{}", coin),
        }
    }
}

impl TryFrom<&Coin> for Collateral {
    fn try_from(value: &Coin) -> Result<Self, CollateralError> {
        // Custom parsing of denom strings here to ensure we only have a secured asset
        // RUNE, RUJI etc tbc
        Ok(Self::Coin(coin(
            value.amount.u128(),
            SecuredAsset::from_denom(&value.denom)?.denom(),
        )))
    }

    type Error = CollateralError;
}

#[derive(Error, Debug)]
pub enum CollateralError {
    #[error("{0}")]
    Oracle(#[from] OracleError),
    #[error("{0}")]
    SecuredAsset(#[from] SecuredAssetError),
}
