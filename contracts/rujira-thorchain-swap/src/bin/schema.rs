use cosmwasm_schema::write_api;

use rujira_rs::thorchain_swap;

fn main() {
    write_api! {
        instantiate: thorchain_swap::InstantiateMsg,
        execute: thorchain_swap::ExecuteMsg,
        query: thorchain_swap::QueryMsg,
        sudo: thorchain_swap::SudoMsg,
    }
}
