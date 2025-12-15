use cosmwasm_std::Decimal;

// Provided as a Trait so that tests can mock an oracle price source
pub trait Premiumable {
    fn adjust(&self, bps: &i16) -> Decimal;
}

impl Premiumable for Decimal {
    fn adjust(&self, bps: &i16) -> Decimal {
        match bps {
            i16::MIN..=-1 => self * Decimal::from_ratio(10000u16 - bps.unsigned_abs(), 10000u16),
            0 => *self,
            1..=i16::MAX => self * Decimal::from_ratio(bps.unsigned_abs() + 10000u16, 10000u16),
        }
    }
}

impl<T: Premiumable> Premiumable for Option<T> {
    fn adjust(&self, bps: &i16) -> Decimal {
        match self {
            Some(x) => x.adjust(bps),
            None => panic!("No price to adjust for Premium"),
        }
    }
}
