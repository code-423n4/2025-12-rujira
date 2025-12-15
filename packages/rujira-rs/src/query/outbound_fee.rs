use std::str::FromStr;

use crate::{
    proto::types::{QueryOutboundFeeRequest, QueryOutboundFeeResponse},
    query::grpc::{QueryError, Queryable},
    Asset,
};
use cosmwasm_std::{QuerierWrapper, StdError, Uint128};
use thiserror::Error;

pub struct OutboundFee(Uint128);

impl OutboundFee {
    pub fn load(q: QuerierWrapper, asset: &Asset) -> Result<Self, OutboundFeeError> {
        let req = QueryOutboundFeeRequest {
            asset: asset.to_string(),
            height: "0".to_string(),
        };
        let res = QueryOutboundFeeResponse::get(q, req)?;
        Ok(Self(Uint128::from_str(res.outbound_fee.as_str())?))
    }
    pub fn value(&self) -> Uint128 {
        self.0
    }
}

#[derive(Error, Debug)]
pub enum OutboundFeeError {
    #[error("{0}")]
    Std(#[from] StdError),
    #[error("{0}")]
    Query(#[from] QueryError),
}
