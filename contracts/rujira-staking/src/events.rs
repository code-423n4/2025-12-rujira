use cosmwasm_std::{Addr, Event, Uint128};

pub fn event_account_bond(owner: Addr, amount: Uint128) -> Event {
    Event::new(format!("{}/account.bond", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("amount", amount)
}

pub fn event_account_claim(owner: Addr, reward_amount: Uint128) -> Event {
    Event::new(format!("{}/account.claim", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("amount", reward_amount)
}

pub fn event_account_withdraw(owner: Addr, amount: Uint128, rewards: Uint128) -> Event {
    Event::new(format!("{}/account.withdraw", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("amount", amount)
        .add_attribute("rewards", rewards)
}

pub fn event_liquid_bond(owner: Addr, amount: Uint128, shares: Uint128) -> Event {
    Event::new(format!("{}/liquid.bond", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("amount", amount)
        .add_attribute("shares", shares)
}

pub fn event_liquid_unbond(owner: Addr, shares: Uint128, returned: Uint128) -> Event {
    Event::new(format!("{}/liquid.unbond", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("shares", shares)
        .add_attribute("returned", returned)
}
