use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, StdError, StdResult};

use super::side::Side;

#[cw_serde]
pub struct Denoms([String; 2]);

impl Denoms {
    pub fn new(base: &str, quote: &str) -> Self {
        Self([base.to_string(), quote.to_string()])
    }

    pub fn base(&self) -> &str {
        &self.0[0]
    }

    pub fn quote(&self) -> &str {
        &self.0[1]
    }

    pub fn ask(&self, side: &Side) -> &str {
        match side {
            Side::Base => self.quote(),
            Side::Quote => self.base(),
        }
    }

    pub fn bid(&self, side: &Side) -> &str {
        match side {
            Side::Base => self.base(),
            Side::Quote => self.quote(),
        }
    }

    /// Gets the side of the book that this coin is the ask token for
    pub fn ask_side(&self, coin: &Coin) -> StdResult<Side> {
        if coin.denom == self.base() {
            return Ok(Side::Quote);
        }

        if coin.denom == self.quote() {
            return Ok(Side::Base);
        }

        Err(StdError::generic_err("invalid denom"))
    }

    pub fn validate(&self) -> StdResult<()> {
        if self.0[0] == self.0[1] {
            return Err(StdError::generic_err("identical denoms"));
        }
        Ok(())
    }
}
