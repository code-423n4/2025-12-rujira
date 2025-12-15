use std::collections::BTreeMap;

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    coin, to_json_binary, Addr, Binary, Coin, CosmosMsg, Decimal, StdResult, Uint128, WasmMsg,
};
use cw_utils::NativeBalance;

#[cw_serde]
pub struct InstantiateMsg {
    /// The Code ID for rujira-account
    pub code_id: u64,
    /// The fee charged when debt repayments are made in the course of a liquidation
    pub fee_liquidation: Decimal,
    /// The fee earned by the executor for solving a liquidation route
    pub fee_liquidator: Decimal,
    /// The destination for liquidation fees
    pub fee_address: Addr,
    /// The maximum slippage in $ value permitted when exchanging collateral for debt tokens during a liquidation
    pub liquidation_max_slip: Decimal,
    /// The collteralization ratio above which the position can be liquidated
    pub liquidation_threshold: Decimal,
    /// The maximum collteralization ratio that an Account owner can manually adjust to
    pub adjustment_threshold: Decimal,
}

#[cw_serde]
pub enum ExecuteMsg {
    Create {
        /// Provide a salt to create predictable Account addresses
        salt: Binary,
        /// Custom label to append to the Account contract on instantiation
        label: String,
        /// Tag to allow filtering of accounts when queried
        tag: String,
    },

    /// Executes msgs on the Account on behalf of the owner
    /// The account must have collateralizaion ratio < 1 after the message has been executed, to succeed
    Account { addr: String, msgs: Vec<AccountMsg> },

    /// NOOP function that checks position health against adjustment_threshold
    CheckAccount { addr: String },

    /// Liquidate the credit account
    /// Can only be called if the account is above a LTV of 1
    /// Will only succeed if the collateralizaion ratio drops either below 1, or by max_liquidate, whichever is smaller
    Liquidate {
        addr: String,
        msgs: Vec<LiquidateMsg>,
    },

    /// Internal entrypoint used to process LiquidateMsg's in sequence. Checks:
    ///     - Previous step against config.liquidation_max_slip
    /// This allows logic to eg read balances following prior LiquidateMsg executions
    /// If liquidation critera are met, then the execution of the queue halts:
    ///     - Account adjusted_ltv < config.liquidation_threshold
    ///     - Account adjusted_ltv >= adjustment_threshold
    /// If queue is empty then final check is made:
    ///     - Collaterals have all strictly decreased; no overliquidations
    DoLiquidate {
        addr: String,
        /// Vec of (msg, is_preference)
        /// When is_preference is set, errors will be ignored, logged and the next message in the queue will be processed
        queue: Vec<(LiquidateMsg, bool)>,
        /// Arbitrary payload to pass through from initial account load to be delivered to CheckLiquidate
        payload: Binary,
    },
}

impl ExecuteMsg {
    pub fn call(&self, address: &Addr) -> StdResult<CosmosMsg> {
        Ok(WasmMsg::Execute {
            contract_addr: address.to_string(),
            msg: to_json_binary(self)?,
            funds: vec![],
        }
        .into())
    }
}

#[cw_serde]
pub enum AccountMsg {
    Borrow(Coin),
    Repay(Coin),
    Execute {
        contract_addr: String,
        msg: Binary,
        funds: Vec<Coin>,
    },
    Send {
        to_address: String,
        funds: Vec<Coin>,
    },
    Transfer(String),
    SetPreferenceMsgs(Vec<LiquidateMsg>),
    SetPreferenceOrder {
        denom: String,
        after: Option<String>,
    },
}

#[cw_serde]
pub enum LiquidateMsg {
    /// Repay all the balance of the denom provided
    Repay(String),
    Execute {
        contract_addr: String,
        msg: Binary,
        funds: Vec<Coin>,
    },
}

#[cw_serde]
pub enum SudoMsg {
    SetVault {
        address: String,
    },

    SetCollateral {
        denom: String,
        collateralization_ratio: Decimal,
    },

    UpdateConfig(ConfigUpdate),
}

#[cw_serde]
pub struct ConfigUpdate {
    pub code_id: Option<u64>,
    pub fee_liquidation: Option<Decimal>,
    pub fee_liquidator: Option<Decimal>,
    pub fee_address: Option<Addr>,
    pub liquidation_max_slip: Option<Decimal>,
    pub liquidation_threshold: Option<Decimal>,
    pub adjustment_threshold: Option<Decimal>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Query global config settings
    #[returns(ConfigResponse)]
    Config {},

    #[returns(crate::ghost::vault::BorrowersResponse)]
    Borrows {},

    /// Queries an account by idx
    #[returns(AccountResponse)]
    Account(String),

    /// Queries all accounts by an owner
    #[returns(AccountsResponse)]
    Accounts {
        owner: String,
        /// Optionally filter by a given tag
        tag: Option<String>,
    },

    /// Pages through all accounts, 100 at a time
    #[returns(AccountsResponse)]
    AllAccounts {
        /// Address of the Credit Account
        cursor: Option<String>,
        /// Number of accounts to return
        limit: Option<usize>,
    },

    /// Returns the predicted next account address for the given owner
    #[returns(Addr)]
    Predict { owner: String, salt: Binary },
}

#[cw_serde]
pub struct ConfigResponse {
    pub code_id: u64,
    pub collateral_ratios: BTreeMap<String, Decimal>,
    pub fee_liquidation: Decimal,
    pub fee_liquidator: Decimal,
    pub fee_address: Addr,
    pub liquidation_max_slip: Decimal,
    pub liquidation_threshold: Decimal,
    pub adjustment_threshold: Decimal,
}

