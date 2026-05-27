#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, token, Address, Env, String as SorobanString, Symbol};

fn base_env() -> (Env, Address, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract(token_admin);
    token::StellarAssetClient::new(&env, &token).mint(&buyer, &10_000);
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_i128);
    (env, admin, seller, buyer, resolver, token, contract_id)
}

#[test]
fn test_create_escrow_blocked_when_paused() {
    let (env, _admin, seller, _buyer, resolver, token, contract_id) = base_env();
    let client = EscrowClient::new(&env, &contract_id);
    client.pause_contract();
    let result = client.try_create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_fund_escrow_blocked_when_paused() {
    let (env, _admin, seller, buyer, resolver, token, contract_id) = base_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    client.pause_contract();
    let result = client.try_fund_escrow(&id, &buyer);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_mark_shipped_blocked_when_paused() {
    let (env, _admin, seller, buyer, resolver, token, contract_id) = base_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.pause_contract();
    let result = client.try_mark_shipped(&id);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_confirm_delivery_blocked_when_paused() {
    let (env, _admin, seller, buyer, resolver, token, contract_id) = base_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    env.ledger().set_timestamp(DISPUTE_WINDOW + 1);
    client.pause_contract();
    let result = client.try_confirm_delivery(&id);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_raise_dispute_blocked_when_paused() {
    let (env, _admin, seller, buyer, resolver, token, contract_id) = base_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.pause_contract();
    let hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    let result = client.try_raise_dispute(
        &id,
        &Symbol::new(&env, "fraud"),
        &SorobanString::from_str(&env, "desc"),
        &hash,
    );
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_resolve_dispute_blocked_when_paused() {
    let (env, _admin, seller, buyer, resolver, token, contract_id) = base_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    let hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    client.raise_dispute(&id, &Symbol::new(&env, "fraud"), &SorobanString::from_str(&env, "desc"), &hash);
    client.pause_contract();
    let result = client.try_resolve_dispute(&id, &ResolutionType::Refund);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_auto_release_blocked_when_paused() {
    let (env, _admin, seller, buyer, resolver, token, contract_id) = base_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &1_u64);
    client.fund_escrow(&id, &buyer);
    env.ledger().set_timestamp(DISPUTE_WINDOW + 10);
    client.pause_contract();
    let result = client.try_auto_release(&id);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_withdraw_fees_blocked_when_paused() {
    let (env, admin, _seller, _buyer, _resolver, token, contract_id) = base_env();
    let client = EscrowClient::new(&env, &contract_id);
    client.pause_contract();
    let result = client.try_withdraw_fees(&token, &admin, &1_i128);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_read_only_views_work_while_paused() {
    let (env, _admin, seller, _buyer, resolver, token, contract_id) = base_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    client.pause_contract();
    let _ = client.get_escrow(&id);
    let _ = client.get_fee_config();
    assert!(client.is_paused());
}

#[test]
fn test_unpause_resumes_operations() {
    let (env, _admin, seller, buyer, resolver, token, contract_id) = base_env();
    let client = EscrowClient::new(&env, &contract_id);
    client.pause_contract();
    client.unpause_contract();
    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    assert_eq!(client.get_escrow(&id).state, EscrowState::Funded);
}
