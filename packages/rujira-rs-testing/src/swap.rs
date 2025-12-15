use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult,
};
use cw_multi_test::ContractWrapper;

#[cw_serde]
pub struct MsgSwap {
    pub min_return: Coin,
    pub return_funds: bool,
}

fn instantiate(_deps: DepsMut, _env: Env, _info: MessageInfo, _msg: ()) -> StdResult<Response> {
    Ok(Response::default())
}

fn execute(_deps: DepsMut, _env: Env, info: MessageInfo, msg: MsgSwap) -> StdResult<Response> {
    let mut funds_to_send = vec![msg.min_return];
    if msg.return_funds {
        funds_to_send.extend(info.funds);
    }
    Ok(
        Response::default().add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: funds_to_send,
        })),
    )
}

fn query(_deps: Deps, _env: Env, _msg: ()) -> StdResult<Binary> {
    Ok(Binary::new(vec![0]))
}

pub fn mock_swap_contract() -> Box<ContractWrapper<MsgSwap, (), (), StdError, StdError, StdError>> {
    Box::new(ContractWrapper::new(execute, instantiate, query))
}
