#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_json_binary, Addr, BankMsg, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QuerierWrapper, Reply, Response, StdResult, Storage, SubMsg, WasmMsg,
};

use crate::error::ContractError;
use crate::events::event_run;
use crate::state::{Action, Config};
use rujira_rs::revenue::{
    ActionResponse, ActionsResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    StatusResponse, SudoMsg,
};

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config::new(deps.api, msg)?;
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
    let config = Config::load(deps.storage)?;
    match msg {
        ExecuteMsg::Run {} => {
            if info.sender != config.executor {
                return Err(ContractError::Unauthorized {});
            }
            let action_msg = get_action_msg(deps.storage, deps.querier, &env.contract.address)?;

            match action_msg {
                Some((action, msg)) => Ok(Response::default()
                    .add_event(event_run(action.denom))
                    .add_submessage(SubMsg::reply_always(msg, 0))),
                // If there's no compatible action, skip to the reply
                None => {
                    let mut sends: Vec<CosmosMsg> = vec![];
                    for target in config.target_denoms() {
                        distribute_denom(deps.as_ref(), &env, &config, &mut sends, target)?;
                    }

                    Ok(Response::default().add_messages(sends))
                }
            }
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    let mut config = Config::load(deps.storage)?;
    match msg {
        SudoMsg::SetOwner(owner) => {
            config.owner = deps.api.addr_validate(&owner)?;
            config.save(deps.storage)?;
            Ok(Response::default())
        }
        SudoMsg::SetAction {
            denom,
            contract,
            limit,
            msg,
        } => {
            Action::set(
                deps.storage,
                denom,
                deps.api.addr_validate(&contract)?,
                limit,
                msg,
            )?;
            Ok(Response::default())
        }
        SudoMsg::UnsetAction(denom) => {
            Action::unset(deps.storage, denom);
            Ok(Response::default())
        }
        SudoMsg::SetExecutor(executor) => {
            config.executor = deps.api.addr_validate(&executor)?;
            config.save(deps.storage)?;
            Ok(Response::default())
        }
        SudoMsg::AddTargetDenom(denom) => {
            config.add_target_denom(denom);
            config.save(deps.storage)?;
            Ok(Response::default())
        }
    }
}

fn get_action_msg(
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    contract: &Addr,
) -> StdResult<Option<(Action, WasmMsg)>> {
    // Fetch the next action in the iterator
    if let Some(action) = Action::next(storage)? {
        let balance = querier.query_balance(contract, action.denom.to_string())?;
        return match action.execute(balance)? {
            None => Ok(None),
            Some(msg) => Ok(Some((action, msg))),
        };
    }
    Ok(None)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, _msg: Reply) -> Result<Response, ContractError> {
    execute_reply(deps.as_ref(), env)
}

pub fn execute_reply(deps: Deps, env: Env) -> Result<Response, ContractError> {
    let config = Config::load(deps.storage)?;
    let mut sends: Vec<CosmosMsg> = vec![];
    for target in config.target_denoms().clone() {
        distribute_denom(deps, &env, &config, &mut sends, target)?;
    }

    Ok(Response::default().add_messages(sends))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&ConfigResponse::from(Config::load(deps.storage)?)),
        QueryMsg::Actions {} => to_json_binary(&ActionsResponse {
            actions: Action::all(deps.storage)?
                .iter()
                .map(|x| ActionResponse::from(x.clone()))
                .collect(),
        }),
        QueryMsg::Status {} => to_json_binary(&StatusResponse {
            last: Action::last(deps.storage)?,
        }),
    }
}

