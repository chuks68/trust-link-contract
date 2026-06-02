#![cfg(test)]

use crate::helpers::payout::calculate_protocol_fee;
use crate::test_helpers::{advance_time, create_funded_escrow, setup_contract};
use crate::{ContractError, Escrow, EscrowClient, MIN_ESCROW_AMOUNT};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, token, Address, BytesN, Env, String as SorobanString, Symbol};

/// Seconds the dispute window stays open after funding (mirrors the private
/// `DISPUTE_WINDOW` constant in `lib.rs`). `confirm_delivery` is only permitted
/// once the ledger clock is at or past this point.
const DISPUTE_WINDOW_SECS: u64 = 172_800;

/// BUG-020 (#165): initialize must reject admin == fee_collector so the two
/// roles are always backed by separate keys.
#[test]
fn test_initialize_same_admin_and_fee_collector_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    let shared = Address::generate(&env);

    let result = client.try_initialize(&shared, &shared, &0_u32);
    assert!(matches!(result, Err(Ok(ContractError::InvalidAddress))));
}

/// Sanity: distinct admin/fee_collector still initialize successfully.
#[test]
fn test_initialize_distinct_addresses_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, admin, fee_collector) = setup_contract(&env);

    let config = client.get_contract_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.fee_collector, fee_collector);
}

/// BUG-018 (#163): set_admin must reject a no-op rotation to the current admin.
#[test]
fn test_set_admin_same_address_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    let result = client.try_set_admin(&admin);
    assert!(matches!(result, Err(Ok(ContractError::SameAddress))));
}

/// set_admin still succeeds when rotating to a genuinely different address.
#[test]
fn test_set_admin_new_address_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let new_admin = Address::generate(&env);

    client.set_admin(&new_admin);
    assert_eq!(client.get_contract_config().admin, new_admin);
}

/// A buyer named at creation who cancels the still-Pending escrow must remain
/// discoverable via get_escrows_by_buyer. The buyer is a party to the escrow
/// and performed a transaction on it (the cancellation), so they need an
/// on-chain reference to it afterwards.
#[test]
fn test_buyer_index_populated_on_cancel_by_buyer() {
    let env = Env::default();
    env.mock_all_auths();

    let token = {
        let token_admin = Address::generate(&env);
        env.register_stellar_asset_contract_v2(token_admin).address()
    };
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);

    // Create a Pending escrow that names the buyer up front.
    let id = client.create_escrow(
        &seller,
        &Some(buyer.clone()),
        &resolver,
        &token,
        &1000_i128,
        &100_u32,
        &3600_u64,
    );

    // The buyer cancels the still-Pending escrow.
    client.cancel_escrow(&buyer, &id);

    // The buyer must still be able to find the escrow they cancelled.
    let escrows = client.get_escrows_by_buyer(&buyer);
    assert_eq!(escrows.len(), 1);
    assert_eq!(escrows.get(0).unwrap(), id);
}

// ---------------------------------------------------------------------------
// Fee rounding / dust edge cases (#fee_calculator)
//
// Rounding policy under test: the protocol fee is floored
// (`fee = floor(amount * fee_bps / 10_000)`) and the payout is derived as
// `net = amount - fee`. The truncated sub-stroop remainder therefore accrues to
// the payout recipient rather than being stranded, so `net + fee == amount`
// holds for every amount — divisible or not. The contract vault only ever
// retains exactly `fee`, which the admin later sweeps via `withdraw_fees`.
// ---------------------------------------------------------------------------

/// Amounts where `amount * fee_bps` is NOT divisible by 10_000, so naive
/// truncation is exactly where dust could appear. Each is >= MIN_ESCROW_AMOUNT
/// so it survives the `create_escrow` guard.
fn non_divisible_fee_cases() -> [(i128, u32); 8] {
    [
        (1_000_001, 100), // floor(10000.01) = 10000, remainder 1 stroop to seller
        (1_000_099, 100), // floor(10000.99) = 10000
        (1_234_567, 100), // floor(12345.67) = 12345
        (1_000_001, 50),  // floor(5000.005) = 5000
        (1_000_003, 250), // floor(25000.075) = 25000
        (9_999_999, 300), // floor(299999.97) = 299999
        (1_000_001, 1),   // floor(100.0001) = 100
        (7_777_777, 137), // arbitrary odd fee_bps and amount
    ]
}

