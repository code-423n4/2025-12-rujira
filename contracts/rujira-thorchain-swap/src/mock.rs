use cosmwasm_std::{coin, Addr, Decimal};
use cw_multi_test::{ContractWrapper, Executor};

use rujira_ghost_vault::mock::GhostVault;
use rujira_rs::thorchain_swap::{ExecuteMsg, InstantiateMsg, SudoMsg};
use rujira_rs_testing::RujiraApp;

/// Wrapper struct for Thorchain Swap contract with convenience methods
#[derive(Debug, Clone)]
pub struct ThorchainSwap(pub Addr);

impl ThorchainSwap {
    /// Get the contract address
    pub fn addr(&self) -> &Addr {
        &self.0
    }

    /// Execute a swap
    pub fn swap(
        &self,
        app: &mut RujiraApp,
        sender: &Addr,
        min_return: cosmwasm_std::Coin,
        offer_amount: u128,
        offer_denom: &str,
        to: Option<String>,
    ) -> anyhow::Result<cw_multi_test::AppResponse> {
        app.execute_contract(
            sender.clone(),
            self.0.clone(),
            &ExecuteMsg::Swap {
                min_return,
                to,
                callback: None,
            },
            &[coin(offer_amount, offer_denom)],
        )
    }

    /// Execute repay
    pub fn repay(
        &self,
        app: &mut RujiraApp,
        sender: &Addr,
    ) -> anyhow::Result<cw_multi_test::AppResponse> {
        app.execute_contract(sender.clone(), self.0.clone(), &ExecuteMsg::Repay {}, &[])
    }

    /// Set market via sudo
    pub fn set_market(
        &self,
        app: &mut RujiraApp,
        addr: &str,
        enabled: bool,
    ) -> anyhow::Result<cw_multi_test::AppResponse> {
        app.wasm_sudo(
            self.0.clone(),
            &SudoMsg::SetMarket {
                addr: addr.to_string(),
                enabled,
            },
        )
    }

    /// Set vault via sudo
    pub fn set_vault(
        &self,
        app: &mut RujiraApp,
        denom: &str,
        vault: Option<&GhostVault>,
    ) -> anyhow::Result<cw_multi_test::AppResponse> {
        app.wasm_sudo(
            self.0.clone(),
            &SudoMsg::SetVault {
                denom: denom.to_string(),
                vault: vault.map(|v| v.addr().clone().into()),
            },
        )
    }

    /// Query config
    pub fn query_config(
        &self,
        app: &RujiraApp,
    ) -> anyhow::Result<rujira_rs::thorchain_swap::ConfigResponse> {
        Ok(app.wrap().query_wasm_smart(
            self.0.clone(),
            &rujira_rs::thorchain_swap::QueryMsg::Config {},
        )?)
    }

    /// Query all vaults
    pub fn query_vaults(
        &self,
        app: &RujiraApp,
    ) -> anyhow::Result<rujira_rs::thorchain_swap::VaultsResponse> {
        Ok(app.wrap().query_wasm_smart(
            self.0.clone(),
            &rujira_rs::thorchain_swap::QueryMsg::Vaults {},
        )?)
    }

    /// Helper method to get a specific vault by denom
    pub fn get_vault_for_denom(
        &self,
        app: &RujiraApp,
        denom: &str,
    ) -> anyhow::Result<Option<Addr>> {
        let vaults_response = self.query_vaults(app)?;
        Ok(vaults_response
            .vaults
            .iter()
            .find(|v| v.denom == denom)
            .map(|v| v.vault.addr()))
    }

    /// Setup Thorchain Swap contract with ghost vaults
    pub fn create(app: &mut RujiraApp, owner: &Addr) -> Self {
        let code = Box::new(
            ContractWrapper::new(
                crate::contract::execute,
                crate::contract::instantiate,
                crate::contract::query,
            )
            .with_sudo(crate::contract::sudo),
        );
        let code_id = app.store_code(code);

        let contract_addr = app
            .instantiate_contract(
                code_id,
                owner.clone(),
                &InstantiateMsg {
                    max_stream_length: 1u32,
                    max_borrow_ratio: Decimal::one(),
                    reserve_fee: Decimal::from_ratio(1u128, 5000u128),
                    stream_step_ratio: Decimal::one(),
                },
                &[],
                "thorchain-swap",
                Some(owner.to_string()),
            )
            .unwrap();

        Self(contract_addr)
    }
}

#[cfg(test)]
mod tests {
    use rujira_rs_testing::mock_rujira_app;

    use super::*;

    #[test]
    fn test_thorchain_swap_setup() {
        let mut app = mock_rujira_app();
        let owner = app.api().addr_make("owner");

        let contract = ThorchainSwap::create(&mut app, &owner);

        assert!(!contract.addr().as_str().is_empty());
    }
}
