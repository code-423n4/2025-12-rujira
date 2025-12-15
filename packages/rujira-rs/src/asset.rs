use std::{
    fmt::{Display, Formatter, Result},
    result,
    str::FromStr,
};

use anybuf::Anybuf;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::ensure;
use thiserror::Error;

use crate::memoed::Memoed;

#[cw_serde]
pub enum Asset {
    Secured(SecuredAsset),
    Layer1(Layer1Asset),
}

impl From<Asset> for Anybuf {
    fn from(asset: Asset) -> Self {
        match asset {
            Asset::Secured(secured) => Anybuf::from(secured),
            Asset::Layer1(l1) => Anybuf::from(l1),
        }
    }
}

impl FromStr for Asset {
    type Err = AssetError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s.split_once(".") {
            Some((chain, symbol)) => Ok(Self::Layer1(Layer1Asset::new(chain, symbol))),
            None => Ok(SecuredAsset::try_from(s)?.into()),
        }
    }
}

#[derive(Error, Debug)]
pub enum AssetError {
    #[error("Layer1 {0}")]
    Layer1(#[from] Layer1AssetError),
    #[error("Secured {0}")]
    Secured(#[from] SecuredAssetError),
}

impl Asset {
    pub fn from_denom(str: &String) -> result::Result<Self, AssetError> {
        match SecuredAsset::from_denom(str) {
            Ok(secured) => Ok(Self::Secured(secured)),
            _ => Ok(Self::Layer1(Layer1Asset::from_denom(str.to_string())?)),
        }
    }

    pub fn denom(&self) -> std::result::Result<String, AssetError> {
        match self {
            Asset::Secured(secured) => Ok(secured.denom()),
            Asset::Layer1(l1) => Ok(l1.denom()?),
        }
    }

    pub fn pool_id(&self) -> String {
        match self {
            Asset::Secured(secured) => secured.to_layer_1().to_string(),
            Asset::Layer1(l1) => l1.to_string(),
        }
    }

    pub fn to_layer_1(&self) -> Layer1Asset {
        match self {
            Asset::Secured(secured_asset) => secured_asset.to_layer_1(),
            Asset::Layer1(layer1_asset) => layer1_asset.clone(),
        }
    }
}

impl Display for Asset {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Asset::Secured(bridge) => bridge.fmt(f),
            Asset::Layer1(l1) => l1.fmt(f),
        }
    }
}

#[cw_serde]
pub struct SecuredAsset {
    chain: String,
    symbol: String,
}

impl SecuredAsset {
    pub fn new(chain: &str, symbol: &str) -> Self {
        Self {
            chain: chain.to_uppercase(),
            symbol: symbol.to_uppercase(),
        }
    }

    pub fn from_denom(str: &String) -> result::Result<Self, SecuredAssetError> {
        match str.split_once("-") {
            Some((chain, symbol)) => {
                // Validate that this isn't eg a token factory pretending to be a secured asset
                // x/btc-btc would otherwise pass
                ensure!(
                    is_lower_alpha(chain),
                    SecuredAssetError::InvalidDenom(str.to_string())
                );
                ensure!(
                    is_valid_symbol(symbol),
                    SecuredAssetError::InvalidDenom(str.to_string())
                );

                Ok(Self::new(chain, symbol))
            }
            None => Err(SecuredAssetError::InvalidDenom(str.to_string())),
        }
    }

    pub fn denom(&self) -> String {
        format!("{}-{}", self.chain, self.symbol).to_ascii_lowercase()
    }

    pub fn ticker(&self) -> String {
        self.symbol
            .split('-')
            .next()
            .unwrap_or_default()
            .to_string()
    }

    pub fn to_layer_1(&self) -> Layer1Asset {
        Layer1Asset::new(&self.chain, &self.symbol)
    }
}

impl From<SecuredAsset> for Anybuf {
    fn from(asset: SecuredAsset) -> Self {
        Anybuf::new()
            .append_string(1, &asset.chain) // chain
            .append_string(2, &asset.symbol) // symbol
            .append_string(3, asset.ticker()) // ticker
            .append_bool(4, false) // synth
            .append_bool(5, false) // trade
            .append_bool(6, true) // secured
    }
}

impl Display for SecuredAsset {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}-{}", self.chain, self.symbol)
    }
}

#[cw_serde]
pub struct Layer1Asset {
    chain: String,
    symbol: String,
}

impl Layer1Asset {
    pub fn new(chain: &str, symbol: &str) -> Self {
        Self {
            chain: chain.to_uppercase(),
            symbol: symbol.to_uppercase(),
        }
    }

    pub fn denom(&self) -> std::result::Result<String, Layer1AssetError> {
        match self.chain.as_str() {
            "THOR" => Ok(match self.symbol.as_str() {
                "RUJI" => "x/ruji".to_string(),
                "RUNE" => "rune".to_string(),
                "TCY" => "tcy".to_string(),
                other => format!("{}.{}", other, self.symbol).to_ascii_lowercase(),
            }),
            _ => Err(Layer1AssetError::InvalidAsset(self.to_string())),
        }
    }

    pub fn ticker(&self) -> String {
        self.symbol
            .split('-')
            .next()
            .unwrap_or_default()
            .to_string()
    }

    pub fn from_denom(denom: String) -> std::result::Result<Self, Layer1AssetError> {
        match denom.as_str() {
            "rune" => Ok(Self::new("THOR", "RUNE")),
            "tcy" => Ok(Self::new("THOR", "TCY")),
            "x/ruji" => Ok(Self::new("THOR", "RUJI")),
            other => match other.split_once(".") {
                Some(("thor", symbol)) => Ok(Self::new("THOR", symbol)),
                _ => Err(Layer1AssetError::InvalidDenom(other.to_string())),
            },
        }
    }

