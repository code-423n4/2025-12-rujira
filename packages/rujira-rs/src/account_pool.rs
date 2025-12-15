use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, OverflowError, Uint128};
use std::ops::Mul;

/// An account-based revenue distribution system
#[cw_serde]
#[derive(Default)]
pub struct AccountPool {
    pub pending: Uint128,
    pub total: Uint128,
    sum: Decimal,
}

/// Basic account-based revenue distribution system
impl AccountPool {
    pub fn join(&mut self, amount: Uint128) -> AccountPoolAccount {
        self.total += amount;
        AccountPoolAccount {
            amount,
            sum_snapshot: self.sum,
        }
    }

    /// Increase stake in the AccountPool
    ///
    /// N.B Ensure that `pending_revenue` is claimed BEFORE calling `increase account`, as this will reset allocations
    pub fn increase_account(
        &mut self,
        account: &AccountPoolAccount,
        amount: Uint128,
    ) -> AccountPoolAccount {
        self.total += amount;
        AccountPoolAccount {
            amount: account.amount + amount,
            sum_snapshot: self.sum,
        }
    }

    /// Decrease stake in the AccountPool
    ///
    /// N.B Ensure that `pending_revenue` is claimed BEFORE calling `decrease account`, as this will reset allocations
    pub fn decrease_account(
        &mut self,
        account: &AccountPoolAccount,
        amount: Uint128,
    ) -> Result<AccountPoolAccount, OverflowError> {
        self.total = self.total.checked_sub(amount)?;
        Ok(AccountPoolAccount {
            amount: account.amount.checked_sub(amount)?,
            sum_snapshot: self.sum,
        })
    }

    pub fn distribute(&mut self, amount: Uint128) {
        if amount.is_zero() {
            return;
        }

        self.pending += amount;
        self.sum += Decimal::from_ratio(amount, self.total);
    }

    pub fn pending_revenue(&self, account: &AccountPoolAccount) -> Uint128 {
        (self.sum - account.sum_snapshot)
            .mul(Decimal::from_ratio(account.amount, 1u128))
            .to_uint_floor()
    }

    pub fn claim(&mut self, account: &mut AccountPoolAccount) -> Uint128 {
        let amount = self.pending_revenue(account);
        self.pending -= amount;
        account.sum_snapshot = self.sum;
        amount
    }
}

#[cw_serde]
pub struct AccountPoolAccount {
    pub amount: Uint128,
    pub sum_snapshot: Decimal,
}

#[cfg(test)]
mod tests {

    use cosmwasm_std::assert_approx_eq;

    use super::*;

    #[test]
    fn test_lifecycle() {
        let mut pool = AccountPool::default();
        let account = pool.join(Uint128::zero());

        assert_eq!(pool.pending, Uint128::zero());
        assert_eq!(pool.total, Uint128::zero());
        assert_eq!(pool.pending_revenue(&account), Uint128::zero());
        assert_eq!(account.amount, Uint128::zero());

        // Single join and claim
        let mut account = pool.join(Uint128::from(1000u128));
        assert_eq!(pool.pending, Uint128::zero());
        assert_eq!(pool.total, Uint128::from(1000u128));
        assert_eq!(pool.pending_revenue(&account), Uint128::zero());
        assert_eq!(account.amount, Uint128::from(1000u128));

        pool.distribute(Uint128::from(500u128));
        assert_eq!(pool.pending, Uint128::from(500u128));
        assert_eq!(pool.total, Uint128::from(1000u128));
        assert_eq!(pool.pending_revenue(&account), Uint128::from(500u128));
        let claimed = pool.claim(&mut account);
        assert_eq!(claimed, Uint128::from(500u128));
        assert_eq!(pool.pending_revenue(&account), Uint128::zero());

        let mut account1 = pool.join(Uint128::from(1000u128));
        let account2 = pool.join(Uint128::from(2000u128));
        assert_eq!(pool.pending, Uint128::zero());
        assert_eq!(pool.total, Uint128::from(4000u128));

        pool.distribute(Uint128::from(10_000u128));
        assert_eq!(pool.pending, Uint128::from(10_000u128));
        assert_eq!(pool.total, Uint128::from(4000u128));

        assert_eq!(pool.pending_revenue(&account), Uint128::from(2500u128));
        assert_eq!(pool.pending_revenue(&account1), Uint128::from(2500u128));
        assert_eq!(pool.pending_revenue(&account2), Uint128::from(5000u128));

        let claimed = pool.claim(&mut account1);
        let account1 = pool
            .decrease_account(&account1, Uint128::from(500u128))
            .unwrap();
        assert_eq!(claimed, Uint128::from(2500u128));
        assert_eq!(pool.pending_revenue(&account), Uint128::from(2500u128));
        assert_eq!(pool.pending_revenue(&account1), Uint128::zero());
        assert_eq!(pool.pending_revenue(&account2), Uint128::from(5000u128));

        pool.decrease_account(&account2, Uint128::from(5001u128))
            .unwrap_err();
    }

    #[test]
    fn test_long_run() {
        let mut pool = AccountPool::default();
        // Simulate a passive majority
        let account = pool.join(Uint128::from(1_000_000_000u128));
        let mut account2 = pool.join(Uint128::from(10_000u128));

        for n in 1..100_000 {
            pool.distribute(Uint128::from(3_000_000u128));
            assert_approx_eq!(
                pool.pending_revenue(&account),
                Uint128::from(2999970u128 * n),
                "0.0000000001"
            );
            assert_eq!(pool.pending_revenue(&account2), Uint128::from(29u128));
            let claimed = pool.claim(&mut account2);
            assert_eq!(claimed, Uint128::from(29u128));
        }
    }
}
