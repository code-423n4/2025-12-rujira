#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;
use rujira_rs::{mint::InstantiateMsg, TokenFactory};

use crate::error::ContractError;

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let factory = TokenFactory::new(&env, msg.id.as_str());
    Ok(Response::default()
        .add_message(factory.create_msg(msg.metadata))
        .add_message(factory.mint_msg(msg.amount, info.sender)))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: (),
) -> Result<Response, ContractError> {
    Err(ContractError::Unauthorized {})
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(_deps: DepsMut, _env: Env, _msg: ()) -> Result<Response, ContractError> {
    Err(ContractError::Unauthorized {})
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: ()) -> Result<Binary, ContractError> {
    Err(ContractError::Unauthorized {})
}

#[cfg(test)]
mod tests {

    use super::*;
    use cosmwasm_std::{coins, Uint128};
    use cw_multi_test::{ContractWrapper, Executor};
    use rujira_rs::TokenMetadata;
    use rujira_rs_testing::mock_rujira_app;

    #[test]
    fn instantiation() {
        let mut app = mock_rujira_app();
        let owner = app.api().addr_make("owner");

        let code = Box::new(ContractWrapper::new(execute, instantiate, query));
        let code_id = app.store_code(code);
        app.instantiate_contract(
            code_id,
            owner.clone(),
            &InstantiateMsg {
                id: "id".to_string(),
                metadata: TokenMetadata {
                    description: "description".to_string(),
                    display: "display".to_string(),
                    name: "name".to_string(),
                    symbol: "symbol".to_string(),
                    uri: None,
                    uri_hash: None,
                },
                amount: Uint128::from(100u128),
            },
            &[],
            "mint",
            None,
        )
        .unwrap();

        #[allow(deprecated)]
        let balance = app.wrap().query_all_balances(owner).unwrap();
        assert_eq!(balance, coins(100, "x/id"));
    }
}
