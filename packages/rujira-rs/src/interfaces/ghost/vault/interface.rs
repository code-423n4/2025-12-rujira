use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    coin, to_json_binary, Addr, Coin, CosmosMsg, Decimal, QuerierWrapper, StdResult, Timestamp,
    Uint128, WasmMsg,
};

use crate::{CallbackData, OracleError, OracleValue, TokenMetadata};

use super::interest::Interest;

#[cw_serde]
pub struct InstantiateMsg {
    /// The denom string that can be deposited and lent
    pub denom: String,
    pub interest: Interest,
    pub receipt: TokenMetadata,
    pub fee: Decimal,
    pub fee_address: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Deposit the borrowable asset into the money market.
    Deposit { callback: Option<CallbackData> },
    /// Withdraw the borrowable asset from the money market.
    Withdraw { callback: Option<CallbackData> },
    /// Privileged Msgs for whitelisted contracts
    Market(MarketMsg),
}

#[cw_serde]
pub enum MarketMsg {
    /// Borrow the borrowable asset from the money market. Only callable by whitelisted market contracts.
    Borrow {
        amount: Uint128,
        callback: Option<CallbackData>,
        /// optional delegate address for the debt obligation to be allocated to
        delegate: Option<String>,
    },
    /// Repay a borrow. Only callable by whitelisted market contracts.
    Repay {
        /// Optionally repay a delegate's debt obligation instead of the caller's
        delegate: Option<String>,
    },
}

#[cw_serde]
pub enum SudoMsg {
    SetBorrower { contract: String, limit: Uint128 },
    SetInterest(Interest),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},

    #[returns(StatusResponse)]
    Status {},

    #[returns(BorrowerResponse)]
    Borrower { addr: String },

    #[returns(DelegateResponse)]
    Delegate { borrower: String, addr: String },

    #[returns(BorrowersResponse)]
    Borrowers {
        limit: Option<u8>,
        start_after: Option<String>,
    },
}

#[cw_serde]
pub struct ConfigResponse {
    pub denom: String,
    pub interest: Interest,
}

#[cw_serde]
pub struct StatusResponse {
    pub last_updated: Timestamp,

    pub utilization_ratio: Decimal,

    pub debt_rate: Decimal,

    pub lend_rate: Decimal,
    // Share pool that accounts for accrued debt interest
    pub debt_pool: PoolResponse,
    // Share pool that allocated collected debt interest to lenders
    pub deposit_pool: PoolResponse,
}

#[cw_serde]
pub struct PoolResponse {
    /// The total deposits into the pool
    pub size: Uint128,
    /// The total ownership of the pool
    pub shares: Uint128,
    /// Ratio of shares / size
    pub ratio: Decimal,
}

#[cw_serde]
pub struct BorrowerResponse {
    pub addr: String,
    /// The denom being borrowed
    pub denom: String,
    /// The borrower's borrow limit
    pub limit: Uint128,
    /// The borrower's current utilization
    pub current: Uint128,
    /// The shares allocated to the current debt
    pub shares: Uint128,
    /// The remaining amount of borrowable funds for this borrower
    pub available: Uint128,
}

#[cw_serde]
pub struct BorrowersResponse {
    pub borrowers: Vec<BorrowerResponse>,
}

#[cw_serde]
pub struct DelegateResponse {
    pub borrower: BorrowerResponse,
    pub addr: String,
    /// The borrower's current utilization
    pub current: Uint128,
    /// The shares allocated to the current debt
    pub shares: Uint128,
}

impl OracleValue for DelegateResponse {
    fn value_usd(&self, q: QuerierWrapper) -> Result<Decimal, OracleError> {
        coin(self.current.u128(), &self.borrower.denom).value_usd(q)
    }
}

#[cw_serde]
pub struct Vault(Addr);

impl From<&Addr> for Vault {
    fn from(value: &Addr) -> Self {
        Self(value.clone())
    }
}

impl Vault {
    pub fn config(&self, q: QuerierWrapper) -> StdResult<ConfigResponse> {
        q.query_wasm_smart(self.0.to_string(), &QueryMsg::Config {})
    }

    pub fn borrower(&self, q: QuerierWrapper, addr: &Addr) -> StdResult<BorrowerResponse> {
        q.query_wasm_smart(
            self.0.to_string(),
            &QueryMsg::Borrower {
                addr: addr.to_string(),
            },
        )
    }
    pub fn delegate(
        &self,
        q: QuerierWrapper,
        borrower: &Addr,
        addr: &Addr,
    ) -> StdResult<DelegateResponse> {
        q.query_wasm_smart(
            self.0.to_string(),
            &QueryMsg::Delegate {
                addr: addr.to_string(),
                borrower: borrower.to_string(),
            },
        )
    }

    pub fn market_msg(&self, msg: MarketMsg, funds: Vec<Coin>) -> StdResult<CosmosMsg> {
        Ok(WasmMsg::Execute {
            contract_addr: self.0.to_string(),
            msg: to_json_binary(&ExecuteMsg::Market(msg))?,
            funds,
        }
        .into())
    }

    pub fn market_msg_repay(
        &self,
        delegate: Option<String>,
        amount: &Coin,
    ) -> StdResult<CosmosMsg> {
        self.market_msg(MarketMsg::Repay { delegate }, vec![amount.clone()])
    }

    pub fn market_msg_borrow(
        &self,
        delegate: Option<String>,
        callback: Option<CallbackData>,
        amount: &Coin,
    ) -> StdResult<CosmosMsg> {
        self.market_msg(
            MarketMsg::Borrow {
                amount: amount.amount,
                callback,
                delegate,
            },
            vec![],
        )
    }
}