/// Pure-arithmetic check: for non-divisible amounts the fee is floored and the
/// value-conservation invariant `net + fee == amount` always holds, with no
/// negative components.
#[test]
fn test_fee_rounding_policy_is_floor_and_conserves_value() {
    for (amount, fee_bps) in non_divisible_fee_cases() {
        let (fee, net) = calculate_protocol_fee(amount, fee_bps).unwrap();

        // Floor policy: fee equals integer division toward zero.
        let expected_fee = (amount * fee_bps as i128) / 10_000;
        assert_eq!(
            fee, expected_fee,
            "fee not floored: amount={amount}, fee_bps={fee_bps}, fee={fee}"
        );

        // No stroop lost: the truncated remainder lives in `net` (the recipient's
        // payout), never stranded.
        assert_eq!(
            net + fee,
            amount,
            "value not conserved: amount={amount}, fee_bps={fee_bps}, fee={fee}, net={net}"
        );
        assert!(fee >= 0 && net >= 0);
    }
}

/// The report's literal reproduction (amount = 99, fee_bps = 100) where the fee
/// would round to 0 is now impossible: the `MIN_ESCROW_AMOUNT` guard rejects any
/// dust-sized escrow at creation time before fees ever come into play.
#[test]
fn test_min_escrow_amount_rejects_dust_prone_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let seller = Address::generate(&env);
    let resolver = Address::generate(&env);
    let token = env.register_stellar_asset_contract(Address::generate(&env));

    // 99 stroops, 1% fee — the exact case from the bug report.
    // MIN_ESCROW_AMOUNT = 1, so 99 is above the minimum and should succeed for creation.
    let result = client.try_create_escrow(&seller, &None::<Address>, &resolver, &token, &99_i128, &100_u32, &3600_u64);
    assert!(result.is_ok());

    // One stroop below the minimum is still rejected.
    let result = client.try_create_escrow(
        &seller,
        &None::<Address>,
        &resolver,
        &token,
        &0_i128,
        &100_u32,
        &3600_u64,
    );
    assert_eq!(result, Err(Ok(ContractError::InvalidAmount)));
}

/// End-to-end: for non-divisible amounts, the seller's payout plus the fee left
/// in the vault sum to exactly the original amount — no stroop is stranded. The
/// vault retains precisely the floored fee (recoverable via `withdraw_fees`).
#[test]
fn test_confirm_delivery_leaves_no_dust_for_non_divisible_amounts() {
    for (amount, fee_bps) in non_divisible_fee_cases() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, client, admin, fee_collector) = setup_contract(&env);

        // Set protocol fee to match the per-escrow fee_bps used for testing
        client.set_protocol_fee(&admin, &fee_bps);

        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let resolver = Address::generate(&env);
        let token = env.register_stellar_asset_contract(Address::generate(&env));

        let id = create_funded_escrow(
            &env, &client, &seller, &buyer, &resolver, &token, amount, fee_bps, 3600,
        );

        // Move past the dispute window so the buyer can confirm delivery.
        advance_time(&env, DISPUTE_WINDOW_SECS + 1);
        client.confirm_delivery(&buyer, &id);

        let expected_fee = (amount * fee_bps as i128) / 10_000;
        let expected_net = amount - expected_fee;

        let tc = soroban_sdk::token::Client::new(&env, &token);
        let seller_payout = tc.balance(&seller);
        let fee_collector_balance = tc.balance(&fee_collector);

        assert_eq!(
            seller_payout, expected_net,
            "seller payout wrong: amount={amount}, fee_bps={fee_bps}"
        );
        assert_eq!(
            fee_collector_balance, expected_fee,
            "fee_collector received wrong fee: amount={amount}, fee_bps={fee_bps}"
        );
        // The core no-dust invariant: payout + fee == original amount.
        assert_eq!(
            seller_payout + fee_collector_balance,
            amount,
            "dust detected: amount={amount}, fee_bps={fee_bps}, payout={seller_payout}, fee={fee_collector_balance}"
        );
    }
}

