#![cfg(test)]

use crate::{Escrow, EscrowClient, DisputeStatus};
use soroban_sdk::{testutils::Address as _, token, Address, Env, Symbol, String, BytesN};

fn setup_env() -> (Env, Address, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(token_admin.clone());

    (env, admin, seller, buyer, resolver, token_address, fee_collector)
}

#[test]
fn test_get_dispute_returns_accurate_data_after_raise() {
    let (env, admin, seller, buyer, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);

    client.initialize(&admin, &fee_collector);

    let amount = 1000_i128;
    let id = client.create_escrow(&seller, &resolver, &token, &amount, &100_u32, &3600_u64);
    
    let sac = token::StellarAssetClient::new(&env, &token);
    sac.mint(&buyer, &amount);
    
    client.fund_escrow(&id, &buyer);

    let reason = Symbol::new(&env, "non_delivery");
    let description = String::from_str(&env, "Item never arrived");
    let evidence_hash = BytesN::from_array(&env, &[0xab; 32]);
    let timestamp = env.ledger().timestamp();

    client.raise_dispute(&id, &reason, &description, &evidence_hash);
    
    let result = client.get_dispute(&id);
    
    assert_eq!(result.escrow_id, id);
    assert_eq!(result.reason, reason);
    assert_eq!(result.description, description);
    assert_eq!(result.evidence_hash, evidence_hash);
    assert_eq!(result.status, DisputeStatus::Active);
    assert!(result.raised_at >= timestamp);
}

#[test]
#[should_panic(expected = "dispute not found")]
fn test_get_dispute_non_existent_id() {
    let (env, admin, _seller, _buyer, _resolver, _token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector);

    client.get_dispute(&999);
}
