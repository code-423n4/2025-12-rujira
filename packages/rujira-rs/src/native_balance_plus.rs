use cw_utils::NativeBalance;
/// Extension for NativeBalance to offer more functionality
pub trait NativeBalancePlus {
    /// Returns the decrease in coins between self and new
    fn sent(&self, new: &Self) -> Self;
    /// Returns the increase in coins between self and new
    fn received(&self, new: &Self) -> Self;
}

impl NativeBalancePlus for NativeBalance {
    fn sent(&self, new: &Self) -> Self {
        let mut spent = self.clone();
        for coin in new.clone().into_vec() {
            // Swallow the error with a NOOP if we have received a new token,
            // which will try and subtract the new from the original balance where it doesn't exist
            spent = spent.clone().sub_saturating(coin).unwrap_or(spent.clone())
        }
        spent
    }

    fn received(&self, new: &Self) -> Self {
        let mut received = new.clone();
        for coin in self.clone().into_vec() {
            // Swallow the error with a NOOP if we have spent all of a token,
            // which will try and subtract the original from the new balance where it doesn't exist
            received = received
                .clone()
                .sub_saturating(coin)
                .unwrap_or(received.clone())
        }
        received
    }
}

#[cfg(test)]
mod test {
    use cosmwasm_std::coin;

    use super::*;

    #[test]
    fn test_native_balance_plus() {
        assert_eq!(
            NativeBalance(vec![coin(10000000, "btc-btc"), coin(200000000, "eth-eth")]).sent(
                &NativeBalance(vec![
                    coin(9000000, "btc-btc"),
                    coin(200000000, "eth-eth"),
                    coin(
                        109664431951,
                        "eth-usdc-0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                    ),
                ])
            ),
            NativeBalance(vec![coin(1000000, "btc-btc")])
        );
        assert_eq!(
            NativeBalance(vec![coin(10000000, "btc-btc"), coin(200000000, "eth-eth")]).received(
                &NativeBalance(vec![
                    coin(9000000, "btc-btc"),
                    coin(200000000, "eth-eth"),
                    coin(
                        109664431951,
                        "eth-usdc-0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                    ),
                ])
            ),
            NativeBalance(vec![coin(
                109664431951,
                "eth-usdc-0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            )])
        );
    }
}
