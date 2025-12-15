use cosmwasm_schema::{cw_serde, QueryResponses};

use crate::Layer1Asset;

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(crate::query::Pool)]
    Pool(Layer1Asset),
}
