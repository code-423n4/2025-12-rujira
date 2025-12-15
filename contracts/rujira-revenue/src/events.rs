use cosmwasm_std::Event;

pub fn event_run(denom: String) -> Event {
    Event::new(format!("{}/run", env!("CARGO_PKG_NAME"))).add_attribute("denom", denom)
}
