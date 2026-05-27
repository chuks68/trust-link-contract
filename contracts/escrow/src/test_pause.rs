#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, token, Address, Env, String as SorobanString, Symbol};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn setup_env() -> (Env, Address, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector);

    (env, admin, seller, buyer, resolver, token_address, contract_id)
}

fn mint_tokens(env: &Env, token: &Address, to: &Address, amount: i128) {
    let sac = token::StellarAssetClient::new(env, token);
    sac.mint(to, &amount);
}

#[test]
fn test_pause_blocks_mutations_but_keeps_views_available() {
    let (env, admin, seller, buyer, resolver, token, contract_id) = setup_env();
    let client = EscrowClient::new(&env, &contract_id);

    mint_tokens(&env, &token, &buyer, 1_000);

    let escrow_id = client.create_escrow(&seller, &resolver, &token, &100_i128, &100_u32, &3600_u64);
    client.pause_contract();

    let config = client.get_fee_config();
    assert_eq!(config.max_fee_bps, 300);

    assert!(catch_unwind(AssertUnwindSafe(|| {
        client.withdraw_fees(&token, &admin, &1_i128);
    }))
    .is_err());

    assert!(catch_unwind(AssertUnwindSafe(|| {
        client.create_escrow(&seller, &resolver, &token, &100_i128, &100_u32, &3600_u64);
    }))
    .is_err());

    assert!(catch_unwind(AssertUnwindSafe(|| {
        client.fund_escrow(&escrow_id, &buyer);
    }))
    .is_err());

    assert!(catch_unwind(AssertUnwindSafe(|| {
        client.confirm_delivery(&escrow_id);
    }))
    .is_err());

    assert!(catch_unwind(AssertUnwindSafe(|| {
        client.raise_dispute(
            &escrow_id,
            &Symbol::new(&env, "reason"),
            &SorobanString::from_str(&env, "desc"),
            &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
        );
    }))
    .is_err());

    assert!(catch_unwind(AssertUnwindSafe(|| {
        client.resolve_dispute(&escrow_id, &ResolutionType::Release);
    }))
    .is_err());

    assert!(catch_unwind(AssertUnwindSafe(|| {
        client.auto_release(&escrow_id);
    }))
    .is_err());

    client.unpause_contract();
    mint_tokens(&env, &token, &buyer, 100);
    let second_id = client.create_escrow(&seller, &resolver, &token, &50_i128, &50_u32, &3600_u64);
    assert_eq!(second_id, 2);
}
