use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Coin, Decimal, Uint128};

use crate::{CallbackData, TokenMetadata};

use super::{strategy::Strategies, xyk::XykState, Xyk};

#[cw_serde]
pub struct InstantiateMsg {
    pub metadata: TokenMetadata,
    pub strategy: Strategies,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Executes a market trade based on current order book.
    Swap {
        min_return: Coin,
        to: Option<String>,
        callback: Option<CallbackData>,
    },
    Deposit {
        /// The minimum amount of LP shares to be returned
        min_return: Option<Uint128>,
        callback: Option<CallbackData>,
    },
    Withdraw {
        callback: Option<CallbackData>,
    },
}

#[cw_serde]
pub enum SudoMsg {
    SetStrategy(Strategies),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(StrategyResponse)]
    Strategy {},
    #[returns(QuoteResponse)]
    Quote(QuoteRequest),
}

#[cw_serde]
pub struct QuoteRequest {
    /// The minimum price that BOW must respond with
    /// This is used eg in FIN to reay the previous price quoted for and in
    /// ORCA to set the min price of the next premium
    pub min_price: Option<Decimal>,

    /// Denom that the Caller is offering
    /// ie the token that will be sent by FIN to BOW
    /// Eg. if a market order USDC buy of RUJI on FIN is being
    /// executed, FIN will require RUJI to execute the swap and so
    /// the offer_denom here will be USDC and the ask_denom RUJI
    pub offer_denom: String,

    /// The denom that the Caller is asking for
    pub ask_denom: String,

    /// Optional binary data that should be pass-through from QuoteResponse::data
    pub data: Option<Binary>,
}

#[cw_serde]
pub struct QuoteResponse {
    /// The price of the offer
    /// Quoted in ask_denom, so eg for RUIJ @ $5,
    /// an offer of RUJI for ask of USDC, price = 5
    /// an offer of USDC for ask of RUJI, price = 0.2
    ///
    pub price: Decimal,

    /// The size of the ask_denom that the market maker is willing to accept
    /// at the QuoteResponse::price
    pub size: Uint128,

    /// Optionally provide arbitrary binary data that is returned in subsequent QuoteQuery's
    /// Provided in order to allow the market maker to load from storage only once, instead of
    /// for each iteration of the query
    pub data: Option<Binary>,
}

#[cw_serde]
pub enum StrategyResponse {
    Xyk((Xyk, XykState)),
}
