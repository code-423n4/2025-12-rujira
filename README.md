# Rujira audit details
- Total Prize Pool: $40,000 in USDC
    - HM awards: up to $35,520 in USDC
        - If no valid Highs or Mediums are found, the HM pool is $0
    - QA awards: $1,480 in USDC
    - Judge awards: $3,000 in USDC
- [Read our guidelines for more details](https://docs.code4rena.com/competitions)
- Starts December 16, 2025 20:00 UTC
- Ends January 16, 2026 20:00 UTC

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

### Audit Findings

Anything already mentioned in the Halborn reports is considered out-of-scope for the purposes of this contest.

### Liquidations

Liquidations must be triggered offchain. The process is permissionless and there is an economic incentive (0.5% liquidator fee taken from the repaid debt) to ensure that some people are doing the job. Wardens must assume there will always be someone taking care of triggering a valid liquidation.

# Overview

Rujira is the App Layer on THORChain built using CosmWasm, offering an integrated suite of DeFi dapps, accessible with native assets from all connected chains in the form of "[Secured Assets](https://docs.thorchain.org/thorchain-finance/secured-assets)" powered by THORChain technology.

This audit is focussed on the lending and borrowing protocols:
* Ghost Lending: Decentralized money market allowing users to deposit assets in lending vaults that can be borrowed by authorized smart contract, mainly the Credit Accounts.
* Ghost Credit: Generic primitive design to allow overcollateralised borrowing of assets on Rujira.
* Rujira Account: generic smart contract for which the admin can call any CosmosMsg through the use of the sudo entrypoint. Useful for siloed accounting in Ghost Credit.

## Links

- **Previous audits:**
  - https://www.halborn.com/audits/thorchain/credit-accounts-21860f 
  - https://www.halborn.com/audits/thorchain/ruji-lending-48bc98
- **Documentation:** https://gitlab.com/thorchain/rujira/-/blob/main/contracts/rujira-ghost-credit/README.md
- **Website:** https://rujira.network/
- **X/Twitter:** https://x.com/RujiraNetwork

---

# Scope

The scope of this contest involves the `rujira-account`, `rujira-ghost-credit`, and `rujira-ghost-vault` modules of the system. 

### Files in scope

| Contract Paths | nSLOC |
| ------ | ------ |
| [contracts/rujira-account/src/\*\*.\*\*](https://github.com/code-423n4/2025-12-rujira/tree/main/contracts/rujira-account/src) | 46 |
| [contracts/rujira-ghost-credit/src/\*\*.\*\*](https://github.com/code-423n4/2025-12-rujira/tree/main/contracts/rujira-ghost-credit/src) | 1351 |
| [contracts/rujira-ghost-vault/src/\*\*.\*\*](https://github.com/code-423n4/2025-12-rujira/tree/main/contracts/rujira-ghost-vault/src) | 1202 |
| **Total** | **2599** |

### Files out of scope

All test files included in the above paths **are to be considered out-of-scope** for the purposes of this contest.

Additionally, any file that is not explicitly contained in the aforementioned list of folders is considered out-of-scope.

# Additional context

## Areas of concern (where to focus for bugs)

A particular attention should be given to anything that could result in liquidations not functioning as intended and leading to bad debt.

## Main invariants

The main invariants across the contracts making up Credit Accounts and Lending vaults are as follows:

### Owner-Gated Accounts

ExecuteMsg::Account compares info.sender to account.owner every time, so only the wallet that owns a credit account (or a new owner after transfer) can initiate borrow/repay/send/execute calls; this keeps debt creation and collateral moves bound to the NFT-like ownership model (contracts/rujira-ghost-credit/src/contract.rs (lines 151-230)).

### Post-Adjustment LTV Check

After processing owner messages, the registry immediately schedules CheckAccount, which reloads the account and enforces adjusted_ltv < adjustment_threshold; if the account slipped too close to liquidation the transaction fails, so user-driven rebalances always finish safely (contracts/rujira-ghost-credit/src/contract.rs (lines 163-170), contracts/rujira-ghost-credit/src/account.rs (lines 152-191)).

### Safe Liquidation Outcomes

Liquidation starts only when adjusted_ltv ≥ liquidation_threshold, then every iteration validates that the final account is under the liquidation threshold yet still above adjustment_threshold and respects user preference order plus max slip; otherwise the queue keeps executing or the tx reverts, ensuring liquidators can’t over-sell (contracts/rujira-ghost-credit/src/contract.rs (lines 73-150), contracts/rujira-ghost-credit/src/account.rs (lines 247-281)).

### Whitelisted Vault Access

The registry can call SetVault only for denoms already listed in collateral_ratios, so borrowing/repaying for any denom always routes through a vetted rujira-ghost-vault, preventing rogue contracts from being used as debt sources (contracts/rujira-ghost-credit/src/contract.rs (lines 253-339), contracts/rujira-ghost-credit/src/contract.rs (lines 354-375)).

### Bounded Config Values

Config::validate runs on instantiate and every sudo update, enforcing fee caps, ratio ≤ 1 constraints, and liquidation_threshold > adjustment_threshold, keeping governance knobs inside parameters that auditors (Halborn) have reviewed (contracts/rujira-ghost-credit/src/config.rs (lines 55-125)).

### Fee-First Liquidation Repay

When a liquidator repays, the contract pulls the entire debt-denom balance, carves out protocol + solver fees, and repays the remainder; if no tokens exist the step errors, so fees are never minted without delivering real debt repayment (contracts/rujira-ghost-credit/src/contract.rs (lines 265-317)).

### Admin-Only Accounts

Every credit account is a rujira-account instance whose execute/query entry points always return Unauthorized, while sudo simply forwards a message supplied by the registry, meaning only the registry can drive account-level contract calls or token transfers (contracts/rujira-account/src/contract.rs (lines 22-40)).

### Governance-Whitelisted Borrowers

Borrowing from the vault requires being pre-registered via SudoMsg::SetBorrower; Borrower::load fails for unknown addresses, so new protocols can’t draw from the vault until governance explicitly approves them (contracts/rujira-ghost-vault/src/contract.rs (lines 204-217), contracts/rujira-ghost-vault/src/borrowers.rs (lines 29-77)).

### Borrow Limit Enforcement

Borrower::borrow recalculates the shares’ USD value and blocks any request that would surpass the configured limit, and delegates call into the same struct so they share the exact headroom; this guarantees no combination of delegate borrowing can exceed the borrower’s cap (contracts/rujira-ghost-vault/src/borrowers.rs (lines 54-113)).

### Always-Accrued Interest

Both execute and query entry points call state.distribute_interest before doing anything else, which accrues debt interest, credits depositors, and mints protocol fees; users therefore always act on up-to-date pool balances and rates (contracts/rujira-ghost-vault/src/contract.rs (lines 42-236), contracts/rujira-ghost-vault/src/state.rs (lines 52-171)).

## All trusted roles in the protocol

Smart contract deployment on THORChained is permissioned. Rujira Deployer Multisig is the whitelisted admin/owner of the Lending Vault and Credit Account smart contracts. It is the only address that has the power to modify protocol parameters and whitelist other contracts that can borrow from the Lending Vaults.

THORChain node operators have the ability via governance (mimir) to pause the entire app layer, or specific contracts, e.g. in the event of an exploit. This could create issues with liquidations in case of a pause during a period of high volatility.

Vulnerabilities requiring a permissioned role to be acted upon (whether it is Rujira Deployer Multisig or THorchain nodes operators) will not be considered as valid.

## Running tests

The codebase is composed of Rust-based contracts that make up the Credit Accounts and Lending Vaults. All instructions have been tested on the following `rust` version:

- Rust (`rustc`): 1.81.0 (eeb90cda1 2024-09-04)
- Cargo (`cargo`): 1.81.0 (2dbb1af80 2024-08-20) 

### Building

The contracts in scope can be compiled by navigating to their respective folders (i.e. `cd contracts/rujira-account`) and issuing the following `cargo` command:

```bash
cargo build
```

#### Compilation Script

For users that are on a UNIX operating system, a dedicated script exists that allows the contracts to be compiled. This script can be run using the following command, and requires Docker to function (`29.1.3` tested):

```bash
bash scripts/optimize.sh
```

### Testing

Similarly, tests for each module can be executed by navigating to its dedicated folder and executing the following `cargo` command:

```bash
cargo test
```

## Miscellaneous
Employees of Rujira and employees' family members are ineligible to participate in this audit.

Code4rena's rules cannot be overridden by the contents of this README. In case of doubt, please check with C4 staff.
