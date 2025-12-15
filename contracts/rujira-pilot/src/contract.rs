use crate::config::{Config, CONFIG};
use crate::error::ContractError;
use crate::order::Order;
use crate::order_manager::OrderManager;
use crate::pool::Pool;
use crate::premium::Premium;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coin, to_json_binary, BankMsg, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    QuerierWrapper, Response,
};
use cw2::set_contract_version;
use cw_utils::{must_pay, NativeBalance};
use rujira_rs::exchange::{SwapError, SwapResult, Swappable, Swapper};
use rujira_rs::fin::SwapRequest;
use rujira_rs::pilot::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, OrderResponse, OrdersResponse, PoolResponse,
    PoolsResponse, QueryMsg, SimulationResponse, SudoMsg,
};

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config::new(deps.api, msg)?;
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
    let config = CONFIG.load(deps.storage)?;
    let oracle = load_oracle_price(deps.querier, &config)?;
    let mut fees = NativeBalance::default();
    let mut messages: Vec<CosmosMsg> = vec![];

    match msg {
        ExecuteMsg::Swap {
            min_return,
            to,
            callback,
        } => {
            let req = match min_return {
                Some(min_return) => SwapRequest::Min {
                    min_return,
                    to: to.clone(),
                    callback,
                },
                None => SwapRequest::Yolo {
                    to: to.clone(),
                    callback,
                },
            };
            let to = to.map(|x| deps.api.addr_validate(&x)).transpose()?;
            let funds = must_pay(&info, config.denoms.ask())?;
            let mut swapper = Swapper::new(env!("CARGO_PKG_NAME"), funds, req, config.fee_taker);
            let res = simulate_swap(&mut swapper, deps.as_ref(), &oracle)?;
            swapper.commit(deps.storage)?;

            let mut funds = NativeBalance(vec![
                coin(res.return_amount.u128(), config.denoms.bid()),
                coin(res.remaining_offer.u128(), config.denoms.ask()),
            ]);

            funds.normalize();

            if !funds.is_empty() {
                messages.push(CosmosMsg::Bank(BankMsg::Send {
                    to_address: to.unwrap_or(info.sender).to_string(),
                    amount: funds.into_vec(),
                }))
            }

            fees += coin(res.fee_amount.u128(), config.denoms.bid());

            fees.normalize();

            if !fees.is_empty() {
                messages.push(CosmosMsg::Bank(BankMsg::Send {
                    to_address: config.fee_address.to_string(),
                    amount: fees.into_vec(),
                }))
            }

            Ok(Response::default()
                .add_messages(messages)
                .add_events(res.events))
        }
        ExecuteMsg::Order(vec) => {
            let mut e = OrderManager::new(
                config.denoms.clone(),
                config.fee_maker,
                config.max_premium,
                info.sender.clone(),
                env.block.time,
                oracle,
                NativeBalance(info.funds),
            );

            let res = e.execute_orders(deps.storage, vec)?;
            fees += res.fees;

            if !res.withdraw.is_empty() {
                messages.push(CosmosMsg::Bank(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: res.withdraw.into_vec(),
                }))
            }

            fees.normalize();

            if !fees.is_empty() {
                messages.push(CosmosMsg::Bank(BankMsg::Send {
                    to_address: config.fee_address.to_string(),
                    amount: fees.into_vec(),
                }))
            }

            Ok(Response::default()
                .add_messages(messages)
                .add_events(res.events))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    match msg {
        SudoMsg::UpdateConfig {
            fee_taker,
            fee_maker,
            fee_address,
        } => {
            let fee_address = fee_address
                .map(|x| deps.api.addr_validate(&x))
                .transpose()?;

            let mut config = CONFIG.load(deps.storage)?;

            config.update(fee_taker, fee_maker, fee_address);
            config.save(deps.storage)?;
            Ok(Response::default())
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let oracle = load_oracle_price(deps.querier, &config)?;
    match msg {
        QueryMsg::Config {} => to_json_binary(&ConfigResponse::from(config)),
        QueryMsg::Order((owner, premium)) => {
            let addr = deps.api.addr_validate(&owner)?;
            let pool = Pool::load(deps.storage, &premium, &oracle);
            let order = pool.load_order(deps.storage, &addr)?;
            to_json_binary(&order_response(&order, &pool.premium, &oracle))
        }
        QueryMsg::Orders {
            owner,
            offset,
            limit,
        } => {
            let addr = deps.api.addr_validate(owner.as_str())?;
            let orders: Result<Vec<OrderResponse>, ContractError> =
                Order::by_owner(deps.storage, &addr, offset, limit)?
                    .iter_mut()
                    .map(|(k, order)| {
                        let pool = Pool::load(deps.storage, k, &oracle);
                        pool.sync_order(deps.storage, order)?;
                        Ok(order_response(order, k, &oracle))
                    })
                    .collect();

            to_json_binary(&OrdersResponse { orders: orders? })
        }
        QueryMsg::Pools { limit, offset } => {
            let limit = limit.unwrap_or(100);
            let offset = offset.unwrap_or(0);

            let pools = Pool::iter(deps.storage, &oracle)
                .skip(offset as usize)
                .take(limit as usize)
                .map(|v| PoolResponse {
                    premium: v.premium,
                    epoch: v.pool.epoch(),
                    price: v.rate(),
                    total: v.total(),
                })
                .collect();

            to_json_binary(&PoolsResponse { pools })
        }
        QueryMsg::Simulate(offer) => {
            let mut swapper = Swapper::new(
                env!("CARGO_PKG_NAME"),
                offer.amount,
                SwapRequest::Yolo {
                    to: None,
                    callback: None,
                },
                config.fee_taker,
            );
            let res = simulate_swap(&mut swapper, deps, &oracle)?;
            to_json_binary(&SimulationResponse {
                returned: res.return_amount,
                fee: res.fee_amount,
            })
        }
    }
    .map_err(ContractError::Std)
}

fn simulate_swap(
    swapper: &mut Swapper<Pool>,
    deps: Deps,
    oracle: &Decimal,
) -> Result<SwapResult, SwapError> {
    let mut iter = Pool::iter(deps.storage, oracle);
    swapper.swap(&mut iter)
}

fn load_oracle_price(_q: QuerierWrapper, _config: &Config) -> Result<Decimal, ContractError> {
    // let v: Decimal =
    //     q.query_wasm_smart(config.executor.to_string(), &ExecutorQueryMsg::Price {})?;

    Ok(Decimal::from_ratio(100000u128, 1u128))
}

fn order_response(order: &Order, premium: &u8, oracle: &Decimal) -> OrderResponse {
    OrderResponse {
        owner: order.owner.to_string(),
        premium: *premium,
        rate: premium.to_rate(oracle),
        updated_at: order.updated_at,
        offer: order.offer,
        remaining: order.bid.amount().try_into().unwrap(),
        filled: order.bid.filled().try_into().unwrap(),
    }
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use super::*;
    use cosmwasm_std::{coin, coins, Addr, Decimal, Event, Uint128};
    use cw_multi_test::{BasicApp, ContractWrapper, Executor};

    use rujira_rs::pilot::Denoms;
    use rujira_rs_testing::{mock_rujira_app, RujiraApp};

    fn setup() -> (RujiraApp, Addr) {
        let mut app = mock_rujira_app();

        let owner = app.api().addr_make("owner");

        let code = Box::new(ContractWrapper::new(execute, instantiate, query));
        let code_id = app.store_code(code);
        let contract = app
            .instantiate_contract(
                code_id,
                owner,
                &InstantiateMsg {
                    denoms: Denoms::new("btc-btc", "eth-usdc"),
                    max_premium: 30,
                    executor: app.api().addr_make("executor").to_string(),
                    fee_taker: Decimal::zero(),
                    fee_maker: Decimal::zero(),
                    fee_address: app.api().addr_make("fee").to_string(),
                },
                &[],
                "template",
                None,
            )
            .unwrap();

        (app, contract)
    }

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
                denoms: Denoms::new("ruji", "eth-usdc"),
                executor: app.api().addr_make("executor").to_string(),
                max_premium: 30,
                fee_taker: Decimal::zero(),
                fee_maker: Decimal::zero(),
                fee_address: app.api().addr_make("fee").to_string(),
            },
            &[],
            "template",
            None,
        )
        .unwrap();
    }

    #[test]
    fn query_pools() {
        let (mut app, contract) = setup();
        let owner = app.api().addr_make("owner");
        let funds = vec![coin(500_000_000, "btc-btc"), coin(500_000_000, "eth-usdc")];
        app.init_modules(|router, _, storage| {
            router
                .bank
                .init_balance(storage, &owner, funds.clone())
                .unwrap();
        });

        app.execute_contract(
            owner.clone(),
            contract.clone(),
            &ExecuteMsg::Order(vec![
                (1, Uint128::from(10000u128)),
                (10, Uint128::from(12500u128)),
                (20, Uint128::from(51000u128)),
            ]),
            &funds,
        )
        .unwrap();

        let pools: PoolsResponse = app
            .wrap()
            .query_wasm_smart(
                contract,
                &QueryMsg::Pools {
                    limit: None,
                    offset: None,
                },
            )
            .unwrap();

        assert_eq!(pools.pools.len(), 3);
        let entry = pools.pools[0].clone();
        assert_eq!(entry.price, Decimal::from_str("99000").unwrap());
        assert_eq!(entry.total, Uint128::from(10000u128));
        let entry = pools.pools[1].clone();
        assert_eq!(entry.price, Decimal::from_str("90000").unwrap());
        assert_eq!(entry.total, Uint128::from(12500u128));

        #[allow(deprecated)]
        let balance = app.wrap().query_all_balances(owner.to_string()).unwrap();
        assert_eq!(
            balance,
            vec![coin(500_000_000, "btc-btc"), coin(499_926_500, "eth-usdc")]
        );
    }

    #[test]
    fn swap() {
        let (mut app, contract) = setup();
        let owner = app.api().addr_make("owner");
        let funds = vec![
            coin(500_000_000_000_000, "btc-btc"),
            coin(500_000_000_000_000, "eth-usdc"),
        ];
        app.init_modules(|router, _, storage| {
            router
                .bank
                .init_balance(storage, &owner, funds.clone())
                .unwrap();
        });

        app.execute_contract(
            owner.clone(),
            contract.clone(),
            &ExecuteMsg::Order(vec![
                (10, Uint128::from(10000000u128)),
                (15, Uint128::from(12500000u128)),
                (20, Uint128::from(51000000u128)),
            ]),
            &coins(73500000, "eth-usdc"),
        )
        .unwrap();
        let swap_amount = coin(100, "btc-btc");

        let sim: SimulationResponse = app
            .wrap()
            .query_wasm_smart(contract.clone(), &QueryMsg::Simulate(swap_amount.clone()))
            .unwrap();

        let res = app
            .execute_contract(
                owner.clone(),
                contract.clone(),
                &ExecuteMsg::Swap {
                    min_return: None,
                    to: None,
                    callback: None,
                },
                &[swap_amount],
            )
            .unwrap();
        dbg!(&res);
        res.assert_event(&Event::new("wasm-rujira-pilot/trade").add_attributes(vec![
            ("offer", "100"),
            ("bid", "9000000"),
            ("rate", "90000"),
            ("premium", "10"),
        ]));
        res.assert_event(&Event::new("transfer").add_attributes(vec![
            ("recipient", owner.as_str()),
            ("sender", contract.as_str()),
            ("amount", "9000000eth-usdc"),
        ]));

        assert_eq!(sim.returned, Uint128::from(9000000u128));

        let res = app
            .execute_contract(
                owner.clone(),
                contract.clone(),
                &ExecuteMsg::Swap {
                    min_return: None,
                    to: None,
                    callback: None,
                },
                &coins(300, "btc-btc"),
            )
            .unwrap();
        res.assert_event(&Event::new("wasm-rujira-pilot/trade").add_attributes(vec![
            ("offer", "11"),
            ("bid", "1000000"),
            ("rate", "90000"),
        ]));
        res.assert_event(&Event::new("wasm-rujira-pilot/trade").add_attributes(vec![
            ("offer", "147"),
            ("bid", "12500000"),
            ("rate", "85000"),
        ]));
        res.assert_event(&Event::new("wasm-rujira-pilot/trade").add_attributes(vec![
            ("offer", "142"),
            ("bid", "11360000"),
            ("rate", "80000"),
        ]));
        res.assert_event(&Event::new("transfer").add_attributes(vec![
            ("recipient", owner.as_str()),
            ("sender", contract.as_str()),
            ("amount", "24860000eth-usdc"),
        ]));

        let res = app
            .execute_contract(
                owner.clone(),
                contract.clone(),
                &ExecuteMsg::Swap {
                    min_return: None,
                    to: None,
                    callback: None,
                },
                &coins(1000, "btc-btc"),
            )
            .unwrap();
        // Check that any unused funds are returned
        res.assert_event(&Event::new("wasm-rujira-pilot/trade").add_attributes(vec![
            ("offer", "495"),
            ("bid", "39640000"),
            ("rate", "80000"),
        ]));
        res.assert_event(&Event::new("transfer").add_attributes(vec![
            ("recipient", owner.as_str()),
            ("sender", contract.as_str()),
            ("amount", "505btc-btc,39640000eth-usdc"),
        ]));
    }
}
