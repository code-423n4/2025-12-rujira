use super::{error::BidPoolError, pool::Pool, sum_snapshot::SumSnapshotKey};
use crate::decimal_scaled::DecimalScaled;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint256;

#[cw_serde]
pub struct Bid {
    pub(crate) amount: Uint256,
    pub(crate) filled: Uint256,

    // Product Snapshot
    #[serde(alias = "a")]
    pub(crate) product_snapshot: DecimalScaled,
    // Sum Snapshot
    #[serde(alias = "b")]
    pub(crate) sum_snapshot: DecimalScaled,
    // Epoch Snapshot
    #[serde(alias = "c")]
    pub(crate) epoch_snapshot: u32,
}

impl Bid {
    pub fn sum_snapshot_key(&self) -> SumSnapshotKey {
        SumSnapshotKey::new(self.epoch_snapshot)
    }

    pub fn amount(&self) -> Uint256 {
        self.amount
    }

    pub fn filled(&self) -> Uint256 {
        self.filled
    }

    pub fn claim_filled(&mut self) -> Uint256 {
        let claimed = self.filled;
        self.filled = Uint256::zero();
        claimed
    }

    pub fn retract(
        &mut self,
        pool: &mut Pool,
        amount: Option<Uint256>,
    ) -> Result<Uint256, BidPoolError> {
        let withdraw_amount: Uint256 = assert_withdraw_amount(amount, self.amount)?;
        self.amount -= withdraw_amount;
        pool.total = pool.total.checked_sub(withdraw_amount)?;
        Ok(withdraw_amount)
    }

    pub fn increase(&mut self, pool: &mut Pool, amount: Uint256) -> Result<Uint256, BidPoolError> {
        self.amount += amount;
        pool.total = pool.total.checked_add(amount)?;
        Ok(amount)
    }

    pub fn is_empty(&self) -> bool {
        self.amount.is_zero() && self.filled.is_zero()
    }
}

fn assert_withdraw_amount(
    withdraw_amount: Option<Uint256>,
    withdrawable_amount: Uint256,
) -> Result<Uint256, BidPoolError> {
    let to_withdraw = if let Some(amount) = withdraw_amount {
        if amount > withdrawable_amount {
            return Err(BidPoolError::WithdrawError {
                available: withdrawable_amount,
            });
        }
        amount
    } else {
        withdrawable_amount
    };

    Ok(to_withdraw)
}
