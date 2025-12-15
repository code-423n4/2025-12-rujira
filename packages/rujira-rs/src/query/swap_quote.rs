use std::{
    num::{ParseIntError, TryFromIntError},
    str::FromStr,
};

use super::grpc::{QueryError, Queryable};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{CanonicalAddr, QuerierWrapper, StdError, Timestamp, Uint128};
use thiserror::Error;

use crate::{
    asset::AssetError,
    coin::Coin,
    msg::deposit::MsgDeposit,
    proto::types::{QueryQuoteSwapRequest, QueryQuoteSwapResponse, QuoteFees},
    Asset,
};
#[cw_serde]
pub struct SwapQuote {
    pub from: Coin,

    /// the number of thorchain blocks the outbound will be delayed
    pub outbound_delay_blocks: u64,
    /// the approximate seconds for the outbound delay before it will be sent
    pub outbound_delay_seconds: u64,
    pub fees: Option<SwapQuoteFees>,
    /// expiration timestamp
    pub expiry: Timestamp,
    /// The recommended minimum inbound amount for this transaction type & inbound asset. Sending less than this amount could result in failed refunds.
    pub recommended_min_amount_in: Uint128,
    /// generated memo for the swap
    pub memo: String,
    /// the amount of the target asset the user can expect to receive after fees
    pub expected_amount_out: Uint128,

    pub max_streaming_quantity: Uint128,
    /// the number of blocks the streaming swap will execute over
    pub streaming_swap_blocks: u64,
    /// approx the number of seconds the streaming swap will execute over
    pub streaming_swap_seconds: u64,
    /// total number of seconds a swap is expected to take (inbound conf + streaming swap + outbound delay)
    pub total_swap_seconds: u64,
}

impl SwapQuote {
    pub fn to_msg(&self, signer: CanonicalAddr) -> MsgDeposit {
        MsgDeposit::new(vec![self.from.clone()], self.memo.clone(), signer)
    }
}

impl SwapQuote {
    fn from_response(
        q: &SwapQuoteQuery,
        value: QueryQuoteSwapResponse,
    ) -> Result<Self, TryFromSwapQuoteError> {
        Ok(Self {
            outbound_delay_blocks: value.outbound_delay_blocks.unsigned_abs(),
            outbound_delay_seconds: value.outbound_delay_seconds.unsigned_abs(),
            fees: value.fees.map(SwapQuoteFees::try_from).transpose()?,
            expiry: Timestamp::from_seconds(value.expiry.unsigned_abs()),
            recommended_min_amount_in: Uint128::from_str(value.recommended_min_amount_in.as_str())?,
            memo: value.memo,
            expected_amount_out: Uint128::from_str(value.expected_amount_out.as_str())?,
            max_streaming_quantity: Uint128::from_str(
                value.max_streaming_quantity.to_string().as_str(),
            )?,
            streaming_swap_blocks: value.streaming_swap_blocks.unsigned_abs(),
            streaming_swap_seconds: value.streaming_swap_seconds.unsigned_abs(),
            total_swap_seconds: value.total_swap_seconds.unsigned_abs(),
            from: Coin::new(q.from_asset.clone(), q.amount.into()),
        })
    }
}

#[cw_serde]
pub struct SwapQuoteQuery {
    pub from_asset: Asset,
    pub to_asset: Asset,
    pub amount: Uint128,
    pub streaming: Option<(u16, u32)>,
    pub destination: String,
    pub tolerance_bps: Option<u8>,
    pub refund_address: Option<String>,
    pub affiliates: Vec<(String, u8)>,
}

impl From<SwapQuoteQuery> for QueryQuoteSwapRequest {
    fn from(value: SwapQuoteQuery) -> Self {
        Self {
            from_asset: value.from_asset.to_string(),
            to_asset: value.to_asset.to_string(),
            amount: value.amount.to_string(),
            streaming_interval: value.streaming.map(|x| x.0.to_string()).unwrap_or_default(),
            streaming_quantity: value.streaming.map(|x| x.1.to_string()).unwrap_or_default(),
            destination: value.destination,
            tolerance_bps: value
                .tolerance_bps
                .map(|x| x.to_string())
                .unwrap_or_default(),
            refund_address: value.refund_address.unwrap_or_default(),
            affiliate: value.affiliates.iter().map(|x| x.0.clone()).collect(),
            affiliate_bps: value.affiliates.iter().map(|x| x.1.to_string()).collect(),
            height: "".to_string(),
        }
    }
}

#[cw_serde]
pub struct SwapQuoteFees {
    /// the target asset used for all fees
    pub asset: Asset,
    /// affiliate fee in the target asset
    pub affiliate: Uint128,
    /// outbound fee in the target asset
    pub outbound: Uint128,
    /// liquidity fees paid to pools in the target asset
    pub liquidity: Uint128,
    /// total fees in the target asset
    pub total: Uint128,
    /// the swap slippage in basis points
    pub slippage_bps: u16,
    /// total basis points in fees relative to amount out
    pub total_bps: u16,
}

impl TryFrom<QuoteFees> for SwapQuoteFees {
    type Error = TryFromSwapQuoteError;

    fn try_from(value: QuoteFees) -> Result<Self, Self::Error> {
        Ok(Self {
            asset: Asset::from_str(value.asset.as_str())?,
            affiliate: Uint128::from_str(value.affiliate.as_str())?,
            outbound: Uint128::from_str(value.outbound.as_str())?,
            liquidity: Uint128::from_str(value.liquidity.as_str())?,
            total: Uint128::from_str(value.total.as_str())?,
            slippage_bps: u16::from_str(value.slippage_bps.to_string().as_str())?,
            total_bps: u16::from_str(value.total_bps.to_string().as_str())?,
        })
    }
}

impl SwapQuoteQuery {
    pub fn quote(&self, q: QuerierWrapper) -> Result<SwapQuote, SwapQuoteError> {
        let req = QueryQuoteSwapRequest::from(self.clone());
        let res = QueryQuoteSwapResponse::get(q, req)?;
        Ok(SwapQuote::from_response(self, res)?)
    }
}

#[derive(Error, Debug)]
pub enum SwapQuoteError {
    #[error("{0}")]
    TryFrom(#[from] TryFromSwapQuoteError),
    #[error("{0}")]
    Query(#[from] QueryError),
}

#[derive(Error, Debug)]
pub enum TryFromSwapQuoteError {
    #[error("{0}")]
    Std(#[from] StdError),
    #[error("{0}")]
    TryFromInt(#[from] TryFromIntError),
    #[error("{0}")]
    ParseInt(#[from] ParseIntError),
    #[error("{0}")]
    Asset(#[from] AssetError),
}
