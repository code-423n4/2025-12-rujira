use cosmwasm_std::Decimal;

pub trait Premium {
    fn to_rate(&self, oracle: &Decimal) -> Decimal;
}

impl Premium for u8 {
    fn to_rate(&self, oracle: &Decimal) -> Decimal {
        oracle * Decimal::from_ratio(100 - self, 100u16)
    }
}
