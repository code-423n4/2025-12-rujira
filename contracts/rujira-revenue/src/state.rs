use std::cmp::min;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coins, Addr, Api, Binary, Coin, Order, StdError, StdResult, Storage, Uint128, WasmMsg,
};
use cw_storage_plus::{Bound, Item, Map};
use rujira_rs::revenue::{ActionResponse, ConfigResponse, InstantiateMsg};

static CONFIG: Item<Config> = Item::new("config");
static LAST: Item<String> = Item::new("last");
static ACTIONS: Map<String, (Addr, Uint128, Binary)> = Map::new("actions");

#[cw_serde]
pub struct Config {
    /// The address permitted to set Actions
    pub owner: Addr,

    /// The address permitted to execute the crank
    pub executor: Addr,

    /// The denoms that are transferred to the fee_collector at the end of every execution
    target_denoms: Vec<String>,

    /// The final destinations that `target_denom` is sent to (address, weight)
    pub target_addresses: Vec<(Addr, u8)>,
}

impl Config {
    pub fn new(api: &dyn Api, value: InstantiateMsg) -> StdResult<Self> {
        Ok(Self {
            owner: api.addr_validate(&value.owner)?,
            executor: api.addr_validate(&value.executor)?,
            target_denoms: value.target_denoms,
            target_addresses: value
                .target_addresses
                .iter()
                .map(|(a, b)| Ok((api.addr_validate(a)?, *b)))
                .collect::<StdResult<Vec<(Addr, u8)>>>()?,
        })
    }

    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        CONFIG.load(storage)
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG.save(storage, self)
    }

    pub fn target_denoms(&self) -> Vec<String> {
        self.clone().target_denoms
    }

    pub fn add_target_denom(&mut self, denom: String) {
        if self.target_denoms.contains(&denom) {
            return;
        }
        self.target_denoms.push(denom);
    }
}

impl From<Config> for ConfigResponse {
    fn from(value: Config) -> Self {
        Self {
            owner: value.owner.to_string(),
            executor: value.executor.to_string(),
            target_denoms: value.target_denoms(),
            target_addresses: value
                .target_addresses
                .iter()
                .map(|(a, b)| (a.to_string(), *b))
                .collect::<Vec<(String, u8)>>(),
        }
    }
}

#[cw_serde]
pub struct Action {
    /// Token denom
    pub denom: String,
    /// The target contract for swapping
    pub contract: Addr,
    /// The maximum amount of the token that can be included in any one execution of the Action
    pub limit: Uint128,
    /// The msg executed on the contract to swap to the target token
    pub msg: Binary,
}

impl Action {
    pub fn last(storage: &dyn Storage) -> StdResult<Option<String>> {
        LAST.may_load(storage)
    }

    pub fn next(storage: &mut dyn Storage) -> StdResult<Option<Self>> {
        let min = LAST.may_load(storage)?.map(Bound::exclusive);
        match ACTIONS
            .range(storage, min, None, Order::Ascending)
            .take(1)
            .collect::<StdResult<Vec<(String, (Addr, Uint128, Binary))>>>()?
            .first()
        {
            Some(res) => Ok(Some(Self::load(storage, res)?)),
            // If there's nothing next, try the start
            None => {
                if let Some(res) = ACTIONS.first(storage)? {
                    return Ok(Some(Self::load(storage, &res)?));
                }
                Ok(None)
            }
        }
    }

    fn load(
        storage: &mut dyn Storage,
        (denom, (contract, limit, msg)): &(String, (Addr, Uint128, Binary)),
    ) -> StdResult<Self> {
        LAST.save(storage, denom)?;
        Ok(Self {
            denom: denom.clone(),
            contract: contract.clone(),
            limit: *limit,
            msg: msg.clone(),
        })
    }

    pub fn all(storage: &dyn Storage) -> StdResult<Vec<Self>> {
        ACTIONS
            .range(storage, None, None, Order::Ascending)
            .map(|res| match res {
                Ok((denom, (contract, limit, msg))) => Ok(Self {
                    denom: denom.clone(),
                    contract,
                    limit,
                    msg,
                }),
                Err(err) => Err(err),
            })
            .collect()
    }

    pub fn set(
        storage: &mut dyn Storage,
        denom: String,
        contract: Addr,
        limit: Uint128,
        msg: Binary,
    ) -> StdResult<()> {
        ACTIONS.save(storage, denom, &(contract, limit, msg))
    }

    pub fn unset(storage: &mut dyn Storage, denom: String) {
        ACTIONS.remove(storage, denom.to_string())
    }

    pub fn execute(&self, amount: Coin) -> StdResult<Option<WasmMsg>> {
        if amount.denom != self.denom {
            return Err(StdError::generic_err("Invalid Denom"));
        }
        let total = min(amount.amount, self.limit);
        if total.is_zero() {
            return Ok(None);
        }
        Ok(Some(WasmMsg::Execute {
            contract_addr: self.contract.to_string(),
            msg: self.msg.clone(),
            funds: coins(total.u128(), amount.denom),
        }))
    }
}

impl From<Action> for ActionResponse {
    fn from(value: Action) -> Self {
        Self {
            denom: value.denom,
            contract: value.contract.to_string(),
            limit: value.limit,
            msg: value.msg,
        }
    }
}
