use std::cmp::min;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coin, ensure, ensure_eq, to_json_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Empty,
    Env, MessageInfo, Order, Response, StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_storage_plus::Map;
use cw_utils::one_coin;
use rujira_rs::query::SwapQuoteQuery;
use rujira_rs::thorchain_swap::{
    Callback, ConfigResponse, ExecuteMsg, InstantiateMsg, MarketsResponse, QueryMsg, SudoMsg,
    Vault, VaultResponse, VaultsResponse,
};
use rujira_rs::Asset;

use crate::config::Config;
use crate::error::ContractError;
use crate::events::event_swap;
use crate::quote::QuoteState;

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub static VAULTS: Map<String, Vault> = Map::new("vaults");
pub static MARKETS: Map<Addr, bool> = Map::new("markets");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config::from(msg);
    config.validate()?;
    config.save(deps.storage)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Repay {} => {
            Ok(Response::default().add_messages(execute_repayments(deps.as_ref(), env, None)?))
        }
        ExecuteMsg::Swap {
            min_return,
            to,
            callback,
        } => {
            let funds = one_coin(&info)?;
            ensure!(
                MARKETS
                    .load(deps.storage, info.sender.clone())
                    .map_err(|_| ContractError::Unauthorized {})?,
                ContractError::Unauthorized {}
            );
            // Validate that the base layer swap will return sufficient funds
            let config = Config::load(deps.storage)?;
            let from_asset = Asset::from_denom(&funds.denom)?;
            let to_asset = Asset::from_denom(&min_return.denom)?;
            let quote = SwapQuoteQuery {
                from_asset,
                to_asset,
                amount: funds.amount,
                streaming: Some((1, config.max_stream_length)),
                destination: env.contract.address.to_string(),
                tolerance_bps: None,
                refund_address: Some(env.contract.address.to_string()),
                affiliates: vec![],
            }
            .quote(deps.querier)?;
            let outbound = quote.fees.clone().map(|x| x.outbound).unwrap_or_default();
            let liquidity = quote.fees.clone().map(|x| x.liquidity).unwrap_or_default();
            // Include the outbound fee in the return calculations. Until this is set to 0 in thornode, the `reserve_fee` must cover it
            let returned = quote.expected_amount_out + outbound;

            // N.B. we can't check including the reserve_fee. The base layer swap simulation executes the msg as though it were
            // in the end block, and as such is subject to ordering amongst other swaps in the swap queue, eg streaming swaps,
            // which will cause deviation from the "pure" CLP quote that is computed by this contract
            // Therefore we must assume that the contract's quoting is correct for pool state, and the reserve_fee must also
            // cover this simulation variance as well as actual execution variance
            ensure!(
                returned >= min_return.amount,
                ContractError::InsufficientReturn {
                    quoted: quote.expected_amount_out,
                    asked: min_return.amount,
                    liquidity,
                    outbound,
                }
            );
            let swap_msg = quote.to_msg(deps.api.addr_canonicalize(env.contract.address.as_str())?);
            let vault = VAULTS.load(deps.storage, min_return.denom.clone())?;
            let to = to.unwrap_or(info.sender.to_string());
            let fee = returned.mul_ceil(config.reserve_fee);
            let net_return = returned.checked_sub(fee).unwrap_or_default();
            Ok(Response::default()
                .add_event(event_swap(
                    &to,
                    &funds,
                    &min_return,
                    &coin(fee.u128(), min_return.denom.clone()),
                    &coin(net_return.u128(), min_return.denom.clone()),
                    &quote.memo,
                ))
                .add_messages(execute_repayments(deps.as_ref(), env, Some(funds))?)
                .add_message(vault.borrow_msg(net_return, to.clone(), callback)?)
                .add_message(swap_msg))
        }
        ExecuteMsg::Callback(msg) => {
            let funds = one_coin(&info)?;
            let data: Callback = msg.deserialize_callback()?;
            let vault = VAULTS.load(deps.storage, funds.denom.clone())?;
            ensure_eq!(info.sender, vault.addr(), ContractError::Unauthorized {});
            match data.callback {
                Some(cb) => Ok(Response::default().add_message(cb.to_message(
                    &deps.api.addr_validate(&data.to)?,
                    Empty {},
                    vec![funds],
                )?)),
                None => Ok(Response::default().add_message(BankMsg::Send {
                    to_address: data.to,
                    amount: vec![funds],
                })),
            }
        }
    }
}

