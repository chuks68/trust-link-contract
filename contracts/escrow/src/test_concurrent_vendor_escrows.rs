#![cfg(test)]

use crate::{Escrow, EscrowClient, EscrowState};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env};

fn setup_env() -> (Env, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env); // The single vendor
    let resolver = Address::generate(&env);
    let token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    (env, admin, seller, resolver, token, fee_collector)
}

#[test]
fn same_vendor_can_create_multiple_escrows_without_collision() {
    let (env, admin, seller, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);

    client.initialize(&admin, &fee_collector, &0_u32);

    // Test 1: Same Vendor Creates Multiple Escrows Sequentially
    let num_escrows = 10_usize;
    let mut ids = [0_u64; 10];
    
    // We do not advance the ledger sequence here to simulate "concurrent" creation
    // within the same block.
    for i in 0..num_escrows {
        // Vary the amount slightly for each escrow to ensure isolated data
        let amount = 100_i128 + ((i + 1) as i128);
        let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &amount, &0_u32, &3600_u64);
        
        // IDs should be strictly monotonic
        assert_eq!(id, (i + 1) as u64);
        ids[i] = id;
    }

    // Ensure IDs do not skip and are unique
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j]);
        }
    }

    // Verify storage for each ID exists and contains correct item data
    for i in 1..=num_escrows {
        let escrow = client.get_escrow(&(i as u64));
        let expected_amount = 100_i128 + (i as i128);
        
        assert_eq!(escrow.seller, seller);
        assert_eq!(escrow.amount, expected_amount);
        assert_eq!(escrow.state, EscrowState::Pending);
    }
    
    // Test 4: Counter Persistence
    // If counter begins at 1 and 10 escrows created, next is 11
    let stats = client.get_contract_config();
    assert_eq!(stats.escrow_count, num_escrows as u64);
}

#[test]
fn escrow_storage_entries_remain_isolated() {
    let (env, admin, seller, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    // Create multiple escrows
    let id1 = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    let id2 = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &200_i128, &0_u32, &3600_u64);
    let id3 = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &300_i128, &0_u32, &3600_u64);

    // Mutate one escrow
    client.cancel_escrow(&seller, &id2);

    // Verify all others remain unchanged
    let escrow1 = client.get_escrow(&id1);
    let escrow2 = client.get_escrow(&id2);
    let escrow3 = client.get_escrow(&id3);

    assert_eq!(escrow1.state, EscrowState::Pending);
    assert_eq!(escrow2.state, EscrowState::Canceled);
    assert_eq!(escrow3.state, EscrowState::Pending);
}

#[test]
fn escrow_counter_remains_monotonic_under_rapid_creation() {
    let (env, admin, seller, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    // Force all creations within same sequence number
    env.ledger().set_sequence_number(100);

    for i in 1..=50 {
        let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
        assert_eq!(id, i as u64);
    }
}
