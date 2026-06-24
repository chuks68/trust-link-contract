# TrustLink — Soroban Escrow Contract
# TrustLink Contract (Soroban Escrow)

> Trustless escrow for social commerce on Stellar: funds move only when the contract can prove the requested lifecycle event has happened.

[![Stellar](https://img.shields.io/badge/Stellar-Soroban-7B68EE?style=flat-square&logo=stellar)](https://stellar.org)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?style=flat-square&logo=rust)](https://rustup.rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

---

## Overview

The TrustLink Escrow Contract is the **trustless vault** of the TrustLink protocol. It holds Stellar assets (USDC and other SEP-41 tokens) in escrow and releases them only when verifiable conditions are met — buyer confirms delivery, dispute resolver decides, or the shipping window expires.

Buyers and sellers never meet. The contract handles the trust gap.

---

## State Machine

```
  create_escrow()
       |
       v
  ┌─────────┐   fund_escrow()   ┌────────┐   mark_shipped()   ┌─────────┐
  │ PENDING │─────────────────▶ │ FUNDED │──────────────────▶ │ SHIPPED │
  └────┬────┘                   └────┬───┘                    └────┬────┘
       │                             │                      ┌──────┴──────┐
       │ cancel_escrow()        raise_dispute()      confirm_delivery() │
       │                             │                      │    raise_dispute()
       v                             v                      v          │
  ┌──────────┐                 ┌──────────┐           ┌──────────┐     │
  │CANCELLED │                 │ DISPUTED │           │COMPLETED │     │
  └──────────┘                 └────┬─────┘           └──────────┘     │
                                    │                                  │
                            resolve_dispute()                    auto_release()
                           ┌───────┴────────┐                         │
                           v                v                         v
                     ┌──────────┐    ┌──────────┐              ┌──────────┐
                     │COMPLETED │    │ REFUNDED │              │COMPLETED │
                     └──────────┘    └──────────┘              └──────────┘
```

Key rules:
- **Pending**: seller cancels freely (no money moved)
- **Funded → Shipped**: only seller can mark shipped
- **Shipped → Completed**: buyer confirms delivery, funds release to seller
- **Funded or Shipped → Disputed**: buyer raises dispute
- **Shipped → Completed (auto)**: anyone triggers after `shipped_at + shipping_window` elapses
- **Disputed → Completed/Refunded**: only the `resolver` address decides

---

## Contract Functions

| Function | Auth | Description |
|---|---|---|
| `create_escrow(seller, resolver, token, amount, shipping_window)` | seller | Creates escrow, assigns sequential `u32` ID. Buyer unknown until funding. |
| `fund_escrow(escrow_id, buyer)` | buyer | Transfers `amount` tokens from buyer to contract, sets `funded_at`. |
| `mark_shipped(escrow_id)` | seller | Sets state to `Shipped`, starts delivery clock at `shipped_at`. |
| `confirm_delivery(escrow_id)` | buyer | Transfers tokens from contract to seller, state → `Completed`. |
| `raise_dispute(escrow_id)` | buyer | Freezes funds, state → `Disputed`. Works from `Funded` or `Shipped`. |
| `resolve_dispute(escrow_id, release_to_seller)` | resolver | Transfers to seller or refunds buyer. |
| `auto_release(escrow_id)` | anyone | After `shipped_at + window`, releases to seller. |
| `cancel_escrow(escrow_id)` | seller | Only in `Pending` state (no funds moved). |
| `get_escrow(escrow_id) → EscrowData` | none | Read-only view. |

---

## Data Structures

```rust
pub struct EscrowData {
    pub seller: Address,           // creator
    pub buyer: Option<Address>,    // set when funded, None during Pending
    pub resolver: Address,         // dispute admin key
    pub token: Address,            // Stellar asset contract (USDC etc.)
    pub amount: i128,              // raw units (incl. decimals)
    pub shipping_window: u64,      // seconds after shipped_at for auto-release
    pub funded_at: u64,            // ledger timestamp when funded (0 if pending)
    pub shipped_at: u64,           // ledger timestamp when shipped (0 if not shipped)
    pub created_at: u64,           // ledger timestamp of creation
    pub state: EscrowState,
}

pub enum EscrowState {
    Pending,
    Funded,
    Shipped,
    Completed,
    Disputed,
    Refunded,
    Cancelled,
}
```

---

## Getting Started
This repository contains the **TrustLink escrow smart contract** implemented for **Stellar’s Soroban** runtime, plus a small set of developer tooling and language bindings to interact with the contract.

At a high level, TrustLink replaces “trust me” payments with a lifecycle that is enforced in code:

- A **seller** creates an escrow agreement.
- A **buyer** funds the escrow by transferring tokens into the contract.
- The **seller** marks the order as shipped.
- The system either:
  - lets the **buyer confirm delivery** (ending the deal), or
  - allows the buyer to **raise a dispute** before a deadline, after which an authorized **resolver/oracle** decides the outcome, or
  - allows **auto-release** after time windows elapse if no dispute remains unresolved.

The core goal is to ensure that each outcome—delivery completion, dispute release, dispute refund, or cancellation—happens via contract-enforced rules with clear authorization boundaries.

---

## Table of Contents

- [1. What is this project?](#1-what-is-this-project)
- [2. Who are the actors?](#2-who-are-the-actors)
- [3. Trust model & oracles](#3-trust-model--oracles)
- [4. Escrow lifecycle (state machine)](#4-escrow-lifecycle-state-machine)
- [5. Contract architecture](#5-contract-architecture)
  - [5.1 Contract entrypoints](#51-contract-entrypoints)
  - [5.2 Storage model](#52-storage-model)
  - [5.3 Events & off-chain indexing](#53-events--off-chain-indexing)
  - [5.4 Token flow (SEP-41)](#54-token-flow-sep-41)
- [6. Fee model](#6-fee-model)
  - [6.1 Fee calculation and fee cap](#61-fee-calculation-and-fee-cap)
  - [6.2 Arbitration fee](#62-arbitration-fee)
  - [6.3 Withdrawing protocol fees](#63-withdrawing-protocol-fees)
- [7. Operational controls](#7-operational-controls)
  - [7.1 Pause / unpause](#71-pause--unpause)
  - [7.2 Admin rotation](#72-admin-rotation)
  - [7.3 TTL extension configuration](#73-ttl-extension-configuration)
- [8. Public API reference](#8-public-api-reference)
  - [8.1 Initialization](#81-initialization)
  - [8.2 Escrow management](#82-escrow-management)
  - [8.3 Delivery & dispute flows](#83-delivery--dispute-flows)
  - [8.4 Resolution & auto-release](#84-resolution--auto-release)
  - [8.5 Read-only views](#85-read-only-views)
- [9. Error codes](#9-error-codes)
- [10. Security considerations](#10-security-considerations)
  - [10.1 Authorization boundaries](#101-authorization-boundaries)
  - [10.2 Reentrancy in Soroban](#102-reentrancy-in-soroban)
  - [10.3 Arithmetic & overflow safety](#103-arithmetic--overflow-safety)
  - [10.4 Trust assumptions & failure modes](#104-trust-assumptions--failure-modes)
- [11. Testing strategy](#11-testing-strategy)
- [12. Repository layout](#12-repository-layout)
- [13. TypeScript bindings & client usage](#13-typescript-bindings--client-usage)
- [14. Contributing](#14-contributing)
- [15. License](#15-license)

---

## 1. What is this project?

TrustLink is a **trustless escrow protocol** designed for **peer-to-peer social commerce**. In typical social commerce scenarios—payments initiated through DMs, chats, or lightweight marketplace workflows—buyer and seller rarely share a traditional, enforceable contract. Disputes are commonly handled manually and inconsistently.

This smart contract enforces payment outcomes on-chain.

### The contract’s purpose

The contract is the **escrow vault and arbitration enforcement layer**. It:

- accepts token deposits from a buyer into contract-held escrow state,
- releases funds to a seller only after delivery-related lifecycle transitions,
- supports dispute raising by the buyer within a time window,
- finalizes disputed escrows by requiring a resolver oracle to sign `resolve_dispute`,
- supports **auto-release** after time windows elapse (permissionless triggering),
- supports a global pause flag for operational safety.

### What “trustless” means here

“Trustless” does not mean “no trust anywhere.” Instead, it means that the protocol eliminates the need to trust a custodian to move funds correctly. With the exception of the dispute resolver/oracle’s judgment, outcomes are determined by:

- deterministic state machine transitions,
- deterministic token transfer logic, and
- deterministic time checks using ledger timestamps.

Because all transfers are initiated by contract code, there is no discretionary third party movement of funds.

---

## 2. Who are the actors?

TrustLink defines several distinct roles:

1. **Seller**
   - Creates the escrow agreement.
   - Marks the escrow as shipped.
   - Receives the payout on success.

2. **Buyer**
   - Funds the escrow.
   - Confirms delivery (ending the escrow).
   - Raises disputes (within the deadline).

3. **Resolver** (oracle for dispute finality)
   - Only role that can finalize an escrow once it enters `Disputed`.
   - Resolver address is stored per escrow at creation time.

4. **Admin** (operational control)
   - Pauses/unpauses the contract.
   - Rotates admin address.
   - Configures default fee parameters and arbitration fee.

5. **Fee Collector**
   - Receives protocol fee withdrawals.

6. **Any caller**
   - Can trigger `auto_release` once the escrow satisfies time conditions.

---

## 3. Trust model & oracles

The repository includes `ORACLE_TRUST_MODEL.md`, which documents the central trust assumptions. Summarizing that document in code terms:

### 3.1 Resolver trust

When the buyer raises a dispute, the contract stores an evidence hash and metadata, but it cannot verify the underlying evidence (shipments, courier records, legal documents). Real-world verification is impossible for on-chain code without trusted inputs.

Therefore, the contract embeds a resolver oracle address per escrow and requires the resolver to authenticate the dispute outcome using Soroban’s `require_auth()`.

If the resolver key is compromised, the attacker can finalize disputes in any direction by signing `resolve_dispute`.

Mitigations recommended by the repository documentation:

- use multisig or hardened accounts for the resolver,
- ensure strong key management and liveness monitoring,
- treat evidence hashes as commitments to off-chain evidence rather than verifiable on-chain content.

### 3.2 Admin trust

Admin can pause the contract and update fee parameters. Admin compromises primarily affect liveness and economics, not the ability for arbitrary accounts to move escrow funds.

---

## 4. Escrow lifecycle (state machine)

The escrow lifecycle is a finite state machine. The escrow states are defined in `contracts/escrow/src/types.rs`:

- Rust 1.75+
- [Stellar CLI](https://developers.stellar.org/docs/tools/stellar-cli) 21+
- wasm32v1-none target (`rustup target add wasm32v1-none`)

### Build & Test

```bash
# Build for Soroban
cargo build --target wasm32v1-none -p trustlink-escrow

# Run all 16 tests
cargo test -p trustlink-escrow
```
- `Pending`
- `Funded`
- `Shipped`
- `Completed`
- `Disputed`
- `Refunded`
- `Canceled`

### State transitions in practice

1. **Creation**
   - `create_escrow` creates a new escrow record and sets state to `Pending`.

2. **Funding**
   - `fund_escrow` requires buyer auth.
   - State must be `Pending`.
   - Tokens are transferred from buyer to the contract.
   - State becomes `Funded`.
   - `funded_at` and `dispute_deadline` are recorded.

3. **Shipping**
   - `mark_shipped` requires seller auth.
   - State must be `Funded`.
   - `tracking_id` is saved (bounded length).
   - State becomes `Shipped`.

4. **Delivery confirmation**
   - `confirm_delivery` requires buyer auth.
   - Allowed from `Funded` and `Shipped`.
   - Requires `ledger.timestamp() >= dispute_deadline`.
   - Transfers payout to the seller.
   - State becomes `Completed`.

5. **Dispute raising**
   - `raise_dispute` requires buyer auth.
   - Allowed from `Funded` and `Shipped`.
   - Requires `ledger.timestamp() < dispute_deadline`.
   - Stores dispute metadata including `evidence_hash`.
   - State becomes `Disputed`.

```bash
stellar keys generate --global deployer --network testnet

stellar contract deploy \
  --wasm target/wasm32v1-none/release/trustlink_escrow.wasm \
  --source deployer \
  --network testnet

# → outputs CONTRACT_ID, save it
```

### Invoke Examples

```bash
# Create escrow (buyer unknown yet)
stellar contract invoke \
  --id $CONTRACT_ID --source seller --network testnet -- \
  create_escrow \
  --seller $SELLER_ADDR \
  --resolver $RESOLVER_ADDR \
  --token $USDC_CONTRACT \
  --amount 50000000 \
  --shipping_window 604800

# Fund escrow (buyer connects wallet)
stellar contract invoke \
  --id $CONTRACT_ID --source buyer --network testnet -- \
  fund_escrow \
  --escrow_id 1 \
  --buyer $BUYER_ADDR

# Mark shipped
stellar contract invoke \
  --id $CONTRACT_ID --source seller --network testnet -- \
  mark_shipped \
  --escrow_id 1
```

### Usage Examples

#### Create escrow
```bash
stellar contract invoke \
  --id $CONTRACT_ID --source seller --network testnet -- \
  create_escrow \
  --seller $SELLER_ADDR \
  --resolver $RESOLVER_ADDR \
  --token $USDC_CONTRACT \
  --amount 50000000 \
  --shipping_window 604800
```

#### Fund escrow
```bash
stellar contract invoke \
  --id $CONTRACT_ID --source buyer --network testnet -- \
  fund_escrow \
  --escrow_id 1 \
  --buyer $BUYER_ADDR
```

#### Confirm delivery
```bash
stellar contract invoke \
  --id $CONTRACT_ID --source buyer --network testnet -- \
  confirm_delivery \
  --escrow_id 1
```

#### Raise dispute
```bash
stellar contract invoke \
  --id $CONTRACT_ID --source buyer --network testnet -- \
  raise_dispute \
  --escrow_id 1 \
  --reason "Item not as described" \
  --description "The received item differs from the listing." \
  --evidence_hash $EVIDENCE_HASH
```

#### Resolve dispute (release to seller)
```bash
stellar contract invoke \
  --id $CONTRACT_ID --source $RESOLVER_ADDR --network testnet -- \
  resolve_dispute \
  --escrow_id 1 \
  --release_to_seller true
```

---

## Test Coverage (16 tests)

| Test | What it verifies |
|---|---|
| `test_create_escrow` | All fields set correctly, id increments |
| `test_fund_escrow` | Tokens move to contract, state → Funded |
| `test_mark_shipped` | State → Shipped, shipped_at set |
| `test_confirm_delivery` | Full happy path: create → fund → ship → confirm |
| `test_raise_dispute_after_funded` | Dispute from Funded state |
| `test_raise_dispute_after_shipped` | Dispute from Shipped state |
| `test_raise_and_resolve_dispute_release_to_seller` | Resolver releases to seller |
| `test_raise_and_resolve_dispute_refund_buyer` | Resolver refunds buyer |
| `test_auto_release` | Auto-release after shipping window elapses |
| `test_cancel_escrow` | Cancel in Pending state |
| `test_fund_non_pending_escrow_fails` | Double-fund prevention |
| `test_confirm_delivery_before_shipped_fails` | Can't confirm before ship |
| `test_auto_release_before_window_fails` | Can't auto-release too early |
| `test_auto_release_before_shipped_fails` | Can't auto-release before shipped |
| `test_cancel_after_fund_fails` | Can't cancel after funding |
| `test_multiple_escrows` | Independent escrows, correct balances |

---

## Project Structure

```
contracts/escrow/
├── Cargo.toml
└── src/
    ├── lib.rs       # Contract + events + storage (single module)
    └── test.rs      # All 16 tests
```

---

## Security Notes

- **Re-entrancy**: Soroban's execution model prevents re-entrancy by design.
- **Access control**: Every state-mutating function validates auth via `require_auth()`.
- **Overflow**: Arithmetic uses `i128` with Soroban's checked operations.
- **No admin key**: The dispute `resolver` is set per-escrow at creation, not a global key.

> This contract has not been formally audited. Use on mainnet at your own risk.

---

## Roadmap

- [x] Core escrow state machine (Pending → Funded → Shipped → Completed / Disputed)
- [x] SEP-41 token support (USDC, native assets)
- [x] Auto-release after shipping window
- [x] Dispute + resolver flow
- [x] Escrow cancellation
- [ ] Multi-asset support with per-escrow token choice
- [ ] Buyer-initiated refund before shipment
- [ ] On-chain dispute evidence hash
- [ ] Formal security audit

---

## License

MIT © TrustLink Contributors
6. **Dispute resolution**
   - `resolve_dispute` requires resolver auth.
   - Allowed only from `Disputed`.
   - Applies configured arbitration fee.
   - Transfers net payout based on resolution type.
   - Updates both escrow state and dispute status.

7. **Auto-release**
   - `auto_release` is permissionless.
   - Allowed from `Funded` or `Shipped`.
   - Requires:
     - ledger time past dispute deadline,
     - ledger time past `funded_at + shipping_window`.
   - Transfers payout to the seller.
   - State becomes `Completed`.

8. **Cancellation**
   - `cancel_escrow` requires seller auth.
   - Allowed only in `Pending`.
   - Sets state to `Canceled`.

The contract also includes an auditing helper `transition_state` in `lib.rs` to express allowed transitions in one place.

---

## 5. Contract architecture

### 5.1 Contract entrypoints

The contract implementation is in:

- `contracts/escrow/src/lib.rs`

This file defines the Soroban contract (`#[contract] pub struct Escrow;`) and a set of `#[contractimpl]` methods.

The entrypoints split into:

- **Initialization and admin actions**
- **Escrow creation and lifecycle actions**
- **Dispute actions**
- **Resolution actions**
- **Read-only query functions**

Each state-mutating method generally follows a pattern:

1. ensure contract is not paused (except some admin/oracle methods depending on call path),
2. load escrow data from persistent storage,
3. check the escrow state and time conditions,
4. verify caller authorization using `require_auth()` on the expected address,
5. perform token transfers via SEP-41 token client,
6. persist updated state and emit events.

### 5.2 Storage model

The contract stores global configuration and counters in **instance storage** and escrow/dispute records in **persistent storage**.

Keys are defined in `contracts/escrow/src/types.rs`:

- `DataKey::Admin`
- `DataKey::Escrow(u64)`
- `DataKey::EscrowCounter`
- `DataKey::Dispute(u64)`
- `DataKey::Paused`
- `DataKey::FeeCollector`
- `DataKey::ArbitrationFee`
- `DataKey::DefaultFeeBps`
- totals such as `DataKey::TotalCompleted`, `DataKey::TotalDisputed`, etc.

TTL extension is configurable (instance key `DataKey::TtlExtensionLedgers`) and is applied when saving/loading escrow and dispute records.

### 5.3 Events & off-chain indexing

Events are defined in `contracts/escrow/src/events.rs`.

Each meaningful lifecycle step emits an event (examples):

- `EscrowCreated`
- `EscrowFunded`
- `EscrowShipped`
- `EscrowCompleted`
- `EscrowCancelled`
- `DisputeRaised`
- `DisputeResolved`
- `AutoReleased`

The tests include numerous snapshot JSON files under `contracts/escrow/test_snapshots/…` that strongly suggests events are checked for stability and correctness.

For backend oracle/indexer designs, the recommended workflow is:

- subscribe to events,
- build a local state index keyed by `escrow_id`,
- present reconciliation views for dispute, deadlines, and payout status.

### 5.4 Token flow (SEP-41)

The escrow contract is token-agnostic, using SEP-41 token interface clients. All token operations are mediated via:

- `soroban_sdk::token::Client`

Token transfers occur in:

- `fund_escrow`: buyer → contract
- `confirm_delivery`: contract → seller
- `auto_release`: contract → seller
- `resolve_dispute`: contract → seller or buyer
- `withdraw_fees`: contract → fee collector recipient

The payout logic is governed by `deduct_and_transfer`, which calculates:

- fee = amount * fee_bps / 10_000 (basis points)
- net = amount - fee

The arbitration fee is handled as a separate deduction in `resolve_dispute`.

---

## 6. Fee model

### 6.1 Fee calculation and fee cap

The contract enforces a fee cap with `MAX_FEE_BPS = 300`, i.e. 3%.

Escrow creation accepts a `fee_bps` parameter, and `create_escrow` rejects any value above the cap.

Additionally, the contract can update a default fee via admin (`set_fee`) stored in `DataKey::DefaultFeeBps`. (Per-escrow fee is passed at creation time.)

The `deduct_and_transfer` helper rejects negative amounts and uses checked arithmetic to avoid silent overflows.

### 6.2 Arbitration fee

Dispute resolution uses an arbitration fee configured on-chain as `ArbitrationFee`.

`resolve_dispute` reads the arbitration fee, checks that the escrow’s `amount` covers it, subtracts it from `escrow.amount`, and tracks total arbitration fees per token.

This creates the effect that arbitration resolution payouts are reduced by arbitration fee before applying the protocol fee model.

### 6.3 Withdrawing protocol fees

`withdraw_fees(token, to, amount)` enables the admin to move accumulated protocol token balances from the contract to the target address.

Guards include:

- paused check,
- admin authorization,
- amount positive,
- sufficient balance in the contract token vault.

---

## 7. Operational controls

### 7.1 Pause / unpause

Pause is stored as `DataKey::Paused`.

When paused, state-mutating escrow operations refuse execution via `ensure_not_paused`.

The pause behavior is tested in `test_pause.rs` and corresponding snapshots.

### 7.2 Admin rotation

Admin rotation is performed by `set_admin(new_admin)`.

Only the current admin can rotate; rotation emits an `AdminRotated` event.

This is useful to recover from lost keys and to evolve operational security posture.

### 7.3 TTL extension configuration

Soroban storage entries can expire if not extended.

The contract uses:

- a default TTL extension value
- a configurable override via `set_ttl_extension(ledgers)`

The helper functions in `lib.rs` apply TTL extension after reading from persistent storage and when writing back.

This reduces the chance of long-lived escrow entries expiring unexpectedly.

---

## 8. Public API reference

This section provides an “operator’s view” of the contract methods as they appear in:

- `contracts/escrow/src/lib.rs`
- TypeScript bindings under `bindings/src`

### 8.1 Initialization

#### `initialize(admin, fee_collector, arbitration_fee)`

- **Guard:** only allowed when not initialized (checks existence of `DataKey::Admin`).
- **Effects:** sets:
  - `DataKey::Admin`
  - `DataKey::FeeCollector`
  - `DataKey::ArbitrationFee`
  - `DataKey::EscrowCounter = 1`
  - `DataKey::Paused = false`

A second call panics in current implementation.

### 8.2 Escrow management

#### `create_escrow(seller, resolver, token, amount, fee_bps, shipping_window)`

- **Auth:** `seller.require_auth()`.
- **Guards:** amount > 0, fee_bps <= 300, not paused.
- **Effects:**
  - creates new escrow record with unique id from `EscrowCounter`,
  - state = `Pending`,
  - buyer is unset (`None`),
  - dispute deadline and funding fields set to zero defaults.

#### `cancel_escrow(escrow_id)`

- **Auth:** seller require auth (escrow.seller).
- **Guards:** escrow must be in `Pending`.
- **Effects:** state = `Canceled` and emits `EscrowCancelled`.

### 8.3 Delivery & dispute flows

#### `fund_escrow(escrow_id, buyer)`

- **Auth:** buyer require auth.
- **Guards:** escrow in `Pending`.
- **Effects:**
  - sets escrow.buyer = Some(buyer)
  - state = `Funded`
  - records `funded_at` and `dispute_deadline`
  - transfers escrow amount into contract
  - emits `EscrowFunded`.

#### `mark_shipped(escrow_id, tracking_id)`

- **Auth:** seller require auth.
- **Guards:** escrow in `Funded`, tracking_id length <= 64.
- **Effects:** state = `Shipped`, store tracking id, emit `EscrowShipped`.

#### `record_delivery(escrow_id)`

- **Auth:** admin require auth.
- **Guards:** escrow must be `Shipped`.
- **Effects:** writes `delivered_at` and emits `DeliveryRecorded`.

Whether clients use this function depends on the deployment; the contract also provides `confirm_delivery` that directly completes escrow based on dispute deadline.

#### `confirm_delivery(escrow_id)`

- **Auth:** buyer require auth.
- **Guards:** escrow in `Funded` or `Shipped`, and the dispute window has closed (`ledger.timestamp >= dispute_deadline`).
- **Effects:** transfers net amount to seller using protocol fee logic, sets `state = Completed`, increments totals, emits `EscrowCompleted`.

#### `raise_dispute(escrow_id, reason, description, evidence_hash)`

- **Auth:** buyer require auth.
- **Guards:** escrow in `Funded` or `Shipped`, and `ledger.timestamp < dispute_deadline`.
- **Effects:**
  - sets `state = Disputed`
  - persists `DisputeData` with `BytesN<32>` evidence hash and metadata
  - emits `DisputeRaised`.

### 8.4 Resolution & auto-release

#### `resolve_dispute(escrow_id, resolution)`

- **Auth:** resolver require auth.
- **Guards:** escrow in `Disputed`.
- **Effects:**
  - subtract arbitration fee from escrow amount
  - transfers net remainder based on resolution direction:
    - `Release` → seller
    - `Refund` → buyer
  - sets escrow state `Completed` or `Refunded`
  - updates dispute status to `Resolved`
  - emits `DisputeResolved`.

#### `auto_release(escrow_id)`

- **Auth:** none.
- **Guards:** escrow state in `Funded` or `Shipped`, and time checks for both:
  - dispute deadline closed,
  - shipping window elapsed (`funded_at + shipping_window`).
- **Effects:** transfers net amount to seller, sets `state = Completed`, emits `AutoReleased`.

### 8.5 Read-only views

- `get_escrow(escrow_id)`: returns `EscrowData`.
- `get_dispute(escrow_id)`: returns `Option<DisputeData>` (or None if no dispute exists for the escrow ID).
- `get_escrows_by_buyer(buyer)`: iterates from 1 to `EscrowCounter-1`, collects matching buyer escrows.
  - This is convenient for clients, but can be expensive as escrow count grows.
- `get_fee_config()`: returns fee collector and max fee.
- `get_contract_config()`: returns admin, default fee bps, fee collector, and escrow count.
- `get_stats()`: returns counters for created/completed/disputed/refunded.

---

## 9. Error codes

The contract uses Soroban typed errors defined in `contracts/escrow/src/types.rs`:

- `InvalidAmount = 1`
- `InsufficientBalance = 2`
- `EscrowNotFound = 3`
- `InvalidState = 4`
- `NotAuthorized = 5`
- `AlreadyInitialized = 6`
- `FeeExceedsMax = 7`
- `EscrowHasNoBuyer = 8`
- `ShippingWindowNotElapsed = 9`
- `InvalidEvidenceHash = 10`
- `DisputeNotFound = 11`
- `ArithmeticError = 12`
- `DisputeWindowClosed = 13`
- `ContractPaused = 14`
- `ArithmeticOverflow = 15`
- `InvalidStateTransition = 16`
- `InputTooLong = 17`

Client applications should handle these errors by showing user-friendly messages or by retrying/correcting inputs depending on the code.

---

## 10. Security considerations

This contract’s security is primarily a combination of:

- correct authorization checks,
- strict state-machine guards,
- deterministic time windows,
- safe arithmetic,
- careful token transfer handling.

Additional security reasoning is documented in `REENTRANCY_ANALYSIS.md`.

### 10.1 Authorization boundaries

Across entrypoints, the contract requires the expected signer:

- seller calls require seller auth,
- buyer calls require buyer auth,
- resolver calls require resolver auth,
- admin calls require admin auth.

This ensures that even if someone can guess or discover an escrow id, they cannot move escrow funds without the correct signature.

### 10.2 Reentrancy in Soroban

Classic EVM external reentrancy patterns rely on the callee executing attacker-controlled callbacks while the caller is mid-execution.

Soroban’s execution model prevents classic external reentrancy patterns. The included `REENTRANCY_ANALYSIS.md` explains why: nested invocation frames are host-managed and there is no ability to inject callbacks that can re-enter the caller mid-frame.

Even so, the contract still enforces good internal structure:

- precondition checks before transfers,
- state transitions that make repeated calls invalid,
- checked arithmetic.

### 10.3 Arithmetic & overflow safety

The contract enables overflow-checking in the Rust release profile (`overflow-checks = true`).

Additionally, `deduct_and_transfer` uses checked operations and returns typed errors instead of panicking.

### 10.4 Trust assumptions & failure modes

TrustLink has explicit operational trust points:

- The resolver is required for dispute finality.
- Admin keys can pause the contract and update fees.

The protocol is therefore not purely “no trust ever,” but it is structured so that trust is limited to clearly defined roles with explicit authentication.

The repository includes further guidance in `ORACLE_TRUST_MODEL.md`.

### 10.5 Dependency Security Scanning

This repository uses `cargo-audit` in CI to automatically detect vulnerable Rust dependencies on every push and pull request.

---

## 11. Testing strategy

The escrow contract has an extensive test suite in:

- `contracts/escrow/src/test.rs`
- multiple `test_*.rs` modules and snapshots.

A non-exhaustive list of what is covered by the repository test files and snapshot folders:

- correct escrow id behavior and counter monotonicity,
- fee bounds and fee update behavior,
- string length validation for `tracking_id` and dispute description,
- dispute timing boundaries and error cases,
- arbitration fee deduction semantics,
- auto-release timing and state constraints,
- admin rotation and admin auth enforcement,
- pause/unpause behavior and blocked mutations,
- TTL behavior.

Snapshot JSONs suggest the tests verify event payloads and/or numeric outputs to prevent regressions.

---

## 12. Repository layout

Important files and directories:

- `Cargo.toml` and `Cargo.lock`: workspace and dependencies.
- `ARCHITECTURE.md`: overall architectural description.
- `ORACLE_TRUST_MODEL.md`: resolver and oracle trust assumptions.
- `REENTRANCY_ANALYSIS.md`: reentrancy security rationale.
- `CONTRIBUTING.md`: contribution workflow.
- `contracts/escrow/`: Soroban escrow contract workspace member.
- `bindings/`: TypeScript bindings package.

Key contract source files:

- `contracts/escrow/src/lib.rs`: contract logic and entrypoints.
- `contracts/escrow/src/types.rs`: states, storage keys, errors, data structures.
- `contracts/escrow/src/events.rs`: event types and emitters.

---

## 13. TypeScript bindings & client usage

The repository provides ABI bindings in `bindings/`. These include:

- a typed `EscrowClient` in `bindings/src/client.ts`
- data types mirroring contract structs (see `bindings/src/types.ts`)
- a transport abstraction (`ContractTransport`) that you can wire to RPC/invocation tooling.

### How clients interact

The typical client flow:

1. Connect to a Soroban RPC endpoint.
2. Prepare and sign a transaction with the appropriate account.
3. Invoke a contract method by name, passing ABI-encoded arguments.
4. Parse return values and handle typed errors.

The `EscrowClient` class is intentionally thin: it forwards method names to the transport layer.

---

## 14. Contributing

Contribution guidelines are in `CONTRIBUTING.md`.

Recommended workflow:

- format with `cargo fmt`,
- lint with `cargo clippy -- -D warnings`,
- run tests with `cargo test`,
- ensure CI passes before opening a PR.

The repository participates in the **Stellar Wave Program**, and the docs describe issue labels and contribution points.

---

## 15. License

MIT © TrustLink Contributors

---

## Appendix: Contract design notes (quick reference)

### Time windows and bounds (as constants in code)

- `DISPUTE_WINDOW = 172_800` seconds (2 days)
- `MAX_FEE_BPS = 300` (3%)
- `DEFAULT_TTL_EXTENSION = 120_960` ledgers
- `MAX_TRACKING_ID_LEN = 64` characters
- `MAX_DESCRIPTION_LEN = 256` characters

### Evidence hash handling

`raise_dispute` stores a `BytesN<32>` evidence hash.

The contract commits to the hash, but does not validate evidence content. The resolver is trusted to interpret the evidence off-chain (based on the repository’s trust model).

---

End of README.

