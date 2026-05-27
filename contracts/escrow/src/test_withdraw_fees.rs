#![cfg(test)]

use crate::{Escrow, EscrowClient, ContractError};
use soroban_sdk::{testutils::Address as _, token, Address, Env};

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

    (env, admin, seller, buyer, resolver, token_address, fee_collector)
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

    client.initialize(&admin, &fee_collector);

    mint_tokens(&env, &token, &buyer, 3000);

    // Complete 3 escrows that each accrue 1% fees
    for _ in 0..3 {
        let id = client.create_escrow(&seller, &resolver, &token, &1000_i128, &100_u32, &3600_u64);
        client.fund_escrow(&id, &buyer);
        client.confirm_delivery(&id);
    }

    // Total fees: 10 * 3 = 30
    let contract_balance = token::Client::new(&env, &token).balance(&contract_id);
    assert_eq!(contract_balance, 30);

    // Admin calls withdraw_fees for full amount
    let to = Address::generate(&env);
    client.withdraw_fees(&token, &to, &30);

    assert_eq!(token::Client::new(&env, &token).balance(&to), 30);
    // Second withdraw for same amount fails with InsufficientBalance
    let result2 = client.try_withdraw_fees(&token, &to, &30);
    assert!(matches!(result2, Err(Ok(ContractError::InsufficientBalance))));
}
