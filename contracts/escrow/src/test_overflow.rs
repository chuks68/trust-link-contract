#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};

fn setup_env() -> (Env, Address, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(token_admin.clone());

    (env, seller, buyer, resolver, token_admin, token_address, fee_collector)
}

fn mint_tokens(env: &Env, token: &Address, to: &Address, amount: i128) {
    let sac = soroban_sdk::token::StellarAssetClient::new(env, token);
    sac.mint(to, &amount);
}

#[test]
fn test_fee_calculation_max_i128() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_i128);

    let amount = i128::MAX;
    let fee_bps = 300; // 3%

    let id = client.create_escrow(&seller, &resolver, &token, &amount, &fee_bps, &3600_u64);
    
    mint_tokens(&env, &token, &buyer, amount);
    client.fund_escrow(&id, &buyer);

    // Set ledger time to after dispute deadline to allow confirm_delivery
    env.ledger().set_timestamp(172800 + 1); // Default dispute deadline is 172800 (2 days)

    // This should not panic now because of split calculation
    client.confirm_delivery(&id);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);

    // Verify balances (roughly)
    // fee = i128::MAX * 3 / 100
    // Using split method: (i128::MAX / 10000) * 300 + (i128::MAX % 10000) * 300 / 10000
    let expected_fee = (i128::MAX / 10_000) * 300 + (i128::MAX % 10_000) * 300 / 10_000;
    let expected_net = i128::MAX - expected_fee;

    let tc = soroban_sdk::token::Client::new(&env, &token);
    assert_eq!(tc.balance(&seller), expected_net);
    assert_eq!(tc.balance(&contract_id), expected_fee);
}

#[test]
fn test_create_escrow_invalid_amount() {
    let (env, seller, _buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_i128);

    let res = client.try_create_escrow(&seller, &resolver, &token, &0, &200, &3600);
    assert!(matches!(res, Err(Ok(ContractError::InvalidAmount))));

    let res2 = client.try_create_escrow(&seller, &resolver, &token, &-1, &200, &3600);
    assert!(matches!(res2, Err(Ok(ContractError::InvalidAmount))));
}

#[test]
fn test_fee_exceeds_max_clean_error() {
    let (env, seller, _buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_i128);

    let res = client.try_create_escrow(&seller, &resolver, &token, &1000, &301, &3600);
    assert!(matches!(res, Err(Ok(ContractError::FeeExceedsMax))));
}
