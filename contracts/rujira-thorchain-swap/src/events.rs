use cosmwasm_std::{Coin, Event};

pub fn event_swap(
    to: &String,
    amount: &Coin,
    min_return: &Coin,
    fee: &Coin,
    returned: &Coin,
    memo: &String,
) -> Event {
    Event::new(format!("{}/swap", env!("CARGO_PKG_NAME")))
        .add_attribute("to", to)
        .add_attribute("amount", amount.to_string())
        .add_attribute("min_return", min_return.to_string())
        .add_attribute("fee", fee.to_string())
        .add_attribute("returned", returned.to_string())
        .add_attribute("memo", memo)
}
