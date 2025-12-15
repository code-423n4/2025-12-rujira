use crate::asset::Asset;
use anybuf::Anybuf;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint256;

#[cw_serde]
pub struct Coin {
    asset: Asset,
    amount: Uint256,
}

impl Coin {
    pub fn new<T: Into<Asset>>(asset: T, amount: Uint256) -> Self {
        Self {
            asset: asset.into(),
            amount,
        }
    }
}

impl From<Coin> for Anybuf {
    fn from(value: Coin) -> Self {
        Anybuf::new()
            .append_message(1, &Anybuf::from(value.asset))
            .append_string(2, value.amount.to_string())
    }
}

impl From<&Coin> for Anybuf {
    fn from(value: &Coin) -> Self {
        value.clone().into()
    }
}

#[cfg(test)]
mod tests {
    use crate::asset::Layer1Asset;

    use super::*;

    #[test]
    fn encoding() {
        let buf: Anybuf =
            Coin::new(Layer1Asset::new("THOR", "uruji"), Uint256::from(100u128)).into();
        assert_eq!(
            buf.into_vec(),
            vec![
                10, 20, 10, 4, 84, 72, 79, 82, 18, 5, 85, 82, 85, 74, 73, 26, 5, 85, 82, 85, 74,
                73, 18, 3, 49, 48, 48
            ]
        );
    }
}
