#![cfg(test)]

use crate::{Escrow, EscrowClient, ContractError, MIN_ESCROW_AMOUNT};
use soroban_sdk::{testutils::Address as _, token, Address, Env};

fn setup(env: &Env) -> (Address, Address, Address, Address, Address, Address) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let seller = Address::generate(env);
    let buyer = Address::generate(env);
    let resolver = Address::generate(env);
    let fee_collector = Address::generate(env);
    let token = env.register_stellar_asset_contract(Address::generate(env));
    (admin, seller, buyer, resolver, fee_collector, token)
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    token::StellarAssetClient::new(env, token).mint(to, &amount);
}

/// Verify that creating an escrow with zero amount throws an error.
#[test]
fn test_create_escrow_zero_amount_fails() {
    let env = Env::default();
    let (admin, seller, _buyer, resolver, fee_collector, token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    let result = client.try_create_escrow(&seller, &resolver, &token, &0_i128, &0_u32, &3600_u64);
    assert_eq!(result, Err(Ok(ContractError::InvalidAmount)));
}

/// Verify that creating an escrow below the minimum throws an error.
#[test]
fn test_create_escrow_below_minimum_fails() {
    let env = Env::default();
    let (admin, seller, _buyer, resolver, fee_collector, token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    let below_minimum = MIN_ESCROW_AMOUNT - 1;
    let result = client.try_create_escrow(&seller, &resolver, &token, &below_minimum, &0_u32, &3600_u64);
    assert_eq!(result, Err(Ok(ContractError::InvalidAmount)));
}

/// Verify that creating an escrow exactly at the minimum succeeds.
#[test]
fn test_create_escrow_at_minimum_succeeds() {
    let env = Env::default();
    let (admin, seller, buyer, resolver, fee_collector, token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    mint(&env, &token, &buyer, MIN_ESCROW_AMOUNT);

    let result = client.try_create_escrow(
        &seller,
        &resolver,
        &token,
        &MIN_ESCROW_AMOUNT,
        &0_u32,
        &3600_u64,
    );
    assert!(matches!(result, Ok(_)));
}

/// Verify that creating an escrow above the minimum succeeds.
#[test]
fn test_create_escrow_above_minimum_succeeds() {
    let env = Env::default();
    let (admin, seller, buyer, resolver, fee_collector, token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    let above_minimum = MIN_ESCROW_AMOUNT + 500_000;
    mint(&env, &token, &buyer, above_minimum);

    let result = client.try_create_escrow(
        &seller,
        &resolver,
        &token,
        &above_minimum,
        &0_u32,
        &3600_u64,
    );
    assert!(matches!(result, Ok(_)));
}
