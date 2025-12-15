# Rujira Staking

Generic staking contract for revenue distribution with inbuilt liquid staking.

Depositors have the option of

1. Staking in an account for yield paid in `revenue_denom`, typically $USDC, periodically manually claiming
1. Minting a liquid token, representing a share of a pool, which claims `revenue_denom` and swaps to `bond_denom`, increasing the size of the pool over time
