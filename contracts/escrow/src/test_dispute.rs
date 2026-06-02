#![cfg(test)]

use crate::{DisputeStatus, Escrow, EscrowClient};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, token, Address, BytesN, Env, String, Symbol};

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

    (
        env,
        admin,
        seller,
        buyer,
        resolver,
        token_address,
        fee_collector,
    )
}

#[test]
fn test_get_dispute_returns_accurate_data_after_raise() {
    let (env, admin, seller, buyer, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);

    client.initialize(&admin, &fee_collector, &0_u32);

    let amount = 1000_i128;
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &amount, &100_u32, &3600_u64);

    let sac = token::StellarAssetClient::new(&env, &token);
    sac.mint(&buyer, &amount);

    client.fund_escrow(&id, &buyer);

    let reason = Symbol::new(&env, "non_delivery");
    let description = String::from_str(&env, "Item never arrived");
    let evidence_hash = BytesN::from_array(&env, &[0xab; 32]);
    let timestamp = env.ledger().timestamp();

    client.mark_shipped(&seller, &id, &String::from_str(&env, "TRACK-001"));
    client.raise_dispute(&buyer, &id, &reason, &description, &evidence_hash);

    let result = client.get_dispute(&id);

    assert!(result.is_some());
    let dispute = result.unwrap();
    assert_eq!(dispute.escrow_id, id);
    assert_eq!(dispute.reason, reason);
    assert_eq!(dispute.description, description);
    assert_eq!(dispute.evidence_hash, evidence_hash);
    assert_eq!(dispute.status, DisputeStatus::Active);
    assert!(dispute.disputed_at >= timestamp);
}

#[test]
fn test_get_dispute_returns_none_when_no_dispute_exists() {
    let (env, admin, _seller, _buyer, _resolver, _token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    // Query for a dispute on an escrow ID that has no dispute
    let result = client.get_dispute(&999);
    assert_eq!(result, None);
}

// Verify disputes can be opened once the escrow has reached Shipped.
#[test]
fn test_dispute_allowed_after_shipping() {
    let (env, admin, seller, buyer, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(crate::Escrow, ());
    let client = crate::EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    let amount = 1000_i128;
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &amount, &100_u32, &3600_u64);
    
    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    sac.mint(&buyer, &amount);
    
    // Use fixed deterministic timestamp
    env.ledger().set_timestamp(1_700_000_000);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &String::from_str(&env, "TRACK-BOUNDARY"));
    
    env.ledger().set_timestamp(1_700_172_798);

    let reason = soroban_sdk::Symbol::new(&env, "reason");
    let description = soroban_sdk::String::from_str(&env, "desc");
    let evidence_hash = soroban_sdk::BytesN::from_array(&env, &[0xab; 32]);
    
    client.raise_dispute(&buyer, &id, &reason, &description, &evidence_hash);
    let result = client.get_dispute(&id);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.status, crate::DisputeStatus::Active);
}

// Verify disputes are still accepted on a later shipped escrow timestamp.
#[test]
fn test_dispute_allowed_on_late_shipped_escrow() {
    let (env, admin, seller, buyer, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(crate::Escrow, ());
    let client = crate::EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    let amount = 1000_i128;
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &amount, &100_u32, &3600_u64);
    
    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    sac.mint(&buyer, &amount);
    
    // Use fixed deterministic timestamp
    env.ledger().set_timestamp(1_700_000_000);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &String::from_str(&env, "TRACK-LATE"));
    env.ledger().set_timestamp(1_700_172_799);

    let reason = soroban_sdk::Symbol::new(&env, "reason");
    let description = soroban_sdk::String::from_str(&env, "desc");
    let evidence_hash = soroban_sdk::BytesN::from_array(&env, &[0xab; 32]);
    client.raise_dispute(&buyer, &id, &reason, &description, &evidence_hash);
    let result = client.get_dispute(&id);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.status, crate::DisputeStatus::Active);
}

// Verify disputes require the escrow to be shipped.
#[test]
fn test_dispute_requires_shipped_state() {
    let (env, admin, seller, buyer, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(crate::Escrow, ());
    let client = crate::EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    let amount = 1000_i128;
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &amount, &100_u32, &3600_u64);
    
    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    sac.mint(&buyer, &amount);
    client.fund_escrow(&id, &buyer);
    
    let escrow = client.get_escrow(&id);
    let funded_at = escrow.funded_at; // Should be 1_700_000_000
    
    // T + 172800 seconds (48:00:00) - EXACTLY 48 HOURS
    env.ledger().set_timestamp(funded_at + 172_800);

    let reason = soroban_sdk::Symbol::new(&env, "reason");
    let description = soroban_sdk::String::from_str(&env, "desc");
    let evidence_hash = soroban_sdk::BytesN::from_array(&env, &[0xab; 32]);
    
    let result = client.try_raise_dispute(&buyer, &id, &reason, &description, &evidence_hash);
    assert_eq!(result, Err(Ok(crate::ContractError::InvalidState)));
    
    // Verify no state mutation on expired action
    let escrow_after = client.get_escrow(&id);
    assert_eq!(escrow_after.state, crate::EscrowState::Funded);
}

// Verify dispute actions remain invalid after the 48-hour deadline passes.
#[test]
fn test_dispute_rejected_after_48h_deadline() {
    let (env, admin, seller, buyer, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(crate::Escrow, ());
    let client = crate::EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    let amount = 1000_i128;
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &amount, &100_u32, &3600_u64);
    
    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    sac.mint(&buyer, &amount);
    
    // Use fixed deterministic timestamp
    env.ledger().set_timestamp(1_700_000_000);
    client.fund_escrow(&id, &buyer);
    
    let escrow = client.get_escrow(&id);
    let funded_at = escrow.funded_at; // Should be 1_700_000_000
    
    // T + 172801 seconds - AFTER DEADLINE
    env.ledger().set_timestamp(funded_at + 172_801);

    let reason = soroban_sdk::Symbol::new(&env, "reason");
    let description = soroban_sdk::String::from_str(&env, "desc");
    let evidence_hash = soroban_sdk::BytesN::from_array(&env, &[0xab; 32]);
    
    let result = client.try_raise_dispute(&buyer, &id, &reason, &description, &evidence_hash);
    assert_eq!(result, Err(Ok(crate::ContractError::InvalidState)));
    
    // Verify no state mutation on expired action
    let escrow_after = client.get_escrow(&id);
    assert_eq!(escrow_after.state, crate::EscrowState::Funded);
}
