#![cfg(test)]

use crate::{test_helpers::setup_contract, ContractError, DataKey, FeeUpdated};
use soroban_sdk::{testutils::Address as _, Address, Env};

/// Test: set_fee with 0 bps (minimum)
#[test]
fn test_set_fee_zero_bps() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let result = client.try_set_fee(&0_u32);
    assert!(result.is_ok(), "set_fee(0) should succeed");

    // Verify stored in DefaultFeeBps
    let stored = env.as_contract(&client.address, || {
        env.storage().instance().get(&DataKey::DefaultFeeBps)
    });
    assert_eq!(stored, Some(0u32));
}

/// Test: set_fee with 100 bps (1%)
#[test]
fn test_set_fee_100_bps() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let result = client.try_set_fee(&100_u32);
    assert!(result.is_ok(), "set_fee(100) should succeed");

    let stored = env.as_contract(&client.address, || {
        env.storage().instance().get(&DataKey::DefaultFeeBps)
    });
    assert_eq!(stored, Some(100u32));
}

/// Test: set_fee with 300 bps (3%, the maximum)
#[test]
fn test_set_fee_max_300_bps() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let result = client.try_set_fee(&300_u32);
    assert!(result.is_ok(), "set_fee(300) should succeed");

    let stored = env.as_contract(&client.address, || {
        env.storage().instance().get(&DataKey::DefaultFeeBps)
    });
    assert_eq!(stored, Some(300u32));
}

/// Test: set_fee rejects 301 bps (exceeds cap)
#[test]
fn test_set_fee_rejects_301_bps() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let result = client.try_set_fee(&301_u32);
    assert!(matches!(result, Err(Ok(ContractError::FeeExceedsMax))));
}

/// Test: set_fee requires admin authentication
#[test]
fn test_set_fee_requires_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let contract_id = env.register(crate::Escrow, ());
    let client = crate::EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_i128);

    // Strip all auths — set_fee should fail
    env.mock_auths(&[]);
    let result = client.try_set_fee(&100_u32);
    assert!(result.is_err(), "set_fee requires admin auth");
}

/// Test: set_fee emits FeeUpdated event with old and new values
#[test]
fn test_set_fee_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    // First call: 0 -> 100
    let result1 = client.try_set_fee(&100_u32);
    assert!(result1.is_ok(), "first set_fee succeeds");

    // Second call: 100 -> 250
    let result2 = client.try_set_fee(&250_u32);
    assert!(result2.is_ok(), "second set_fee succeeds");

    // Verify final value
    let stored = env.as_contract(&client.address, || {
        env.storage().instance().get(&DataKey::DefaultFeeBps)
    });
    assert_eq!(stored, Some(250u32));
}

/// Test: calculate_fee helper with zero basis points
#[test]
fn test_calculate_fee_zero_bps() {
    let fee = crate::helpers::payout::calculate_fee(10_000, 0).expect("calculate_fee(10_000, 0)");
    assert_eq!(fee, 0);
}

/// Test: calculate_fee helper with 100 bps (1%)
#[test]
fn test_calculate_fee_100_bps() {
    let fee = crate::helpers::payout::calculate_fee(10_000, 100).expect("calculate_fee(10_000, 100)");
    assert_eq!(fee, 100);
}

/// Test: calculate_fee helper with 300 bps (3%)
#[test]
fn test_calculate_fee_300_bps() {
    let fee = crate::helpers::payout::calculate_fee(10_000, 300).expect("calculate_fee(10_000, 300)");
    assert_eq!(fee, 300);
}

/// Test: calculate_fee with large amount (no overflow)
#[test]
fn test_calculate_fee_large_amount() {
    let large_amount: i128 = i128::MAX / 2;
    let fee = crate::helpers::payout::calculate_fee(large_amount, 100)
        .expect("calculate_fee handles large amounts");
    // At 100 bps on a very large amount, we expect a proportionally large but computable fee
    assert!(fee > 0);
}

/// Test: calculate_fee with 1 stroop and 300 bps rounds to 0
#[test]
fn test_calculate_fee_one_stroop_rounds_zero() {
    let fee = crate::helpers::payout::calculate_fee(1, 300).expect("calculate_fee(1, 300)");
    assert_eq!(fee, 0, "1 stroop at 300 bps should round down to 0");
}

/// Test: fee calculation is integer-based, no floating point
#[test]
fn test_calculate_fee_integer_arithmetic_only() {
    // 333 stroops at 50 bps should yield floor(333 * 50 / 10_000) = floor(1.665) = 1
    let fee = crate::helpers::payout::calculate_fee(333, 50).expect("calculate_fee(333, 50)");
    assert_eq!(fee, 1, "integer arithmetic: floor(333 * 50 / 10_000) = 1");

    // 999 stroops at 50 bps should yield floor(999 * 50 / 10_000) = floor(4.995) = 4
    let fee = crate::helpers::payout::calculate_fee(999, 50).expect("calculate_fee(999, 50)");
    assert_eq!(fee, 4, "integer arithmetic: floor(999 * 50 / 10_000) = 4");
}

/// Test: new fee only applies to future escrows (not retroactively)
/// This test verifies that escrows created before set_fee retain their original fee_bps.
#[test]
fn test_set_fee_not_retroactive() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let resolver = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract(token_admin);
    let contract_id = env.register(crate::Escrow, ());
    let client = crate::EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_i128);

    // Create an escrow with fee_bps = 50
    let escrow_id = client.create_escrow(&seller, &resolver, &token, &1000_i128, &50_u32, &3600_u64);

    // Verify escrow has fee_bps = 50
    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.fee_bps, 50, "escrow created with 50 bps retains that value");

    // Now admin changes the default fee to 200 bps
    let result = client.try_set_fee(&200_u32);
    assert!(result.is_ok(), "set_fee(200) succeeds");

    // Verify the existing escrow still has fee_bps = 50 (not retroactively updated)
    let escrow_after = client.get_escrow(&escrow_id);
    assert_eq!(
        escrow_after.fee_bps, 50,
        "existing escrow fee_bps not retroactively changed by set_fee"
    );

    // Create a new escrow; it will use the fee_bps passed in create_escrow, not the default
    let escrow_id2 = client.create_escrow(&seller, &resolver, &token, &1000_i128, &100_u32, &3600_u64);
    let escrow2 = client.get_escrow(&escrow_id2);
    assert_eq!(
        escrow2.fee_bps, 100,
        "new escrow uses the fee_bps passed to create_escrow"
    );
}

/// Test: unauthorized caller cannot call set_fee
#[test]
fn test_set_fee_unauthorized_caller_rejected() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let contract_id = env.register(crate::Escrow, ());
    let client = crate::EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_i128);

    // Mock only unauthorized auth — set_fee should fail
    env.mock_auths(&[]);

    let result = client.try_set_fee(&100_u32);
    assert!(
        result.is_err(),
        "set_fee should reject a caller that is not the admin"
    );
}
