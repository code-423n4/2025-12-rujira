use crate::proto::types::{
    QueryMimirWithKeyRequest, QueryMimirWithKeyResponse, QueryNetworkRequest, QueryNetworkResponse,
    QueryOraclePriceRequest, QueryOraclePriceResponse, QueryOutboundFeeRequest,
    QueryOutboundFeeResponse, QueryPoolRequest, QueryPoolResponse, QueryQuoteSwapRequest,
    QueryQuoteSwapResponse,
};
use cosmwasm_std::{Binary, QuerierWrapper, StdError};
use prost::{DecodeError, EncodeError, Message};
use thiserror::Error;

pub trait QueryablePair {
    type Request: Message + Default;
    type Response: Message + Sized + Default;

    fn grpc_path() -> &'static str;
}

pub trait Queryable: Sized {
    type Pair: QueryablePair;

    fn get(
        querier: QuerierWrapper,
        req: <Self::Pair as QueryablePair>::Request,
    ) -> Result<Self, QueryError>;
}

impl<T> Queryable for T
where
    T: QueryablePair<Response = Self> + Message + Default,
{
    type Pair = T;

    fn get(
        querier: QuerierWrapper,
        req: <Self::Pair as QueryablePair>::Request,
    ) -> Result<Self, QueryError> {
        let mut buf = Vec::new();
        req.encode(&mut buf)?;
        let path = Self::grpc_path().to_string();
        let data = Binary::from(buf);
        let res = querier
            .query_grpc(path.clone(), data.clone())
            .map_err(|_| QueryError::Grpc { path, data })?
            .to_vec();
        Ok(Self::decode(&*res)?)
    }
}

#[derive(Error, Debug)]
pub enum QueryError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Encode(#[from] EncodeError),

    #[error("{0}")]
    Decode(#[from] DecodeError),

    #[error("GRPC query error: {path} {data}")]
    Grpc { path: String, data: Binary },
}

impl QueryablePair for QueryPoolResponse {
    type Request = QueryPoolRequest;
    type Response = QueryPoolResponse;

    fn grpc_path() -> &'static str {
        "/types.Query/Pool"
    }
}

impl QueryablePair for QueryNetworkResponse {
    type Request = QueryNetworkRequest;
    type Response = QueryNetworkResponse;

    fn grpc_path() -> &'static str {
        "/types.Query/Network"
    }
}

impl QueryablePair for QueryQuoteSwapResponse {
    type Request = QueryQuoteSwapRequest;
    type Response = QueryQuoteSwapResponse;

    fn grpc_path() -> &'static str {
        "/types.Query/QuoteSwap"
    }
}

impl QueryablePair for QueryOutboundFeeResponse {
    type Request = QueryOutboundFeeRequest;
    type Response = QueryOutboundFeeResponse;

    fn grpc_path() -> &'static str {
        "/types.Query/OutboundFee"
    }
}

impl QueryablePair for QueryOraclePriceResponse {
    type Request = QueryOraclePriceRequest;
    type Response = QueryOraclePriceResponse;

    fn grpc_path() -> &'static str {
        "/types.Query/OraclePrice"
    }
}

impl QueryablePair for QueryMimirWithKeyResponse {
    type Request = QueryMimirWithKeyRequest;
    type Response = QueryMimirWithKeyResponse;

    fn grpc_path() -> &'static str {
        "/types.Query/MimirWithKey"
    }
}