fn execute_repayments(deps: Deps, env: Env, funds: Option<Coin>) -> StdResult<Vec<WasmMsg>> {
    // Execute repayments first
    // Deprecated but the only other options are a) explicit list to repay or iterate over vaults every time, both of which are sub optimal
    #[allow(deprecated)]
    let balances = deps
        .querier
        .query_all_balances(env.contract.address.clone())?;
    let mut repay_msgs: Vec<WasmMsg> = vec![];
    for balance in balances {
        let amount = match funds.clone() {
            Some(coin) => {
                if balance.denom == coin.denom {
                    balance.amount - coin.amount
                } else {
                    balance.amount
                }
            }
            None => balance.amount,
        };

        match VAULTS.load(deps.storage, balance.denom.clone()) {
            Ok(vault) => {
                let debt = vault.debt(deps.querier, &env.contract.address)?;
                let repay = min(debt, amount);
                // Ghost Vault has truncation issues when calculating repaid shares
                // TODO: Add a min_repay to ghost vault to protect against small values
                if repay.gt(&Uint128::from(1000u128)) {
                    repay_msgs.push(vault.repay_msg(coin(repay.u128(), balance.denom))?)
                }
            }
            // Prevent execution from halting if random tokens get sent to the contract
            Err(_) => continue,
        }
    }
    Ok(repay_msgs)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    let mut config = Config::load(deps.storage)?;
    match msg {
        SudoMsg::SetConfig(update) => {
            config.update(&update);
            config.validate()?;
            config.save(deps.storage)?;
            Ok(Response::default())
        }
        SudoMsg::SetMarket { addr, enabled } => {
            let k = deps.api.addr_validate(&addr)?;
            MARKETS.save(deps.storage, k, &enabled)?;
            Ok(Response::default())
        }
        SudoMsg::SetVault { denom, vault } => {
            match vault {
                Some(vault) => VAULTS.save(deps.storage, denom, &vault)?,
                None => VAULTS.remove(deps.storage, denom),
            }
            Ok(Response::default())
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    let config = Config::load(deps.storage)?;
    match msg {
        QueryMsg::Config {} => Ok(to_json_binary(&ConfigResponse::from(config))?),
        QueryMsg::Quote(req) => {
            let vault = VAULTS.load(deps.storage, req.ask_denom.clone())?;
            let mut state = match req.data {
                Some(data) => QuoteState::decode(&data)?,
                None => {
                    let borrow_limit = vault.available(deps.querier, &env.contract.address)?;

                    QuoteState::load(
                        deps.querier,
                        &req.offer_denom,
                        &req.ask_denom,
                        &config,
                        borrow_limit,
                    )?
                }
            };
            // Clamp the borrow limit to the smaller of vault available and the pool percentage cap

            Ok(to_json_binary(&state.quote()?)?)
        }
        QueryMsg::Markets {} => Ok(to_json_binary(&MarketsResponse {
            markets: MARKETS
                .range(deps.storage, None, None, Order::Ascending)
                .fold(vec![], |mut agg, x| match x {
                    Ok((addr, true)) => {
                        agg.push(addr);
                        agg
                    }
                    _ => agg,
                }),
        })?),
        QueryMsg::Vaults {} => Ok(to_json_binary(&VaultsResponse {
            vaults: VAULTS
                .range(deps.storage, None, None, Order::Ascending)
                .map(|x| x.map(|(denom, vault)| VaultResponse { denom, vault }))
                .collect::<StdResult<Vec<VaultResponse>>>()?,
        })?),
    }
}
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: ()) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use crate::testing::setup;

    use super::*;
    use cosmwasm_std::{Decimal, Uint128};
    use cw_multi_test::{BasicApp, ContractWrapper, Executor};
    use rujira_rs::bow::{QuoteRequest, QuoteResponse};
    use rujira_rs_testing::mock_rujira_app;

    #[test]
    fn instantiation() {
        let mut app = BasicApp::default();
        let owner = app.api().addr_make("owner");

        let code = Box::new(ContractWrapper::new(execute, instantiate, query));
        let code_id = app.store_code(code);
        app.instantiate_contract(
            code_id,
            owner,
            &InstantiateMsg {
                max_stream_length: 1u32,
                max_borrow_ratio: Decimal::one(),
                reserve_fee: Decimal::from_ratio(10u128, 500u128),
                stream_step_ratio: Decimal::one(),
            },
            &[],
            "template",
            None,
        )
        .unwrap();
    }

    #[test]
    fn quote() {
        let mut app = mock_rujira_app();
        let owner = app.api().addr_make("owner");
        let contract = setup(&mut app, &owner);
        // RUNE to B
        let quote_1: QuoteResponse = app
            .wrap()
            .query_wasm_smart(
                contract.clone(),
                &QueryMsg::Quote(QuoteRequest {
                    min_price: None,
                    offer_denom: "rune".to_string(),
                    ask_denom: "btc-btc".to_string(),
                    data: None,
                }),
            )
            .unwrap();

        assert_eq!(
            quote_1.price,
            Decimal::from_str("0.000058384829789226").unwrap()
        );
        assert_eq!(quote_1.size, Uint128::from(68451955u128));

        let quote_2: QuoteResponse = app
            .wrap()
            .query_wasm_smart(
                contract.clone(),
                &QueryMsg::Quote(QuoteRequest {
                    min_price: None,
                    offer_denom: "rune".to_string(),
                    ask_denom: "btc-btc".to_string(),
                    data: quote_1.data,
                }),
            )
            .unwrap();

        assert_eq!(
            quote_2.price,
            Decimal::from_str("0.000058151872868767").unwrap()
        );

        assert_eq!(quote_2.size, Uint128::from(68178830u128));

        assert!(quote_2.price < quote_1.price);

        // A to RUNE
        let quote_1: QuoteResponse = app
            .wrap()
            .query_wasm_smart(
                contract.clone(),
                &QueryMsg::Quote(QuoteRequest {
                    min_price: None,
                    offer_denom: "btc-btc".to_string(),
                    ask_denom: "rune".to_string(),
                    data: None,
                }),
            )
            .unwrap();

        assert_eq!(
            quote_1.price,
            Decimal::from_str("17052.703394166808800955").unwrap()
        );
        // The vault is only funded with 1000000000, so the offer should be clamped, less 0.2% fee
        assert_eq!(quote_1.size, Uint128::from(999800000u128));
        // assert_eq!(quote.size, Uint128::from(136658118u128));

        // Out of borrowable funds, no more quotes
        app.wrap()
            .query_wasm_smart::<QuoteResponse>(
                contract.clone(),
                &QueryMsg::Quote(QuoteRequest {
                    min_price: None,
                    offer_denom: "btc-btc".to_string(),
                    ask_denom: "rune".to_string(),
                    data: quote_1.data,
                }),
            )
            .unwrap_err();

        // A to B
        let quote_1: QuoteResponse = app
            .wrap()
            .query_wasm_smart(
                contract.clone(),
                &QueryMsg::Quote(QuoteRequest {
                    min_price: None,
                    ask_denom: "btc-btc".to_string(),
                    offer_denom: "eth-usdc-0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
                    data: None,
                }),
            )
            .unwrap();

        assert_eq!(
            quote_1.price,
            Decimal::from_str("0.000011886568780303").unwrap()
        );
        assert_eq!(quote_1.size, Uint128::from(12705082u128));

        let quote_2: QuoteResponse = app
            .wrap()
            .query_wasm_smart(
                contract.clone(),
                &QueryMsg::Quote(QuoteRequest {
                    min_price: None,
                    ask_denom: "btc-btc".to_string(),
                    offer_denom: "eth-usdc-0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
                    data: quote_1.data,
                }),
            )
            .unwrap();

        assert_eq!(
            quote_2.price,
            Decimal::from_str("0.000011830388381886").unwrap()
        );

        assert_eq!(quote_2.size, Uint128::from(12645033u128));

        assert!(quote_2.price < quote_1.price);
    }
}
