# TrustLink Escrow Bindings

This package contains the checked-in TypeScript surface for the escrow contract ABI.

## ABI Surface

The contract exports the following entrypoints:

- `initialize(admin, fee_collector)`
- `pause_contract()`
- `unpause_contract()`
- `withdraw_fees(token, to, amount)`
- `create_escrow(seller, resolver, token, amount, fee_bps, shipping_window)`
- `fund_escrow(escrow_id, buyer)`
- `confirm_delivery(escrow_id)`
- `raise_dispute(escrow_id, reason, description, evidence_hash)`
- `resolve_dispute(escrow_id, resolution)`
- `auto_release(escrow_id)`
- `get_escrow(escrow_id)`
- `get_dispute(escrow_id)`
- `get_fee_config()`

The exported data types mirror the contract storage and event payloads:

- `EscrowState`
- `DisputeStatus`
- `ResolutionType`
- `ContractError`
- `FeeConfig`
- `FeesWithdrawn`
- `ContractPaused`
- `ContractUnpaused`
- `EscrowData`
- `DisputeData`

## Regenerating The Bindings

When the ABI changes, rebuild the contract Wasm and regenerate the checked-in TypeScript source:

```bash
cargo build --target wasm32-unknown-unknown --release
stellar contract bindings typescript \
  --wasm ../target/wasm32-unknown-unknown/release/trustlink_escrow.wasm \
  --output-dir src \
  --overwrite
```

Commit the updated `src/` output alongside the contract change.