use std::num::TryFromIntError;

use cosmwasm_std::{CheckedFromRatioError, StdError, Uint128};
use cw_utils::PaymentError;
use rujira_rs::{
    query::{grpc::QueryError, OutboundFeeError, PoolError, SwapQuoteError},
    AssetError, Layer1AssetError, SecuredAssetError, SharePoolError,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("{0}")]
    CheckedFromRatio(#[from] CheckedFromRatioError),

    #[error("{0}")]
    SharePool(#[from] SharePoolError),

    #[error("{0}")]
    SwapQuote(#[from] SwapQuoteError),

    #[error("{0}")]
    Asset(#[from] AssetError),

    #[error("{0}")]
    Layer1Asset(#[from] Layer1AssetError),

    #[error("{0}")]
    SecuredAsset(#[from] SecuredAssetError),

    #[error("{0}")]
    Pool(#[from] PoolError),

    #[error("{0}")]
    OutboundFee(#[from] OutboundFeeError),

    #[error("{0}")]
    Query(#[from] QueryError),

    #[error("{0}")]
    TryFromInt(#[from] TryFromIntError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("InsufficientFunds")]
    InsufficientFunds {},
    #[error("InsufficientReturn asked {asked} quoted {quoted} liquidity {liquidity} outbound {outbound}")]
    InsufficientReturn {
        quoted: Uint128,
        asked: Uint128,
        liquidity: Uint128,
        outbound: Uint128,
    },

    #[error("Invalid Route")]
    InvalidRoute {},

    #[error("Invalid: {0}")]
    Invalid(String),
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
