#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, from_json, to_json_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo,
    Reply, Response, StdResult, Storage, SubMsg, SubMsgResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_utils::{may_pay, must_pay, nonpayable, NativeBalance};
use rujira_rs::reply::sub_msg_response_to_info;
use rujira_rs::staking::{
    AccountMsg, ConfigResponse, ExecuteMsg, InstantiateMsg, LiquidMsg, QueryMsg, SudoMsg,
};
use rujira_rs::TokenFactory;

use crate::config::Config;
use crate::error::ContractError;
use crate::events::{
    event_account_bond, event_account_claim, event_account_withdraw, event_liquid_bond,
    event_liquid_unbond,
};
use crate::state::{
    account, distribute, execute_account_bond, execute_account_claim, execute_account_withdraw,
    execute_liquid_bond, execute_liquid_unbond, increase_pending_swap, init, status,
};

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPLY_ID: u64 = 0;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config::new(deps.api, msg.clone())?;
    config.validate()?;
    config.save(deps.storage)?;
    init(deps.storage)?;
    let share_denom = TokenFactory::new(&env, format!("staking-{}", config.bond_denom).as_str());
    Ok(Response::default().add_message(share_denom.create_msg(msg.receipt_token_metadata)))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: ()) -> Result<Response, ContractError> {
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
    let bond_amount_sent = may_pay(&info, config.bond_denom.as_str()).unwrap_or_default();
    let (swap_amount, fee_amount) =
        distribute(&env, deps.querier, deps.storage, &config, &bond_amount_sent)?;
    let mut res = match msg {
        ExecuteMsg::Account(account_msg) => {
            execute_account(deps.storage, info, &config, account_msg)
        }
        ExecuteMsg::Liquid(liquid_msg) => {
            execute_liquid(deps.storage, &env, info, &config, liquid_msg)
        }
    }?;
    if swap_amount.gt(&Uint128::zero()) {
        let sub_msg = SubMsg::reply_always(
            WasmMsg::Execute {
                contract_addr: config.revenue_converter.0.to_string(),
                msg: config.revenue_converter.1.clone(),
                funds: coins(swap_amount.u128(), config.revenue_denom.clone()),
            },
            REPLY_ID,
        )
        .with_payload(to_json_binary(&swap_amount)?);
        res = res.add_submessage(sub_msg)
    }
    if fee_amount.gt(&Uint128::zero()) {
        res = res.add_message(BankMsg::Send {
            to_address: config
                .fee
                .as_ref()
                .ok_or(ContractError::Invalid("".to_string()))?
                .recipient
                .to_string(),
            amount: coins(fee_amount.u128(), config.revenue_denom.clone()),
        });
    }
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    let mut config = Config::load(deps.storage)?;

    match msg {
        SudoMsg::SetRevenueConverter {
            contract,
            msg,
            limit,
        } => {
            config.revenue_converter = (deps.api.addr_validate(&contract)?, msg, limit);
            config.save(deps.storage)?;
            Ok(Response::default())
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    // Match on ID for completeness
    match msg.id {
        REPLY_ID => {
            match &msg.result {
                SubMsgResult::Err(_) => {
                    // Swap failed, we need to return all the swap amount
                    let ongoing_swap = from_json(&msg.payload)?;
                    increase_pending_swap(deps.storage, ongoing_swap)?;
                }
                SubMsgResult::Ok(res) => {
                    // Swap succeeded, we need to check if there were any returned funds
                    let info = sub_msg_response_to_info(res, &deps, &env)?;
                    let config = Config::load(deps.storage)?;
                    let amount = info
                        .funds
                        .iter()
                        .find(|c| c.denom == config.revenue_denom)
                        .map(|c| c.amount)
                        .unwrap_or_else(Uint128::zero);
                    if !amount.is_zero() {
                        increase_pending_swap(deps.storage, amount)?;
                    }
                }
            }
            Ok(Response::default())
        }
        _ => Err(ContractError::Unauthorized {}),
    }
}

fn execute_account(
    storage: &mut dyn Storage,
    info: MessageInfo,
    config: &Config,
    msg: AccountMsg,
) -> Result<Response, ContractError> {
    match msg {
        AccountMsg::Bond {} => {
            let amount = must_pay(&info, config.bond_denom.as_str())?;
            let reward_amount = execute_account_bond(storage, &info.sender, amount)?;
            let mut response =
                Response::default().add_event(event_account_bond(info.sender.clone(), amount));
            if reward_amount.gt(&Uint128::zero()) {
                response = response.add_message(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: coins(reward_amount.u128(), config.revenue_denom.clone()),
                });
            }
            Ok(response)
        }
        AccountMsg::Claim {} => {
            nonpayable(&info)?;
            let reward_amount = execute_account_claim(storage, &info.sender)?;
            let mut response = Response::default()
                .add_event(event_account_claim(info.sender.clone(), reward_amount));
            if reward_amount.gt(&Uint128::zero()) {
                response = response.add_message(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: coins(reward_amount.u128(), config.revenue_denom.clone()),
                });
            }
            Ok(response)
        }
        AccountMsg::Withdraw { amount } => {
            nonpayable(&info)?;
            let (rewards, amount) = execute_account_withdraw(storage, &info.sender, amount)?;
            let mut send = NativeBalance(vec![
                Coin::new(rewards, config.revenue_denom.clone()),
                Coin::new(amount, config.bond_denom.clone()),
            ]);
            send.normalize();

            let mut response = Response::default().add_event(event_account_withdraw(
                info.sender.clone(),
                amount,
                rewards,
            ));
            if !send.is_empty() {
                response = response.add_message(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: send.into_vec(),
                });
            }
            Ok(response)
        }
    }
}

