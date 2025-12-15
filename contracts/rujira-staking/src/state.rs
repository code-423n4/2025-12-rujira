use cosmwasm_std::{
    Addr, Decimal, Deps, Env, QuerierWrapper, StdError, StdResult, Storage, Uint128,
};
use cw_storage_plus::{Item, Map};
use rujira_rs::{
    staking::{AccountResponse, StatusResponse},
    AccountPool, AccountPoolAccount, SharePool,
};
use std::{cmp::min, ops::Add, ops::Sub};

use crate::{config::Config, ContractError};

static POOL_LIQUID: Item<SharePool> = Item::new("l");
static ACCOUNTS: Map<Addr, AccountPoolAccount> = Map::new("a");
static POOL_ACCOUNTS: Item<AccountPool> = Item::new("p");
static PENDING_SWAP: Item<Uint128> = Item::new("s");

pub fn init(storage: &mut dyn Storage) -> StdResult<()> {
    POOL_LIQUID.save(storage, &Default::default())?;
    POOL_ACCOUNTS.save(storage, &Default::default())?;
    PENDING_SWAP.save(storage, &Default::default())?;
    Ok(())
}

pub fn execute_account_bond(
    storage: &mut dyn Storage,
    owner: &Addr,
    amount: Uint128,
) -> Result<Uint128, ContractError> {
    let mut pool = POOL_ACCOUNTS.load(storage)?;

    match ACCOUNTS.load(storage, owner.clone()) {
        Ok(mut account) => {
            let rewards = pool.claim(&mut account);
            let account = pool.increase_account(&account, amount);
            ACCOUNTS.save(storage, owner.clone(), &account)?;
            POOL_ACCOUNTS.save(storage, &pool)?;

            Ok(rewards)
        }
        Err(StdError::NotFound { .. }) => {
            let account = pool.join(amount);
            ACCOUNTS.save(storage, owner.clone(), &account)?;
            POOL_ACCOUNTS.save(storage, &pool)?;

            Ok(Uint128::default())
        }
        Err(err) => Err(ContractError::Std(err)),
    }
}

pub fn execute_account_claim(storage: &mut dyn Storage, owner: &Addr) -> StdResult<Uint128> {
    let mut pool = POOL_ACCOUNTS.load(storage)?;
    let mut account = ACCOUNTS.load(storage, owner.clone())?;
    let rewards = pool.claim(&mut account);
    ACCOUNTS.save(storage, owner.clone(), &account)?;
    POOL_ACCOUNTS.save(storage, &pool)?;
    Ok(rewards)
}

pub fn execute_account_withdraw(
    storage: &mut dyn Storage,
    owner: &Addr,
    amount: Option<Uint128>,
) -> Result<(Uint128, Uint128), ContractError> {
    let mut pool = POOL_ACCOUNTS.load(storage)?;
    let mut account = ACCOUNTS.load(storage, owner.clone())?;
    let rewards = pool.claim(&mut account);
    let amount = amount.unwrap_or(account.amount);
    let account = pool.decrease_account(&account, amount)?;
    ACCOUNTS.save(storage, owner.clone(), &account)?;
    POOL_ACCOUNTS.save(storage, &pool)?;
    Ok((rewards, amount))
}

pub fn execute_liquid_bond(
    storage: &mut dyn Storage,
    amount: Uint128,
) -> Result<Uint128, ContractError> {
    // Add Bond token to the Compounding pool, mint and return the Compound Share Token
    let mut pool = POOL_LIQUID.load(storage)?;
    let shares = pool.join(amount)?;
    POOL_LIQUID.save(storage, &pool)?;
    Ok(shares)
}

pub fn execute_liquid_unbond(
    storage: &mut dyn Storage,
    shares: Uint128,
) -> Result<Uint128, ContractError> {
    let mut pool = POOL_LIQUID.load(storage)?;
    let returned = pool.leave(shares)?;
    POOL_LIQUID.save(storage, &pool)?;
    Ok(returned)
}

