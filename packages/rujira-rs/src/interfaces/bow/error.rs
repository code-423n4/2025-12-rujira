use cosmwasm_std::{Coin, ConversionOverflowError, OverflowError, StdError};
use cw_utils::PaymentError;
use thiserror::Error;

use crate::asset::Layer1AssetError;

#[derive(Error, Debug)]
pub enum StrategyError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("{0}")]
    ConversionOverflow(#[from] ConversionOverflowError),

    #[error("{0}")]
    Layer1Asset(#[from] Layer1AssetError),

    #[error("InvalidPair")]
    InvalidPair {},

    #[error("InvalidRoute")]
    InvalidRoute {},

    #[error("InvalidDeposit")]
    InvalidDeposit {},

    #[error("Underflow")]
    Underflow {},

    #[error("InvalidStrategyState")]
    InvalidStrategyState {},

    #[error("InsufficientReturn expected {expected} got {returned}")]
    InsufficientReturn { expected: Coin, returned: Coin },

    #[error("Invalid Config {0}")]
    InvalidConfig(String),
}
