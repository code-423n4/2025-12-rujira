use cosmwasm_std::{Addr, CanonicalAddr, CosmosMsg};

use crate::coin::Coin;
use crate::memoed::{Memo, Memoed};

use super::deposit::MsgDeposit;

#[derive(Clone)]
pub struct MsgSecuredAssetWithdraw {
    to: Addr,
    amount: Coin,
    signer: CanonicalAddr,
}

impl MsgSecuredAssetWithdraw {
    pub fn new(amount: Coin, to: Addr, signer: CanonicalAddr) -> Self {
        Self { amount, to, signer }
    }
}

impl Memoed for MsgSecuredAssetWithdraw {
    fn to_memo(&self) -> String {
        Memo::default().push(&"secure-").push(&self.to).to_string()
    }
}

impl From<MsgSecuredAssetWithdraw> for CosmosMsg {
    fn from(value: MsgSecuredAssetWithdraw) -> Self {
        MsgDeposit::new(vec![value.amount.clone()], value.to_memo(), value.signer).into()
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{
        testing::{mock_dependencies, MockApi},
        Api, Uint256,
    };

    use crate::SecuredAsset;

    use super::*;

    #[test]
    fn encoding() {
        let signer = mock_dependencies()
            .api
            .addr_canonicalize("cosmwasm1uae0t6xzae6nrnj6f3vvh40zgy9sah3cfazs4s")
            .unwrap();

        let msg = MsgSecuredAssetWithdraw::new(
            Coin::new(SecuredAsset::new("THOR", "RUJI"), Uint256::from(100u128)),
            MockApi::default().addr_make("recipient"),
            signer,
        );
        assert_eq!(
            msg.to_memo(),
            "secure-:cosmwasm1vewsdxxmeraett7ztsaym88jsrv85kzm0xvjg09xqz8aqvjcja0syapxq9"
        );
    }
}
