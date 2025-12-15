use cosmwasm_schema::cw_serde;
use cosmwasm_std::{ensure, Decimal, StdError, StdResult};
use std::{
    cmp::min,
    ops::{Add, Div, Mul, Sub},
};

#[cw_serde]
pub struct Interest {
    pub target_utilization: Decimal,
    // The base rate charged when utilization = 0
    pub base_rate: Decimal,
    // The additional rate at the target_utilization, added on top of base rate
    pub step1: Decimal,
    // The additional rate at the full utilization, added on top of base rate and step1
    pub step2: Decimal,
}

impl Default for Interest {
    fn default() -> Self {
        Self {
            target_utilization: Decimal::from_ratio(4u128, 5u128),
            base_rate: Decimal::zero(),
            step1: Decimal::one(),
            step2: Decimal::from_ratio(2u128, 1u128),
        }
    }
}

impl Interest {
    pub fn validate(&self) -> StdResult<()> {
        ensure!(
            self.step2.gt(&self.step1),
            StdError::generic_err("step2 must be > step1".to_string())
        );

        ensure!(
            !self.target_utilization.is_zero(),
            StdError::generic_err("target_utilization must be > 0".to_string())
        );

        ensure!(
            self.target_utilization.lt(&Decimal::one()),
            StdError::generic_err("target_utilization must be < 1".to_string())
        );
        Ok(())
    }

    pub fn rate(&self, utilization: Decimal) -> StdResult<Decimal> {
        ensure!(
            utilization.le(&Decimal::one()),
            StdError::generic_err("utilization must be <= 1".to_string())
        );

        // step1 as a percentage of the 0-target range
        let part1 = min(utilization, self.target_utilization)
            .div(self.target_utilization)
            .mul(self.step1);

        let part2 = utilization
            .sub(min(utilization, self.target_utilization))
            .checked_div(Decimal::one().sub(self.target_utilization))
            .unwrap_or_default()
            .mul(self.step2);

        Ok(self.base_rate.add(part1).add(part2))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn interest_curves() {
        let i = Interest {
            target_utilization: Decimal::one(),
            base_rate: Decimal::zero(),
            step1: Decimal::zero(),
            step2: Decimal::zero(),
        };

        assert_eq!(i.rate(Decimal::zero()).unwrap(), Decimal::zero());
        assert_eq!(i.rate(Decimal::one()).unwrap(), Decimal::zero());

        let i = Interest {
            target_utilization: Decimal::one(),
            base_rate: Decimal::from_ratio(1u128, 10u128),
            step1: Decimal::zero(),
            step2: Decimal::zero(),
        };

        assert_eq!(
            i.rate(Decimal::zero()).unwrap(),
            Decimal::from_ratio(1u128, 10u128)
        );
        assert_eq!(
            i.rate(Decimal::one()).unwrap(),
            Decimal::from_ratio(1u128, 10u128)
        );

        let i = Interest {
            target_utilization: Decimal::one(),
            base_rate: Decimal::from_ratio(1u128, 10u128),
            step1: Decimal::zero(),
            step2: Decimal::zero(),
        };

        assert_eq!(
            i.rate(Decimal::zero()).unwrap(),
            Decimal::from_ratio(1u128, 10u128)
        );
        assert_eq!(
            i.rate(Decimal::one()).unwrap(),
            Decimal::from_ratio(1u128, 10u128)
        );

        let i = Interest {
            target_utilization: Decimal::from_ratio(8u128, 10u128),
            base_rate: Decimal::from_ratio(1u128, 10u128),
            step1: Decimal::from_ratio(1u128, 10u128),
            step2: Decimal::from_ratio(3u128, 1u128),
        };

        assert_eq!(
            i.rate(Decimal::zero()).unwrap(),
            Decimal::from_ratio(1u128, 10u128)
        );
        // 10% in, we should be at 10% base rate + 12.5% of the way in from 1 to 10,
        assert_eq!(
            i.rate(Decimal::from_ratio(1u128, 10u128)).unwrap(),
            Decimal::from_ratio(1125u128, 10000u128)
        );
        // At target, 10% base plus 10% step1
        assert_eq!(
            i.rate(Decimal::from_ratio(8u128, 10u128)).unwrap(),
            Decimal::from_ratio(20u128, 100u128)
        );

        // At target, 10% base plus 10% step1 and half of step2
        assert_eq!(
            i.rate(Decimal::from_ratio(9u128, 10u128)).unwrap(),
            Decimal::from_ratio(170u128, 100u128)
        );
        // At fill
        assert_eq!(
            i.rate(Decimal::one()).unwrap(),
            Decimal::from_ratio(320u128, 100u128)
        );
    }
}
