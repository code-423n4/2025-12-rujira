use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, Decimal256, Uint128};
use std::ops::{AddAssign, Div, SubAssign};
use thiserror::Error;

/// Simple share-based pool implementation where your membership entitles you to a share of the pool
#[cw_serde]
#[derive(Default)]
pub struct SharePool {
    size: Uint128,
    shares: Decimal,
}

impl SharePool {
    /// Adds a deposit to the pool, returning the new shares issued
    pub fn join(&mut self, amount: Uint128) -> Result<Uint128, SharePoolError> {
        if amount.is_zero() {
            return Err(SharePoolError::Zero("Amount".to_string()));
        }
        if self.shares.is_zero() {
            self.size.add_assign(amount);
            self.shares.add_assign(Decimal::from_ratio(amount, 1u128));
            return Ok(amount);
        }

        let issuance = self.shares * Decimal::from_ratio(amount, self.size);
        if issuance.floor().is_zero() {
            return Err(SharePoolError::Zero("Shares".to_string()));
        }

        self.shares.add_assign(issuance.floor());
        self.size.add_assign(amount);
        Ok(issuance.to_uint_floor())
    }

    /// Removes a share from the pool, returning the amount removed from the pool and deducted
    pub fn leave(&mut self, amount: Uint128) -> Result<Uint128, SharePoolError> {
        if amount.is_zero() {
            return Err(SharePoolError::Zero("Amount".to_string()));
        }

        if amount.gt(&self.shares()) {
            return Err(SharePoolError::ShareOverflow {});
        }

        if amount.eq(&self.shares()) {
            let claim = self.size;
            self.size = Uint128::zero();
            self.shares = Decimal::zero();
            return Ok(claim);
        }

        let claim: Uint128 = self.ownership(amount);
        self.size.sub_assign(claim);
        self.shares.sub_assign(Decimal::from_ratio(amount, 1u128));
        Ok(claim)
    }

    pub fn ratio(&self) -> Decimal {
        if self.shares.is_zero() {
            return Decimal::zero();
        }
        Decimal::from_ratio(self.size, 1u128).div(self.shares)
    }

    pub fn size(&self) -> Uint128 {
        self.size
    }

    pub fn shares(&self) -> Uint128 {
        self.shares.to_uint_floor()
    }

    pub fn ownership(&self, shares: Uint128) -> Uint128 {
        if shares.is_zero() {
            return Uint128::zero();
        }
        self.size.multiply_ratio(shares, self.shares())
    }

