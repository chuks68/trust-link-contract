# 🔐 TrustLink — Soroban Escrow Contract

> **Trustless commerce on Stellar. Every transaction protected by code, not promises.**

[![Stellar](https://img.shields.io/badge/Stellar-Soroban-7B68EE?style=flat-square&logo=stellar)](https://stellar.org)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?style=flat-square&logo=rust)](https://rustup.rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)
[![Stellar Wave](https://img.shields.io/badge/Stellar%20Wave-Open%20Issues-blue?style=flat-square)](https://www.drips.network/wave/stellar)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?style=flat-square)](CONTRIBUTING.md)

---

## 📖 Overview

The TrustLink Escrow Contract is the **trustless core** of the TrustLink protocol — a Soroban smart contract that acts as an autonomous judge and vault for social commerce transactions. It eliminates the "Trust Gap" between buyers and sellers on platforms like Instagram, WhatsApp, and Facebook by holding funds in escrow and releasing them only when verifiable conditions are met.

**No middlemen. No manual releases. No fraud.**

### Why This Matters

| Problem | TrustLink Solution |
|---|---|
| Buyers pay upfront and get scammed | Funds are locked in the contract until delivery is confirmed |
| Sellers ship goods and buyers ghost | Seller is guaranteed payment upon verified delivery |
| Centralized escrow is slow & expensive | Stellar settles in ~5s for a fraction of a cent |
| Trust relies on reputation systems | Trust is enforced by immutable code |

---

## 🏗️ Architecture

```
┌──────────────────────────────────────────────────────────┐
│                  TrustLink Escrow Contract                │
│                                                          │
│  ┌─────────┐    ┌──────────┐    ┌──────────────────────┐ │
│  │  State  │───▶│  Events  │───▶│   Release Logic      │ │
│  │ Machine │    │ Emitter  │    │ (Buyer / Auto / Admin)│ │
│  └─────────┘    └──────────┘    └──────────────────────┘ │
│       │                                    │             │
│  ┌────▼────────────────────────────────────▼───────────┐ │
│  │              Escrow Vault (Token Storage)            │ │
│  └─────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

### Transaction State Machine

```
  create_escrow()
       │
       ▼
  ┌─────────┐    fund_escrow()    ┌────────┐
  │ PENDING │──────────────────▶ │ FUNDED │
  └─────────┘                    └────┬───┘
                                      │ mark_shipped()
                                      ▼
                                 ┌─────────┐
                                 │ SHIPPED │
                                 └────┬────┘
                          ┌──────────┴──────────┐
                          │                     │
               confirm_delivery()          raise_dispute()
                          │                     │
                          ▼                     ▼
                    ┌──────────┐          ┌──────────┐
                    │COMPLETED │          │DISPUTED  │
                    └──────────┘          └──────────┘
                                               │
                                    admin_resolve() / refund()
                                               │
                                    ┌──────────┴──────────┐
                                    │                     │
                              ┌──────────┐          ┌──────────┐
                              │COMPLETED │          │ REFUNDED │
                              └──────────┘          └──────────┘
```

---

## ⚙️ Contract Functions

### Core Escrow Operations

| Function | Access | Description |
|---|---|---|
| `create_escrow(vendor, buyer, token, amount, shipping_window)` | Public | Initializes a new escrow instance |
| `fund_escrow(escrow_id)` | Buyer | Transfers tokens into the contract vault |
| `mark_shipped(escrow_id, tracking_id)` | Vendor | Updates state to `SHIPPED`, starts delivery clock |
| `confirm_delivery(escrow_id)` | Buyer | Releases funds to vendor immediately |
| `auto_release(escrow_id)` | System/Admin | Releases funds 48h after delivery if no dispute |
| `raise_dispute(escrow_id, evidence_hash)` | Buyer | Freezes funds and opens dispute window |
| `resolve_dispute(escrow_id, release_to)` | Admin | Admin resolves dispute — releases or refunds |
| `cancel_escrow(escrow_id)` | Vendor/Buyer | Cancels a `PENDING` escrow and refunds buyer |

### View Functions

| Function | Returns | Description |
|---|---|---|
| `get_escrow(escrow_id)` | `EscrowData` | Full escrow state and metadata |
| `get_escrows_by_vendor(vendor)` | `Vec<EscrowId>` | All escrows created by a vendor |
| `get_escrows_by_buyer(buyer)` | `Vec<EscrowId>` | All escrows funded by a buyer |

---

## 📦 Data Structures

```rust
pub struct EscrowData {
    pub id: u64,
    pub vendor: Address,
    pub buyer: Address,
    pub token: Address,          // USDC or any Stellar asset
    pub amount: i128,
    pub fee_bps: u32,            // basis points (100 = 1%)
    pub state: EscrowState,
    pub tracking_id: Option<String>,
    pub created_at: u64,
    pub shipped_at: Option<u64>,
    pub delivered_at: Option<u64>,
    pub evidence_hash: Option<BytesN<32>>,
}

pub enum EscrowState {
    Pending,
    Funded,
    Shipped,
    Completed,
    Disputed,
    Refunded,
}
```

---

## 🚀 Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) `1.75+`
- [Stellar CLI](https://developers.stellar.org/docs/tools/stellar-cli) (formerly `soroban-cli`) `21+`
- A funded Stellar testnet account ([Friendbot](https://friendbot.stellar.org/))

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/trustlink-contract
cd trustlink-contract

# Install the Soroban target
rustup target add wasm32-unknown-unknown

# Build the contract
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test
```

### Deploy to Testnet

```bash
# Configure your identity
stellar keys generate --global deployer --network testnet

# Deploy the contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/trustlink_escrow.wasm \
  --source deployer \
  --network testnet

# The command outputs a CONTRACT_ID — save it!
```

### Regenerate TypeScript Bindings

After changing the contract ABI, rebuild the Wasm and regenerate the checked-in bindings:

```bash
cargo build --target wasm32-unknown-unknown --release
stellar contract bindings typescript \
  --wasm target/wasm32-unknown-unknown/release/trustlink_escrow.wasm \
  --output-dir bindings \
  --overwrite
```

Commit the updated `bindings/` directory together with the contract change so consumers stay in sync.

The checked-in TypeScript package lives in [bindings/](bindings/README.md).

### Invoke Functions (Example)

```bash
# Create an escrow
stellar contract invoke \
  --id $CONTRACT_ID \
  --source vendor_key \
  --network testnet \
  -- \
  create_escrow \
  --vendor $VENDOR_ADDRESS \
  --buyer $BUYER_ADDRESS \
  --token $USDC_CONTRACT_ID \
  --amount 50000000 \
  --shipping_window 604800
```

---

## 🧪 Testing

The test suite covers all state transitions, edge cases, and attack vectors.

```bash
# Run all tests
cargo test

# Run tests with verbose output
cargo test -- --nocapture

# Run a specific test module
cargo test escrow_dispute_flow
```

### Test Coverage

- ✅ Full happy-path flow (create → fund → ship → confirm → complete)
- ✅ Auto-release after 48-hour delivery window
- ✅ Dispute raise and admin resolution (release to vendor)
- ✅ Dispute raise and admin resolution (refund to buyer)
- ✅ Escrow cancellation (pending state only)
- ✅ Unauthorized access reverts
- ✅ Double-funding prevention
- ✅ Expired escrow handling
- ✅ Fee calculation accuracy

---

## 🔒 Security Considerations

- **Re-entrancy**: Soroban's execution model prevents re-entrancy by design.
- **Integer overflow**: All arithmetic uses checked operations via `i128`.
- **Access control**: Every state-mutating function validates `Address` authorization using `require_auth()`.
- **Admin key rotation**: The admin address is upgradeable via a 2-of-3 multisig pattern to prevent single point of failure.
- **Fee cap**: Protocol fee is hardcoded to a maximum of 300 bps (3%) to prevent governance exploits.

> ⚠️ This contract has not yet been formally audited. Use on mainnet at your own risk. An audit is planned before v1.0 release.

---

## 📁 Project Structure

```
trustlink-contract/
├── src/
│   ├── lib.rs              # Contract entry point & public interface
│   ├── escrow.rs           # Core escrow logic & state machine
│   ├── storage.rs          # Persistent storage helpers
│   ├── events.rs           # Contract event definitions
│   ├── errors.rs           # Custom error codes
│   └── types.rs            # Shared data structures
├── tests/
│   ├── happy_path.rs       # Full flow integration tests
│   ├── dispute_flow.rs     # Dispute & resolution tests
│   ├── edge_cases.rs       # Boundary & attack vector tests
│   └── helpers.rs          # Test utilities & fixtures
├── Cargo.toml
└── README.md
```

---

## 🌊 Contributing via Stellar Wave

This repository is part of the **[Stellar Wave Program](https://www.drips.network/wave/stellar)** — a sprint-based contribution initiative by the Stellar Development Foundation where developers earn rewards for solving real open-source issues.

### How to Contribute

1. Browse open issues labelled [`Stellar Wave`](../../issues?q=label%3A%22Stellar+Wave%22) or [`good first issue`](../../issues?q=label%3A%22good+first+issue%22)
2. Sign in at [drips.network/wave](https://www.drips.network/wave) with your GitHub account
3. Apply to an issue you want to work on
4. Once assigned, submit a PR — get reviewed, get merged, earn points

### Issue Complexity Guide

| Label | Points | Examples |
|---|---|---|
| `trivial` | 100 pts | Fix a typo, add a missing error code, improve a comment |
| `medium` | 150 pts | Add a test case, implement a view function, fix a bug |
| `high` | 200 pts | New contract function, refactor storage model, security fix |

**Good First Issues** are specifically scoped and documented to help new Soroban developers ramp up quickly. The contract is thoroughly commented — even if you're new to Rust or Soroban, there's a path in.

---

## 🗺️ Roadmap

- [x] Core escrow state machine
- [x] USDC token support
- [x] Auto-release oracle hook
- [ ] Multi-asset support (any Stellar SEP-41 token)
- [ ] Time-locked refund without admin intervention
- [ ] On-chain dispute evidence storage (via IPFS CID)
- [ ] Contract upgrade pathway (via admin proxy)
- [ ] Formal security audit (v1.0)
- [ ] Mainnet deployment

---

## 📜 License

MIT © TrustLink Contributors

---

> Built with ❤️ on Stellar Soroban. Part of the Stellar Wave open-source ecosystem.
