use std::fmt::{Display, Formatter, Result};

use cosmwasm_std::Addr;

#[derive(Default)]
pub struct Memo {
    parts: Vec<String>,
}

impl Memo {
    pub fn push<T: Memoed>(&self, part: &T) -> Self {
        let mut parts = self.parts.clone();
        parts.push(part.to_memo());
        Self { parts }
    }
}

impl Display for Memo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.parts.join(":").trim_end_matches(':').fmt(f)
    }
}

pub trait Memoed {
    fn to_memo(&self) -> String;
}

impl Memoed for Addr {
    fn to_memo(&self) -> String {
        self.to_string()
    }
}

impl Memoed for String {
    fn to_memo(&self) -> String {
        self.to_string()
    }
}

impl Memoed for &str {
    fn to_memo(&self) -> String {
        self.to_string()
    }
}

impl<T> Memoed for Vec<T>
where
    T: Memoed,
{
    fn to_memo(&self) -> String {
        self.iter()
            .map(|x| x.to_memo())
            .collect::<Vec<String>>()
            .join(":")
    }
}
