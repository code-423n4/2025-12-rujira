use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, Binary, Decimal, StdResult, Storage, Uint128};
use cw_storage_plus::Item;
use rujira_rs::staking::{ConfigResponse, InstantiateMsg};

use crate::ContractError;

static CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct FeeConfig {
    pub percentage: Decimal,
    pub recipient: Addr,
}

#[cw_serde]
pub struct Config {
    pub bond_denom: String,
    pub revenue_denom: String,
    pub revenue_converter: (Addr, Binary, Uint128),
    pub fee: Option<FeeConfig>,
}

impl FeeConfig {
    pub fn new(api: &dyn Api, value: InstantiateMsg) -> StdResult<Option<Self>> {
        match value.fee {
            Some(fee) => Ok(Some(Self {
                percentage: fee.0,
                recipient: api.addr_validate(&fee.1)?,
            })),
            None => Ok(None),
        }
    }

    fn is_percentage(value: &Decimal) -> bool {
        value > &Decimal::zero() && value <= &Decimal::one()
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if !FeeConfig::is_percentage(&self.percentage) {
            return Err(ContractError::Invalid("fee_range".to_string()));
        }
        Ok(())
    }
}

impl Config {
    pub fn new(api: &dyn Api, value: InstantiateMsg) -> StdResult<Self> {
        Ok(Self {
            fee: FeeConfig::new(api, value.clone())?,
            bond_denom: value.bond_denom,
            revenue_denom: value.revenue_denom,
            revenue_converter: (
                api.addr_validate(&value.revenue_converter.0)?,
                value.revenue_converter.1,
                value.revenue_converter.2,
            ),
        })
    }

    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        CONFIG.load(storage)
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if self.bond_denom.is_empty() {
            return Err(ContractError::Invalid("ruji_denom".to_string()));
        }
        if self.revenue_denom.is_empty() {
            return Err(ContractError::Invalid("ruji_denom".to_string()));
        }
        if let Some(fee) = &self.fee {
            fee.validate()?;
        }

        Ok(())
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG.save(storage, self)
    }
}

impl From<Config> for ConfigResponse {
    fn from(value: Config) -> Self {
        Self {
            bond_denom: value.bond_denom,
            revenue_denom: value.revenue_denom,
            revenue_converter: (
                value.revenue_converter.0.to_string(),
                value.revenue_converter.1,
                value.revenue_converter.2,
            ),
            fee: value
                .fee
                .map(|fee| (fee.percentage, fee.recipient.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        Config {
            bond_denom: "".to_string(),
            revenue_denom: "uusdc".to_string(),
            revenue_converter: (Addr::unchecked(""), Binary::new(vec![0]), Uint128::one()),
            fee: None,
        }
        .validate()
        .unwrap_err();

        Config {
            bond_denom: "uruji".to_string(),
            revenue_denom: "".to_string(),
            revenue_converter: (Addr::unchecked(""), Binary::new(vec![0]), Uint128::one()),
            fee: None,
        }
        .validate()
        .unwrap_err();

        Config {
            bond_denom: "uruji".to_string(),
            revenue_denom: "uusdc".to_string(),
            revenue_converter: (Addr::unchecked(""), Binary::new(vec![0]), Uint128::one()),
            fee: None,
        }
        .validate()
        .unwrap();

        // Failing because fee_percentage > 1
        Config {
            bond_denom: "uruji".to_string(),
            revenue_denom: "uusdc".to_string(),
            revenue_converter: (Addr::unchecked(""), Binary::new(vec![0]), Uint128::one()),
            fee: Some(FeeConfig {
                percentage: Decimal::percent(150),
                recipient: Addr::unchecked(""),
            }),
        }
        .validate()
        .unwrap_err();

        Config {
            bond_denom: "uruji".to_string(),
            revenue_denom: "uusdc".to_string(),
            revenue_converter: (Addr::unchecked(""), Binary::new(vec![0]), Uint128::one()),
            fee: Some(FeeConfig {
                percentage: Decimal::percent(10),
                recipient: Addr::unchecked(""),
            }),
        }
        .validate()
        .unwrap();
    }
}
