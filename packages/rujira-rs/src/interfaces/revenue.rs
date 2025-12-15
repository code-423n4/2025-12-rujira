use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub executor: String,
    pub target_denoms: Vec<String>,
    pub target_addresses: Vec<(String, u8)>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Run {},
}

#[cw_serde]
pub enum SudoMsg {
    SetOwner(String),
    SetExecutor(String),
    SetAction {
        denom: String,
        /// The target contract for swapping
        contract: String,
        /// The maximum amount of the token that can be included in any one execution of the Action
        limit: Uint128,
        /// The msg executed on the contract to swap to the target token
        msg: Binary,
    },
    UnsetAction(String),
    AddTargetDenom(String),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(ActionsResponse)]
    Actions {},
    #[returns(StatusResponse)]
    Status {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: String,
    pub executor: String,
    pub target_denoms: Vec<String>,
    pub target_addresses: Vec<(String, u8)>,
}

#[cw_serde]
pub struct ActionsResponse {
    pub actions: Vec<ActionResponse>,
}
#[cw_serde]
pub struct ActionResponse {
    pub denom: String,
    pub contract: String,
    pub limit: Uint128,
    pub msg: Binary,
}

#[cw_serde]
pub struct StatusResponse {
    pub last: Option<String>,
}
