#![cfg(test)]
//! Calling `initialize` a second time must return `ContractError::AlreadyInitialized`
//! and leave the storage values from the first call intact (#14).

use crate::{DataKey, Escrow, EscrowClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn deploy_and_init(env: &Env) -> (EscrowClient, Address, Address) {
    env.mock_all_auths();
    let admin_a = Address::generate(env);
    let fee_collector_a = Address::generate(env);
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(env, &contract_id);
    client.initialize(&admin_a, &fee_collector_a, &42_u32);
    (client, admin_a, fee_collector_a)
}

#[test]
fn second_initialize_reverts() {
    let env = Env::default();
    let (client, _admin_a, _fc_a) = deploy_and_init(&env);
    let admin_b = Address::generate(&env);
    let fee_collector_b = Address::generate(&env);
    // Second call must return AlreadyInitialized.
    let res = client.try_initialize(&admin_b, &fee_collector_b, &99_u32);
    assert_eq!(res, Err(Ok(crate::ContractError::AlreadyInitialized)));
}

#[test]
fn storage_from_the_first_initialize_is_unchanged_after_a_failed_second_call() {
    let env = Env::default();
    let (client, admin_a, fee_collector_a) = deploy_and_init(&env);
    let admin_b = Address::generate(&env);
    let fee_collector_b = Address::generate(&env);

    // Use `try_initialize` so the host-side contract panic comes back as Err
    // and the test can keep running to verify the storage invariant.
    let res = client.try_initialize(&admin_b, &fee_collector_b, &99_u32);
    assert!(res.is_err(), "second initialize must revert");

    // Storage still reflects the first call's values.
    let stored_admin: Address = env
        .as_contract(&client.address, || env.storage().instance().get(&DataKey::Admin))
        .expect("admin set");
    let stored_collector: Address = env
        .as_contract(&client.address, || env.storage().instance().get(&DataKey::FeeCollector))
        .expect("fee collector set");
    let stored_fee: crate::FeeConfig = env
        .as_contract(&client.address, || env.storage().instance().get(&DataKey::FeeConfig))
        .expect("fee config set");

    assert_eq!(stored_admin, admin_a);
    assert_eq!(stored_collector, fee_collector_a);
    assert_eq!(stored_fee.arbitration_fee_bps, 42);
}
