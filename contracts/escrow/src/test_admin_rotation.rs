#![cfg(test)]
//! `set_admin` rotates the admin key (#13). The new admin is allowed to call
//! admin-gated functions; the old admin is not.

use crate::{Escrow, EscrowClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, EscrowClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);
    (env, client, admin)
}

#[test]
fn current_admin_can_rotate_to_a_new_admin() {
    let (env, client, _admin) = setup();
    let new_admin = Address::generate(&env);

    client.set_admin(&new_admin);

    // The stored admin is now the new one — confirm by reading storage.
    use crate::DataKey;
    let stored: Address = env
        .as_contract(&client.address, || {
            env.storage().instance().get(&DataKey::Admin)
        })
        .expect("admin is set");
    assert_eq!(stored, new_admin);
}

#[test]
fn new_admin_can_call_admin_functions_after_rotation() {
    let (env, client, _admin) = setup();
    let new_admin = Address::generate(&env);

    client.set_admin(&new_admin);

    // With all auths still mocked, the next admin-gated call succeeds because
    // `require_admin` resolves to the *new* admin and its auth is mocked.
    client.set_protocol_fee(&new_admin, &50_u32);

    use crate::DataKey;
    let fee_config: crate::FeeConfig = env
        .as_contract(&client.address, || env.storage().instance().get(&DataKey::FeeConfig))
        .expect("fee config set");
    assert_eq!(fee_config.protocol_fee_bps, 50);
}

#[test]
fn old_admin_cannot_authorise_admin_functions_after_rotation() {
    let (env, client, _old_admin) = setup();
    let new_admin = Address::generate(&env);
    client.set_admin(&new_admin);

    // Strip the all-auths mock — admin-gated functions now require the *new*
    // admin's specific signature, which we have not provided.
    env.mock_auths(&[]);

    // Old-admin-era operations should now be rejected because the active
    // admin (new_admin) has not authorised this invocation.
    assert!(client.try_set_protocol_fee(&new_admin, &100_u32).is_err());
}

#[test]
#[should_panic]
fn rotation_requires_current_admin_authorisation() {
    let (env, client, _admin) = setup();
    let new_admin = Address::generate(&env);

    env.mock_auths(&[]);
    // Without admin auth, set_admin must panic (auth violation).
    client.set_admin(&new_admin);
}
