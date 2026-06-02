#![cfg(test)]
//! Tests for `rotate_resolver`: seller and admin can rotate, buyer cannot,
//! same-address is rejected, and terminal states are rejected.

use crate::{ContractError, Escrow, EscrowClient, EscrowState};
use soroban_sdk::{testutils::Address as _, token, Address, Env};

struct Fx {
    env: Env,
    client: EscrowClient<'static>,
    admin: Address,
    seller: Address,
    buyer: Address,
    resolver: Address,
    escrow_id: u64,
}

fn setup() -> Fx {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_addr = sac.address();
    token::StellarAssetClient::new(&env, &token_addr).mint(&buyer, &1_000_i128);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    let escrow_id = client.create_escrow(
        &seller,
        &None::<Address>,
        &resolver,
        &token_addr,
        &500_i128,
        &0_u32,
        &0_u64,
    );

    Fx { env, client, admin, seller, buyer, resolver, escrow_id }
}

#[test]
fn seller_can_rotate_resolver() {
    let fx = setup();
    let new_resolver = Address::generate(&fx.env);

    fx.client.rotate_resolver(&fx.seller, &fx.escrow_id, &new_resolver);

    use crate::{DataKey, EscrowData};
    let escrow: EscrowData = fx
        .env
        .as_contract(&fx.client.address, || {
            fx.env.storage().persistent().get(&DataKey::Escrow(fx.escrow_id))
        })
        .expect("escrow exists");
    assert_eq!(escrow.resolver, new_resolver);
}

#[test]
fn admin_can_rotate_resolver() {
    let fx = setup();
    let new_resolver = Address::generate(&fx.env);

    fx.client.rotate_resolver(&fx.admin, &fx.escrow_id, &new_resolver);

    use crate::{DataKey, EscrowData};
    let escrow: EscrowData = fx
        .env
        .as_contract(&fx.client.address, || {
            fx.env.storage().persistent().get(&DataKey::Escrow(fx.escrow_id))
        })
        .expect("escrow exists");
    assert_eq!(escrow.resolver, new_resolver);
}

#[test]
fn buyer_cannot_rotate_resolver() {
    let fx = setup();
    fx.client.fund_escrow(&fx.escrow_id, &fx.buyer);

    let new_resolver = Address::generate(&fx.env);
    let result = fx.client.try_rotate_resolver(&fx.buyer, &fx.escrow_id, &new_resolver);
    assert_eq!(result, Err(Ok(ContractError::NotAuthorized)));
}

#[test]
fn same_address_rejected() {
    let fx = setup();
    let result = fx.client.try_rotate_resolver(&fx.seller, &fx.escrow_id, &fx.resolver);
    assert_eq!(result, Err(Ok(ContractError::SameAddress)));
}

#[test]
fn new_resolver_cannot_be_seller() {
    let fx = setup();
    // resolver != seller (both generated independently), so passing seller as
    // new_resolver hits the InvalidAddress guard, not SameAddress.
    let result = fx.client.try_rotate_resolver(&fx.admin, &fx.escrow_id, &fx.seller);
    assert_eq!(result, Err(Ok(ContractError::InvalidAddress)));
}

#[test]
fn new_resolver_cannot_be_buyer() {
    let fx = setup();
    fx.client.fund_escrow(&fx.escrow_id, &fx.buyer);

    let result = fx.client.try_rotate_resolver(&fx.seller, &fx.escrow_id, &fx.buyer);
    assert_eq!(result, Err(Ok(ContractError::InvalidAddress)));
}

#[test]
fn terminal_state_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let resolver = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_addr = sac.address();

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    let escrow_id = client.create_escrow(
        &seller,
        &None::<Address>,
        &resolver,
        &token_addr,
        &100_i128,
        &0_u32,
        &0_u64,
    );

    // Cancel moves to Canceled (terminal)
    client.cancel_escrow(&seller, &escrow_id);

    let new_resolver = Address::generate(&env);
    let result = client.try_rotate_resolver(&seller, &escrow_id, &new_resolver);
    assert_eq!(result, Err(Ok(ContractError::InvalidState)));
}
