#![cfg(test)]

use crate::test_helpers::{create_funded_escrow, setup_contract};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn register_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract(token_admin)
}

#[test]
fn test_escrow_stored_in_persistent_storage() {
    let env = Env::default();
    env.mock_all_auths();

    let token = register_token(&env);
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);

    let id = create_funded_escrow(
        &env, &client, &seller, &buyer, &resolver, &token, 1000, 100, 3600,
    );

    // Escrow is readable after funding (persistent storage works)
    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.amount, 1000);
}

#[test]
fn test_set_ttl_extension_persists() {
    let env = Env::default();
    env.mock_all_auths();

    let token = register_token(&env);
    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    // Configure custom TTL extension
    client.set_ttl_extension(&admin, &60_480_u32);

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);

    // Escrow operations still work with custom TTL
    let id = create_funded_escrow(
        &env, &client, &seller, &buyer, &resolver, &token, 500, 0, 3600,
    );
    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.amount, 500);
}

#[test]
fn test_dispute_stored_in_persistent_storage() {
    let env = Env::default();
    env.mock_all_auths();

    let token = register_token(&env);
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);

    let id = create_funded_escrow(
        &env, &client, &seller, &buyer, &resolver, &token, 1000, 100, 3600,
    );
    client.mark_shipped(&seller, &id, &soroban_sdk::String::from_str(&env, "TRACK-TTL"));

    client.raise_dispute(
        &buyer,
        &id,
        &soroban_sdk::Symbol::new(&env, "test"),
        &soroban_sdk::String::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0xab; 32]),
    );

    // Dispute readable from persistent storage
    let dispute = client.get_dispute(&id);
    assert!(dispute.is_some());
    let dispute = dispute.unwrap();
    assert_eq!(dispute.escrow_id, id);
}
