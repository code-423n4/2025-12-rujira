use cosmwasm_std::{Addr, CanonicalAddr, CosmosMsg, Uint256};

use crate::asset::Asset;
use crate::coin::Coin;
use crate::memoed::{Memo, Memoed};

use super::deposit::MsgDeposit;

#[derive(Clone)]
pub struct MsgSwap {
    from: Coin,
    to: Asset,
    destination: Destination,
    slip: Option<Slip>,
    affiliate: Option<Affiliate>,
    dex: Option<Dex>,
    signer: CanonicalAddr,
}

impl MsgSwap {
    pub fn new<T: Into<Asset>>(
        from: Coin,
        to: T,
        destination: Destination,
        slip: Option<Slip>,
        affiliate: Option<Affiliate>,
        dex: Option<Dex>,
        signer: CanonicalAddr,
    ) -> Self {
        Self {
            from,
            to: to.into(),
            destination,
            slip,
            affiliate,
            dex,
            signer,
        }
    }
}

impl Memoed for MsgSwap {
    fn to_memo(&self) -> String {
        Memo::default()
            .push(&"=")
            .push(&self.to)
            .push(&self.destination)
            .push(&self.slip)
            .push(&self.affiliate)
            .push(&self.dex)
            .to_string()
    }
}

impl From<MsgSwap> for CosmosMsg {
    fn from(value: MsgSwap) -> Self {
        MsgDeposit::new(vec![value.from.clone()], value.to_memo(), value.signer).into()
    }
}

#[derive(Clone)]
pub enum Slip {
    Limit(Uint256),
    Stream {
        limit: Uint256,
        interval: u8,
        quantity: Uint256,
    },
}

#[derive(Clone)]
pub enum Destination {
    Direct(Addr),
    Refundable {
        destination_addr: Addr,
        refund_addr: Addr,
    },
}

#[derive(Clone)]
pub struct Affiliate {
    pub addr: Addr,
    pub basis_points: i8,
}

impl Affiliate {
    pub fn new(addr: Addr, basis_points: i8) -> Self {
        Self { addr, basis_points }
    }
}

#[derive(Clone)]
pub struct Dex {
    aggregator: String,
    target_address: Addr,
    limit: Option<Uint256>,
}

impl Memoed for Destination {
    fn to_memo(&self) -> String {
        match self {
            Destination::Direct(addr) => addr.to_string(),
            Destination::Refundable {
                destination_addr,
                refund_addr,
            } => format!("{}/{}", destination_addr, refund_addr),
        }
    }
}

impl Memoed for Slip {
    fn to_memo(&self) -> String {
        match self {
            Slip::Limit(lim) => lim.to_string(),
            Slip::Stream {
                limit,
                interval,
                quantity,
            } => format!("{}/{}/{}", limit, interval, quantity),
        }
    }
}

impl Memoed for Option<Slip> {
    fn to_memo(&self) -> String {
        match self {
            Some(x) => x.to_memo(),
            None => "".to_string(),
        }
    }
}

impl Memoed for Affiliate {
    fn to_memo(&self) -> String {
        format!("{}:{}", self.addr, self.basis_points)
    }
}

impl Memoed for Option<Affiliate> {
    fn to_memo(&self) -> String {
        match self {
            Some(x) => x.to_memo(),
            None => ":".to_string(),
        }
    }
}

impl Memoed for Dex {
    fn to_memo(&self) -> String {
        format!(
            "{}:{}:{}",
            self.aggregator,
            self.target_address,
            match self.limit {
                Some(x) => x.to_string(),
                None => "".to_string(),
            }
        )
    }
}

impl Memoed for Option<Dex> {
    fn to_memo(&self) -> String {
        match self {
            Some(x) => x.to_memo(),
            None => "".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::mock_dependencies, Api, Uint256};

    use crate::asset::SecuredAsset;

    use super::*;

    #[test]
    fn encoding() {
        let signer = mock_dependencies()
            .api
            .addr_canonicalize("cosmwasm1uae0t6xzae6nrnj6f3vvh40zgy9sah3cfazs4s")
            .unwrap();

        let msg = MsgSwap::new(
            Coin::new(SecuredAsset::new("BTC", "BTC"), Uint256::from(100u128)),
            SecuredAsset::new("ETH", "ETH"),
            Destination::Refundable {
                destination_addr: Addr::unchecked("recipient"),
                refund_addr: Addr::unchecked("refund"),
            },
            Some(Slip::Stream {
                limit: Uint256::from(200u128),
                interval: 5,
                quantity: Uint256::from(50u128),
            }),
            Some(Affiliate::new(Addr::unchecked("affiliate"), 5)),
            Some(Dex {
                aggregator: "dexagg".to_string(),
                target_address: Addr::unchecked("target"),
                limit: Some(Uint256::from(500u128)),
            }),
            signer.clone(),
        );
        assert_eq!(
            msg.to_memo(),
            "=:ETH-ETH:recipient/refund:200/5/50:affiliate:5:dexagg:target:500"
        );

        let msg = MsgSwap::new(
            Coin::new(SecuredAsset::new("BTC", "BTC"), Uint256::from(100u128)),
            SecuredAsset::new("ETH", "ETH"),
            Destination::Direct(Addr::unchecked("recipient")),
            Some(Slip::Limit(Uint256::from(200u128))),
            Some(Affiliate::new(Addr::unchecked("affiliate"), 5)),
            Some(Dex {
                aggregator: "dexagg".to_string(),
                target_address: Addr::unchecked("target"),
                limit: None,
            }),
            signer.clone(),
        );
        assert_eq!(
            msg.to_memo(),
            "=:ETH-ETH:recipient:200:affiliate:5:dexagg:target"
        );

        let msg = MsgSwap::new(
            Coin::new(SecuredAsset::new("BTC", "BTC"), Uint256::from(100u128)),
            SecuredAsset::new("ETH", "ETH"),
            Destination::Direct(Addr::unchecked("recipient")),
            None,
            None,
            Some(Dex {
                aggregator: "dexagg".to_string(),
                target_address: Addr::unchecked("target"),
                limit: None,
            }),
            signer.clone(),
        );
        assert_eq!(msg.to_memo(), "=:ETH-ETH:recipient::::dexagg:target");

        let msg = MsgSwap::new(
            Coin::new(SecuredAsset::new("BTC", "BTC"), Uint256::from(100u128)),
            SecuredAsset::new("ETH", "ETH"),
            Destination::Direct(Addr::unchecked("recipient")),
            None,
            None,
            None,
            signer.clone(),
        );
        assert_eq!(msg.to_memo(), "=:ETH-ETH:recipient");
    }
}
