use cosmwasm_schema::write_api;

use rujira_rs::fin;

fn main() {
    write_api! {
        instantiate: fin::InstantiateMsg,
        execute: fin::ExecuteMsg,
        query: fin::QueryMsg,
        sudo: fin::SudoMsg,
    }
}
