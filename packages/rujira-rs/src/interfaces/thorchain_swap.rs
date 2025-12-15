use crate::{bow, ghost, CallbackData, CallbackMsg};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    to_json_binary, Addr, Coin, Decimal, QuerierWrapper, StdResult, Uint128, WasmMsg,
};

#[cw_serde]
pub struct InstantiateMsg {
    /// The maximum number of blocks that a settlement can be chunked into over a streaming swap
    pub max_stream_length: u32,
    /// The amount by which the `size` of the quote changes for each N of max_stream_length
    /// Allows the time-based price risk to be managed for quotes that settle over streaming swaps
    pub stream_step_ratio: Decimal,
    /// The premium charged on quotes to generate a local surplus
    pub reserve_fee: Decimal,
    /// The maximum percentage of a pool's depth that can be borrowed
    pub max_borrow_ratio: Decimal,
}

#[cw_serde]
pub struct Callback {
    /// Destination
    pub to: String,
    /// Original callback requested with Swap
    pub callback: Option<CallbackData>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Executes a market trade based on current order book.
    Swap {
        min_return: Coin,
        to: Option<String>,
        callback: Option<CallbackData>,
    },

    /// Only executes the loan repayment function
    Repay {},

    /// Called when a market returns funds
    Callback(CallbackMsg),
}

#[cw_serde]
pub enum SudoMsg {
    SetConfig(ConfigUpdate),
    SetMarket { addr: String, enabled: bool },
    SetVault { denom: String, vault: Option<Vault> },
}

#[cw_serde]
pub struct ConfigUpdate {
    pub max_stream_length: Option<u32>,
    pub stream_step_ratio: Option<Decimal>,
    pub max_borrow_ratio: Option<Decimal>,
    pub reserve_fee: Option<Decimal>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},

    #[returns(bow::QuoteResponse)]
    Quote(bow::QuoteRequest),

    #[returns(MarketsResponse)]
    Markets {},

    #[returns(VaultsResponse)]
    Vaults {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub max_stream_length: u32,
    pub stream_step_ratio: Decimal,
    pub max_borrow_ratio: Decimal,
    pub reserve_fee: Decimal,
}

#[cw_serde]
pub struct VaultsResponse {
    pub vaults: Vec<VaultResponse>,
}

#[cw_serde]
pub struct MarketsResponse {
    pub markets: Vec<Addr>,
}

#[cw_serde]
pub struct VaultResponse {
    pub denom: String,
    pub vault: Vault,
}

#[cw_serde]
pub struct Vault(Addr);

impl From<Addr> for Vault {
    fn from(value: Addr) -> Self {
        Self(value)
    }
}

impl Vault {
    pub fn borrow_msg(
        &self,
        amount: Uint128,
        to: String,
        callback: Option<CallbackData>,
    ) -> StdResult<WasmMsg> {
        Ok(WasmMsg::Execute {
            contract_addr: self.0.to_string(),
            msg: to_json_binary(&ghost::vault::ExecuteMsg::Market(
                ghost::vault::MarketMsg::Borrow {
                    amount,
                    callback: Some(to_json_binary(&Callback { to, callback })?.into()),
                    delegate: None,
                },
            ))?,
            funds: vec![],
        })
    }

    pub fn repay_msg(&self, amount: Coin) -> StdResult<WasmMsg> {
        Ok(WasmMsg::Execute {
            contract_addr: self.0.to_string(),
            msg: to_json_binary(&ghost::vault::ExecuteMsg::Market(
                ghost::vault::MarketMsg::Repay { delegate: None },
            ))?,
            funds: vec![amount],
        })
    }

    pub fn debt(&self, q: QuerierWrapper, owner: &Addr) -> StdResult<Uint128> {
        let res: ghost::vault::BorrowerResponse = q.query_wasm_smart(
            self.0.clone(),
            &ghost::vault::QueryMsg::Borrower {
                addr: owner.to_string(),
            },
        )?;
        Ok(res.current)
    }

    pub fn available(&self, q: QuerierWrapper, owner: &Addr) -> StdResult<Uint128> {
        let res: ghost::vault::BorrowerResponse = q.query_wasm_smart(
            self.0.clone(),
            &ghost::vault::QueryMsg::Borrower {
                addr: owner.to_string(),
            },
        )?;
        Ok(res.available)
    }

    pub fn addr(&self) -> Addr {
        self.0.clone()
    }
}
