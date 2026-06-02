#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};
use crate::{test_helpers::setup_contract, DataKey, EscrowState};

fn register_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract(token_admin)
}

#[test]
fn test_cancel_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let token = register_token(&env);
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let seller = Address::generate(&env);
    let resolver = Address::generate(&env);

    // Create escrow in Pending state
    let id = client.create_escrow(&seller, &resolver, &token, 500_i128, 0_u32, 3600_u64);

    // Cancel escrow as seller
    client.cancel_escrow(&seller, &id);

    // Verify the escrow state is Cancelled
    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Cancelled);

    // Verify that an EscrowCancelled event was emitted
    let events = env.events().all();
    // Look for a topic with "escrow_cancelled"
    let found = events.iter().any(|(topics, _data)| {
        topics.iter().any(|sym| *sym == Symbol::new(&env, "escrow_cancelled"))
    });
    assert!(found, "EscrowCancelled event not found");
}
