# Rujira audit details
- Total Prize Pool: $40,000 in USDC
    - HM awards: up to $35,520 in USDC
        - If no valid Highs or Mediums are found, the HM pool is $0
    - QA awards: $1,480 in USDC
    - Judge awards: $3,000 in USDC
- [Read our guidelines for more details](https://docs.code4rena.com/competitions)
- Starts December 15, 2025 20:00 UTC
- Ends January 15, 2026 20:00 UTC

### ❗ Important notes for wardens
1. Since this audit includes live/deployed code, **all submissions will be treated as sensitive**:
    - Wardens are encouraged to submit High-risk submissions affecting live code promptly, to ensure timely disclosure of such vulnerabilities to the sponsor and guarantee payout in the case where a sponsor patches a live critical during the audit.
    - Submissions will be hidden from all wardens (SR and non-SR alike) by default, to ensure that no sensitive issues are erroneously shared.
    - If the submissions include findings affecting live code, there will be no post-judging QA phase. This ensures that awards can be distributed in a timely fashion, without compromising the security of the project. (Senior members of C4 staff will review the judges’ decisions per usual.)
    - By default, submissions will not be made public until the report is published.
    - Exception: if the sponsor indicates that no submissions affect live code, then we’ll make submissions visible to all authenticated wardens, and open PJQA to SR wardens per the usual C4 process.
    - [The "live criticals" exception](https://docs.code4rena.com/awarding#the-live-criticals-exception) therefore applies.
2. Judging phase risk adjustments (upgrades/downgrades):
    - High- or Medium-risk submissions downgraded by the judge to Low-risk (QA) will be ineligible for awards.
    - Upgrading a Low-risk finding from a QA report to a Medium- or High-risk finding is not supported.
    - As such, wardens are encouraged to select the appropriate risk level carefully during the submission phase.

## Publicly known issues

_Anything included in this section is considered a publicly known issue and is therefore ineligible for awards._

Anything already mentioned in the Halborn reports, or anything that would not directly lead to users losing funds or bad debt incurring.

Liquidations must be triggered offchain. The process is permissionless and there is an economic incentive (0.5% liquidator fee taken from the repaid debt) to ensure that some people are doing the job. Wardens must assume there will always be someone taking care of triggering a valid liquidation.

✅ SCOUTS: Please format the response above 👆 so its not a wall of text and its readable.

# Overview

[ ⭐️ SPONSORS: add info here ]

## Links

- **Previous audits:**  https://www.halborn.com/audits/thorchain/credit-accounts-21860f and https://www.halborn.com/audits/thorchain/ruji-lending-48bc98
  - ✅ SCOUTS: If there are multiple report links, please format them in a list.
- **Documentation:** https://gitlab.com/thorchain/rujira/-/blob/main/contracts/rujira-ghost-credit/README.md
- **Website:** https://rujira.network/
- **X/Twitter:** https://x.com/RujiraNetwork

---

# Scope

[ ✅ SCOUTS: add scoping and technical details here ]

### Files in scope
- ✅ This should be completed using the `metrics.md` file
- ✅ Last row of the table should be Total: SLOC
- ✅ SCOUTS: Have the sponsor review and and confirm in text the details in the section titled "Scoping Q amp; A"

*For sponsors that don't use the scoping tool: list all files in scope in the table below (along with hyperlinks) -- and feel free to add notes to emphasize areas of focus.*

| Contract | SLOC | Purpose | Libraries used |  
| ----------- | ----------- | ----------- | ----------- |
| [contracts/folder/sample.sol](https://github.com/code-423n4/repo-name/blob/contracts/folder/sample.sol) | 123 | This contract does XYZ | [`@openzeppelin/*`](https://openzeppelin.com/contracts/) |

### Files out of scope
✅ SCOUTS: List files/directories out of scope

# Additional context

## Areas of concern (where to focus for bugs)
A particular attention should be given to anything that could result in liquidations not functioning as intended and leading to bad debt.

✅ SCOUTS: Please format the response above 👆 so its not a wall of text and its readable.

## Main invariants

The main invariants across the contracts making up Credit Accounts and Lending vaults:

Owner-Gated Accounts: ExecuteMsg::Account compares info.sender to account.owner every time, so only the wallet that owns a credit account (or a new owner after transfer) can initiate borrow/repay/send/execute calls; this keeps debt creation and collateral moves bound to the NFT-like ownership model (contracts/rujira-ghost-credit/src/contract.rs (lines 151-230)).

Post-Adjustment LTV Check: After processing owner messages, the registry immediately schedules CheckAccount, which reloads the account and enforces adjusted_ltv < adjustment_threshold; if the account slipped too close to liquidation the transaction fails, so user-driven rebalances always finish safely (contracts/rujira-ghost-credit/src/contract.rs (lines 163-170), contracts/rujira-ghost-credit/src/account.rs (lines 152-191)).

Safe Liquidation Outcomes: Liquidation starts only when adjusted_ltv ≥ liquidation_threshold, then every iteration validates that the final account is under the liquidation threshold yet still above adjustment_threshold and respects user preference order plus max slip; otherwise the queue keeps executing or the tx reverts, ensuring liquidators can’t over-sell (contracts/rujira-ghost-credit/src/contract.rs (lines 73-150), contracts/rujira-ghost-credit/src/account.rs (lines 247-281)).

Whitelisted Vault Access: The registry can call SetVault only for denoms already listed in collateral_ratios, so borrowing/repaying for any denom always routes through a vetted rujira-ghost-vault, preventing rogue contracts from being used as debt sources (contracts/rujira-ghost-credit/src/contract.rs (lines 253-339), contracts/rujira-ghost-credit/src/contract.rs (lines 354-375)).

Bounded Config Values: Config::validate runs on instantiate and every sudo update, enforcing fee caps, ratio ≤ 1 constraints, and liquidation_threshold > adjustment_threshold, keeping governance knobs inside parameters that auditors (Halborn) have reviewed (contracts/rujira-ghost-credit/src/config.rs (lines 55-125)).

Fee-First Liquidation Repay: When a liquidator repays, the contract pulls the entire debt-denom balance, carves out protocol + solver fees, and repays the remainder; if no tokens exist the step errors, so fees are never minted without delivering real debt repayment (contracts/rujira-ghost-credit/src/contract.rs (lines 265-317)).

Admin-Only Accounts: Every credit account is a rujira-account instance whose execute/query entry points always return Unauthorized, while sudo simply forwards a message supplied by the registry, meaning only the registry can drive account-level contract calls or token transfers (contracts/rujira-account/src/contract.rs (lines 22-40)).

Governance-Whitelisted Borrowers: Borrowing from the vault requires being pre-registered via SudoMsg::SetBorrower; Borrower::load fails for unknown addresses, so new protocols can’t draw from the vault until governance explicitly approves them (contracts/rujira-ghost-vault/src/contract.rs (lines 204-217), contracts/rujira-ghost-vault/src/borrowers.rs (lines 29-77)).

Borrow Limit Enforcement: Borrower::borrow recalculates the shares’ USD value and blocks any request that would surpass the configured limit, and delegates call into the same struct so they share the exact headroom; this guarantees no combination of delegate borrowing can exceed the borrower’s cap (contracts/rujira-ghost-vault/src/borrowers.rs (lines 54-113)).

Always-Accrued Interest: Both execute and query entry points call state.distribute_interest before doing anything else, which accrues debt interest, credits depositors, and mints protocol fees; users therefore always act on up-to-date pool balances and rates (contracts/rujira-ghost-vault/src/contract.rs (lines 42-236), contracts/rujira-ghost-vault/src/state.rs (lines 52-171)).


✅ SCOUTS: Please format the response above 👆 so its not a wall of text and its readable.

## All trusted roles in the protocol

Smart contract deployment on THORChained is permissioned. Rujira Deployer Multisig is the whitelisted admin/owner of the Lending Vault and Credit Account smart contracts. It is the only address that has the power to modify protocol parameters and whitelist other contracts that can borrow from the Lending Vaults.

THORChain node operators have the ability via governance (mimir) to pause the entire app layer, or specific contracts, e.g. in the event of an exploit. This could create issues with liquidations in case of a pause during a period of high volatility.

Vulnerabilities requiring a permissioned role to be acted upon (whether it is Rujira Deployer Multisig or THorchain nodes operators) will not be considered as valid.


✅ SCOUTS: Please format the response above 👆 using the template below👇

| Role                                | Description                       |
| --------------------------------------- | ---------------------------- |
| Owner                          | Has superpowers                |
| Administrator                             | Can change fees                       |

✅ SCOUTS: Please format the response above 👆 so its not a wall of text and its readable.

## Running tests

The root README.md provides guidance to both build and test the contracts making up both Credit Accounts and Lending vaults:

From a fresh git clone, cd into /rujira/ and run cargo build && cargo test && cargo coverage inside each contract crate, so invoke cargo build from contracts/<contract-name> (or at the workspace root with --workspace) to compile everything in debug mode.

Once debug builds succeed, if you want to run basic unit/integration tests then run cargo build && cargo test && cargo coverage commands in sequence from each contract’s directory:
contracts/rujira-ghost-credit
contracts/rujira-ghost-vault
contracts/rujira-account

If you need to release artifacts, the “Compile & Commit” checklist instructs to run ./scripts/optimize.sh, which rebuilds all contracts for x86_64 Linux and drops optimized .wasm files under artifacts/.


✅ SCOUTS: Please format the response above 👆 using the template below👇

```bash
git clone https://github.com/code-423n4/2023-08-arbitrum
git submodule update --init --recursive
cd governance
foundryup
make install
make build
make sc-election-test
```
To run code coverage
```bash
make coverage
```

✅ SCOUTS: Add a screenshot of your terminal showing the test coverage

## Miscellaneous
Employees of Rujira and employees' family members are ineligible to participate in this audit.

Code4rena's rules cannot be overridden by the contents of this README. In case of doubt, please check with C4 staff.
