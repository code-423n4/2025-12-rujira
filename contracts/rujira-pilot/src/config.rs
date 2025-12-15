use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, Decimal, StdResult, Storage};
use cw_storage_plus::Item;
use rujira_rs::pilot::{ConfigResponse, Denoms, InstantiateMsg};

pub static CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct Config {
    pub denoms: Denoms,
    pub executor: Addr,
    pub max_premium: u8,
    pub fee_maker: Decimal,
    pub fee_taker: Decimal,
    pub fee_address: Addr,
}

impl Config {
    pub fn new(api: &dyn Api, value: InstantiateMsg) -> StdResult<Self> {
        Ok(Self {
            denoms: value.denoms.clone(),
            max_premium: value.max_premium,
            executor: api.addr_validate(&value.executor)?,
            fee_taker: value.fee_taker,
            fee_maker: value.fee_maker,
            fee_address: api.addr_validate(value.fee_address.as_str())?,
        })
    }

    pub fn validate(&self) -> StdResult<()> {
        Ok(())
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG.save(storage, self)
    }

    pub fn update(
        &mut self,
        fee_taker: Option<Decimal>,
        fee_maker: Option<Decimal>,
        fee_address: Option<Addr>,
    ) {
        if let Some(fee_taker) = fee_taker {
            self.fee_taker = fee_taker;
        }
        if let Some(fee_maker) = fee_maker {
            self.fee_maker = fee_maker;
        }
        if let Some(fee_address) = fee_address {
            self.fee_address = fee_address;
        }
    }
}

impl From<Config> for ConfigResponse {
    fn from(value: Config) -> Self {
        Self {
            denoms: value.denoms,
            executor: value.executor.to_string(),
            fee_maker: value.fee_maker,
            fee_taker: value.fee_taker,
            fee_address: value.fee_address.to_string(),
        }
    }
}
