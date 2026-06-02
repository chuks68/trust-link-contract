#![cfg(test)]

use crate::ContractError;
use crate::test_helpers::setup_contract;
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol, String as SorobanString, BytesN};

const MISSING_ID: u64 = 999_999;

#[test]
fn test_get_escrow_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let res = client.try_get_escrow(&MISSING_ID);
    assert!(matches!(res, Err(Ok(ContractError::EscrowNotFound))));
}

#[test]
fn test_fund_escrow_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let buyer = Address::generate(&env);
    let res = client.try_fund_escrow(&MISSING_ID, &buyer);
    assert!(matches!(res, Err(Ok(ContractError::EscrowNotFound))));
}

#[test]
fn test_mark_shipped_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let seller = Address::generate(&env);
    let res = client.try_mark_shipped(&seller, &MISSING_ID, &SorobanString::from_str(&env, "TRACK"));
    assert!(matches!(res, Err(Ok(ContractError::EscrowNotFound))));
}

#[test]
fn test_confirm_delivery_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let buyer = Address::generate(&env);
    let res = client.try_confirm_delivery(&buyer, &MISSING_ID);
    assert!(matches!(res, Err(Ok(ContractError::EscrowNotFound))));
}

#[test]
fn test_raise_dispute_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let buyer = Address::generate(&env);
    let res = client.try_raise_dispute(
        &buyer,
        &MISSING_ID,
        &Symbol::new(&env, "reason"),
        &SorobanString::from_str(&env, "desc"),
        &BytesN::from_array(&env, &[0u8; 32]),
    );
    assert!(matches!(res, Err(Ok(ContractError::EscrowNotFound))));
}

#[test]
fn test_resolve_dispute_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let resolver = Address::generate(&env);
    let res = client.try_resolve_dispute(&resolver, &MISSING_ID, &crate::ResolutionType::Release);
    assert!(matches!(res, Err(Ok(ContractError::EscrowNotFound))));
}
