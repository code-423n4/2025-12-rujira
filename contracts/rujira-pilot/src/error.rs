use cosmwasm_std::{
    CheckedFromRatioError, Coin, ConversionOverflowError, OverflowError, StdError, Uint128,
};
use cw_utils::PaymentError;
use rujira_rs::{bid_pool::BidPoolError, exchange::SwapError, query::PoolError};
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
    ConversionOverflow(#[from] ConversionOverflowError),

    #[error("{0}")]
    BidPool(#[from] BidPoolError),

    #[error("{0}")]
    Pool(#[from] PoolError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("{0}")]
    Swap(#[from] SwapError),

    #[error("InsufficientReturn expected {expected} got {returned}")]
    InsufficientReturn {
        expected: Uint128,
        returned: Uint128,
    },

    #[error("InsufficientFunds expected {expected} got {returned}")]
    InsufficientFunds { expected: Coin, returned: Coin },

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("NotFound")]
    NotFound {},

    #[error("Invalid Premium: {premium}")]
    InvalidPremium { premium: u8 },
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