/// Calculates the amount to be distributed between (account, liquid) pools;
/// Revenue balance is queried and surplus (ie not allocated to Account stakers) is split pro-rata between ACCOUNT stakers, and LIQUID pool size.
/// Revenue allocated to LIQUID is transformed to a Wasm Execute msg to swap to the bond token
/// The surplus of bond_balance - LIQUID.size() - ACCOUNT.total is the return value of the previous swap and can be allocated to the total liquid pool
pub fn distribute(
    env: &Env,
    querier: QuerierWrapper,
    storage: &mut dyn Storage,
    config: &Config,
    bond_amount_sent: &Uint128,
) -> Result<(Uint128, Uint128), ContractError> {
    let mut account = POOL_ACCOUNTS.load(storage)?;
    let mut liquid = POOL_LIQUID.load(storage)?;
    let swap_pending = PENDING_SWAP.load(storage)?;

    let bond_balance = querier
        .query_balance(env.contract.address.clone(), config.bond_denom.clone())?
        .amount;

    let revenue_balance = querier
        .query_balance(env.contract.address.clone(), config.revenue_denom.clone())?
        .amount;

    let revenue_surplus_with_fees = revenue_balance
        .checked_sub(account.pending)?
        .checked_sub(swap_pending)?;

    let fee_amount = match &config.fee {
        None => Uint128::zero(),
        Some(fee) => (Decimal::from_atomics(revenue_surplus_with_fees, 0).unwrap()
            * fee.percentage)
            .to_uint_ceil(),
    };
    let revenue_surplus = revenue_surplus_with_fees - fee_amount;

    let account_allocation = if account.total.is_zero() {
        Uint128::zero()
    } else {
        Decimal::from_ratio(
            account.total * revenue_surplus,
            account.total.add(liquid.size()),
        )
        .to_uint_floor()
    };

    let liquid_allocation = if liquid.size().is_zero() {
        Uint128::zero()
    } else {
        Decimal::from_ratio(
            liquid.size() * revenue_surplus,
            account.total.add(liquid.size()),
        )
        .to_uint_floor()
    };

    account.distribute(account_allocation);
    POOL_ACCOUNTS.save(storage, &account)?;

    let bond_surplus = bond_balance
        // Discount any bond tokens sent in the tx, so they're not incorrectly allocated to the Share pool size as swap returned funds
        .checked_sub(*bond_amount_sent)?
        .checked_sub(liquid.size())?
        .checked_sub(account.total)?;

    liquid.deposit(bond_surplus)?;
    POOL_LIQUID.save(storage, &liquid)?;

    // Take pending swaps off the queue, add back any remaining
    let swap_total = swap_pending.add(liquid_allocation);
    let swap_amount = min(config.revenue_converter.2, swap_total);
    let swap_remainder = swap_total.sub(swap_amount);
    PENDING_SWAP.save(storage, &swap_remainder)?;

    Ok((swap_amount, fee_amount))
}

pub fn increase_pending_swap(storage: &mut dyn Storage, amount: Uint128) -> StdResult<()> {
    let swap_pending = PENDING_SWAP.load(storage)?;
    PENDING_SWAP.save(storage, &(swap_pending + amount))
}

pub fn status(env: Env, deps: Deps, config: &Config) -> StdResult<StatusResponse> {
    let liquid = POOL_LIQUID.load(deps.storage)?;
    let account = POOL_ACCOUNTS.load(deps.storage)?;
    let swap_pending = PENDING_SWAP.load(deps.storage)?;

    let revenue_balance = deps
        .querier
        .query_balance(env.contract.address.clone(), config.revenue_denom.clone())?
        .amount;

    let revenue_surplus = revenue_balance
        .checked_sub(account.pending)?
        .checked_sub(swap_pending)?;

    Ok(StatusResponse {
        account_bond: account.total,
        assigned_revenue: account.pending,
        liquid_bond_shares: liquid.shares(),
        liquid_bond_size: liquid.size(),
        undistributed_revenue: revenue_surplus,
    })
}

