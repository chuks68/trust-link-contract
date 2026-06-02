#![cfg(test)]
//! Systematic unauthorized-access tests for every admin-gated state-mutating
//! entry point (#19).
//!
//! Each test invokes the entry point with a non-admin caller and asserts the
//! `try_*` client method returns an error. This validates the contract's
//! `caller != admin → NotAuthorized` guard, not just the host's `require_auth`
//! reject path.

use crate::{ContractError, Escrow, EscrowClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

/// Fresh contract with admin/fee_collector initialised. All auths are mocked
/// so tests can drive the API freely; each test then exercises authorization
/// via the explicit caller arg rather than by toggling mocks.
fn fresh_contract(env: &Env) -> (EscrowClient<'static>, Address) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let fee_collector = Address::generate(env);
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);
    (client, admin)
}

#[test]
fn pause_contract_rejects_unauthorized_caller() {
    let env = Env::default();
    let (client, _admin) = fresh_contract(&env);
    let intruder = Address::generate(&env);
    assert_eq!(
        client.try_pause_contract(&intruder),
        Err(Ok(ContractError::NotAuthorized)),
    );
}

#[test]
fn unpause_contract_rejects_unauthorized_caller() {
    let env = Env::default();
    let (client, admin) = fresh_contract(&env);
    client.pause_contract(&admin);

    let intruder = Address::generate(&env);
    assert_eq!(
        client.try_unpause_contract(&intruder),
        Err(Ok(ContractError::NotAuthorized)),
    );
}

#[test]
fn set_admin_rejects_unauthorized_caller() {
    // set_admin reads the current admin from storage and requires its auth.
    // With no mocked auths the host-level check fails and the call errors.
    let env = Env::default();
    let (client, _admin) = fresh_contract(&env);
    let new_admin = Address::generate(&env);

    env.mock_auths(&[]);
    assert!(client.try_set_admin(&new_admin).is_err());
}

#[test]
fn set_fee_rejects_unauthorized_caller() {
    let env = Env::default();
    let (client, _admin) = fresh_contract(&env);
    let intruder = Address::generate(&env);
    assert_eq!(
        client.try_set_fee(&intruder, &100_u32),
        Err(Ok(ContractError::NotAuthorized)),
    );
}

#[test]
fn set_ttl_extension_rejects_unauthorized_caller() {
    let env = Env::default();
    let (client, _admin) = fresh_contract(&env);
    let intruder = Address::generate(&env);
    assert_eq!(
        client.try_set_ttl_extension(&intruder, &60_u32),
        Err(Ok(ContractError::NotAuthorized)),
    );
}

#[test]
fn set_arbitration_fee_rejects_unauthorized_caller() {
    let env = Env::default();
    let (client, _admin) = fresh_contract(&env);
    let intruder = Address::generate(&env);
    assert_eq!(
        client.try_set_arbitration_fee(&intruder, &10_u32),
        Err(Ok(ContractError::NotAuthorized)),
    );
}

#[test]
fn withdraw_fees_rejects_unauthorized_caller() {
    let env = Env::default();
    let (client, _admin) = fresh_contract(&env);
    let intruder = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_addr = sac.address();
    let recipient = Address::generate(&env);

    assert_eq!(
        client.try_withdraw_fees(&intruder, &token_addr, &recipient, &1_i128),
        Err(Ok(ContractError::NotAuthorized)),
    );
}
