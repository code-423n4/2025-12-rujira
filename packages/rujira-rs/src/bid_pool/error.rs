use cosmwasm_std::{OverflowError, Uint256};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BidPoolError {
    #[error("WithdrawError {available}")]
    WithdrawError { available: Uint256 },

    #[error("DistributionError")]
    DistributionError {},

    #[error("{0}")]
    Overflow(#[from] OverflowError),
}
