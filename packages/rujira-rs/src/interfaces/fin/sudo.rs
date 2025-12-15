use cosmwasm_schema::cw_serde;
use cosmwasm_std::Decimal;

use crate::Layer1Asset;

use super::Tick;

#[cw_serde]
pub enum SudoMsg {
    UpdateConfig {
        tick: Option<Tick>,
        fee_taker: Option<Decimal>,
        fee_maker: Option<Decimal>,
        fee_address: Option<String>,
        market_makers: Option<Vec<String>>,
        oracles: Option<[Layer1Asset; 2]>,
    },
}
