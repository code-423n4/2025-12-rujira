use std::{collections::HashSet, str::FromStr};

use cosmwasm_std::{Coins, DepsMut, Env, MessageInfo, StdError, StdResult, SubMsgResponse};

pub fn sub_msg_response_to_info(
    response: &SubMsgResponse,
    deps: &DepsMut,
    env: &Env,
) -> StdResult<MessageInfo> {
    let mut total_funds: Coins = Coins::default();
    let mut senders: HashSet<String> = HashSet::new();

    for event in &response.events {
        if event.ty != "transfer" {
            continue;
        }

        let mut recipient: Option<String> = None;
        let mut amount_str: Option<String> = None;
        let mut sender: Option<String> = None;

        for attr in &event.attributes {
            match attr.key.as_str() {
                "recipient" => recipient = Some(attr.value.clone()),
                "amount" => amount_str = Some(attr.value.clone()),
                "sender" => sender = Some(attr.value.clone()),
                _ => {}
            }
        }

        // We only need transfers to this contract
        if recipient.as_deref() != Some(env.contract.address.as_str()) {
            continue;
        }

        if let Some(s) = sender {
            senders.insert(s);
        }

        if let Some(a) = amount_str {
            let coins = Coins::from_str(&a)?;
            let merged = [total_funds.into_vec(), coins.into_vec()].concat();
            total_funds = Coins::try_from(merged)?;
        }
    }

    if senders.len() > 1 {
        return Err(StdError::generic_err(
            "Multiple senders found in reply transfer events",
        ));
    }

    let sender = senders
        .into_iter()
        .next()
        .unwrap_or_else(|| "unknown".to_string());
    let sender_addr = deps.api.addr_validate(&sender)?;
    Ok(MessageInfo {
        sender: sender_addr,
        funds: total_funds.into_vec(),
    })
}
