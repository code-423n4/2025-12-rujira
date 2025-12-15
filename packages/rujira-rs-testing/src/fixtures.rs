use std::collections::BTreeMap;

use anyhow::Error;
use cosmwasm_std::{Binary, Decimal, StdError};
use prost::Message;
use rujira_rs::proto;

pub fn mock_mimir(request: Binary) -> Result<Binary, Error> {
    let req = proto::types::QueryMimirWithKeyRequest::decode(request.as_slice()).unwrap();
    let mut buf = Vec::new();

    match req.key.to_uppercase().as_str() {
        "SECUREDASSETSLIPMINBPS" => proto::types::QueryMimirWithKeyResponse { value: 10i64 }
            .encode(&mut buf)
            .unwrap(),
        _ => return Err(StdError::generic_err("Mimir Key not found").into()),
    };
    Ok(buf.into())
}

fn mock_pool_btc() -> Binary {
    let pool = proto::types::QueryPoolResponse {
        asset: "BTC.BTC".to_string(),
        short_code: "b".to_string(),
        status: "Available".to_string(),
        decimals: 8,
        pending_inbound_asset: "156524579".to_string(),
        pending_inbound_rune: "0".to_string(),
        balance_asset: "68602648901".to_string(),
        balance_rune: "1172427071332399".to_string(),
        asset_tor_price: "10010000000000".to_string(),
        pool_units: "613518358320559".to_string(),
        lp_units: "347866097255926".to_string(),
        synth_units: "265652261064633".to_string(),
        synth_supply: "59409628248".to_string(),
        savers_depth: "58882558588".to_string(),
        savers_units: "56192173382".to_string(),
        savers_fill_bps: "8660".to_string(),
        savers_capacity_remaining: "9193020653".to_string(),
        synth_mint_paused: false,
        synth_supply_remaining: "22913550433".to_string(),
        derived_depth_bps: "9639".to_string(),
        trading_halted: false,
    };

    let mut buf = Vec::new();
    pool.encode(&mut buf).unwrap();
    buf.into()
}

fn mock_pool_usdc() -> Binary {
    let pool = proto::types::QueryPoolResponse {
        asset: "ETH.USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48".to_string(),
        status: "Available".to_string(),
        decimals: 6,
        pending_inbound_asset: "0".to_string(),
        pending_inbound_rune: "0".to_string(),
        balance_asset: "1068860344382528".to_string(),
        balance_rune: "217689972512615".to_string(),
        asset_tor_price: "100100000".to_string(),
        pool_units: "51619557902356".to_string(),
        lp_units: "33369405984602".to_string(),
        synth_units: "18250151917754".to_string(),
        synth_supply: "755793519221676".to_string(),
        savers_depth: "727032247104330".to_string(),
        savers_units: "646314302227834".to_string(),
        savers_fill_bps: "7071".to_string(),
        savers_capacity_remaining: "313066825160852".to_string(),
        synth_mint_paused: false,
        synth_supply_remaining: "526838894037357".to_string(),
        derived_depth_bps: "0".to_string(),
        short_code: "".to_string(),
        trading_halted: false,
    };

    let mut buf = Vec::new();
    pool.encode(&mut buf).unwrap();
    buf.into()
}

pub fn mock_pool(request: Binary) -> Result<Binary, Error> {
    let req = proto::types::QueryPoolRequest::decode(request.as_slice()).unwrap();

    match req.asset.as_str() {
        "BTC.BTC" => Ok(mock_pool_btc()),
        "ETH.USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48" => Ok(mock_pool_usdc()),
        _ => Err(StdError::generic_err("Asset not found").into()),
    }
}

pub fn mock_network() -> Result<Binary, Error> {
    let network = proto::types::QueryNetworkResponse {
        bond_reward_rune: "1356955381093".to_string(),
        total_bond_units: "1042720".to_string(),
        effective_security_bond: "5569290313641446".to_string(),
        total_reserve: "7400760525247188".to_string(),
        vaults_migrating: false,
        gas_spent_rune: "179282527796318".to_string(),
        gas_withheld_rune: "231180386614277".to_string(),
        outbound_fee_multiplier: "1000".to_string(),
        native_outbound_fee_rune: "2000000".to_string(),
        native_tx_fee_rune: "2000000".to_string(),
        tns_register_fee_rune: "1000000000".to_string(),
        tns_fee_per_block_rune: "20".to_string(),
        rune_price_in_tor: "135585271".to_string(),
        tor_price_in_rune: "73754324".to_string(),
    };

    let mut buf = Vec::new();
    network.encode(&mut buf).unwrap();
    Ok(buf.into())
}

pub fn mock_quote(_req: Binary) -> Result<Binary, Error> {
    let quote = proto::types::QueryQuoteSwapResponse {
        outbound_delay_blocks: 0,
        outbound_delay_seconds: 0,
        fees: Some(proto::types::QuoteFees {
            asset: "THOR.RUNE".to_string(),
            affiliate: "4527004".to_string(),
            outbound: "2000000".to_string(),
            liquidity: "1134587".to_string(),
            total: "7661591".to_string(),
            slippage_bps: 24,
            total_bps: 166
        }),
        expiry: 1757937922,
        warning: "Do not cache this response. Do not send funds after the expiry.".to_string(),
        notes: "Broadcast a MsgDeposit to the THORChain network with the appropriate memo. Do not use multi-in, multi-out transactions.".to_string(),
        dust_threshold: "1000".to_string(),
        recommended_min_amount_in: "44".to_string(),
        recommended_gas_rate: "3".to_string(),
        gas_rate_units: "satsperbyte".to_string(),
        memo: "dummy".to_string(),
        expected_amount_out: "446173434".to_string(),
        max_streaming_quantity: 0,
        streaming_swap_blocks: 0,
        inbound_address: "".to_string(),
        inbound_confirmation_blocks: 0,
        inbound_confirmation_seconds: 0,
        router: "".to_string(),
        streaming_swap_seconds: 0,
        total_swap_seconds: 0,
    };

    let mut buf = Vec::new();
    quote.encode(&mut buf).unwrap();
    Ok(buf.into())
}

pub fn mock_oracle_price(
    request: Binary,
    prices: &BTreeMap<String, Decimal>,
) -> Result<Binary, Error> {
    let req = proto::types::QueryOraclePriceRequest::decode(request.as_slice()).unwrap();
    let quote = proto::types::QueryOraclePriceResponse {
        price: match prices.get(&req.symbol) {
            Some(price) => Some(proto::types::OraclePrice {
                symbol: req.symbol,
                price: price.to_string(),
            }),
            _ => None,
        },
    };

    let mut buf = Vec::new();
    quote.encode(&mut buf).unwrap();
    Ok(buf.into())
}
