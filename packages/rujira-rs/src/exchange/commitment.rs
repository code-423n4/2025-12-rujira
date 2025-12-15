use cosmwasm_std::{coin, coins, to_json_binary, Addr, CosmosMsg, StdResult, Uint128, WasmMsg};
use std::{
    collections::HashMap,
    ops::{Add, AddAssign},
};

use crate::{
    bow,
    fin::{Denoms, Side},
};

/// An aggregate commitment by external market makers to execute the swap
#[derive(Debug, Default)]
pub struct Commitment {
    quotes: HashMap<Addr, (Uint128, Uint128)>,
}

impl Commitment {
    pub fn new(contract: &Addr, quote: (Uint128, Uint128)) -> Self {
        Self {
            quotes: HashMap::from([(contract.clone(), quote)]),
        }
    }
    pub fn to_msgs(&self, denoms: &Denoms, side: &Side) -> StdResult<Vec<CosmosMsg>> {
        let bid_denom = denoms.bid(side);
        let ask_denom = denoms.ask(side);
        self.quotes
            .iter()
            .map(|(addr, (offer, ask))| {
                Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: addr.to_string(),
                    msg: to_json_binary(&bow::ExecuteMsg::Swap {
                        min_return: coin(ask.u128(), bid_denom),
                        to: None,
                        callback: None,
                    })?,
                    funds: coins(offer.u128(), ask_denom),
                }))
            })
            .collect()
    }
}

impl Add for Commitment {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut quotes = self.quotes;
        for (addr, (base, quote)) in rhs.quotes {
            quotes
                .entry(addr)
                .and_modify(|e| {
                    e.0 += base;
                    e.1 += quote;
                })
                .or_insert((base, quote));
        }
        Commitment { quotes }
    }
}

impl AddAssign for Commitment {
    fn add_assign(&mut self, rhs: Self) {
        for (addr, (base, quote)) in rhs.quotes {
            self.quotes
                .entry(addr)
                .and_modify(|e| {
                    e.0 += base;
                    e.1 += quote;
                })
                .or_insert((base, quote));
        }
    }
}
