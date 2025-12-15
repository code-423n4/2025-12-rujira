use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Timestamp, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    /// The token being merged into RUJI
    pub merge_denom: String,

    /// The total supply of the merged token
    pub merge_supply: Uint128,

    /// The denom string for $RUJI
    pub ruji_denom: String,

    /// The total allocation of $RUJI to be distributed amongst `merge_denom` mergers
    pub ruji_allocation: Uint128,

    /// The end of the grace period where deposit ratio = `ruji_allocation / merge_supply`
    pub decay_starts_at: Timestamp,

    /// The end of the merge period, when deposit ratio = `0`
    pub decay_ends_at: Timestamp,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Send `[InstantiateMsg::merge_denom]` to the contract.
    /// This increments TOTAL_ALLOCATED by `deposit_value` where
    /// `deposit_value = deposit_amount * share_ratio * decay_factor`
    /// `share_ratio = ruji_allocation / merge_supply` and
    /// `decay_factor = (decay_ends_at - env.block.time) / (decay_ends_at - decay_starts_at)`
    /// `TOTAL_MERGED` is incremented by the quantity of tokens sent
    /// Finally, Shares are issued to info.sender such that the value of `[QueryMsg::StatusResponse] * (shares / total_shares) = deposit_value`,
    /// in order to maintain the current deposit_value and also accrue future increases of `[QueryMsg::StatusResponse]` proportionally based on share ownership
    Deposit {},

    /// Withdraws the amount `[InstantiateMsg::ruji_denom]` allocated to the `share_amount`, and decreases the Share Account of `info.sender`
    ///
    Withdraw { share_amount: Uint128 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},

    /// Queries the total amount of unallocated `[InstantiateMsg::ruji_denom]` that cannot mathematically be allocated to new deposits
    /// Unallocated $RUJI is dependent on individual Deposit actions. Each time there is a deposit, Shares are claimed at
    /// Defined as `amount = ([InstantiateMsg::ruji_allocation] - TOTAL_ALLOCATED) + (decay_factor * base_ratio * ([InstantiateMsg::merge_supply] - TOTAL_MERGED)` where
    /// `base_ratio = ruji_allocation / merge_supply` and
    /// `decay_factor = (env.block.time - [InstantiateMsg::decay_starts_at]) / ([InstantiateMsg::decay_ends_at] - [InstantiateMsg::decay_starts_at])`
    #[returns(StatusResponse)]
    Status {},

    #[returns(AccountResponse)]
    Account { addr: String },
}

#[cw_serde]
pub struct ConfigResponse {
    pub merge_denom: String,
    pub merge_supply: Uint128,
    pub ruji_denom: String,
    pub ruji_allocation: Uint128,
    pub decay_starts_at: Timestamp,
    pub decay_ends_at: Timestamp,
}

#[cw_serde]
pub struct StatusResponse {
    /// Total `[InstantiateMsg::merge_denom]` merged
    pub merged: Uint128,

    /// Total shares issued
    pub shares: Uint128,

    /// Total `[InstantiateMsg::ruji_denom]` allocated to shareholders
    pub size: Uint128,
}

#[cw_serde]
pub struct AccountResponse {
    pub addr: String,

    /// Total `[InstantiateMsg::merge_denom]` merged by this account
    pub merged: Uint128,

    /// Total shares issued
    pub shares: Uint128,

    /// Total `[InstantiateMsg::ruji_denom]` allocation that `shares` represents
    pub size: Uint128,
}