fn execute_liquid(
    storage: &mut dyn Storage,
    env: &Env,
    info: MessageInfo,
    config: &Config,
    msg: LiquidMsg,
) -> Result<Response, ContractError> {
    let share_denom = TokenFactory::new(env, format!("staking-{}", config.bond_denom).as_str());

    match msg {
        LiquidMsg::Bond {} => {
            let amount = must_pay(&info, config.bond_denom.as_str())?;
            let shares = execute_liquid_bond(storage, amount)?;
            Ok(Response::default()
                .add_event(event_liquid_bond(info.sender.clone(), amount, shares))
                .add_message(share_denom.mint_msg(shares, info.sender)))
        }
        LiquidMsg::Unbond {} => {
            let shares = must_pay(&info, share_denom.denom().as_str())?;
            let returned = execute_liquid_unbond(storage, shares)?;
            Ok(Response::default()
                .add_event(event_liquid_unbond(info.sender.clone(), shares, returned))
                .add_message(share_denom.burn_msg(shares))
                .add_message(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: coins(returned.u128(), config.bond_denom.clone()),
                }))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let config = Config::load(deps.storage)?;
    match msg {
        QueryMsg::Config {} => to_json_binary(&ConfigResponse::from(config)),
        QueryMsg::Status {} => to_json_binary(&status(env, deps, &config)?),
        QueryMsg::Account { addr } => {
            to_json_binary(&account(deps.storage, deps.api.addr_validate(&addr)?)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{coin, Addr, Decimal, Event};
    use cw_multi_test::{AppResponse, ContractWrapper, Executor};
    use rujira_rs::{
        staking::{AccountResponse, StatusResponse},
        TokenMetadata,
    };
    use rujira_rs_testing::{mock_rujira_app, mock_swap_contract, MsgSwap, RujiraApp};

    enum TestFee {
        FivePercent,
        TenPercent,
    }

    #[test]
    fn lifecycle_without_fee() {
        lifecycle(None);
    }

    #[test]
    fn lifecycle_with_5_percent_fee() {
        lifecycle(Some(TestFee::FivePercent));
    }

    #[test]
    fn lifecycle_with_10_percent_fee() {
        lifecycle(Some(TestFee::TenPercent));
    }

    // Helper functions
    fn bond_account(
        app: &mut RujiraApp,
        contract: &Addr,
        account: &Addr,
        amount: u128,
        denom: &str,
    ) -> AppResponse {
        app.execute_contract(
            account.clone(),
            contract.clone(),
            &ExecuteMsg::Account(AccountMsg::Bond {}),
            &coins(amount, denom),
        )
        .unwrap()
    }

    fn bond_liquid(
        app: &mut RujiraApp,
        contract: &Addr,
        account: &Addr,
        amount: u128,
        denom: &str,
    ) -> AppResponse {
        app.execute_contract(
            account.clone(),
            contract.clone(),
            &ExecuteMsg::Liquid(LiquidMsg::Bond {}),
            &coins(amount, denom),
        )
        .unwrap()
    }

    fn assert_balance(app: &RujiraApp, account: &Addr, denom: &str, amount: Uint128) {
        let balance = app.wrap().query_balance(account.clone(), denom).unwrap();
        assert_eq!(balance.amount, amount);
    }

    fn get_pending_swap(app: &RujiraApp, contract: &Addr) -> Uint128 {
        let pending_swap_key = "s".as_bytes();
        let raw_value = app
            .wrap()
            .query_wasm_raw(contract, pending_swap_key)
            .unwrap()
            .unwrap();
        let raw_value_str = String::from_utf8(raw_value).unwrap();
        let inner_value = raw_value_str.trim_matches('"'); // We need to remove the external quotes
        let value: u128 = inner_value.parse().unwrap();
        Uint128::from(value)
    }

    fn assert_pending_swap(app: &RujiraApp, contract: &Addr, expected: Uint128) {
        assert_eq!(get_pending_swap(app, contract), expected);
    }

    fn assert_state(app: &RujiraApp, contract: &Addr, _comment: &str, expected: &StatusResponse) {
        let status: StatusResponse = app
            .wrap()
            .query_wasm_smart(contract.clone(), &QueryMsg::Status {})
            .unwrap();
        // dbg!(comment, &status);
        assert_eq!(status, *expected);
    }

    fn assert_account(app: &RujiraApp, contract: &Addr, expected: &AccountResponse) {
        let account: AccountResponse = app
            .wrap()
            .query_wasm_smart(
                contract.clone(),
                &QueryMsg::Account {
                    addr: expected.addr.clone(),
                },
            )
            .unwrap();
        assert_eq!(account, *expected);
    }

    struct Fees {
        pub index: usize,
        pub value: Option<(Decimal, String)>,
        pub percent: Decimal,
    }

    struct Stakers {
        pub liquid: Addr,
        pub account_1: Addr,
        pub account_2: Addr,
    }

    fn init_app(test_fee: Option<TestFee>) -> (RujiraApp, Addr, Addr, Stakers, Fees) {
        let mut app = mock_rujira_app();
        let apps = app.api().addr_make("apps");
        let owner = app.api().addr_make("owner");
        let stakers = Stakers {
            liquid: app.api().addr_make("stakers.liquid"),
            account_1: app.api().addr_make("stakers.account_1"),
            account_2: app.api().addr_make("stakers.account_2"),
        };
        let (fee_index, fee) = match test_fee {
            None => (0, None),
            Some(TestFee::FivePercent) => (1, Some((Decimal::percent(5), owner.to_string()))),
            Some(TestFee::TenPercent) => (2, Some((Decimal::percent(10), owner.to_string()))),
        };
        let fee_percentage = fee.as_ref().unwrap_or(&(Decimal::zero(), "".to_string())).0;
        let fees = Fees {
            index: fee_index,
            value: fee,
            percent: fee_percentage,
        };
        app.init_modules(|router, _, storage| {
            router
                .bank
                .init_balance(storage, &owner, coins(500_000_000, "uruji"))
                .unwrap();
            router
                .bank
                .init_balance(storage, &apps, coins(500_000_000, "uusdc"))
                .unwrap();
            router
                .bank
                .init_balance(storage, &stakers.liquid, coins(500_000_000, "uruji"))
                .unwrap();
            router
                .bank
                .init_balance(storage, &stakers.account_1, coins(500_000_000, "uruji"))
                .unwrap();
            router
                .bank
                .init_balance(storage, &stakers.account_2, coins(500_000_000, "uruji"))
                .unwrap()
        });
        (app, apps, owner, stakers, fees)
    }

    fn create_swap_contract(app: &mut RujiraApp, owner: &Addr) -> Addr {
        let swap_code_id = app.store_code(mock_swap_contract());
        app.instantiate_contract(
            swap_code_id,
            owner.clone(),
            &(),
            &coins(10_000_000u128, "uruji"),
            "swapper",
            None,
        )
        .unwrap()
    }

    fn create_staking_contract(
        app: &mut RujiraApp,
        owner: &Addr,
        fee: Option<(Decimal, String)>,
        swap_contract: Addr,
        converter_message: Binary,
    ) -> Addr {
        let code = Box::new(ContractWrapper::new(execute, instantiate, query).with_reply(reply));
        let code_id = app.store_code(code);
        app.instantiate_contract(
            code_id,
            owner.clone(),
            &InstantiateMsg {
                bond_denom: "uruji".to_string(),
                revenue_denom: "uusdc".to_string(),
                revenue_converter: (
                    swap_contract.to_string(),
                    converter_message,
                    Uint128::from(20u128),
                ),
                fee,
                receipt_token_metadata: TokenMetadata {
                    description: "description".to_string(),
                    name: "Liquid Staked RUJI".to_string(),
                    display: "sRUJI".to_string(),
                    symbol: "sRUJI".to_string(),
                    uri: None,
                    uri_hash: None,
                },
            },
            &[],
            "staking",
            None,
        )
        .unwrap()
    }
    // ----------

    fn lifecycle(test_fee: Option<TestFee>) {
        let (mut app, apps, owner, stakers, fees) = init_app(test_fee);
        let swap_contract = create_swap_contract(&mut app, &owner);
        let converter_message = to_json_binary(&MsgSwap {
            min_return: coin(10, "uruji"),
            return_funds: false,
        })
        .unwrap();
        let contract = create_staking_contract(
            &mut app,
            &owner,
            fees.value,
            swap_contract,
            converter_message,
        );

        // Liquid bonds its first 1_000
        bond_liquid(&mut app, &contract, &stakers.liquid, 1_000, "uruji");

        // share balance
        assert_balance(
            &app,
            &stakers.liquid,
            "x/staking-uruji",
            Uint128::from(1_000u128),
        );

        /*--- Liquid Bond 1_000 ----------------------
              | Acc_1 | Acc_2 | Accounts | Liquid | Total
        Stake |     0 |     0 |        0 |  1_000 | 1_000
        Rev%  |     0 |     0 |        0 |    100 |   100
        */
        assert_state(
            &app,
            &contract,
            "After stakers.liquid first bond of 1_000",
            &StatusResponse {
                account_bond: Uint128::zero(),
                assigned_revenue: Uint128::zero(),
                liquid_bond_shares: Uint128::from(1_000u128),
                liquid_bond_size: Uint128::from(1_000u128),
                undistributed_revenue: Uint128::zero(),
            },
        );

        // pool balance
        assert_balance(&app, &contract, "uruji", Uint128::from(1_000u128));

        // share balance
        assert_balance(
            &app,
            &stakers.liquid,
            "x/staking-uruji",
            Uint128::from(1_000u128),
        );

        // stakers.account_1 bonds its first 500
        bond_account(&mut app, &contract, &stakers.account_1, 500, "uruji");

        /*--- Acc_1 Bond 500 ----------------------
              | Acc_1    | Acc_2 | Accounts    | Liquid    | Total
        Stake |   500    |     0 |      500    |  1_000    | 1_500
        Rev%  |    33.33 |     0 |       33.33 |     66.67 |   100
                   (##1)
        */
        assert_state(
            &app,
            &contract,
            "After stakers.account_1 first bond of 500",
            &StatusResponse {
                account_bond: Uint128::from(500u128),
                assigned_revenue: Uint128::zero(),
                liquid_bond_shares: Uint128::from(1_000u128),
                liquid_bond_size: Uint128::from(1_000u128),
                undistributed_revenue: Uint128::zero(),
            },
        );

        let revenue_amount = 100;
        let revenue_amount_fee =
            (Decimal::from_atomics(revenue_amount, 0).unwrap() * fees.percent).to_uint_ceil();
        let revenue_amount_remaining = revenue_amount - revenue_amount_fee.u128();

        // Revenue collection 1
        app.send_tokens(
            apps.clone(),
            contract.clone(),
            &coins(revenue_amount, "uusdc"),
        )
        .unwrap();

        // stakers.account_2 bonds its first 500
        bond_account(&mut app, &contract, &stakers.account_2, 500, "uruji");

        /*--- Acc_2 Bond 500 ----------------------
              | Acc_1 | Acc_2 | Accounts | Liquid | Total
        Stake |   500 |   500 |    1_000 |  1_000 | 2_000
        Rev%  |    25 |    25 |       50 |     50 |   100
                 (##2)   (##3)
        */
        assert_state(
            &app,
            &contract,
            "After stakers.account_2 first bond of 500",
            &StatusResponse {
                account_bond: Uint128::from(1_000u128),
                assigned_revenue: [
                    Uint128::from(33u128),
                    Uint128::from(31u128),
                    Uint128::from(30u128),
                ][fees.index],
                liquid_bond_shares: Uint128::from(1_000u128),
                liquid_bond_size: Uint128::from(1_000u128),
                undistributed_revenue: [Uint128::one(), Uint128::one(), Uint128::zero()]
                    [fees.index],
            },
        );

        // Ensure the fees were collected
        assert_balance(&app, &owner, "uusdc", revenue_amount_fee);

        // We've staked _after_ the revenue has been collected. 33% share (given 1000/500 split when revenue was collected, see ##1)
        let expected_pending_revenue_500_1500 =
            Decimal::from_ratio(revenue_amount_remaining * 500, 1500u128).to_uint_floor();
        assert_account(
            &app,
            &contract,
            &AccountResponse {
                addr: stakers.account_1.to_string(),
                bonded: Uint128::from(500u128),
                pending_revenue: expected_pending_revenue_500_1500,
            },
        );

        // We've staked _after_ the revenue has been collected. No ownership
        assert_account(
            &app,
            &contract,
            &AccountResponse {
                addr: stakers.account_2.to_string(),
                bonded: Uint128::from(500u128),
                pending_revenue: Uint128::zero(),
            },
        );

        // Revenue collection 2
        app.send_tokens(
            apps.clone(),
            contract.clone(),
            &coins(revenue_amount, "uusdc"),
        )
        .unwrap();

        // Another bond to trigger a distribution
        // stakers.account_1 bonds its second 500, total 1000
        bond_account(&mut app, &contract, &stakers.account_1, 500, "uruji");

        let expected_pending_revenue_500_2000 =
            Decimal::from_ratio(revenue_amount_remaining * 500, 2000u128).to_uint_floor();
        let expected_pending_revenue_1000_2000 =
            Decimal::from_ratio(revenue_amount_remaining * 1000, 2000u128).to_uint_floor();

        // 1000/500 split of 100 (##1) + 1500/500 split of 100 (##2)
        assert_account(
            &app,
            &contract,
            &AccountResponse {
                addr: stakers.account_1.to_string(),
                bonded: Uint128::from(1000u128),
                pending_revenue: Uint128::zero(),
            },
        );

        let expected_claim_account_1 =
            expected_pending_revenue_500_1500 + expected_pending_revenue_500_2000;
        assert_balance(&app, &stakers.account_1, "uusdc", expected_claim_account_1); // WITHOUT_FEE: 58, WITH_FEE_5%: 54, WITH_FEE_10%: ??

        // 1500/500 split of 100 (##3)
        assert_account(
            &app,
            &contract,
            &AccountResponse {
                addr: stakers.account_2.to_string(),
                bonded: Uint128::from(500u128),
                pending_revenue: expected_pending_revenue_500_2000, // WITHOUT_FEE: 25, WITH_FEE: 23, WITH_FEE_10%: ??
            },
        );

        let expected_account_revenue = expected_pending_revenue_500_1500
            + expected_pending_revenue_1000_2000
            - expected_claim_account_1;

        /*--- Acc_1 Bond 500 ----------------------
              | Acc_1 | Acc_2 | Accounts | Liquid | Total
        Stake | 1_000 |   500 |    1_500 |  1_010 | 2_510   Notice the 10 extra in liquid coming from the swap
        Rev%  |    40 |    20 |       60 |     40 |   100
        */
        assert_state(
            &app,
            &contract,
            "After stakers.account_1 second bond of 500",
            &StatusResponse {
                account_bond: Uint128::from(1500u128),
                assigned_revenue: expected_account_revenue, // [Uint128::from(25u128), Uint128::from(24u128), Uint128::from(23u128)][fees.index]
                liquid_bond_shares: Uint128::from(1000u128),
                liquid_bond_size: Uint128::from(1010u128),
                undistributed_revenue: [Uint128::one(), Uint128::one(), Uint128::zero()]
                    [fees.index],
            },
        );

        // stakers.account_2 claims all
        let res = app
            .execute_contract(
                stakers.account_2.clone(),
                contract.clone(),
                &ExecuteMsg::Account(AccountMsg::Claim {}),
                &[],
            )
            .unwrap();
        let expected_claim_account_2 = expected_pending_revenue_500_2000; // WITHOUT_FEE: 25, WITH_FEE_5%: 23, WITH_FEE_10%: ??
        let expected_amount_attribute_value = format!("{}uusdc", expected_claim_account_2);

        res.assert_event(
            &Event::new("transfer")
                .add_attribute("amount", expected_amount_attribute_value)
                .add_attribute("recipient", stakers.account_2.clone()),
        );

        /*--- Acc_2 Claim All ----------------------
              | Acc_1 | Acc_2 | Accounts | Liquid | Total
        Stake | 1_000 |   500 |    1_500 |  1_020 | 2_520   Notice the 10 extra in liquid coming from the swap
        Rev%  |    40 |    20 |       60 |     40 |   100
        */
        let expected_account_revenue_2 = expected_account_revenue - expected_claim_account_2;
        assert_state(
            &app,
            &contract,
            "After stakers.account_2 claim all",
            &StatusResponse {
                account_bond: Uint128::from(1500u128),
                assigned_revenue: expected_account_revenue - expected_claim_account_2, // [Uint128::zero(), Uint128::one(), Uint128::one()][fees.index]
                liquid_bond_shares: Uint128::from(1000u128),
                liquid_bond_size: Uint128::from(1020u128),
                undistributed_revenue: [Uint128::one(), Uint128::zero(), Uint128::zero()]
                    [fees.index],
            },
        );

        // stakers.liquid undounds 500
        let res = app
            .execute_contract(
                stakers.liquid.clone(),
                contract.clone(),
                &ExecuteMsg::Liquid(LiquidMsg::Unbond {}),
                &coins(500, "x/staking-uruji".to_string()),
            )
            .unwrap();

        res.assert_event(
            &Event::new("burn")
                .add_attribute("amount", "500")
                .add_attribute("denom", "x/staking-uruji".to_string()),
        );

        // total distribution of 30 across 1000 shares, so withdrawal should be 515 uruji
        res.assert_event(
            &Event::new("transfer")
                .add_attribute("amount", "515uruji")
                .add_attribute("recipient", stakers.liquid),
        );

        /*--- Liquid Unbound 500 ----------------------
              | Acc_1 | Acc_2 | Accounts | Liquid | Total
        Stake | 1_000 |   500 |    1_500 |    515 | 2_015
        Rev%  |   ~50 |   ~25 |      ~75 |    ~25 |   100
        */
        assert_state(
            &app,
            &contract,
            "After stakers.liquid unbound 500",
            &StatusResponse {
                account_bond: Uint128::from(1500u128),
                assigned_revenue: expected_account_revenue_2, // [Uint128::zero(),  Uint128::one(), Uint128::one()][fees.index]
                liquid_bond_shares: Uint128::from(500u128),
                liquid_bond_size: Uint128::from(515u128),
                undistributed_revenue: [Uint128::one(), Uint128::zero(), Uint128::zero()]
                    [fees.index],
            },
        );

        // Test partial withdrawal and that it claims rewards
        // revenue_amount_remaining        split 75:25 account:liquid.
        // revenue_amount_remaining * 0.75 split 2:1 between staker 1 and 2 => revenue_amount_remaining * 75/100 * 2/3
        app.send_tokens(
            apps.clone(),
            contract.clone(),
            &coins(revenue_amount, "uusdc"),
        )
        .unwrap();

        let res = app
            .execute_contract(
                stakers.account_1.clone(),
                contract.clone(),
                &ExecuteMsg::Account(AccountMsg::Withdraw {
                    amount: Some(Uint128::from(50u128)),
                }),
                &[],
            )
            .unwrap();

        let expected_auto_claim = [50u128, 46u128, 44u128][fees.index];
        let expected_amount_attribute_value_2 = format!("50uruji,{}uusdc", expected_auto_claim);
        res.assert_event(
            &Event::new("transfer")
                .add_attribute("amount", expected_amount_attribute_value_2) // WITHOUT_FEE: 50uruji,50uusdc, WITH_FEE: 50uruji,46uusdc
                .add_attribute("recipient", stakers.account_1.clone()),
        );

        /*--- Liquid Unbound 500 ----------------------
              | Acc_1 | Acc_2 | Accounts | Liquid | Total
        Stake |   950 |   500 |    1_450 |    525 | 1_975
        Rev%  |   ~48 |   ~25 |      ~73 |    ~27 |   100
        */
        assert_state(
            &app,
            &contract,
            "After stakers.account_1 withdraw of 50",
            &StatusResponse {
                account_bond: Uint128::from(1450u128),
                assigned_revenue: [
                    Uint128::from(25u128),
                    Uint128::from(25u128),
                    Uint128::from(23u128),
                ][fees.index],
                liquid_bond_shares: Uint128::from(500u128),
                liquid_bond_size: Uint128::from(525u128),
                undistributed_revenue: [Uint128::one(), Uint128::one(), Uint128::one()][fees.index],
            },
        );

        assert_account(
            &app,
            &contract,
            &AccountResponse {
                addr: stakers.account_1.to_string(),
                bonded: Uint128::from(950u128),
                pending_revenue: Uint128::zero(),
            },
        );

        assert_account(
            &app,
            &contract,
            &AccountResponse {
                addr: stakers.account_2.to_string(),
                bonded: Uint128::from(500u128),
                pending_revenue: expected_pending_revenue_500_2000, // WITHOUT_FEE: 25u128, WITH_FEE: 23u128
            },
        );
    }

    #[test]
    fn swap_sends_back_funds() {
        let (mut app, apps, owner, stakers, fees) = init_app(None);
        let swap_contract = create_swap_contract(&mut app, &owner);
        let converter_message = to_json_binary(&MsgSwap {
            min_return: coin(10, "uruji"),
            return_funds: true,
        })
        .unwrap();
        let contract = create_staking_contract(
            &mut app,
            &owner,
            fees.value,
            swap_contract,
            converter_message,
        );

        // Liquid bonds its first 1_000
        bond_liquid(&mut app, &contract, &stakers.liquid, 1_000, "uruji");

        // share balance
        assert_balance(
            &app,
            &stakers.liquid,
            "x/staking-uruji",
            Uint128::from(1_000u128),
        );

        assert_state(
            &app,
            &contract,
            "After stakers.liquid first bond of 1_000",
            &StatusResponse {
                account_bond: Uint128::zero(),
                assigned_revenue: Uint128::zero(),
                liquid_bond_shares: Uint128::from(1_000u128),
                liquid_bond_size: Uint128::from(1_000u128),
                undistributed_revenue: Uint128::zero(),
            },
        );

        // contract balance
        assert_balance(&app, &contract, "uruji", Uint128::from(1_000u128));

        let revenue_amount = 100;

        // Revenue collection 1
        app.send_tokens(
            apps.clone(),
            contract.clone(),
            &coins(revenue_amount, "uusdc"),
        )
        .unwrap();

        assert_pending_swap(&app, &contract, Uint128::from(0u128));

        // Liquid bonds 2_000 more
        bond_liquid(&mut app, &contract, &stakers.liquid, 2_000, "uruji");

        assert_state(
            &app,
            &contract,
            "After stakers.liquid bonds 2_000 more",
            &StatusResponse {
                account_bond: Uint128::zero(),
                assigned_revenue: Uint128::zero(),
                liquid_bond_shares: Uint128::from(3_000u128),
                liquid_bond_size: Uint128::from(3_000u128),
                // NOTE: We set `return_funds: true` in the swap message, the 20 uusdc were sent back
                //       but they must not be accounted as undistributed revenue
                undistributed_revenue: Uint128::zero(),
            },
        );

        // contract balance
        assert_balance(&app, &contract, "uruji", Uint128::from(3_010u128));
        // NOTE: As we set `return_funds: true` in the swap message, the 20 uusdc were sent back
        //       and the uusdc balance will never decrease
        assert_balance(&app, &contract, "uusdc", Uint128::from(revenue_amount));

        assert_pending_swap(&app, &contract, Uint128::from(100u128));

        // stakers.account_1 bonds its first 500
        bond_account(&mut app, &contract, &stakers.account_1, 500, "uruji");

        assert_pending_swap(&app, &contract, Uint128::from(100u128));

        assert_state(
            &app,
            &contract,
            "stakers.account_1 bonds its first 500",
            &StatusResponse {
                account_bond: Uint128::from(500u128),
                assigned_revenue: Uint128::zero(),
                liquid_bond_shares: Uint128::from(3_000u128),
                liquid_bond_size: Uint128::from(3_010u128),
                // NOTE: We set `return_funds: true` in the swap message, the 20 uusdc were sent back
                //       but they must not be accounted as undistributed revenue
                undistributed_revenue: Uint128::zero(),
            },
        );

        assert_account(
            &app,
            &contract,
            &AccountResponse {
                addr: stakers.account_1.to_string(),
                bonded: Uint128::from(500u128),
                pending_revenue: Uint128::zero(),
            },
        );

        // Revenue collection 1
        app.send_tokens(
            apps.clone(),
            contract.clone(),
            &coins(revenue_amount, "uusdc"),
        )
        .unwrap();

        assert_balance(&app, &contract, "uusdc", Uint128::from(200u128));

        // stakers.account_1 bonds 1500 more
        bond_account(&mut app, &contract, &stakers.account_1, 1500, "uruji");

        assert_pending_swap(&app, &contract, Uint128::from(185u128));

        assert_state(
            &app,
            &contract,
            "stakers.account_1 bonds its first 500",
            &StatusResponse {
                account_bond: Uint128::from(2000u128),
                assigned_revenue: Uint128::zero(),
                liquid_bond_shares: Uint128::from(3_000u128),
                liquid_bond_size: Uint128::from(3_020u128),
                // NOTE: We set `return_funds: true` in the swap message, the 20 uusdc were sent back
                //       but they must not be accounted as undistributed revenue
                undistributed_revenue: Uint128::from(1u128),
            },
        );

        assert_account(
            &app,
            &contract,
            &AccountResponse {
                addr: stakers.account_1.to_string(),
                bonded: Uint128::from(2000u128),
                pending_revenue: Uint128::zero(),
            },
        );
    }

    #[test]
    fn swap_with_error() {
        let (mut app, apps, owner, stakers, fees) = init_app(None);
        let swap_contract = create_swap_contract(&mut app, &owner);
        let converter_message = to_json_binary("not a valid message").unwrap();
        let contract = create_staking_contract(
            &mut app,
            &owner,
            fees.value,
            swap_contract,
            converter_message,
        );

        // Liquid bonds its first 1_000
        bond_liquid(&mut app, &contract, &stakers.liquid, 1_000, "uruji");

        // share balance
        assert_balance(
            &app,
            &stakers.liquid,
            "x/staking-uruji",
            Uint128::from(1_000u128),
        );

        assert_state(
            &app,
            &contract,
            "After stakers.liquid first bond of 1_000",
            &StatusResponse {
                account_bond: Uint128::zero(),
                assigned_revenue: Uint128::zero(),
                liquid_bond_shares: Uint128::from(1_000u128),
                liquid_bond_size: Uint128::from(1_000u128),
                undistributed_revenue: Uint128::zero(),
            },
        );

        // contract balance
        assert_balance(&app, &contract, "uruji", Uint128::from(1_000u128));

        let revenue_amount = 100;

        // Revenue collection 1
        app.send_tokens(
            apps.clone(),
            contract.clone(),
            &coins(revenue_amount, "uusdc"),
        )
        .unwrap();

        assert_pending_swap(&app, &contract, Uint128::from(0u128));

        // Liquid bonds 2_000 more
        bond_liquid(&mut app, &contract, &stakers.liquid, 2_000, "uruji");

        assert_state(
            &app,
            &contract,
            "After stakers.liquid bonds 2_000 more",
            &StatusResponse {
                account_bond: Uint128::zero(),
                assigned_revenue: Uint128::zero(),
                liquid_bond_shares: Uint128::from(3_000u128),
                liquid_bond_size: Uint128::from(3_000u128),
                // NOTE: We set `return_funds: true` in the swap message, the 20 uusdc were sent back
                //       but they must not be accounted as undistributed revenue
                undistributed_revenue: Uint128::zero(),
            },
        );

        // contract balance
        assert_balance(&app, &contract, "uruji", Uint128::from(3_000u128));
        // NOTE: As we set `return_funds: true` in the swap message, the 20 uusdc were sent back
        //       and the uusdc balance will never decrease
        assert_balance(&app, &contract, "uusdc", Uint128::from(revenue_amount));

        assert_pending_swap(&app, &contract, Uint128::from(100u128));

        // stakers.account_1 bonds its first 500
        bond_account(&mut app, &contract, &stakers.account_1, 500, "uruji");

        assert_pending_swap(&app, &contract, Uint128::from(100u128));

        assert_state(
            &app,
            &contract,
            "stakers.account_1 bonds its first 500",
            &StatusResponse {
                account_bond: Uint128::from(500u128),
                assigned_revenue: Uint128::zero(),
                liquid_bond_shares: Uint128::from(3_000u128),
                liquid_bond_size: Uint128::from(3_000u128),
                // NOTE: We set `return_funds: true` in the swap message, the 20 uusdc were sent back
                //       but they must not be accounted as undistributed revenue
                undistributed_revenue: Uint128::zero(),
            },
        );

        assert_account(
            &app,
            &contract,
            &AccountResponse {
                addr: stakers.account_1.to_string(),
                bonded: Uint128::from(500u128),
                pending_revenue: Uint128::zero(),
            },
        );

        // Revenue collection 1
        app.send_tokens(
            apps.clone(),
            contract.clone(),
            &coins(revenue_amount, "uusdc"),
        )
        .unwrap();

        assert_balance(&app, &contract, "uusdc", Uint128::from(200u128));

        // stakers.account_1 bonds 1500 more
        bond_account(&mut app, &contract, &stakers.account_1, 1500, "uruji");

        assert_pending_swap(&app, &contract, Uint128::from(185u128));

        assert_state(
            &app,
            &contract,
            "stakers.account_1 bonds its first 500",
            &StatusResponse {
                account_bond: Uint128::from(2000u128),
                assigned_revenue: Uint128::zero(),
                liquid_bond_shares: Uint128::from(3_000u128),
                liquid_bond_size: Uint128::from(3_000u128),
                // NOTE: We set `return_funds: true` in the swap message, the 20 uusdc were sent back
                //       but they must not be accounted as undistributed revenue
                undistributed_revenue: Uint128::from(1u128),
            },
        );

        assert_account(
            &app,
            &contract,
            &AccountResponse {
                addr: stakers.account_1.to_string(),
                bonded: Uint128::from(2000u128),
                pending_revenue: Uint128::zero(),
            },
        );
    }
}
