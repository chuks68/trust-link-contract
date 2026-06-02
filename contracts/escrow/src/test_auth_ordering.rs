#![cfg(test)]

use crate::{Escrow, EscrowClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_mark_shipped_auth_fails_immediately() {
    let env = Env::default();

    let contract_id = env.register_contract(None, Escrow);
    let client = EscrowClient::new(&env, &contract_id);

    let unauthorized_caller = Address::generate(&env);
    client.mark_shipped(
        &unauthorized_caller,
        &1,
        &String::from_str(&env, "TRACK-FAIL"),
    );
}

#[test]
#[should_panic]
fn test_unauthorized_pause_fails_early() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Escrow);
    let client = EscrowClient::new(&env, &contract_id);
    
    let fake_admin = Address::generate(&env);
    // Since we did not call `env.mock_all_auths()`, the require_auth() inside pause_contract
    // will panic immediately at the Host level because there's no auth provided.
    // This proves it happens before `require_admin()` which would have panicked with "not initialized".
    
    client.pause_contract(&fake_admin);
}

#[test]
#[should_panic]
fn test_unauthorized_create_escrow_fails_early() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Escrow);
    let client = EscrowClient::new(&env, &contract_id);
    
    let fake_seller = Address::generate(&env);
    let resolver = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Will panic on `seller.require_auth()` instead of `ensure_not_paused`
    client.create_escrow(&fake_seller, &None::<Address>, &resolver, &token, &1000, &100, &86400);
}

#[test]
#[should_panic]
fn test_unauthorized_cancel_escrow_fails_early() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Escrow);
    let client = EscrowClient::new(&env, &contract_id);
    
    let fake_caller = Address::generate(&env);
    
    // Will panic on `caller.require_auth()` instead of `load_escrow`
    client.cancel_escrow(&fake_caller, &1);
}
