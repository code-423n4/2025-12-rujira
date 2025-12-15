use cosmwasm_std::{CheckedMultiplyRatioError, ConversionOverflowError, StdError, Uint128};
use thiserror::Error;

use crate::bid_pool::BidPoolError;

#[derive(Error, Debug)]
pub enum SwapError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    ConversionOverflow(#[from] ConversionOverflowError),

    #[error("{0}")]
    CheckedMultiplyRatio(#[from] CheckedMultiplyRatioError),

    #[error("{0}")]
    BidPool(#[from] BidPoolError),

    #[error("InsufficientReturn expected {expected} got {returned}")]
    InsufficientReturn {
        expected: Uint128,
        returned: Uint128,
    },
}
