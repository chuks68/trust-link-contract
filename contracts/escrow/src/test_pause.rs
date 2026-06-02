#![cfg(test)]

use crate::{ContractError, Escrow, EscrowClient, EscrowState, ResolutionType};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, token, Address, Env, String as SorobanString, Symbol};

const DISPUTE_WINDOW: u64 = 172_800;

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
    let contract_id = env.register(Escrow, ());
    {
        let client = EscrowClient::new(&env, &contract_id);
        client.initialize(&admin, &fee_collector, &0_u32);
    }
    (env, admin, seller, buyer, resolver, token_address, contract_id)
}

fn mint_tokens(env: &Env, token: &Address, to: &Address, amount: i128) {
    let sac = token::StellarAssetClient::new(env, token);
    sac.mint(to, &amount);
}

#[test]
fn test_create_escrow_blocked_when_paused() {
    let (env, admin, seller, _buyer, resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);
    client.pause_contract(&admin);
    let result = client.try_create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_fund_escrow_blocked_when_paused() {
    let (env, admin, seller, buyer, resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    client.pause_contract(&admin);
    let result = client.try_fund_escrow(&id, &buyer);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_pause_blocks_mutations_but_keeps_views_available() {
    let (env, admin, seller, buyer, resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    mint_tokens(&env, &token, &buyer, 100);
    client.fund_escrow(&id, &buyer);
    client.pause_contract(&admin);
    let result = client.try_mark_shipped(&seller, &id, &soroban_sdk::String::from_str(&env, "TRACK001"));
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_confirm_delivery_blocked_when_paused() {
    let (env, admin, seller, buyer, resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    mint_tokens(&env, &token, &buyer, 100);
    client.fund_escrow(&id, &buyer);
    env.ledger().set_timestamp(DISPUTE_WINDOW + 1);
    client.pause_contract(&admin);
    let result = client.try_confirm_delivery(&buyer, &id);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_raise_dispute_blocked_when_paused() {
    let (env, admin, seller, buyer, resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    mint_tokens(&env, &token, &buyer, 100);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-PAUSE"));
    client.pause_contract(&admin);
    let hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    let result = client.try_raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "fraud"),
        &SorobanString::from_str(&env, "desc"),
        &hash,
    );
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_resolve_dispute_blocked_when_paused() {
    let (env, admin, seller, buyer, resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    mint_tokens(&env, &token, &buyer, 100);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-PAUSE2"));
    let hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    client.raise_dispute(&buyer, &id, &Symbol::new(&env, "fraud"), &SorobanString::from_str(&env, "desc"), &hash);
    client.pause_contract(&admin);
    let result = client.try_resolve_dispute(&resolver, &id, &ResolutionType::Refund);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_auto_release_blocked_when_paused() {
    let (env, admin, seller, buyer, resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &1_u64);
    mint_tokens(&env, &token, &buyer, 100);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-AR"));
    client.record_delivery(&admin, &id);
    env.ledger().set_timestamp(DISPUTE_WINDOW + 10);
    client.pause_contract(&admin);
    let result = client.try_auto_release(&id);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_withdraw_fees_blocked_when_paused() {
    let (env, admin, _seller, _buyer, _resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);
    client.pause_contract(&admin);
    let result = client.try_withdraw_fees(&admin, &token, &admin, &1_i128);
    assert!(matches!(result, Err(Ok(ContractError::ContractPaused))));
}

#[test]
fn test_read_only_views_work_while_paused() {
    let (env, admin, seller, _buyer, resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    client.pause_contract(&admin);
    let _ = client.get_escrow(&id);
    let _ = client.get_fee_config();
    assert!(client.is_paused());
}

#[test]
fn test_is_paused_reflects_state() {
    let (env, admin, _seller, _buyer, _resolver, _token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);

    assert!(!client.is_paused());

    client.pause_contract(&admin);
    assert!(client.is_paused());

    client.unpause_contract(&admin);
    assert!(!client.is_paused());
}

#[test]
fn test_unpause_resumes_operations() {
    let (env, admin, seller, buyer, resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);
    client.pause_contract(&admin);
    client.unpause_contract(&admin);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    mint_tokens(&env, &token, &buyer, 100);
    client.fund_escrow(&id, &buyer);
    assert_eq!(client.get_escrow(&id).state, EscrowState::Funded);

    mint_tokens(&env, &token, &buyer, 1_000);
    let escrow_id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &100_u32, &3600_u64);
    client.pause_contract(&admin);

    let config = client.get_fee_config();
    assert_eq!(config.protocol_fee_bps, 0);
    assert_eq!(config.arbitration_fee_bps, 0);

    assert!(client.try_withdraw_fees(&admin, &token, &admin, &1_i128).is_err());
    assert!(client.try_create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &100_u32, &3600_u64).is_err());
    assert!(client.try_fund_escrow(&escrow_id, &buyer).is_err());
    assert!(client.try_confirm_delivery(&buyer, &escrow_id).is_err());
    assert!(client
        .try_raise_dispute(
            &buyer,
            &escrow_id,
            &Symbol::new(&env, "reason"),
            &SorobanString::from_str(&env, "desc"),
            &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
        )
        .is_err());
    assert!(client.try_resolve_dispute(&resolver, &escrow_id, &ResolutionType::Release).is_err());
    assert!(client.try_auto_release(&escrow_id).is_err());

    client.unpause_contract(&admin);
    mint_tokens(&env, &token, &buyer, 100);
    let second_id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &50_i128, &50_u32, &3600_u64);
    assert_eq!(second_id, 3);
}
}
