#![cfg(test)]

use crate::{ContractError, Escrow, EscrowClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

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

    (
        env,
        admin,
        seller,
        buyer,
        resolver,
        token_address,
        fee_collector,
    )
}

fn mint_tokens(env: &Env, token: &Address, to: &Address, amount: i128) {
    let sac = token::StellarAssetClient::new(env, token);
    sac.mint(to, &amount);
}

#[test]
fn test_withdraw_fees_after_multiple_escrows() {
    let (env, admin, seller, buyer, resolver, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);

    client.initialize(&admin, &fee_collector, &0_u32);

    mint_tokens(&env, &token, &buyer, 3000);

    // Complete 3 escrows that each accrue 1% fees via dispute release.
    for _ in 0..3 {
        let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &100_u32, &3600_u64);
        client.fund_escrow(&id, &buyer);
        client.mark_shipped(&seller, &id, &soroban_sdk::String::from_str(&env, "TRACK-WITHDRAW-1"));
        client.raise_dispute(
            &buyer,
            &id,
            &soroban_sdk::Symbol::new(&env, "release"),
            &soroban_sdk::String::from_str(&env, "ok"),
            &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
        );
        client.resolve_dispute(&resolver, &id, &crate::ResolutionType::Release);
    }

    // Total fees: 10 * 3 = 30
    let contract_balance = token::Client::new(&env, &token).balance(&contract_id);
    assert_eq!(contract_balance, 30);

    // Admin calls withdraw_fees for full amount
    let to = Address::generate(&env);
    client.withdraw_fees(&admin, &token, &to, &30);

    assert_eq!(token::Client::new(&env, &token).balance(&to), 30);
    // Second withdraw for same amount fails with InsufficientBalance
    let result2 = client.try_withdraw_fees(&admin, &token, &to, &30);
    assert!(matches!(
        result2,
        Err(Ok(ContractError::InsufficientBalance))
    ));
}

#[test]
fn test_withdraw_fees_multiple_tokens() {
    let (env, admin, seller, buyer, resolver, token_a, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);

    client.initialize(&admin, &fee_collector, &0_u32);

    // Register a second token
    let token_admin_b = Address::generate(&env);
    let token_b = env.register_stellar_asset_contract(token_admin_b);

    // Accrue fees for Token A (1000 amount, 1% fee = 10)
    mint_tokens(&env, &token_a, &buyer, 1000);
    let id_a = client.create_escrow(&seller, &resolver, &token_a, &1000_i128, &100_u32, &3600_u64);
    client.fund_escrow(&id_a, &buyer);
    client.mark_shipped(&seller, &id_a, &soroban_sdk::String::from_str(&env, "TRACK-WITHDRAW-A"));
    client.raise_dispute(
        &buyer,
        &id_a,
        &soroban_sdk::Symbol::new(&env, "release"),
        &soroban_sdk::String::from_str(&env, "ok"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );
    client.resolve_dispute(&resolver, &id_a, &crate::ResolutionType::Release);

    // Accrue fees for Token B (2000 amount, 2% fee = 40)
    mint_tokens(&env, &token_b, &buyer, 2000);
    let id_b = client.create_escrow(&seller, &resolver, &token_b, &2000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id_b, &buyer);
    client.mark_shipped(&seller, &id_b, &soroban_sdk::String::from_str(&env, "TRACK-WITHDRAW-B"));
    client.raise_dispute(
        &buyer,
        &id_b,
        &soroban_sdk::Symbol::new(&env, "release"),
        &soroban_sdk::String::from_str(&env, "ok"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );
    client.resolve_dispute(&resolver, &id_b, &crate::ResolutionType::Release);

    // Verify contract balances
    assert_eq!(token::Client::new(&env, &token_a).balance(&contract_id), 10);
    assert_eq!(token::Client::new(&env, &token_b).balance(&contract_id), 40);

    let to = Address::generate(&env);

    // Withdraw Token A
    client.withdraw_fees(&admin, &token_a, &to, &10);
    assert_eq!(token::Client::new(&env, &token_a).balance(&to), 10);
    assert_eq!(token::Client::new(&env, &token_a).balance(&contract_id), 0);
    // Token B balance should remain unchanged
    assert_eq!(token::Client::new(&env, &token_b).balance(&contract_id), 40);

    // Withdraw Token B
    client.withdraw_fees(&admin, &token_b, &to, &40);
    assert_eq!(token::Client::new(&env, &token_b).balance(&to), 40);
    assert_eq!(token::Client::new(&env, &token_b).balance(&contract_id), 0);
}
