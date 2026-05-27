#![cfg(test)]

use crate::{Escrow, EscrowClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

fn setup_env() -> (Env, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let resolver = Address::generate(&env);
    let token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    (env, admin, seller, resolver, token, fee_collector)
}

#[test]
fn test_escrow_ids_monotonic_and_unique() {
    let (env, admin, seller, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    
    client.initialize(&admin, &fee_collector);
    
    let mut ids = Vec::new(&env);
    for i in 1..=10 {
        let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &3600_u64);
        assert_eq!(id, i as u64);
        ids.push_back(id);
    }
    
    // Verify persistence: new client instance sees counter at 11
    let client2 = EscrowClient::new(&env, &contract_id);
    let next_id = client2.create_escrow(&seller, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    assert_eq!(next_id, 11);
}