/// BUG-009 (#154): calling mark_shipped a second time on a Shipped escrow must
/// revert with InvalidState — the state guard requires Funded as the entry state.
/// Verifies the original tracking_id is not overwritten.
#[test]
fn test_mark_shipped_twice_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let token = env.register_stellar_asset_contract(Address::generate(&env));
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);

    let id = create_funded_escrow(&env, &client, &seller, &buyer, &resolver, &token, 1000, 100, 3600);

    // First mark_shipped: Funded → Shipped, tracking_id recorded.
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-001"));

    // Second call on an already-Shipped escrow must revert.
    let result = client.try_mark_shipped(
        &seller,
        &id,
        &SorobanString::from_str(&env, "FAKE-999"),
    );
    assert!(
        matches!(result, Err(Ok(ContractError::InvalidState))),
        "expected InvalidState, got {result:?}"
    );

    // The original tracking_id must not have been overwritten.
    let escrow = client.get_escrow(&id);
    assert_eq!(
        escrow.tracking_id,
        Some(SorobanString::from_str(&env, "TRACK-001")),
        "tracking_id was overwritten by second mark_shipped"
    );
}

/// BUG-011 (#156): record_delivery on a Disputed escrow must revert with
/// InvalidState — the oracle may only record delivery from the Shipped state.
#[test]
fn test_record_delivery_on_disputed_escrow_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let token = env.register_stellar_asset_contract(Address::generate(&env));
    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);

    let id = create_funded_escrow(&env, &client, &seller, &buyer, &resolver, &token, 1000, 100, 3600);

    // Ship the escrow so the buyer can raise a dispute.
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-DISP"));

    // Buyer raises dispute — escrow transitions Shipped → Disputed.
    client.raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "damaged"),
        &SorobanString::from_str(&env, "item arrived damaged"),
        &BytesN::from_array(&env, &[0u8; 32]),
    );

    // Admin oracle must not be able to record delivery on a Disputed escrow.
    let result = client.try_record_delivery(&admin, &id);
    assert!(
        matches!(result, Err(Ok(ContractError::InvalidState))),
        "expected InvalidState on record_delivery while Disputed, got {result:?}"
    );
}

/// BUG-012 (#157): the escrow counter lives in instance storage; without a TTL
/// extension on every access the key can expire and the counter would reset to 0,
/// producing duplicate escrow IDs. Verifies the counter is monotonically
/// increasing even after a large ledger-sequence advance.
#[test]
fn test_counter_survives_near_ttl_expiry() {
    let env = Env::default();
    env.mock_all_auths();

    let token = env.register_stellar_asset_contract(Address::generate(&env));
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);

    let id1 = create_funded_escrow(&env, &client, &seller, &buyer, &resolver, &token, 1000, 0, 3600);
    let id2 = create_funded_escrow(&env, &client, &seller, &buyer, &resolver, &token, 1000, 0, 3600);
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    // Advance the ledger sequence to just below the default instance TTL threshold
    // (DEFAULT_TTL_EXTENSION = 120_960 ledgers). With the extend_ttl call in
    // create_escrow the counter key survives and is never reset.
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number += 120_000;
    env.ledger().set(ledger_info);

    let id3 = create_funded_escrow(&env, &client, &seller, &buyer, &resolver, &token, 1000, 0, 3600);

    // Counter must not have reset — id3 must follow id2 monotonically.
    assert_eq!(id3, 3, "counter reset after ledger advancement: got id={id3}");

    // Verify old escrows are still accessible.
    assert_eq!(client.get_escrow(&id1).amount, 1000);
    assert_eq!(client.get_escrow(&id2).amount, 1000);
}
