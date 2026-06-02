#![cfg(test)]
//! Boundary + authorization tests for `set_fee` (#26).
//!
//! Covers:
//! - valid values (0 / 100 / 300 bps) accepted and persisted
//! - 301 bps rejected with `FeeExceedsMax`
//! - non-admin caller rejected with `NotAuthorized`

use crate::{ContractError, DataKey, Escrow, EscrowClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn deploy(env: &Env) -> (EscrowClient<'static>, Address, Address) {
    let admin = Address::generate(env);
    let fee_collector = Address::generate(env);
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);
    (client, admin, contract_id)
}

fn stored_fee(env: &Env, contract_id: &Address) -> u32 {
    env.as_contract(contract_id, || {
        env.storage()
            .instance()
            .get(&DataKey::DefaultFeeBps)
            .unwrap_or(0u32)
    })
}

#[test]
fn accepts_valid_fee_values_and_persists_them() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, cid) = deploy(&env);

    for fee in [0_u32, 100, 300] {
        client.set_fee(&admin, &fee);
        assert_eq!(stored_fee(&env, &cid), fee);
    }
}

#[test]
fn rejects_fee_above_maximum() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, cid) = deploy(&env);

    // 300 bps is the boundary — passes.
    client.set_fee(&admin, &300_u32);
    assert_eq!(stored_fee(&env, &cid), 300);

    // 301 bps is one above MAX_FEE_BPS — rejected with FeeExceedsMax.
    let result = client.try_set_fee(&admin, &301_u32);
    assert_eq!(result, Err(Ok(ContractError::FeeExceedsMax)));

    // Storage is unchanged at the previously accepted value.
    assert_eq!(stored_fee(&env, &cid), 300);
}

#[test]
fn rejects_unauthorized_caller() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, cid) = deploy(&env);

    let intruder = Address::generate(&env);
    let result = client.try_set_fee(&intruder, &100_u32);
    assert_eq!(result, Err(Ok(ContractError::NotAuthorized)));

    // Storage is unchanged at the initial 0 value.
    assert_eq!(stored_fee(&env, &cid), 0);
}
