use anybuf::Anybuf;
use cosmwasm_std::{AnyMsg, CanonicalAddr, CosmosMsg};

use crate::coin::Coin;

pub struct MsgDeposit {
    memo: String,
    coins: Vec<Coin>,
    signer: CanonicalAddr,
}

impl MsgDeposit {
    pub fn new(coins: Vec<Coin>, memo: String, signer: CanonicalAddr) -> Self {
        Self {
            memo,
            coins,
            signer,
        }
    }
}

impl From<MsgDeposit> for CosmosMsg {
    fn from(value: MsgDeposit) -> Self {
        let coins: Vec<Anybuf> = value.coins.iter().map(Anybuf::from).collect();
        let value = Anybuf::new()
            .append_repeated_message(1, &coins)
            .append_string(2, value.memo)
            .append_bytes(3, value.signer.to_vec());

        CosmosMsg::Any(AnyMsg {
            type_url: "/types.MsgDeposit".to_string(),
            value: value.as_bytes().into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::mock_dependencies, Api, Binary, Uint256};

    use crate::asset::Layer1Asset;

    use super::*;

    #[test]
    fn encoding() {
        let deps = mock_dependencies();
        let signer = deps
            .api
            .addr_canonicalize("cosmwasm1uae0t6xzae6nrnj6f3vvh40zgy9sah3cfazs4s")
            .unwrap();
        let msg: CosmosMsg = MsgDeposit::new(
            vec![Coin::new(
                Layer1Asset::new("THOR", "RUJI"),
                Uint256::from(1_200_000_000u128),
            )],
            "foobarbaz".to_string(),
            signer,
        )
        .into();
        assert_eq!(
            msg,
            CosmosMsg::Any(AnyMsg {
                type_url: "/types.MsgDeposit".to_string(),
                value: Binary::from_base64("CiAKEgoEVEhPUhIEUlVKSRoEUlVKSRIKMTIwMDAwMDAwMBIJZm9vYmFyYmF6GhTncvXowu51Mc5aTFjL1eJBCw7eOA==").unwrap(),
            })
        )
    }
}
