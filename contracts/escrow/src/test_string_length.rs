#![cfg(test)]

use crate::{ContractError, MAX_TRACKING_ID_LEN, MAX_DESCRIPTION_LEN};
use crate::test_helpers::{setup_contract, create_funded_escrow};
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Bytes, Env, String as SorobanString, Symbol, BytesN};

fn register_token(env: &Env) -> Address {
    env.register_stellar_asset_contract(Address::generate(env))
}

/// Build a SorobanString of exactly `len` ASCII 'a' characters (max 512).
fn make_string(env: &Env, len: u32) -> SorobanString {
    assert!(len <= 512);
    let buf = [b'a'; 512];
    let slice = &buf[..(len as usize)];
    SorobanString::from_bytes(env, slice)
}

// ── tracking_id ──────────────────────────────────────────────────────────────

#[test]
fn test_tracking_id_at_limit_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let token = register_token(&env);
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let id = create_funded_escrow(&env, &client, &seller, &buyer, &resolver, &token, 100, 0, 3600);
    // Exactly MAX_TRACKING_ID_LEN characters — must succeed
    let tracking = make_string(&env, MAX_TRACKING_ID_LEN);
    client.mark_shipped(&seller, &id, &tracking);
    
    // Verify the full boundary string is recorded precisely as intended
    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.tracking_id, Some(tracking));
}

#[test]
fn test_tracking_id_over_limit_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let token = register_token(&env);
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let id = create_funded_escrow(&env, &client, &seller, &buyer, &resolver, &token, 100, 0, 3600);
    // One character over the limit — must revert
    let tracking = make_string(&env, MAX_TRACKING_ID_LEN + 1);
    let res = client.try_mark_shipped(&seller, &id, &tracking);
    assert!(matches!(res, Err(Ok(ContractError::InputTooLong))));
}

// ── description ──────────────────────────────────────────────────────────────

#[test]
fn test_description_at_limit_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let token = register_token(&env);
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let id = create_funded_escrow(&env, &client, &seller, &buyer, &resolver, &token, 100, 0, 3600);
    // Exactly MAX_DESCRIPTION_LEN characters — must succeed
    let desc = make_string(&env, MAX_DESCRIPTION_LEN);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-DESC"));
    client.raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "reason"),
        &desc,
        &BytesN::from_array(&env, &[0u8; 32]),
    );
}

#[test]
fn test_description_over_limit_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let token = register_token(&env);
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let id = create_funded_escrow(&env, &client, &seller, &buyer, &resolver, &token, 100, 0, 3600);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-DESC"));
    // One character over the limit — must revert
    let desc = make_string(&env, MAX_DESCRIPTION_LEN + 1);
    let res = client.try_raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "reason"),
        &desc,
        &BytesN::from_array(&env, &[0u8; 32]),
    );
    assert!(matches!(res, Err(Ok(ContractError::InputTooLong))));
}
