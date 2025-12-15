use cosmwasm_std::{coin, coins, Addr, Decimal, Event, Uint128};

use cw_multi_test::{ContractWrapper, Executor};
use rujira_rs::{
    ghost::{self, vault::Interest},
    thorchain_swap::{ExecuteMsg, InstantiateMsg, SudoMsg},
    TokenMetadata,
};
use rujira_rs_testing::{mock_rujira_app, RujiraApp};

use crate::contract;

#[test]
fn complete_swap() {
    let mut app = mock_rujira_app();
    let owner = app.api().addr_make("owner");
    let contract = setup(&mut app, &owner);
    app.wasm_sudo(
        contract.clone(),
        &SudoMsg::SetMarket {
            addr: owner.to_string(),
            enabled: true,
        },
    )
    .unwrap();
    let res = app
        .execute_contract(
            owner.clone(),
            contract.clone(),
            &ExecuteMsg::Swap {
                min_return: coin(1000000, "rune"),
                to: None,
                callback: None,
            },
            &coins(58, "btc-btc"),
        )
        .unwrap();
    res.assert_event(
        &Event::new("wasm-rujira-thorchain-swap/swap").add_attributes(vec![
            ("to", owner.as_str()),
            ("amount", "58btc-btc"),
            ("min_return", "1000000rune"),
            ("fee", "89635rune"),
            ("returned", "448083799rune"),
            ("memo", "dummy"),
        ]),
    );

    res.assert_event(
        &Event::new("wasm-rujira-ghost-vault/borrow").add_attributes(vec![
            ("borrower", contract.as_str()),
            ("amount", "448083799"),
        ]),
    );

    res.assert_event(&Event::new("transfer").add_attributes(vec![
        ("recipient", owner.as_str()),
        ("sender", contract.as_str()),
        ("amount", "448083799rune"),
    ]));

    // Simulate the endblock execution

    app.send_tokens(owner.clone(), contract.clone(), &coins(448083799, "rune"))
        .unwrap();

    let res = app
        .execute_contract(owner.clone(), contract.clone(), &ExecuteMsg::Repay {}, &[])
        .unwrap();

    res.assert_event(
        &Event::new("wasm-rujira-ghost-vault/repay").add_attributes(vec![
            ("borrower", contract.as_str()),
            ("amount", "448083799"),
        ]),
    );
}

pub fn setup(app: &mut RujiraApp, owner: &Addr) -> Addr {
    app.init_modules(|x, _api, storage| {
        x.bank.init_balance(
            storage,
            owner,
            vec![coin(10000000000, "rune"), coin(10000000000, "btc-btc")],
        )
    })
    .unwrap();

    let code = Box::new(
        ContractWrapper::new(contract::execute, contract::instantiate, contract::query)
            .with_sudo(contract::sudo),
    );
    let code_id = app.store_code(code);
    let contract = app
        .instantiate_contract(
            code_id,
            owner.clone(),
            &InstantiateMsg {
                max_stream_length: 1u32,
                max_borrow_ratio: Decimal::one(),
                reserve_fee: Decimal::from_ratio(1u128, 5000u128),
                stream_step_ratio: Decimal::one(),
            },
            &[],
            "template",
            Some(owner.to_string()),
        )
        .unwrap();

    let vault_code = Box::new(
        ContractWrapper::new(
            rujira_ghost_vault::contract::execute,
            rujira_ghost_vault::contract::instantiate,
            rujira_ghost_vault::contract::query,
        )
        .with_sudo(rujira_ghost_vault::contract::sudo),
    );
    let vault_code_id = app.store_code(vault_code);
    let vault_btc = app
        .instantiate_contract(
            vault_code_id,
            owner.clone(),
            &ghost::vault::InstantiateMsg {
                denom: "btc-btc".to_string(),
                interest: Interest::default(),
                receipt: TokenMetadata {
                    description: "XBTC".to_string(),
                    display: "XBTC".to_string(),
                    name: "XBTC".to_string(),
                    symbol: "XBTC".to_string(),
                    uri: None,
                    uri_hash: None,
                },
                fee: Decimal::zero(),
                fee_address: owner.to_string(),
            },
            &[],
            "ghost btc",
            Some(owner.to_string()),
        )
        .unwrap();
    app.execute_contract(
        owner.clone(),
        vault_btc.clone(),
        &ghost::vault::ExecuteMsg::Deposit { callback: None },
        &coins(1000000000, "btc-btc"),
    )
    .unwrap();

    let vault_rune = app
        .instantiate_contract(
            vault_code_id,
            owner.clone(),
            &ghost::vault::InstantiateMsg {
                denom: "rune".to_string(),
                interest: Interest::default(),
                receipt: TokenMetadata {
                    description: "XRUNE".to_string(),
                    display: "XRUNE".to_string(),
                    name: "XRUNE".to_string(),
                    symbol: "XRUNE".to_string(),
                    uri: None,
                    uri_hash: None,
                },
                fee: Decimal::zero(),
                fee_address: owner.to_string(),
            },
            &[],
            "ghost rune",
            Some(owner.to_string()),
        )
        .unwrap();

    app.execute_contract(
        owner.clone(),
        vault_rune.clone(),
        &ghost::vault::ExecuteMsg::Deposit { callback: None },
        &coins(1000000000, "rune"),
    )
    .unwrap();

    app.wasm_sudo(
        contract.clone(),
        &SudoMsg::SetVault {
            denom: "btc-btc".to_owned(),
            vault: Some(vault_btc.clone().into()),
        },
    )
    .unwrap();

    app.wasm_sudo(
        contract.clone(),
        &SudoMsg::SetVault {
            denom: "rune".to_owned(),
            vault: Some(vault_rune.clone().into()),
        },
    )
    .unwrap();

    app.wasm_sudo(
        vault_btc.clone(),
        &ghost::vault::SudoMsg::SetBorrower {
            contract: contract.to_string(),
            limit: Uint128::MAX,
        },
    )
    .unwrap();

    app.wasm_sudo(
        vault_rune.clone(),
        &ghost::vault::SudoMsg::SetBorrower {
            contract: contract.to_string(),
            limit: Uint128::MAX,
        },
    )
    .unwrap();

    contract
}
