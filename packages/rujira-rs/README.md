# rujira-rs

CosmWasm interfaces for building Smart Contracts on the THORChain Blockchain

## Getting Started

Install `rujira-rs` into your smart contract project.

`cargo install rujira-rs`

### Call on-chain actions

#### Swap a Bridge Asset to another Bridge Asset using the native L1 pools

```rust
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdResult};
use cw_utils::must_pay;
use rujira::{BridgeAsset, Coin, Chain, SwapMsg};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: (),
) -> StdResult<Response> {
    let from_asset = BridgeAsset::new(Chain::BTC, "BTC");
    let from_amount = must_pay(info, from_asset.denom_str());
    let to = BridgeAsset::new(Chain::ETH, "ETH");
    let from = Coin::new(from_asset, from_amount);
    let swap_msg = SwapMsg::new(from, to, info.sender, None, None, None);
    Response::default().add_message(swap_msg)
}
```

#### Withdraw a Bridge Asset to its native chain

```rust
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdResult};
use cw_utils::must_pay;
use rujira::{BridgeAsset, Coin, Chain, BridgeAssetWithdrawMsg};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: (),
) -> StdResult<Response> {
    let asset = BridgeAsset::new(Chain::BTC, "BTC");
    let amount = must_pay(info, asset.denom_str());
    let coin = Coin::new(asset, amount);
    let withdraw_msg = BridgeAssetWithdrawMsg::new(coin, "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq");
    Response::default().add_message(withdraw_msg)
}
```
