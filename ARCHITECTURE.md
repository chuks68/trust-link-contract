# TrustLink Contract — Architecture

## Overview

TrustLink is a Soroban smart contract on the Stellar network that implements a trustless escrow system between a **buyer** and a **seller**, mediated by an optional **resolver** in case of disputes. All state lives on-chain; no off-chain service is required for core escrow operations.

---

## Components

### 1. Contract Entry Points (`contracts/escrow/src/lib.rs`)

| Function | Caller | Description |
|---|---|---|
| `create_escrow` | Seller | Creates a new escrow in `Pending` state |
| `fund_escrow` | Buyer | Locks tokens into the contract, moves escrow to `Funded` |
| `confirm_delivery` | Buyer | Releases funds to seller on satisfied delivery |
| `raise_dispute` | Buyer | Moves escrow to `Disputed` with a 32-byte evidence hash |
| `resolve_dispute` | Resolver | Pays out to seller or refunds buyer based on dispute finding |
| `auto_release` | Anyone | Releases to seller once the shipping window has elapsed |
| `get_escrow` | Anyone | Read-only view of an escrow record |

### 2. Data Types (`EscrowData`)

```
EscrowData {
    seller:          Address        — party receiving funds on success
    buyer:           Option<Address>— set at fund time; None before funding
    resolver:        Address        — trusted third-party mediator
    token:           Address        — SEP-41 token contract address
    amount:          i128           — locked token amount
    shipping_window: u64            — seconds after funding before auto-release is allowed
    funded_at:       u64            — ledger timestamp recorded at fund time
    state:           EscrowState    — current lifecycle state
}
```

### 3. State Machine (`EscrowState`)

```
Pending ──fund_escrow──► Funded ──confirm_delivery──► Completed
                           │                               ▲
                           ├──raise_dispute──► Disputed ───┤ (release_to_seller)
                           │                      │
                           │                      └──────► Refunded (refund_buyer)
                           │
                           └──auto_release (after shipping_window)──► Completed
```

Valid transitions:

| From | To | Trigger |
|---|---|---|
| `Pending` | `Funded` | `fund_escrow` |
| `Funded` | `Completed` | `confirm_delivery` or `auto_release` |
| `Funded` | `Disputed` | `raise_dispute` |
| `Disputed` | `Completed` | `resolve_dispute(release_to_seller=true)` |
| `Disputed` | `Refunded` | `resolve_dispute(release_to_seller=false)` |

`Completed` and `Refunded` are terminal states — no further transitions are possible.

---

## Storage Layout

All storage uses Soroban **instance** storage (entries share the contract instance's TTL).

| `DataKey` | Type | Description |
|---|---|---|
| `EscrowCount` | `u32` | Monotonically increasing counter; also the ID of the most-recently created escrow |
| `Escrow(id: u32)` | `EscrowData` | Full escrow record keyed by its numeric ID |

IDs start at `1`. The counter is read, incremented, and stored atomically inside `create_escrow`.

---

## Token Flow

TrustLink uses the [SEP-41](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md) token interface (`soroban_sdk::token::Client`).

```
fund_escrow:         buyer ──transfer──► contract
confirm_delivery:    contract ──transfer──► seller
auto_release:        contract ──transfer──► seller
resolve_dispute:     contract ──transfer──► seller  (if release_to_seller)
                     contract ──transfer──► buyer   (if refund)
```

The contract never holds more than one escrow's tokens per escrow ID. Multiple concurrent escrows each lock their own `amount` independently.

---

## Evidence Hash

`raise_dispute` accepts an `evidence_hash: Bytes` parameter that must be **exactly 32 bytes** (a SHA-256 digest of off-chain evidence). The hash is validated before any state change and emitted in the `raise_dispute` event for off-chain indexers.

---

## Events

| Topic | Data | Emitted by |
|---|---|---|
| `("create_escrow",)` | `escrow_id: u32` | `create_escrow` |
| `("fund_escrow",)` | `escrow_id: u32` | `fund_escrow` |
| `("confirm_delivery",)` | `escrow_id: u32` | `confirm_delivery` |
| `("raise_dispute",)` | `(escrow_id: u32, evidence_hash: Bytes)` | `raise_dispute` |
| `("resolve_dispute",)` | `(escrow_id: u32, release_to_seller: bool)` | `resolve_dispute` |
| `("auto_release",)` | `escrow_id: u32` | `auto_release` |

---

## Authorization Model

| Operation | Who must sign |
|---|---|
| `create_escrow` | `seller` |
| `fund_escrow` | `buyer` |
| `confirm_delivery` | `buyer` (retrieved from stored `EscrowData`) |
| `raise_dispute` | `buyer` (retrieved from stored `EscrowData`) |
| `resolve_dispute` | `resolver` (retrieved from stored `EscrowData`) |
| `auto_release` | No auth required — permissionless after window expires |
| `get_escrow` | No auth required — read-only |

---

## Cross-Contract Interactions

TrustLink calls one external contract: the **token contract** at `EscrowData.token`. All token interactions use `soroban_sdk::token::Client`, which conforms to the SEP-41 interface. No other cross-contract calls are made.

---

## Workspace Structure

```
trust-link-contract/
├── Cargo.toml                     — workspace manifest
├── ARCHITECTURE.md                — this file
├── README.md
└── contracts/
    └── escrow/
        ├── Cargo.toml
        └── src/
            ├── lib.rs             — contract implementation
            └── test.rs            — unit and integration tests
```
