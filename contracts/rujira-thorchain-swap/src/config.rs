use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, StdResult, Storage};
use cw_storage_plus::Item;
use rujira_rs::thorchain_swap::{ConfigResponse, ConfigUpdate, InstantiateMsg};

use crate::ContractError;

static CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct Config {
    pub max_stream_length: u32,
    pub stream_step_ratio: Decimal,
    pub max_borrow_ratio: Decimal,
    pub reserve_fee: Decimal,
}

impl From<InstantiateMsg> for Config {
    fn from(msg: InstantiateMsg) -> Self {
        Self {
            max_stream_length: msg.max_stream_length,
            stream_step_ratio: msg.stream_step_ratio,
            max_borrow_ratio: msg.max_borrow_ratio,
            reserve_fee: msg.reserve_fee,
        }
    }
}

impl Config {
    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        CONFIG.load(storage)
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        Ok(())
    }

    pub fn update(&mut self, update: &ConfigUpdate) {
        if let Some(max_stream_length) = update.max_stream_length {
            self.max_stream_length = max_stream_length;
        }
        if let Some(stream_step_ratio) = update.stream_step_ratio {
            self.stream_step_ratio = stream_step_ratio;
        }
        if let Some(max_borrow_ratio) = update.max_borrow_ratio {
            self.max_borrow_ratio = max_borrow_ratio;
        }
        if let Some(reserve_fee) = update.reserve_fee {
            self.reserve_fee = reserve_fee;
        }
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG.save(storage, self)
    }
}

impl From<Config> for ConfigResponse {
    fn from(value: Config) -> Self {
        Self {
            max_stream_length: value.max_stream_length,
            stream_step_ratio: value.stream_step_ratio,
            max_borrow_ratio: value.max_borrow_ratio,
            reserve_fee: value.reserve_fee,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        Config {
            max_stream_length: 1,
            max_borrow_ratio: Decimal::one(),
            reserve_fee: Decimal::from_ratio(10u128, 500u128),
            stream_step_ratio: Decimal::one(),
        }
        .validate()
        .unwrap();
    }
}
