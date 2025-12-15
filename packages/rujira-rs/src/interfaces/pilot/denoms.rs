use cosmwasm_schema::cw_serde;

#[cw_serde]
pub struct Denoms([String; 2]);

impl Denoms {
    pub fn new(base: &str, quote: &str) -> Self {
        Self([base.to_string(), quote.to_string()])
    }

    pub fn ask(&self) -> &str {
        self.0[0].as_str()
    }

    pub fn bid(&self) -> &str {
        self.0[1].as_str()
    }
}
