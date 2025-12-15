use anybuf::Anybuf;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    instantiate2_address, to_json_binary, Addr, AnyMsg, BankMsg, Binary, Coin, CosmosMsg, Deps,
    Instantiate2AddressError, StdError, StdResult, WasmMsg,
};
use thiserror::Error;

#[cw_serde]
pub struct Account {
    addr: Addr,
    admin: Addr,
}

impl Account {
    pub fn contract(&self) -> Addr {
        self.addr.clone()
    }

    pub fn load(deps: Deps, addr: &Addr) -> StdResult<Self> {
        Ok(Self {
            admin: deps.querier.query_wasm_contract_info(addr)?.admin.unwrap(),
            addr: addr.clone(),
        })
    }

    pub fn create(
        deps: Deps,
        admin: Addr,
        code_id: u64,
        label: String,
        salt: Binary,
    ) -> Result<(Self, WasmMsg), AccountError> {
        let checksum = deps.querier.query_wasm_code_info(code_id)?.checksum;
        let contract = instantiate2_address(
            checksum.as_slice(),
            &deps.api.addr_canonicalize(admin.as_str())?,
            &salt,
        )?;
        Ok((
            Self {
                addr: deps.api.addr_humanize(&contract)?,
                admin: admin.clone(),
            },
            WasmMsg::Instantiate2 {
                admin: Some(admin.to_string()),
                code_id,
                label: format!("rujira-account/{label}"),
                msg: to_json_binary(&())?,
                funds: vec![],
                salt,
            },
        ))
    }

    pub fn sudo(&self, msg: &CosmosMsg) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Any(AnyMsg {
            type_url: "/cosmwasm.wasm.v1.MsgSudoContract".to_string(),
            value: Anybuf::new()
                .append_string(1, &self.admin)
                .append_string(2, &self.addr)
                .append_bytes(3, to_json_binary(msg)?)
                .into_vec()
                .into(),
        }))
    }

    pub fn send(&self, to_address: impl Into<String>, amount: Vec<Coin>) -> StdResult<CosmosMsg> {
        self.sudo(&CosmosMsg::Bank(BankMsg::Send {
            to_address: to_address.into(),
            amount,
        }))
    }

    pub fn execute(
        &self,
        contract_addr: String,
        msg: Binary,
        funds: Vec<Coin>,
    ) -> StdResult<CosmosMsg> {
        self.sudo(&CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            funds,
        }))
    }
}

#[derive(Error, Debug)]
pub enum AccountError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Instantiate2Address(#[from] Instantiate2AddressError),
}
