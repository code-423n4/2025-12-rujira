use anybuf::Bufany;
use anyhow::{Error, Result as AnyResult};
use cosmwasm_std::{
    attr, coins, ensure_eq,
    testing::{MockApi, MockStorage},
    Addr, AnyMsg, Api, BankMsg, Binary, BlockInfo, CosmosMsg, CustomMsg, CustomQuery, Decimal,
    DenomMetadata, DenomUnit, Empty, Event, GrpcQuery, Querier, Storage, Uint128,
};
use cw_multi_test::{
    App, AppResponse, BankKeeper, BankSudo, BasicAppBuilder, CosmosRouter, FailingModule,
    GovFailingModule, IbcFailingModule, Stargate, SudoMsg, WasmKeeper, WasmSudo,
};
use cw_storage_plus::Map;
use serde::de::DeserializeOwned;
use std::{collections::BTreeMap, str::FromStr};

use crate::fixtures::{mock_mimir, mock_network, mock_oracle_price, mock_pool, mock_quote};

pub type RujiraApp = App<
    BankKeeper,
    MockApi,
    MockStorage,
    // Custom
    FailingModule<Empty, Empty, Empty>,
    WasmKeeper<Empty, Empty>,
    // SDK Staking
    FailingModule<Empty, Empty, Empty>,
    // SDK Distribution
    FailingModule<Empty, Empty, Empty>,
    IbcFailingModule,
    GovFailingModule,
    RujiraStargate,
>;

static DENOM_ADMIN: Map<String, String> = Map::new("denom_admin");
static DENOM_METADATA: Map<String, DenomMetadata> = Map::new("denom_meta");

pub fn mock_rujira_app() -> RujiraApp {
    BasicAppBuilder::new()
        .with_stargate(RujiraStargate::default())
        .build(|_, _, _| {})
}

#[derive(Default)]
pub struct RujiraStargate {
    prices: BTreeMap<String, Decimal>,
}

impl RujiraStargate {
    pub fn with_price(&mut self, symbol: &str, price: Decimal) {
        self.prices.insert(symbol.to_string(), price);
    }

    pub fn with_prices(&mut self, prices: Vec<(&str, Decimal)>) {
        for (symbol, price) in prices {
            self.with_price(symbol, price);
        }
    }
}

impl Stargate for RujiraStargate {
    fn execute_stargate<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        sender: Addr,
        type_url: String,
        value: Binary,
    ) -> AnyResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        anyhow::bail!(
            "Unexpected stargate execute: type_url={}, value={} from {}",
            type_url,
            value,
            sender,
        )
    }

    fn query_stargate(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        path: String,
        data: Binary,
    ) -> AnyResult<Binary> {
        anyhow::bail!("Unexpected stargate query: path={}, data={}", path, data)
    }

    fn execute_any<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: AnyMsg,
    ) -> AnyResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let type_url = msg.type_url.clone();
        let serialized = msg.value.to_vec();
        let buf = Bufany::deserialize(&serialized)?;
        match type_url.as_str() {
            "/types.MsgDeposit" => Ok(AppResponse {
                events: vec![],
                data: None,
            }),

            "/thorchain.denom.v1.MsgCreateDenom" => {
                let sender = buf.string(1).unwrap();
                let id = buf.string(2).unwrap();
                let m = buf.message(3).unwrap();
                let metadata = decode_metadata(m);
                let full = format!("x/{id}");
                DENOM_ADMIN.save(storage, full.clone(), &sender)?;
                DENOM_METADATA.save(storage, full, &metadata)?;

                Ok(AppResponse {
                    events: vec![],
                    data: None,
                })
            }
            "/thorchain.denom.v1.MsgMintTokens" => {
                let sender = buf.string(1).unwrap();
                let coin: Bufany<'_> = buf.message(2).unwrap();
                let recipient = buf.string(3).unwrap();

                let denom = coin.string(1).unwrap();
                let amount = Uint128::from_str(&coin.string(2).unwrap())?;

                let admin = DENOM_ADMIN.load(storage, denom.clone())?;
                ensure_eq!(admin, sender, Error::msg("Unauthorized"));

                router.sudo(
                    api,
                    storage,
                    block,
                    SudoMsg::Bank(BankSudo::Mint {
                        to_address: recipient.clone(),
                        amount: coins(amount.u128(), denom.clone()),
                    }),
                )?;
                Ok(AppResponse {
                    events: vec![Event::new("mint").add_attributes(vec![
                        attr("amount", amount),
                        attr("denom", denom.to_string()),
                        attr("recipient", recipient),
                    ])],
                    data: None,
                })
            }
            "/thorchain.denom.v1.MsgBurnTokens" => {
                let sender = buf.string(1).unwrap();
                let coin: Bufany<'_> = buf.message(2).unwrap();

                let denom = coin.string(1).unwrap();
                let amount = Uint128::from_str(&coin.string(2).unwrap())?;

                let admin = DENOM_ADMIN.load(storage, denom.clone())?;
                ensure_eq!(admin, sender, Error::msg("Unauthorized"));

                router.execute(
                    api,
                    storage,
                    block,
                    api.addr_validate(&sender)?,
                    CosmosMsg::Bank(BankMsg::Burn {
                        amount: coins(amount.u128(), denom.clone()),
                    }),
                )?;
                Ok(AppResponse {
                    events: vec![Event::new("burn").add_attributes(vec![
                        attr("amount", amount),
                        attr("denom", denom.to_string()),
                    ])],
                    data: None,
                })
            }
            "/thorchain.denom.v1.MsgSetDenomAdmin" => {
                let sender = buf.string(1).unwrap();
                let denom = buf.string(2).unwrap();
                let new_admin = buf.string(3).unwrap();

                let admin = DENOM_ADMIN.load(storage, denom.clone())?;
                ensure_eq!(admin, sender, Error::msg("Unauthorized"));
                DENOM_ADMIN.save(storage, denom, &new_admin)?;

                Ok(AppResponse {
                    events: vec![],
                    data: None,
                })
            }
            "/thorchain.denom.v1.MsgSetDenomMetadata" => {
                let sender = buf.string(1).unwrap();
                let denom = buf.string(2).unwrap();
                let m = buf.message(3).unwrap();
                let metadata = decode_metadata(m);

                let admin = DENOM_ADMIN.load(storage, denom.clone())?;
                ensure_eq!(admin, sender, Error::msg("Unauthorized"));
                DENOM_METADATA.save(storage, denom, &metadata)?;

                Ok(AppResponse {
                    events: vec![],
                    data: None,
                })
            }
            "/cosmwasm.wasm.v1.MsgSudoContract" => {
                let _sender = buf.string(1).unwrap();
                let contract = buf.string(2).unwrap();
                let msg = buf.bytes(3).unwrap();
                router.sudo(
                    api,
                    storage,
                    block,
                    SudoMsg::Wasm(WasmSudo {
                        contract_addr: api.addr_validate(&contract)?,
                        message: msg.into(),
                    }),
                )
            }
            _ => {
                anyhow::bail!("Unexpected any execute: msg={:?} from {}", msg, sender)
            }
        }
    }

    fn query_grpc(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        request: GrpcQuery,
    ) -> AnyResult<Binary> {
        match request.path.as_str() {
            "/types.Query/MimirWithKey" => mock_mimir(request.data),
            "/types.Query/Pool" => mock_pool(request.data),
            "/types.Query/Network" => mock_network(),
            "/types.Query/QuoteSwap" => mock_quote(request.data),
            "/types.Query/OraclePrice" => mock_oracle_price(request.data, &self.prices),
            _ => {
                anyhow::bail!("Unexpected grpc query: request={:?}", request)
            }
        }
    }
}

