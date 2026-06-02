# feat: tracking_id validation, cancel_escrow, get_escrow_count, FAQ

## Summary

- closes #78 — boundary test for `mark_shipped` empty tracking ID rejection
- closes #59 — `get_escrow_count` public view function
- closes #90 — `cancel_escrow` by vendor in Pending state + tests
- closes #76 — `docs/FAQ.md` contributor reference guide

Also fixes 153 compilation errors introduced by a bad PR merge (duplicate
`EscrowState`, missing type definitions, broken function signatures).

## Changes

### `contracts/escrow/src/lib.rs`
- Restored all missing type definitions (`EscrowData`, `DisputeData`,
  `ResolutionType`, `DisputeStatus`, `AdminRotated`, `DeliveryRecorded`) lost
  in the merge conflict.
- Removed duplicate `EscrowState` definition; kept the canonical one with the
  `Shipped` variant.
- Fixed all function return types to `Result<_, ContractError>` where required.
- Added pause/unpause contract functionality (`pause_contract`,
  `unpause_contract`, `ensure_not_paused`, `require_admin`).
- **#78** `mark_shipped` now accepts `tracking_id: String`; returns
  `InvalidTrackingId` (error code 14) if the string is empty.
- **#59** New `get_escrow_count() -> u64` reads the `EscrowCounter` storage
  slot and returns the total number of escrows created.
- **#90** New `cancel_escrow(escrow_id)` allows the seller to cancel an escrow
  in `Pending` state; transitions to `EscrowState::Cancelled`.
- Added `EscrowState::Cancelled` and `ContractError::InvalidTrackingId`.
- Added `tracking_id: Option<String>` field to `EscrowData`.

### `contracts/escrow/src/test_delivery.rs`
- Updated all existing `mark_shipped` calls to pass a non-empty `tracking_id`.
- **#78** Added `test_mark_shipped_rejects_empty_tracking_id`: verifies that an
  empty string returns `InvalidTrackingId` and leaves escrow state unchanged.

### `contracts/escrow/src/test_pause.rs`
- Fixed `initialize` call to pass the required `arbitration_fee` argument.
- Replaced `std::panic::catch_unwind` (incompatible with `#![no_std]`) with the
  Soroban `try_*` client method pattern.

### `contracts/escrow/src/test_cancel_escrow.rs` *(new)*
- **#90** `test_cancel_escrow_by_vendor_in_pending_state`: seller successfully
  cancels an unfunded escrow; state becomes `Cancelled`.
- `test_cancel_escrow_returns_funds_if_buyer_present`: verifies `InvalidState`
  when escrow is already `Funded`.
- `test_cancel_escrow_non_pending_fails`: confirms funded escrows cannot be
  cancelled.

### `docs/FAQ.md` *(new)*
- **#76** Contributor FAQ covering local test commands, snapshot regeneration,
  common compile errors, sandbox setup, and PR workflow.

### `contracts/escrow/test_snapshots/`
- Regenerated all snapshot JSON files to reflect the new `tracking_id` field
  in `EscrowData` and updated `mark_shipped` call signatures.

## Test plan

- [x] `cargo test --manifest-path contracts/escrow/Cargo.toml` — 51 tests pass,
  0 failures.
- [x] `test_mark_shipped_rejects_empty_tracking_id` confirms `InvalidTrackingId`
  error and `Funded` state preserved.
- [x] `test_cancel_escrow_by_vendor_in_pending_state` confirms `Cancelled` state
  after vendor cancellation.
- [x] `test_pause_blocks_mutations_but_keeps_views_available` passes with the
  `try_*`-based rewrite.
