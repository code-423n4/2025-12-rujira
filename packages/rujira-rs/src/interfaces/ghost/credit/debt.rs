use cosmwasm_schema::cw_serde;
use cosmwasm_std::{coins, Coin, Decimal};
use cw_utils::NativeBalance;
use thiserror::Error;

use crate::{ghost::vault::DelegateResponse, OracleError, OracleValue};

#[cw_serde]
pub struct Debt(DelegateResponse);

impl From<DelegateResponse> for Debt {
    fn from(value: DelegateResponse) -> Self {
        Self(value)
    }
}

impl Debt {
    /// Determine whether a receive event matches the debt
    pub fn can_accept(&self, coin: &Coin) -> bool {
        coin.denom == self.0.borrower.denom && coin.amount.le(&self.0.current)
    }
}

impl OracleValue for Debt {
    fn value_usd(&self, q: cosmwasm_std::QuerierWrapper) -> Result<Decimal, OracleError> {
        self.0.value_usd(q)
    }
}

impl From<&Debt> for NativeBalance {
    fn from(value: &Debt) -> Self {
        Self(coins(
            value.0.current.u128(),
            value.0.borrower.denom.clone(),
        ))
    }
}

#[derive(Error, Debug)]
pub enum DebtError {
    #[error("{0}")]
    Oracle(#[from] OracleError),
}
