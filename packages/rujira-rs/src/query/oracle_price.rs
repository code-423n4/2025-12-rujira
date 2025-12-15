use super::grpc::{QueryError, Queryable};
use crate::{
    asset::Layer1AssetError,
    proto::types::{QueryOraclePriceRequest, QueryOraclePriceResponse},
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, QuerierWrapper, StdError};
use std::{
    num::{ParseIntError, TryFromIntError},
    ops::Sub,
    str::FromStr,
};
use thiserror::Error;

#[cw_serde]
pub struct OraclePrice {
    pub symbol: String,
    pub price: Decimal,
}

impl TryFrom<QueryOraclePriceResponse> for OraclePrice {
    type Error = TryFromOraclePriceError;
    fn try_from(v: QueryOraclePriceResponse) -> Result<Self, Self::Error> {
        match v.price {
            Some(price_data) => {
                // Trim fractional digits > 18
                let len = price_data.price.len();
                let fractional_len = {
                    let mut parts_iter = price_data.price.split('.');
                    parts_iter.next().unwrap(); // split always returns at least one element
                    parts_iter.next().unwrap_or_default().len()
                };
                let price_str = &price_data.price
                    [..len.sub(fractional_len.checked_sub(18).unwrap_or_default())];
                Ok(Self {
                    symbol: price_data.symbol,
                    price: Decimal::from_str(price_str)?,
                })
            }
            None => Err(TryFromOraclePriceError::NotFound {}),
        }
    }
}

#[derive(Error, Debug)]
pub enum TryFromOraclePriceError {
    #[error("{0}")]
    Std(#[from] StdError),
    #[error("{0}")]
    TryFromInt(#[from] TryFromIntError),
    #[error("{0}")]
    ParseInt(#[from] ParseIntError),
    #[error("{0}")]
    Layer1Asset(#[from] Layer1AssetError),
    #[error("Oracle price not found")]
    NotFound {},
}

impl OraclePrice {
    pub fn load(q: QuerierWrapper, symbol: &str) -> Result<Self, OraclePriceError> {
        let req = QueryOraclePriceRequest {
            height: "0".to_string(),
            symbol: symbol.to_owned(),
        };
        let res = QueryOraclePriceResponse::get(q, req)?;
        Ok(OraclePrice::try_from(res)?)
    }
}

#[derive(Error, Debug)]
pub enum OraclePriceError {
    #[error("{0}")]
    TryFrom(#[from] TryFromOraclePriceError),
    #[error("{0}")]
    Query(#[from] QueryError),
}
#[cfg(test)]
mod tests {
    use crate::proto::types;

    use super::*;

    #[test]
    fn price_parsing() {
        // Ensure that > 18 decimals doesn't break parsing
        assert_eq!(
            OraclePrice::try_from(QueryOraclePriceResponse {
                price: Some(types::OraclePrice {
                    symbol: "USDT".to_string(),
                    price: "0.99985000000000000004".to_string()
                })
            })
            .unwrap(),
            OraclePrice {
                symbol: "USDT".to_string(),
                price: Decimal::from_str("0.99985").unwrap()
            }
        );

        // Ensure that > 18 decimals doesn't break parsing
        assert_eq!(
            OraclePrice::try_from(QueryOraclePriceResponse {
                price: Some(types::OraclePrice {
                    symbol: "USDT".to_string(),
                    price: "100234".to_string()
                })
            })
            .unwrap(),
            OraclePrice {
                symbol: "USDT".to_string(),
                price: Decimal::from_str("100234").unwrap()
            }
        );

        // Ensure that > 18 decimals doesn't break parsing
        assert_eq!(
            OraclePrice::try_from(QueryOraclePriceResponse {
                price: Some(types::OraclePrice {
                    symbol: "USDT".to_string(),
                    price: "123.2453".to_string()
                })
            })
            .unwrap(),
            OraclePrice {
                symbol: "USDT".to_string(),
                price: Decimal::from_str("123.2453").unwrap()
            }
        );
    }
}