pub fn account(storage: &dyn Storage, addr: Addr) -> StdResult<AccountResponse> {
    let accounts = POOL_ACCOUNTS.load(storage)?;
    let account = ACCOUNTS.load(storage, addr.clone())?;

    Ok(AccountResponse {
        addr: addr.to_string(),
        bonded: account.amount,
        pending_revenue: accounts.pending_revenue(&account),
    })
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{
        coin, coins,
        testing::{mock_dependencies_with_balances, mock_env},
        Binary,
    };
    use cw_multi_test::BasicApp;

    use super::*;

    #[test]
    fn test_distribution() {
        let app = BasicApp::default();
        let env = mock_env();

        let mut deps = mock_dependencies_with_balances(&[
            (
                app.api().addr_make("app").as_str(),
                &coins(1_000_000u128, "uusdc"),
            ),
            (
                env.contract.address.as_str(),
                &[
                    coin(1_000u128, "uusdc"),
                    // Two operations below bond total of 3000 ruji
                    // More complex testing executed in contract.rs with cw-multi-test
                    coin(3_000u128, "uruji"),
                ],
            ),
        ]);

        let config = Config {
            bond_denom: "uruji".to_string(),
            revenue_denom: "uusdc".to_string(),
            revenue_converter: (
                app.api().addr_make("revenue"),
                Binary::new(vec![0]),
                Uint128::from(100u128),
            ),
            fee: None,
        };

        init(deps.as_mut().storage).unwrap();

        assert_eq!(
            POOL_LIQUID.load(deps.as_mut().storage).unwrap(),
            SharePool::default()
        );

        assert_eq!(
            POOL_ACCOUNTS.load(deps.as_mut().storage).unwrap(),
            AccountPool::default()
        );
        let mutdeps = deps.as_mut();

        execute_account_bond(
            mutdeps.storage,
            &app.api().addr_make("account"),
            Uint128::from(750u128),
        )
        .unwrap();

        execute_account_bond(
            mutdeps.storage,
            &app.api().addr_make("account2"),
            Uint128::from(250u128),
        )
        .unwrap();

        execute_liquid_bond(mutdeps.storage, Uint128::from(2_000u128)).unwrap();

        assert_eq!(
            POOL_LIQUID.load(mutdeps.storage).unwrap().shares(),
            Uint128::from(2_000u128)
        );

        assert_eq!(
            POOL_LIQUID.load(mutdeps.storage).unwrap().size(),
            Uint128::from(2_000u128)
        );

        assert_eq!(
            POOL_ACCOUNTS.load(mutdeps.storage).unwrap().total,
            Uint128::from(1_000u128)
        );

        let (swap_amount, _fee_amount) = distribute(
            &env,
            mutdeps.querier,
            mutdeps.storage,
            &config,
            &Uint128::zero(),
        )
        .unwrap();
        // Balance of 1000 USDC split across 3000 RUJI - 2000 liquid and 1000 account. so 666 to be swapped, 333 to be allocated

        assert_eq!(swap_amount, Uint128::from(100u128));

        assert_eq!(
            PENDING_SWAP.load(mutdeps.storage).unwrap(),
            Uint128::from(566u128)
        );

        assert_eq!(
            POOL_LIQUID.load(mutdeps.storage).unwrap().shares(),
            Uint128::from(2_000u128)
        );

        assert_eq!(
            POOL_LIQUID.load(mutdeps.storage).unwrap().size(),
            Uint128::from(2_000u128)
        );

        assert_eq!(
            POOL_ACCOUNTS.load(mutdeps.storage).unwrap().pending,
            Uint128::from(333u128)
        );

        assert_eq!(
            account(mutdeps.storage, app.api().addr_make("account")).unwrap(),
            AccountResponse {
                addr: app.api().addr_make("account").to_string(),
                bonded: Uint128::from(750u128),
                pending_revenue: Uint128::from(249u128)
            }
        );
    }
}
