#![cfg(test)]

use crate::{DisputeResolved, Escrow, EscrowClient, ResolutionType};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger},
    token, Address, Env, IntoVal, String as SorobanString, Symbol, TryFromVal, Val,
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

#[test]
fn test_arbitration_fee_deduction_on_resolve_release() {
    let env = Env::default();
    let (admin, seller, buyer, resolver, fee_collector, token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);

    let arb_fee_bps = 500_u32; // 5% of 1000 = 50
    client.initialize(&admin, &fee_collector, &arb_fee_bps);

    let amount = 1000_i128;
    let fee_bps = 200; // 2%

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &amount, &fee_bps, &3600_u64);

    mint(&env, &token, &buyer, amount);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-ARB-1"));

    // Advance time to allow dispute
    env.ledger().set_timestamp(env.ledger().timestamp() + 10);

    client.raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "reason"),
        &SorobanString::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );

    // Initial total arbitration fees should be 0
    assert_eq!(client.get_total_arbitration_fees(&token), 0);
    assert_eq!(client.get_arbitration_fee(), arb_fee_bps);

    client.resolve_dispute(&resolver, &id, &ResolutionType::Release);

    // Calculation:
    // 1. amount = 1000
    // 2. arbitration_fee = 50 (5% of 1000)
    // 3. remaining = 1000 - 50 = 950
    // 4. protocol_fee (2% of 950) = 950 * 200 / 10000 = 19
    // 5. final_net = 950 - 19 = 931

    assert_eq!(balance(&env, &token, &seller), 931);

    // contract balance should hold the protocol fees (19) AND the arbitration fees (50)
    // wait, our contract doesn't transfer arbitration fees out yet, they just stay in the balance
    // so total in contract = 50 + 19 = 69
    assert_eq!(balance(&env, &token, &contract_id), 69);

    // Dedicated tracking variable should be updated
    assert_eq!(client.get_total_arbitration_fees(&token), 50);
}

#[test]
fn test_arbitration_fee_deduction_on_resolve_refund() {
    let env = Env::default();
    let (admin, seller, buyer, resolver, fee_collector, token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);

    let arb_fee_bps = 1000_u32; // 10% of 1000 = 100
    client.initialize(&admin, &fee_collector, &arb_fee_bps);

    let amount = 1000_i128;
    let fee_bps = 300; // 3%

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &amount, &fee_bps, &3600_u64);

    mint(&env, &token, &buyer, amount);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-ARB-2"));

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "reason"),
        &SorobanString::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );

    client.resolve_dispute(&resolver, &id, &ResolutionType::Refund);

    let expected_topic = Symbol::new(&env, "dispute_resolved");
    let saw_refund_event = env
        .events()
        .all()
        .filter_by_contract(&contract_id)
        .events()
        .iter()
        .any(|event| match &event.body {
            soroban_sdk::xdr::ContractEventBody::V0(v0) => {
                let Some(topic) = v0.topics.iter().next() else {
                    return false;
                };
                let Ok(topic) = Symbol::try_from_val(&env, topic) else {
                    return false;
                };
                if topic != expected_topic {
                    return false;
                }

                let Ok(data) = Val::try_from_val(&env, &v0.data) else {
                    return false;
                };

                DisputeResolved::try_from_val(&env, &data)
                    .map(|event| event.escrow_id == id && event.resolution == ResolutionType::Refund)
                    .unwrap_or(false)
            }
            _ => false,
        });
    assert!(saw_refund_event, "dispute_resolved refund event should be emitted");

    // Calculation:
    // 1. amount = 1000
    // 2. arbitration_fee = 100 (10% of 1000)
    // 3. remaining = 1000 - 100 = 900
    // 4. protocol_fee (3% of 900) = 900 * 300 / 10000 = 27
    // 5. final_net = 900 - 27 = 873

    assert_eq!(balance(&env, &token, &buyer), 873);
    assert_eq!(balance(&env, &token, &contract_id), 127); // 100 + 27
    assert_eq!(client.get_total_arbitration_fees(&token), 100);
}

#[test]
fn test_set_and_get_arbitration_fee() {
    let env = Env::default();
    let (admin, _seller, _buyer, _resolver, fee_collector, _token) = setup(&env);

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);

    client.initialize(&admin, &fee_collector, &50_u32);
    assert_eq!(client.get_arbitration_fee(), 50);

    client.set_arbitration_fee(&admin, &150_u32);
    assert_eq!(client.get_arbitration_fee(), 150);
}
