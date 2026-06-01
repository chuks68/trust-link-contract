#![cfg(test)]

use crate::{Escrow, EscrowClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

pub fn setup_contract(env: &Env) -> (Address, EscrowClient, Address, Address) {
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let fee_collector = Address::generate(env);
    client.initialize(&admin, &fee_collector, &0_u32);
    (contract_id, client, admin, fee_collector)
}

pub fn mint_token(env: &Env, token: &Address, to: &Address, amount: i128) {
    token::StellarAssetClient::new(env, token).mint(to, &amount);
}

pub fn advance_time(env: &Env, seconds: u64) {
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + seconds);
}

pub fn create_funded_escrow(
    env: &Env,
    client: &EscrowClient,
    seller: &Address,
    buyer: &Address,
    resolver: &Address,
    token: &Address,
    amount: i128,
    fee_bps: u32,
    shipping_window: u64,
) -> u64 {
    mint_token(env, token, buyer, amount);
    let id = client.create_escrow(seller, &None::<Address>, resolver, token, &amount, &fee_bps, &shipping_window);
    client.fund_escrow(&id, buyer);
    id
}
