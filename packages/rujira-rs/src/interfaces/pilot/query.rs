use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Decimal, Timestamp, Uint128};

use super::Denoms;

/// Standard interface to query contract state
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},

    #[returns(SimulationResponse)]
    Simulate(Coin),

    /// Find a specific order for a user at a premium
    #[returns(OrderResponse)]
    Order((String, u8)),

    /// Paginate user orders. Upper limit of 30 per page
    #[returns(OrdersResponse)]
    Orders {
        owner: String,
        offset: Option<u8>,
        limit: Option<u8>,
    },

    #[returns(PoolsResponse)]
    Pools {
        limit: Option<u8>,
        offset: Option<u8>,
    },
}

#[cw_serde]
pub struct ConfigResponse {
    /// See [InstantiateMsg::denoms]
    pub denoms: Denoms,

    /// See [InstantiateMsg::executor]
    pub executor: String,

    /// See [InstantiateMsg::fee_taker]    
    pub fee_taker: Decimal,

    /// See [InstantiateMsg::fee_maker]
    pub fee_maker: Decimal,

    /// See [InstantiateMsg::fee_address]
    pub fee_address: String,
}

#[cw_serde]
pub struct OrderResponse {
    /// The account which placed the order
    pub owner: String,

    /// The premium price of this order, in %
    pub premium: u8,

    /// The rate at which this order would execute at the current moment in time
    pub rate: Decimal,

    /// The last time this order was touched (created, incremented or reduced) in an Order execution
    pub updated_at: Timestamp,

    /// Offer amount at updated_at time
    pub offer: Uint128,

    /// The remaining offer amount
    pub remaining: Uint128,

    /// Amount of filled order awaiting withdrawal
    pub filled: Uint128,
}

#[cw_serde]
pub struct OrdersResponse {
    pub orders: Vec<OrderResponse>,
}

#[cw_serde]
pub struct PoolsResponse {
    pub pools: Vec<PoolResponse>,
}

#[cw_serde]
pub struct PoolResponse {
    pub premium: u8,
    pub epoch: u32,
    pub price: Decimal,
    pub total: Uint128,
}

#[cw_serde]
pub struct SimulationResponse {
    pub returned: Uint128,
    pub fee: Uint128,
}