#[cw_serde]
pub struct AccountsResponse {
    pub accounts: Vec<AccountResponse>,
}

#[cw_serde]
pub struct AccountResponse {
    pub owner: Addr,
    pub account: Addr,
    pub tag: String,
    pub collaterals: Vec<CollateralResponse>,
    pub debts: Vec<DebtResponse>,
    pub ltv: Decimal,
    pub liquidation_preferences: LiquidationPreferences,
}

#[cw_serde]
pub struct CollateralResponse {
    pub collateral: super::Collateral,
    pub value_full: Decimal,
    pub value_adjusted: Decimal,
}

#[cw_serde]
pub struct DebtResponse {
    pub debt: super::Debt,
    pub value: Decimal,
}

#[cw_serde]
pub struct ContractStatusResponse {
    pub contract: String,

    pub collateral_denom: String,

    pub debt_denom: String,

    /// Total amount of collateral deposited
    pub collateral_amount: Uint128,

    /// Total amount of debt issued
    pub debt_amount: Uint128,

    /// Current debt rate
    pub debt_rate: Decimal,

    /// Current debt limit
    pub debt_limit: Uint128,

    /// Maximum LTV value before the contract is allowed to begin liqudiating collateral
    pub max_ltv: Decimal,

    /// Current collateral price denmincated in debt token
    /// Adjusted for decimal delta between tokens
    pub price: Decimal,
}

/// User preferences that are enforced during a liquidation attempt
#[cw_serde]
#[derive(Default)]
pub struct LiquidationPreferences {
    /// A list of LiquidateMsg's that are injected into the
    /// start of a ExecuteMsg::Liquidate
    /// This is designed to enable an Account holder to
    /// have assurance over their liquidation route(s) in order
    /// to minimise slippage and
    ///
    /// These sub-messages are emitted as "Reply Always", and if the
    /// reply is an error state, we ignore the error.
    /// We can't have invalid messages blocking an account liquidation:
    /// User experience is the preference, but system solvency is the priority
    pub messages: Vec<LiquidateMsg>,

    /// A set of constraints that state:
    /// Liquidation of denom KEY is invalid whilst the account still owns denom VALUE
    /// This is designed to enable a set of preferences over which order collaterals can be liquidated,
    /// typically to constrain free-form liquidations once `messages` have been exhausted
    pub order: LiquidationPreferenceOrder,
}

#[cw_serde]
pub struct LiquidationPreferenceOrder {
    map: BTreeMap<String, String>,
    limit: u8,
}

impl Default for LiquidationPreferenceOrder {
    fn default() -> Self {
        Self {
            map: Default::default(),
            limit: 100,
        }
    }
}

impl LiquidationPreferenceOrder {
    pub fn insert(
        &mut self,
        key: String,
        value: String,
    ) -> Result<Option<String>, LiquidationPreferenceOrderError> {
        if self.map.len() >= self.limit.into() {
            return Err(LiquidationPreferenceOrderError::LimitReached(self.limit));
        }

        let res = self.map.insert(key, value.clone());
        // Check for circular constraints by ensuring dependency chain terminates
        for key in self.map.keys() {
            self.validate_chain(&value, key)?;
        }
        Ok(res)
    }

    pub fn remove(&mut self, key: &String) -> Option<String> {
        self.map.remove(key)
    }

    pub fn validate(
        &self,
        spent: &Coin,
        remaining: &NativeBalance,
    ) -> Result<(), LiquidationPreferenceOrderError> {
        for dep in self.dependencies(&spent.denom) {
            if remaining.has(&coin(0, &dep)) {
                return Err(LiquidationPreferenceOrderError::Invalid {
                    coin: spent.clone(),
                    before: dep,
                });
            }
        }
        Ok(())
    }

    fn dependencies(&self, key: &String) -> Vec<String> {
        match self.map.get(key) {
            Some(v) => {
                let mut deps = self.dependencies(v);
                deps.push(v.clone());
                deps
            }
            None => vec![],
        }
    }

    fn validate_chain(
        &self,
        key: &String,
        start: &String,
    ) -> Result<(), LiquidationPreferenceOrderError> {
        if key == start {
            return Err(LiquidationPreferenceOrderError::Circular(start.clone()));
        }
        match self.map.get(key) {
            None => Ok(()),
            Some(next) => self.validate_chain(next, start),
        }
    }
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum LiquidationPreferenceOrderError {
    #[error("Invalid liquidation attempted {coin} before {before}")]
    Invalid { coin: Coin, before: String },
    #[error("Circular preference found {0}")]
    Circular(String),
    #[error("Max preferences reached {0}")]
    LimitReached(u8),
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::coins;
    #[test]
    fn validate_self_reference() {
        let mut o = LiquidationPreferenceOrder::default();
        o.insert("BTC".to_string(), "BTC".to_string()).unwrap_err();
    }

    #[test]
    fn validate_transitive_reference() {
        let mut o = LiquidationPreferenceOrder::default();
        o.insert("A".to_string(), "B".to_string()).unwrap();
        o.insert("B".to_string(), "C".to_string()).unwrap();
        o.insert("C".to_string(), "A".to_string()).unwrap_err();
    }

    #[test]
    fn validate_transitive_validation() {
        let mut o = LiquidationPreferenceOrder::default();
        o.insert("A".to_string(), "B".to_string()).unwrap();
        o.insert("B".to_string(), "C".to_string()).unwrap();

        o.validate(&coin(100, "A"), &NativeBalance(coins(100, "C")))
            .unwrap_err();
    }
}
