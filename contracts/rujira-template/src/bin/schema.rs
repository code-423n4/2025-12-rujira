use cosmwasm_schema::write_api;

use rujira_rs::template;

fn main() {
    write_api! {
        instantiate: template::InstantiateMsg,
        execute: template::ExecuteMsg,
        query: template::QueryMsg,
        sudo: template::SudoMsg,
    }
}
