# Rujira Revenue Converter

Simple smart-contract to collect reward tokens and convert them into a smaller number of assets to be distributed to $RUJI and $RUNE stakers.

One contract instance is deployed per revenue token.

Each instance has a set of `Action`s that are stepped through on subsequent executions of `ExecuteMsg::Run`.
This is designed to keep execution of the contract in fixed time, and also support more complex routing of token swaps.
At the end of each execution, `target_denoms` balances are read and is distributed to the `target_addresses` address according to weights.
