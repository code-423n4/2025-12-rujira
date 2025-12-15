use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;

use crate::TokenMetadata;

#[cw_serde]
pub struct InstantiateMsg {
    pub id: String,
    pub metadata: TokenMetadata,
    pub amount: Uint128,
}
