#![cfg(test)]

use crate::test_helpers::{create_funded_escrow, setup_contract};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn register_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin).address()
}

#[test]
fn test_get_escrows_by_buyer() {
    let env = Env::default();
    env.mock_all_auths();

    let token = register_token(&env);
    let (_contract_id, client, _admin, _fee_collector) = setup_contract(&env);

    let seller = Address::generate(&env);
    let buyer_1 = Address::generate(&env);
    let buyer_2 = Address::generate(&env);
    let resolver = Address::generate(&env);

    // Create 2 escrows for buyer 1
    let id1 = create_funded_escrow(
        &env, &client, &seller, &buyer_1, &resolver, &token, 1000, 100, 3600,
    );
    let id2 = create_funded_escrow(
        &env, &client, &seller, &buyer_1, &resolver, &token, 2000, 100, 3600,
    );

    // Create 1 escrow for buyer 2
    let id3 = create_funded_escrow(
        &env, &client, &seller, &buyer_2, &resolver, &token, 3000, 100, 3600,
    );

    // Create 1 pending escrow (no buyer yet)
    let _id4 = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &4000_i128, &100_u32, &3600_u64);

    // Check escrows for buyer 1
    let escrows_1 = client.get_escrows_by_buyer(&buyer_1);
    assert_eq!(escrows_1.len(), 2);
    assert_eq!(escrows_1.get(0).unwrap(), id1);
    assert_eq!(escrows_1.get(1).unwrap(), id2);

    // Check escrows for buyer 2
    let escrows_2 = client.get_escrows_by_buyer(&buyer_2);
    assert_eq!(escrows_2.len(), 1);
    assert_eq!(escrows_2.get(0).unwrap(), id3);

    // Check escrows for a buyer with no escrows
    let buyer_3 = Address::generate(&env);
    let escrows_3 = client.get_escrows_by_buyer(&buyer_3);
    assert_eq!(escrows_3.len(), 0);
}