fn decode_metadata(m: Bufany) -> DenomMetadata {
    let denom_units = m
        .repeated_bytes(2)
        .unwrap_or_default()
        .iter()
        .map(|x| {
            let m = Bufany::deserialize(x).unwrap();
            DenomUnit {
                denom: m.string(1).unwrap_or_default(),
                exponent: m.uint32(2).unwrap_or_default(),
                aliases: m.repeated_string(3).unwrap_or_default(),
            }
        })
        .collect();

    DenomMetadata {
        description: m.string(1).unwrap_or_default(),
        denom_units,
        base: m.string(3).unwrap_or_default(),
        display: m.string(4).unwrap_or_default(),
        name: m.string(5).unwrap_or_default(),
        symbol: m.string(6).unwrap_or_default(),
        uri: m.string(7).unwrap_or_default(),
        uri_hash: m.string(8).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::Decimal;
    use rujira_rs::{
        query::{Pool, PoolStatus},
        Asset, Layer1Asset,
    };

    use super::*;

    #[test]
    fn query_pool() {
        let app = mock_rujira_app();
        let asset = Layer1Asset::new("BTC", "BTC");
        let res = Pool::load(app.wrap(), &asset).unwrap();
        assert_eq!(res.asset, Asset::Layer1(Layer1Asset::new("BTC", "BTC")),);
        assert_eq!(res.short_code, "b".to_string());
        assert_eq!(res.status, PoolStatus::Available);
        assert_eq!(res.decimals, 8);
        assert_eq!(res.pending_inbound_asset, Uint128::from(156524579u128));
        assert_eq!(res.pending_inbound_rune, Uint128::from(0u128));
        assert_eq!(res.balance_asset, Uint128::from(68602648901u128));
        assert_eq!(res.balance_rune, Uint128::from(1172427071332399u128));
        assert_eq!(res.asset_tor_price, Decimal::from_str("100100").unwrap(),);
        assert_eq!(res.pool_units, Uint128::from(613518358320559u128));
        assert_eq!(res.lp_units, Uint128::from(347866097255926u128));
        assert_eq!(res.synth_units, Uint128::from(265652261064633u128));
        assert_eq!(res.synth_supply, Uint128::from(59409628248u128));
        assert_eq!(res.savers_depth, Uint128::from(58882558588u128));
        assert_eq!(res.savers_units, Uint128::from(56192173382u128));
        assert_eq!(res.savers_fill_bps, 8660);
        assert_eq!(res.savers_capacity_remaining, Uint128::from(9193020653u128));
        assert!(!res.synth_mint_paused);
        assert_eq!(res.synth_supply_remaining, Uint128::from(22913550433u128));
        assert_eq!(res.derived_depth_bps, 9639);
    }
}
