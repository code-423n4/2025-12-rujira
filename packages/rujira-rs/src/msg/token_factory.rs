use cosmos_sdk_proto::cosmos::{bank::v1beta1::Metadata, base::v1beta1::Coin};
use cosmwasm_std::{AnyMsg, CosmosMsg};
use prost::Message;

static TYPE_URL_PREFIX: &str = "/thorchain.denom.v1.";

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct MsgCreateDenom {
    #[prost(string, tag = "1")]
    pub sender: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub id: ::prost::alloc::string::String,
    #[prost(message, required, tag = "3")]
    pub metadata: Metadata,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct MsgMintTokens {
    #[prost(string, tag = "1")]
    pub sender: ::prost::alloc::string::String,
    #[prost(message, required, tag = "2")]
    pub amount: Coin,
    #[prost(string, tag = "3")]
    pub recipient: ::prost::alloc::string::String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct MsgBurnTokens {
    #[prost(string, tag = "1")]
    pub sender: ::prost::alloc::string::String,
    #[prost(message, required, tag = "2")]
    pub amount: Coin,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct MsgSetDenomAdmin {
    #[prost(string, tag = "1")]
    pub sender: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub denom: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub new_admin: ::prost::alloc::string::String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct MsgSetMetadata {
    #[prost(string, tag = "1")]
    pub sender: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub denom: ::prost::alloc::string::String,
    #[prost(message, required, tag = "3")]
    pub new_metadata: Metadata,
}

impl From<MsgCreateDenom> for CosmosMsg {
    fn from(value: MsgCreateDenom) -> Self {
        CosmosMsg::Any(AnyMsg {
            type_url: format!("{}MsgCreateDenom", TYPE_URL_PREFIX),
            value: value.encode_to_vec().into(),
        })
    }
}

impl From<MsgMintTokens> for CosmosMsg {
    fn from(value: MsgMintTokens) -> Self {
        CosmosMsg::Any(AnyMsg {
            type_url: format!("{}MsgMintTokens", TYPE_URL_PREFIX),
            value: value.encode_to_vec().into(),
        })
    }
}

impl From<MsgBurnTokens> for CosmosMsg {
    fn from(value: MsgBurnTokens) -> Self {
        CosmosMsg::Any(AnyMsg {
            type_url: format!("{}MsgBurnTokens", TYPE_URL_PREFIX),
            value: value.encode_to_vec().into(),
        })
    }
}

impl From<MsgSetDenomAdmin> for CosmosMsg {
    fn from(value: MsgSetDenomAdmin) -> Self {
        CosmosMsg::Any(AnyMsg {
            type_url: format!("{}MsgSetDenomAdmin", TYPE_URL_PREFIX),
            value: value.encode_to_vec().into(),
        })
    }
}

impl From<MsgSetMetadata> for CosmosMsg {
    fn from(value: MsgSetMetadata) -> Self {
        CosmosMsg::Any(AnyMsg {
            type_url: format!("{}MsgSetMetadata", TYPE_URL_PREFIX),
            value: value.encode_to_vec().into(),
        })
    }
}