fn distribute_denom(
    deps: Deps,
    env: &Env,
    config: &Config,
    sends: &mut Vec<CosmosMsg>,
    denom: String,
) -> StdResult<()> {
    let balance = deps
        .querier
        .query_balance(env.contract.address.clone(), denom.to_string())?;

    let total_weight = config.target_addresses.iter().fold(0, |a, e| e.1 + a);
    if !balance.amount.is_zero() {
        let mut remaining = balance.amount;
        let mut targets = config.target_addresses.iter().peekable();

        while let Some((addr, weight)) = targets.next() {
            let amount = if targets.peek().is_none() {
                remaining
            } else {
                let ratio = Decimal::from_ratio(*weight, total_weight);
                balance.amount.mul_floor(ratio)
            };

            if amount.is_zero() {
                continue;
            }
            remaining -= amount;
            sends.push(
                BankMsg::Send {
                    to_address: addr.to_string(),
                    amount: coins(amount.u128(), denom.clone()),
                }
                .into(),
            )
        }
    };
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use cosmwasm_std::{
        coin, from_json,
        testing::{message_info, mock_dependencies, mock_env},
        Uint128,
    };
    use cw_multi_test::{BasicApp, ContractWrapper, Executor};

    #[test]
    fn instantiation() {
        let mut deps = mock_dependencies();
        let app = BasicApp::default();
        let owner = app.api().addr_make("owner");
        let fees = app.api().addr_make("fees");
        let executor = app.api().addr_make("executor");
        let info = message_info(&owner, &[]);
        let msg = InstantiateMsg {
            owner: owner.to_string(),
            target_denoms: vec!["uruji".to_string(), "another".to_string()],
            target_addresses: vec![(fees.to_string(), 1)],
            executor: executor.to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        let config: ConfigResponse =
            from_json(query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
        assert_eq!(config.owner, owner.to_string());
        assert_eq!(
            config.target_denoms,
            vec!["uruji".to_string(), "another".to_string()],
        );
        let status: StatusResponse =
            from_json(query(deps.as_ref(), mock_env(), QueryMsg::Status {}).unwrap()).unwrap();
        assert_eq!(status.last, None);
        let actions: ActionsResponse =
            from_json(query(deps.as_ref(), mock_env(), QueryMsg::Actions {}).unwrap()).unwrap();
        assert_eq!(actions.actions, vec![]);
    }
    #[test]
    fn authorization() {
        let mut deps = mock_dependencies();
        let app = BasicApp::default();
        let owner = app.api().addr_make("owner");
        let fees = app.api().addr_make("fees");
        let executor = app.api().addr_make("executor");

        let info = message_info(&owner, &[]);
        let msg = InstantiateMsg {
            owner: owner.to_string(),
            target_denoms: vec!["uruji".to_string(), "another".to_string()],
            target_addresses: vec![(fees.to_string(), 1)],
            executor: executor.to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        sudo(
            deps.as_mut(),
            mock_env(),
            SudoMsg::SetOwner(app.api().addr_make("owner-new").to_string()),
        )
        .unwrap();

        let contract = app.api().addr_make("fin");
        let action = Action {
            denom: "uatom".to_string(),
            contract,
            limit: Uint128::MAX,
            msg: Binary::new(vec![0]),
        };

        let a = action.clone();

        sudo(
            deps.as_mut(),
            mock_env(),
            SudoMsg::SetAction {
                denom: a.denom,
                contract: a.contract.to_string(),
                limit: a.limit,
                msg: a.msg,
            },
        )
        .unwrap();

        let actions: ActionsResponse =
            from_json(query(deps.as_ref(), mock_env(), QueryMsg::Actions {}).unwrap()).unwrap();
        assert_eq!(
            actions.actions,
            vec![ActionResponse {
                denom: action.denom.clone(),
                contract: action.contract.to_string(),
                limit: action.limit,
                msg: action.msg
            }]
        );

        sudo(
            deps.as_mut(),
            mock_env(),
            SudoMsg::UnsetAction(action.denom),
        )
        .unwrap();

        let actions: ActionsResponse =
            from_json(query(deps.as_ref(), mock_env(), QueryMsg::Actions {}).unwrap()).unwrap();
        assert_eq!(actions.actions, vec![]);

        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&executor, &[]),
            ExecuteMsg::Run {},
        )
        .unwrap();

        sudo(
            deps.as_mut(),
            mock_env(),
            SudoMsg::SetExecutor(app.api().addr_make("owner-new").to_string()),
        )
        .unwrap();

        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&app.api().addr_make("owner-new"), &[]),
            ExecuteMsg::Run {},
        )
        .unwrap();
    }

    #[test]
    fn cranking() {
        let mut app = BasicApp::default();
        let owner = app.api().addr_make("owner");
        let fees = app.api().addr_make("fees");

        let funds = vec![
            // coin(1000u128, "token-a"),
            coin(1000u128, "token-b"),
            coin(1000u128, "token-c"),
            coin(1000u128, "token-d"),
            coin(1000u128, "token-e"),
        ];

        app.init_modules(|router, _, storage| {
            router.bank.init_balance(storage, &owner, funds.clone())
        })
        .unwrap();
        let code = Box::new(
            ContractWrapper::new(execute, instantiate, query)
                .with_reply(reply)
                .with_sudo(sudo),
        );
        let code_id = app.store_code(code);
        let contract = app
            .instantiate_contract(
                code_id,
                owner.clone(),
                &InstantiateMsg {
                    owner: owner.to_string(),
                    target_denoms: vec!["uruji".to_string(), "another".to_string()],
                    target_addresses: vec![(fees.to_string(), 1)],
                    executor: owner.to_string(),
                },
                &[],
                "revenue",
                None,
            )
            .unwrap();

        app.send_tokens(owner.clone(), contract.clone(), &funds)
            .unwrap();

        app.execute_contract(owner.clone(), contract.clone(), &ExecuteMsg::Run {}, &[])
            .unwrap();

        // Make sure that execution ends when there are no actions
        let status: StatusResponse = app
            .wrap()
            .query_wasm_smart(contract.clone(), &QueryMsg::Status {})
            .unwrap();
        assert_eq!(status.last, None);

        // Set some actions
        set_action(&mut app, &contract, "token-a", "contract-a", Uint128::MAX);
        set_action(&mut app, &contract, "token-b", "contract-b", Uint128::MAX);
        set_action(
            &mut app,
            &contract,
            "token-c",
            "contract-c",
            Uint128::from(100u128),
        );
        set_action(&mut app, &contract, "token-d", "contract-d", Uint128::MAX);
        set_action(&mut app, &contract, "token-e", "contract-e", Uint128::MAX);

        let res = app
            .execute_contract(owner.clone(), contract.clone(), &ExecuteMsg::Run {}, &[])
            .unwrap();
        assert_eq!(res.events.len(), 1);
        let status: StatusResponse = app
            .wrap()
            .query_wasm_smart(contract.clone(), &QueryMsg::Status {})
            .unwrap();
        assert_eq!(status.last, Some("token-a".to_string()));

        // Iterator should start at the beginning again and execute token-a
        let res = app
            .execute_contract(owner.clone(), contract.clone(), &ExecuteMsg::Run {}, &[])
            .unwrap();
        let status: StatusResponse = app
            .wrap()
            .query_wasm_smart(contract.clone(), &QueryMsg::Status {})
            .unwrap();
        assert_eq!(status.last, Some("token-b".to_string()));
        assert_eq!(res.events[1].clone().ty, "wasm-rujira-revenue/run");
        assert_eq!(res.events[1].clone().attributes[1].clone().key, "denom");
        assert_eq!(res.events[1].clone().attributes[1].clone().value, "token-b");

        // Run for c, d, e and then loop back to a
        let res = app
            .execute_contract(owner.clone(), contract.clone(), &ExecuteMsg::Run {}, &[])
            .unwrap();
        assert_eq!(res.events[1].clone().attributes[1].clone().value, "token-c");

        let res = app
            .execute_contract(owner.clone(), contract.clone(), &ExecuteMsg::Run {}, &[])
            .unwrap();
        assert_eq!(res.events[1].clone().attributes[1].clone().value, "token-d");

        let res = app
            .execute_contract(owner.clone(), contract.clone(), &ExecuteMsg::Run {}, &[])
            .unwrap();
        assert_eq!(res.events[1].clone().attributes[1].clone().value, "token-e");

        let res = app
            .execute_contract(owner.clone(), contract.clone(), &ExecuteMsg::Run {}, &[])
            .unwrap();

        assert_eq!(res.events.len(), 1);
        let status: StatusResponse = app
            .wrap()
            .query_wasm_smart(contract.clone(), &QueryMsg::Status {})
            .unwrap();
        assert_eq!(status.last, Some("token-a".to_string()));
    }

    fn set_action(app: &mut BasicApp, contract: &Addr, denom: &str, target: &str, limit: Uint128) {
        app.wasm_sudo(
            contract.clone(),
            &SudoMsg::SetAction {
                denom: denom.to_string(),
                contract: app.api().addr_make(target).to_string(),
                limit,
                msg: Binary::new(vec![0]),
            },
        )
        .unwrap();
    }

    #[test]
    fn distribution() {
        let mut app = BasicApp::default();
        let fees = app.api().addr_make("fees");
        let owner = app.api().addr_make("owner");
        let another = app.api().addr_make("another");

        let funds = vec![
            coin(1000u128, "uruji"),
            coin(2000u128, "another"),
            coin(2000u128, "ignored"),
        ];

        app.init_modules(|router, _, storage| {
            router.bank.init_balance(storage, &owner, funds.clone())
        })
        .unwrap();

        let code = Box::new(
            ContractWrapper::new(execute, instantiate, query)
                .with_reply(reply)
                .with_sudo(sudo),
        );
        let code_id = app.store_code(code);
        let contract = app
            .instantiate_contract(
                code_id,
                owner.clone(),
                &InstantiateMsg {
                    owner: owner.to_string(),
                    target_denoms: vec!["uruji".to_string(), "another".to_string()],
                    target_addresses: vec![
                        (fees.to_string(), 1),
                        (another.to_string(), 3),
                        (app.api().addr_make("nope").to_string(), 0),
                    ],
                    executor: owner.to_string(),
                },
                &[],
                "revenue",
                None,
            )
            .unwrap();

        app.send_tokens(owner.clone(), contract.clone(), &funds)
            .unwrap();

        // Dummy action to make sure it cranks the reply
        set_action(&mut app, &contract, "token-a", "contract-a", Uint128::MAX);

        // Make sure that execution ends when there are no actions
        app.execute_contract(owner.clone(), contract.clone(), &ExecuteMsg::Run {}, &[])
            .unwrap();

        assert_eq!(
            app.wrap()
                .query_balance(fees.clone(), "uruji")
                .unwrap()
                .amount,
            Uint128::from(250u128)
        );

        assert_eq!(
            app.wrap()
                .query_balance(another.clone(), "uruji")
                .unwrap()
                .amount,
            Uint128::from(750u128)
        );

        assert_eq!(
            app.wrap()
                .query_balance(another.clone(), "ignored")
                .unwrap()
                .amount,
            Uint128::zero()
        );

        assert_eq!(
            app.wrap()
                .query_balance(fees.clone(), "another")
                .unwrap()
                .amount,
            Uint128::from(500u128)
        );

        assert_eq!(
            app.wrap()
                .query_balance(another.clone(), "another")
                .unwrap()
                .amount,
            Uint128::from(1500u128)
        );

        assert_eq!(
            app.wrap()
                .query_balance(another.clone(), "ignored")
                .unwrap()
                .amount,
            Uint128::zero()
        );

        app.wasm_sudo(
            contract.clone(),
            &SudoMsg::AddTargetDenom("ignored".to_string()),
        )
        .unwrap();

        app.execute_contract(owner.clone(), contract.clone(), &ExecuteMsg::Run {}, &[])
            .unwrap();

        assert_eq!(
            app.wrap().query_balance(fees, "ignored").unwrap().amount,
            Uint128::from(500u128)
        );

        assert_eq!(
            app.wrap().query_balance(another, "ignored").unwrap().amount,
            Uint128::from(1500u128)
        );
    }
}
