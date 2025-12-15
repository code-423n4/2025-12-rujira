use cosmwasm_schema::cw_serde;
use cosmwasm_std::Decimal;

#[cw_serde]
pub enum SudoMsg {
    UpdateConfig {
        fee_taker: Option<Decimal>,
        fee_maker: Option<Decimal>,
        fee_address: Option<String>,
    },
}