    pub fn deposit(&mut self, amount: Uint128) -> Result<(), SharePoolError> {
        let mut checked = self.clone();
        checked.size.add_assign(amount);
        let deposit = Uint128::from(1000u128);
        let test = checked.join(deposit)?;
        let value = checked.ownership(test);
        let ratio = Decimal256::from_ratio(value, deposit);

        // If a deposit causes a new `join` of 1000 units to lose more than 1% of its value,
        // the share pool is in an invalid state, error and don't change state
        if ratio < Decimal256::from_ratio(99u128, 100u128) {
            return Err(SharePoolError::InvalidDeposit {});
        }
        self.size.add_assign(amount);
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum SharePoolError {
    #[error("ShareOverflow")]
    ShareOverflow {},
    #[error("Zero{0}")]
    Zero(String),
    #[error("InvalidDeposit")]
    InvalidDeposit {},
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::ops::Mul;

    #[test]
    fn lifecycle() {
        let mut pool = SharePool::default();
        pool.leave(Uint128::one()).unwrap_err();
        pool.join(Uint128::zero()).unwrap_err();
        let shares = pool.join(Uint128::from(1000u128)).unwrap();
        assert_eq!(shares, Uint128::from(1000u128));
        assert_eq!(pool.shares, Decimal::from_ratio(1000u128, 1u128));
        assert_eq!(pool.size, Uint128::from(1000u128));

        assert_eq!(
            pool.ownership(Uint128::from(1000u128)),
            Uint128::from(1000u128)
        );

        let shares = pool.join(Uint128::from(5000u128)).unwrap();
        assert_eq!(shares, Uint128::from(5000u128));
        assert_eq!(pool.shares, Decimal::from_ratio(6000u128, 1u128));
        assert_eq!(pool.size, Uint128::from(6000u128));

        pool.deposit(Uint128::from(2000u128)).unwrap();

        assert_eq!(pool.shares, Decimal::from_ratio(6000u128, 1u128));
        assert_eq!(pool.size, Uint128::from(8000u128));

        assert_eq!(
            pool.ownership(Uint128::from(1000u128)),
            Uint128::from(1333u128)
        );

        let shares = pool.join(Uint128::from(1000u128)).unwrap();
        assert_eq!(shares, Uint128::from(750u128));
        assert_eq!(pool.shares, Decimal::from_ratio(6750u128, 1u128));
        assert_eq!(pool.size, Uint128::from(9000u128));
        assert_eq!(pool.ownership(shares), Uint128::from(1000u128));

        let redeem = pool.leave(Uint128::from(500u128)).unwrap();
        assert_eq!(redeem, Uint128::from(666u128));
        assert_eq!(pool.shares, Decimal::from_ratio(6250u128, 1u128));
        assert_eq!(pool.size, Uint128::from(8334u128));
        assert_eq!(pool.ownership(shares), Uint128::from(1000u128));

        pool.leave(Uint128::from(6251u128)).unwrap_err();
        let redeem = pool.leave(Uint128::from(6250u128)).unwrap();
        assert_eq!(redeem, Uint128::from(8334u128));
        assert_eq!(pool.shares, Decimal::zero());
        assert_eq!(pool.size, Uint128::zero());
    }

    #[test]
    fn overflow() {
        let mut pool = SharePool::default();
        pool.deposit(Uint128::from(1_000_000_000_000_000_000_000_000u128))
            .unwrap();
        pool.join(Uint128::from(100_000_000_000_000_000u128))
            .unwrap();
        pool.ownership(Uint128::from(100_000_000_000_000_000u128));
    }

    #[test]
    fn inflation_protection() {
        // Ensure that the SharePool can't be manipulated as per https://docs.openzeppelin.com/contracts/4.x/erc4626
        let mut pool = SharePool::default();
        pool.join(Uint128::one()).unwrap();
        pool.deposit(Uint128::from(111u128)).unwrap_err();
        let shares = pool.join(Uint128::from(1000u128)).unwrap();
        assert_eq!(shares, Uint128::from(1000u128));
        pool.deposit(Uint128::from(110u128)).unwrap();
        let shares = pool.join(Uint128::from(1000u128)).unwrap();
        assert_eq!(shares, Uint128::from(900u128));
        let value = pool.ownership(shares);
        assert_eq!(value, Uint128::from(999u128));
    }

    #[test]
    fn mint_zero_share_error() {
        // Ensure that the SharePool can't be manipulated creating zero issuance but increases the shares by fractions
        let mut pool = SharePool::default();

        pool.join(Uint128::from(100u128)).unwrap();
        assert_eq!(pool.shares, Decimal::from_ratio(100u128, 1u128));

        // deposit to increase the ratio
        pool.deposit(Uint128::from(100u128)).unwrap();

        // assert ratio is 2
        assert_eq!(pool.ratio(), Decimal::from_ratio(2u128, 1u128));

        for _ in 0..100_000 {
            // for each cycle join so that the minted share is 1.5 => 1.5 * pool.ratio()
            let amount = pool.ratio().mul(Decimal::from_ratio(3u128, 2u128));
            let shares = pool.join(amount.to_uint_floor()).unwrap();
            // shares should always be 1
            assert_eq!(shares, Uint128::from(1u128));
        }
        //  total pool shares should be 100 + 100_000
        assert_eq!(pool.shares, Decimal::from_ratio(100_100u128, 1u128));
    }

    #[test]
    fn replicate_inflation_attack() {
        // Setup: honest initial depositor
        let mut pool = SharePool::default();
        // honest deposits 100 shares/size
        pool.join(Uint128::from(100u128)).unwrap();
        // allow a small donation via deposit so we get into the state used in the earlier test
        pool.deposit(Uint128::from(111u128)).unwrap();
        // At this point:
        // shares = 100, size = 211
        assert_eq!(pool.shares, Decimal::from_ratio(100u128, 1u128));
        assert_eq!(pool.size, Uint128::from(211u128));

        // Attacker: repeatedly call join(1) which (in current code) mints 0 shares but increases `size`.
        // We'll do 100_000 such calls to inflate `size` beyond 100 * 1000 = 100_000,
        // so that a later join(1000) mints zero shares (because issuance = shares * amount / size < 1).
        // EDIT: After the fix the join call will fail
        for _ in 0..100_000 {
            pool.join(Uint128::one()).unwrap_err();
        }

        // Sanity checks on state after the attack:
        assert_eq!(pool.shares(), Uint128::from(100u128)); // shares unchanged

        // size should be > 100_000 (initially 211 + 100_000)
        // EDIT: After the fix the size should be unchanged 211
        assert_eq!(pool.size(), Uint128::from(211u128));

        // Now a new honest depositor tries to join 1000 units.
        // With inflated size, issuance will floor to zero in current implementation.
        let issuance = pool.join(Uint128::from(1000u128)).unwrap();
        // EDIT: After the fix the join call will return 473
        assert_eq!(issuance, Uint128::from(473u128));

        // The honest depositor's 1000 units were added to pool.size(), but they received no shares.
        // EDIT: After the fix the shares should be 573 and size should be 1211
        assert_eq!(pool.shares(), Uint128::from(573u128));
        assert!(pool.size().u128() == 1211u128);
    }
}
