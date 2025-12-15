use cosmwasm_schema::{cw_serde, QueryResponses};

/// Interfaces that the executor of a Pilot sale must conform to
#[cw_serde]
#[derive(QueryResponses)]
pub enum ExecutorQueryMsg {
    #[returns(cosmwasm_std::Decimal)]
    Price {},
}