    pub fn is_rune(&self) -> bool {
        self.chain == "THOR" && self.symbol == "RUNE"
    }

    pub fn migrate(&self) -> Self {
        Self {
            chain: self.chain.to_uppercase(),
            symbol: self.symbol.clone(),
        }
    }
}

impl From<Layer1Asset> for Anybuf {
    fn from(asset: Layer1Asset) -> Self {
        Anybuf::new()
            .append_string(1, &asset.chain) // chain
            .append_string(2, &asset.symbol) // symbol
            .append_string(3, asset.ticker()) // ticker
            .append_bool(4, false) // synth
            .append_bool(5, false) // trade
            .append_bool(6, false) // secured
    }
}

impl From<Layer1Asset> for Asset {
    fn from(value: Layer1Asset) -> Self {
        Self::Layer1(value)
    }
}

#[derive(Error, Debug)]
pub enum Layer1AssetError {
    #[error("Invalid layer 1 string {0}")]
    Invalid(String),

    #[error("Invalid layer 1 denom string {0}")]
    InvalidDenom(String),

    #[error("No denom string for layer 1 asset {0}")]
    InvalidAsset(String),
}

impl Display for Layer1Asset {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}.{}", self.chain, self.symbol)
    }
}

impl TryFrom<&String> for Layer1Asset {
    type Error = Layer1AssetError;

    fn try_from(value: &String) -> result::Result<Self, Self::Error> {
        value.as_str().try_into()
    }
}

impl TryFrom<String> for Layer1Asset {
    type Error = Layer1AssetError;

    fn try_from(value: String) -> result::Result<Self, Self::Error> {
        value.as_str().try_into()
    }
}

impl TryFrom<&str> for Layer1Asset {
    type Error = Layer1AssetError;

    fn try_from(value: &str) -> result::Result<Self, Self::Error> {
        match value.split_once(".") {
            Some((chain, symbol)) => Ok(Self::new(chain, symbol)),
            None => Err(Layer1AssetError::Invalid(value.to_owned())),
        }
    }
}

impl FromStr for Layer1Asset {
    type Err = Layer1AssetError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        s.try_into()
    }
}

impl TryFrom<&str> for SecuredAsset {
    type Error = SecuredAssetError;

    fn try_from(value: &str) -> result::Result<Self, Self::Error> {
        match value.split_once("-") {
            Some((chain, symbol)) => Ok(Self::new(chain, symbol)),
            _ => Err(SecuredAssetError::Invalid(value.to_owned())),
        }
    }
}

#[derive(Error, Debug)]
pub enum SecuredAssetError {
    #[error("Invalid secured asset string {0}")]
    Invalid(String),

    #[error("Invalid secured asset denom string {0}")]
    InvalidDenom(String),
}

fn is_lower_alpha(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b: u8| b.is_ascii_lowercase())
}

fn is_lower_hex(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_valid_symbol(symbol: &str) -> bool {
    if let Some((base, hex)) = symbol.split_once("-0x") {
        is_lower_alpha(base) && hex.len() == 40 && is_lower_hex(hex)
    } else {
        is_lower_alpha(symbol)
    }
}

impl Memoed for Asset {
    fn to_memo(&self) -> String {
        match self {
            Asset::Secured(secured) => secured.to_string(),
            Asset::Layer1(l1) => l1.to_string(),
        }
    }
}

impl From<SecuredAsset> for Asset {
    fn from(value: SecuredAsset) -> Self {
        Self::Secured(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge() {
        let native = SecuredAsset::new("BTC", "BTC");
        assert_eq!(native.to_string(), "BTC-BTC");
        assert_eq!(native.denom(), "btc-btc");
        assert_eq!(format!("{}", native), "BTC-BTC");

        let native = SecuredAsset::new("ETH", "RUNE-0X3155BA85D5F96B2D030A4966AF206230E46849CB");
        assert_eq!(
            native.to_string(),
            "ETH-RUNE-0X3155BA85D5F96B2D030A4966AF206230E46849CB"
        );
        assert_eq!(native.ticker(), "RUNE");

        assert_eq!(
            format!("{}", native),
            "ETH-RUNE-0X3155BA85D5F96B2D030A4966AF206230E46849CB"
        );
    }

    #[test]
    fn l1() {
        let l1 = Layer1Asset::new("BTC", "BTC");
        assert_eq!(l1.to_string(), "BTC.BTC");
        l1.denom().unwrap_err();
        assert_eq!(format!("{}", l1), "BTC.BTC");

        let l1 = Layer1Asset::new("ETH", "RUNE-0X3155BA85D5F96B2D030A4966AF206230E46849CB");
        assert_eq!(
            l1.to_string(),
            "ETH.RUNE-0X3155BA85D5F96B2D030A4966AF206230E46849CB"
        );
        assert_eq!(l1.ticker(), "RUNE");

        assert_eq!(
            format!("{}", l1),
            "ETH.RUNE-0X3155BA85D5F96B2D030A4966AF206230E46849CB"
        );
    }

    #[test]
    fn memo() {
        let asset: Asset = Layer1Asset::new("THOR", "RUNE").into();
        assert_eq!(asset.to_memo(), "THOR.RUNE");

        let asset: Asset = SecuredAsset::new("BTC", "BTC").into();
        assert_eq!(asset.to_memo(), "BTC-BTC");
    }
}
