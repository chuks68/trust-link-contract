#![cfg(test)]

use crate::{Escrow, EscrowClient, ResolutionType};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env, String as SorobanString, Symbol,
};

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

fn balance(env: &Env, token: &Address, who: &Address) -> i128 {
    token::Client::new(env, token).balance(who)
}

/// With amount = 1 stroop and any non-zero fee_bps, integer division yields
/// fee = 1 * fee_bps / 10_000 = 0, so the full 1 stroop reaches the recipient.
#[test]
fn test_fee_rounds_to_zero_on_one_stroop_confirm_delivery() {
    let env = Env::default();
    let (admin, seller, buyer, resolver, fee_collector, token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);
    client.set_protocol_fee(&admin, &300_u32);

    mint(&env, &token, &buyer, 1);

    // MAX_FEE_BPS = 300 (3%) — still rounds to 0 on 1 stroop
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1_i128, &300_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-ONE"));

    let escrow = client.get_escrow(&id);
    env.ledger().set_timestamp(escrow.dispute_deadline + 1);
    client.confirm_delivery(&buyer, &id);

    let _escrow = client.get_escrow(&id);
    // fee = 1 * 300 / 10_000 = 0  →  net = 1
    assert_eq!(balance(&env, &token, &seller), 1);
    assert_eq!(balance(&env, &token, &fee_collector), 0);
    assert_eq!(balance(&env, &token, &contract_id), 0);
}

#[test]
fn test_fee_rounds_to_zero_on_one_stroop_auto_release() {
    let env = Env::default();
    let (admin, seller, buyer, resolver, fee_collector, token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    mint(&env, &token, &buyer, 1);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1_i128, &300_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-FEE-AUTO"));
    env.ledger().set_timestamp(1_700_000_000);
    client.record_delivery(&admin, &id);

    // Advance 48 hours past delivery.
    let escrow = client.get_escrow(&id);
    env.ledger().set_timestamp(escrow.delivered_at.unwrap() + 172_801);
    client.auto_release(&id);

    assert_eq!(balance(&env, &token, &seller), 1);
    assert_eq!(balance(&env, &token, &contract_id), 0);
}

#[test]
fn test_fee_rounds_to_zero_on_one_stroop_resolve_dispute_release() {
    let env = Env::default();
    let (admin, seller, buyer, resolver, fee_collector, token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    mint(&env, &token, &buyer, 1);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1_i128, &300_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-FEE-REL"));
    client.raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "fraud"),
        &SorobanString::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );
    client.resolve_dispute(&resolver, &id, &ResolutionType::Release);

    assert_eq!(balance(&env, &token, &seller), 1);
    assert_eq!(balance(&env, &token, &contract_id), 0);
}

#[test]
fn test_fee_rounds_to_zero_on_one_stroop_resolve_dispute_refund() {
    let env = Env::default();
    let (admin, seller, buyer, resolver, fee_collector, token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);

    mint(&env, &token, &buyer, 1);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1_i128, &300_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-FEE-REF"));
    client.raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "fraud"),
        &SorobanString::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );
    client.resolve_dispute(&resolver, &id, &ResolutionType::Refund);

    // Buyer gets back the full 1 stroop; no fee retained
    assert_eq!(balance(&env, &token, &buyer), 1);
    assert_eq!(balance(&env, &token, &contract_id), 0);
}
