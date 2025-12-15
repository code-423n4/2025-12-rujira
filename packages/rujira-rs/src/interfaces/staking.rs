use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Decimal, Uint128};

use crate::TokenMetadata;

#[cw_serde]
pub struct InstantiateMsg {
    pub bond_denom: String,
    pub revenue_denom: String,
    pub receipt_token_metadata: TokenMetadata,
    /// Configuration to convert `revenue_denom` into `bond_denom`
    ///
    /// `(contract_address, execute_msg, threshold_limit)`
    pub revenue_converter: (String, Binary, Uint128),
    pub fee: Option<(Decimal, String)>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Actions relating to account-based yield
    Account(AccountMsg),
    /// Actions relating to liquid staking
    Liquid(LiquidMsg),
}

#[cw_serde]
pub enum SudoMsg {
    /// Update the configuration of the revenue converter
    SetRevenueConverter {
        contract: String,
        msg: Binary,
        limit: Uint128,
    },
}

#[cw_serde]
pub enum AccountMsg {
    /// Bond [InstantiateMsg::bond_denom] tokens to an account
    Bond {},
    /// Claim earned revenue for all bonded tokens
    Claim {},
    /// Withdraw `amount` or all of [InstantiateMsg::bond_denom] from the sender's account
    /// This will also claim all revenue
    Withdraw { amount: Option<Uint128> },
}

#[cw_serde]
pub enum LiquidMsg {
    /// Bond [InstantiateMsg::bond_denom] tokens, retuning minted Share tokens
    Bond {},
    /// Burn the provided Share tokens, returning the share of the underlying pooled assets
    Unbond {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},

    #[returns(StatusResponse)]
    Status {},

    #[returns(AccountResponse)]
    Account { addr: String },
}

#[cw_serde]
pub struct ConfigResponse {
    pub bond_denom: String,
    pub revenue_denom: String,
    pub revenue_converter: (String, Binary, Uint128),
    pub fee: Option<(Decimal, String)>,
}

#[cw_serde]
pub struct StatusResponse {
    /// The amount of [InstantiateMsg::bond_denom] bonded in Accounts
    pub account_bond: Uint128,

    /// The total amount of [InstantiateMsg::revenue_denom] available for Account staking to claim
    pub assigned_revenue: Uint128,

    /// The total shares issued for the liquid bonded tokens
    pub liquid_bond_shares: Uint128,

    /// The total size of the Share Pool of liquid bonded tokens
    pub liquid_bond_size: Uint128,

    /// The amount of [InstantiateMsg::revenue_denom] pending distribution
    pub undistributed_revenue: Uint128,
}

#[cw_serde]
pub struct AccountResponse {
    pub addr: String,
    pub bonded: Uint128,
    pub pending_revenue: Uint128,
}
