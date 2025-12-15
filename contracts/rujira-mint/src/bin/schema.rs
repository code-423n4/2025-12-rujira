use cosmwasm_schema::write_api;

use rujira_rs::mint;

fn main() {
    write_api! {
        instantiate: mint::InstantiateMsg,
    }
}
