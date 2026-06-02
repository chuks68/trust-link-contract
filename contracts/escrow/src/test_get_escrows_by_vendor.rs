#![cfg(test)]

use crate::test_helpers::setup_contract;
use crate::{EscrowState, EscrowData};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env};

fn register_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin).address()
}

#[test]
fn test_get_escrows_by_vendor_empty() {
    let env = Env::default();
    env.mock_all_auths();

    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);
    let vendor = Address::generate(&env);

    // Should return empty Vec (not error) for vendors with no escrows
    let escrows = client.get_escrows_by_vendor(&vendor);
    assert_eq!(escrows.len(), 0);
}

#[test]
fn test_get_escrows_by_vendor_multiple() {
    let env = Env::default();
    env.mock_all_auths();

    let token = register_token(&env);
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let vendor_1 = Address::generate(&env);
    let vendor_2 = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);

    // Create escrows for vendor 1
    let id1 = client.create_escrow(&vendor_1, &None::<Address>, &resolver, &token, &1000_i128, &0_u32, &3600_u64);
    let id2 = client.create_escrow(&vendor_1, &None::<Address>, &resolver, &token, &2000_i128, &0_u32, &3600_u64);

    // Create escrow for vendor 2
    let id3 = client.create_escrow(&vendor_2, &None::<Address>, &resolver, &token, &3000_i128, &0_u32, &3600_u64);

    // Check escrows for vendor 1
    let escrows_1 = client.get_escrows_by_vendor(&vendor_1);
    assert_eq!(escrows_1.len(), 2);
    assert_eq!(escrows_1.get(0).unwrap(), id1);
    assert_eq!(escrows_1.get(1).unwrap(), id2);

    // Check escrows for vendor 2
    let escrows_2 = client.get_escrows_by_vendor(&vendor_2);
    assert_eq!(escrows_2.len(), 1);
    assert_eq!(escrows_2.get(0).unwrap(), id3);
}

#[test]
fn test_vendor_escrow_data_integrity_and_state_transitions() {
    let env = Env::default();
    env.mock_all_auths();

    let token = register_token(&env);
    let (_contract_id, client, admin, fee_collector) = setup_contract(&env);

    let vendor = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);

    // Create
    let id = client.create_escrow(&vendor, &None::<Address>, &resolver, &token, &1000_i128, &0_u32, &3600_u64);
    
    // Assert initial state and data integrity
    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.seller, vendor);
    assert_eq!(escrow.state, EscrowState::Pending);
    assert_eq!(escrow.amount, 1000);

    // Fund
    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    sac.mint(&buyer, &1000);
    client.fund_escrow(&id, &buyer);
    
    // Assert state after funding
    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Funded);

    // Shipped
    let tracking = soroban_sdk::String::from_str(&env, "TRACK-001");
    client.mark_shipped(&vendor, &id, &tracking);
    
    // Assert state after shipping
    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Shipped);

    // Record delivery
    client.record_delivery(&admin, &id);
    
    // Confirm delivery
    env.ledger().set_timestamp(escrow.dispute_deadline + 1);
    client.confirm_delivery(&buyer, &id);
    
    // Assert final completed state
    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);

    // Verify vendor index still contains the escrow ID
    let escrows = client.get_escrows_by_vendor(&vendor);
    assert_eq!(escrows.len(), 1);
    assert_eq!(escrows.get(0).unwrap(), id);
}
