use std::str::FromStr;
use std::{num::ParseIntError, ops::Div};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, QuerierWrapper, StdError, Uint128};
use thiserror::Error;

use crate::proto::types::{QueryNetworkRequest, QueryNetworkResponse};

use super::grpc::{QueryError, Queryable};

#[cw_serde]
pub struct Network {
    /// total amount of RUNE awarded to node operators
    pub bond_reward_rune: Uint128,
    /// total bonded RUNE
    pub total_bond_units: Uint128,
    /// effective security bond used to determine maximum pooled RUNE
    pub effective_security_bond: Uint128,
    /// total reserve RUNE
    pub total_reserve: Uint128,
    /// Returns true if there exist RetiringVaults which have not finished migrating funds to new ActiveVaults
    pub vaults_migrating: bool,
    /// Sum of the gas the network has spent to send outbounds
    pub gas_spent_rune: Uint128,
    /// Sum of the gas withheld from users to cover outbound gas
    pub gas_withheld_rune: Uint128,
    /// Current outbound fee multiplier, in basis points
    pub outbound_fee_multiplier: u16,
    /// the outbound transaction fee in rune, converted from the NativeOutboundFeeUSD mimir (after USD fees are enabled)
    pub native_outbound_fee_rune: Uint128,
    /// the native transaction fee in rune, converted from the NativeTransactionFeeUSD mimir (after USD fees are enabled)
    pub native_tx_fee_rune: Uint128,
    /// the thorname register fee in rune, converted from the TNSRegisterFeeUSD mimir (after USD fees are enabled)
    pub tns_register_fee_rune: Uint128,
    /// the thorname fee per block in rune, converted from the TNSFeePerBlockUSD mimir (after USD fees are enabled)
    pub tns_fee_per_block_rune: Uint128,
    /// the rune price in tor
    pub rune_price_in_tor: Decimal,
    /// the tor price in rune
    pub tor_price_in_rune: Decimal,
}

impl TryFrom<QueryNetworkResponse> for Network {
    type Error = TryFromNetworkError;

    fn try_from(value: QueryNetworkResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            bond_reward_rune: Uint128::from_str(value.bond_reward_rune.as_str())?,
            total_bond_units: Uint128::from_str(value.total_bond_units.as_str())?,
            effective_security_bond: Uint128::from_str(value.effective_security_bond.as_str())?,
            total_reserve: Uint128::from_str(value.total_reserve.as_str())?,
            vaults_migrating: value.vaults_migrating,
            gas_spent_rune: Uint128::from_str(value.gas_spent_rune.as_str())?,
            gas_withheld_rune: Uint128::from_str(value.gas_withheld_rune.as_str())?,
            outbound_fee_multiplier: u16::from_str(value.outbound_fee_multiplier.as_str())?,
            native_outbound_fee_rune: Uint128::from_str(value.native_outbound_fee_rune.as_str())?,
            native_tx_fee_rune: Uint128::from_str(value.native_tx_fee_rune.as_str())?,
            tns_register_fee_rune: Uint128::from_str(value.tns_register_fee_rune.as_str())?,
            tns_fee_per_block_rune: Uint128::from_str(value.tns_fee_per_block_rune.as_str())?,
            rune_price_in_tor: Decimal::from_str(value.rune_price_in_tor.as_str())?
                .div(Uint128::from(10u128).pow(8)),
            tor_price_in_rune: Decimal::from_str(value.tor_price_in_rune.as_str())?
                .div(Uint128::from(10u128).pow(8)),
        })
    }
}

impl Network {
    pub fn load(q: QuerierWrapper) -> Result<Self, TryFromNetworkError> {
        let req = QueryNetworkRequest {
            height: "0".to_string(),
        };
        let res = QueryNetworkResponse::get(q, req)?;
        Network::try_from(res)
    }
}

#[derive(Error, Debug)]
pub enum TryFromNetworkError {
    #[error("{0}")]
    Std(#[from] StdError),
    #[error("{0}")]
    ParseInt(#[from] ParseIntError),
    #[error("{0}")]
    Query(#[from] QueryError),
}
