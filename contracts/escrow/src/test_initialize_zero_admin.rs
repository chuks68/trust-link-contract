#![cfg(test)]
//! `initialize` must reject the all-zero (empty) Stellar account address for
//! the admin role and report a clear `ContractError::InvalidAddress` (#55).
//!
//! Acceptance criteria covered:
//!  * Setup requests throw immediate, clear errors on empty address parameters.
//!  * Contract state configuration remains uninitialized on failed setup calls.

use crate::{ContractError, DataKey, Escrow, EscrowClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

/// Strkey for the all-zero ed25519 public key — the canonical "empty" Stellar
/// account address used here as the sentinel for an unset admin.
const ZERO_ADDRESS_STRKEY: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

fn zero_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(env, ZERO_ADDRESS_STRKEY))
}

fn deploy(env: &Env) -> EscrowClient<'_> {
    env.mock_all_auths();
    let contract_id = env.register(Escrow, ());
    EscrowClient::new(env, &contract_id)
}

#[test]
fn initialize_with_zero_admin_returns_invalid_address_error() {
    let env = Env::default();
    let client = deploy(&env);

    let zero_admin = zero_address(&env);
    let fee_collector = Address::generate(&env);

    let res = client.try_initialize(&zero_admin, &fee_collector, &42_u32);

    // `try_initialize` returns `Result<Result<(), ContractError>, _>`.
    // The contract returned an explicit, recognised error variant — not a host panic.
    assert_eq!(
        res,
        Err(Ok(ContractError::InvalidAddress)),
        "initialize must return ContractError::InvalidAddress for a zero admin address",
    );
}

#[test]
fn initialize_with_zero_fee_collector_returns_invalid_address_error() {
    let env = Env::default();
    let client = deploy(&env);

    let admin = Address::generate(&env);
    let zero_collector = zero_address(&env);

    let res = client.try_initialize(&admin, &zero_collector, &42_u32);

    assert_eq!(
        res,
        Err(Ok(ContractError::InvalidAddress)),
        "initialize must reject a zero fee_collector with InvalidAddress",
    );
}

#[test]
fn failed_initialize_with_zero_admin_leaves_storage_uninitialized() {
    let env = Env::default();
    let client = deploy(&env);

    let zero_admin = zero_address(&env);
    let fee_collector = Address::generate(&env);

    let res = client.try_initialize(&zero_admin, &fee_collector, &42_u32);
    assert_eq!(res, Err(Ok(ContractError::InvalidAddress)));

    // None of the initialization storage keys may be set after a rejected call.
    env.as_contract(&client.address, || {
        let storage = env.storage().instance();
        assert!(!storage.has(&DataKey::Admin), "Admin must not be set");
        assert!(
            !storage.has(&DataKey::FeeCollector),
            "FeeCollector must not be set",
        );
        assert!(
            !storage.has(&DataKey::ArbitrationFee),
            "ArbitrationFee must not be set",
        );
        assert!(
            !storage.has(&DataKey::EscrowCounter),
            "EscrowCounter must not be set",
        );
        assert!(!storage.has(&DataKey::Paused), "Paused must not be set");
    });
}

#[test]
fn contract_can_be_initialized_after_a_failed_zero_admin_attempt() {
    let env = Env::default();
    let client = deploy(&env);

    // First attempt: zero admin → rejected.
    let zero_admin = zero_address(&env);
    let fee_collector = Address::generate(&env);
    let first = client.try_initialize(&zero_admin, &fee_collector, &42_u32);
    assert_eq!(first, Err(Ok(ContractError::InvalidAddress)));

    // Because nothing was persisted, a follow-up call with valid addresses
    // must succeed — proving the failed call did not partially initialize the
    // contract (the "AlreadyInitialized" guard is not tripped).
    let real_admin = Address::generate(&env);
    client.initialize(&real_admin, &fee_collector, &99_u32);

    let stored_admin: Address = env
        .as_contract(&client.address, || env.storage().instance().get(&DataKey::Admin))
        .expect("admin set after successful initialize");
    assert_eq!(stored_admin, real_admin);
}
