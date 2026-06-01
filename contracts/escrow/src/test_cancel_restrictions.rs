#![cfg(test)]
//! `cancel_escrow` is only legal while the escrow is `Pending` (#21). From
//! any other state it must reject with `InvalidState`.

use crate::{ContractError, DataKey, Escrow, EscrowClient, EscrowData, EscrowState};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, BytesN, Env, String, Symbol,
};

struct Fx {
    env: Env,
    client: EscrowClient<'static>,
    contract_id: Address,
    escrow_id: u64,
    seller: Address,
    buyer: Address,
    resolver: Address,
    token_addr: Address,
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
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &0_u32);
    let amount: i128 = 1_000;
    let escrow_id = client.create_escrow(&seller, &resolver, &token_addr, &amount, &0_u32, &0_u64);
    token::StellarAssetClient::new(&env, &token_addr).mint(&buyer, &amount);
    Fx { env, client, contract_id, escrow_id, seller, buyer, resolver, token_addr }
}

fn ship(fx: &Fx) {
    let tracking = String::from_str(&fx.env, "TRK-001");
    fx.client.mark_shipped(&fx.seller, &fx.escrow_id, &tracking);
}

#[test]
fn cancel_succeeds_in_pending_state() {
    let fx = setup();
    fx.client.cancel_escrow(&fx.seller, &fx.escrow_id);

    let data: EscrowData = fx
        .env
        .as_contract(&fx.contract_id, || {
            fx.env.storage().persistent().get(&DataKey::Escrow(fx.escrow_id))
        })
        .expect("escrow exists");
    assert_eq!(data.state, EscrowState::Canceled);
}

#[test]
fn cancel_fails_in_funded_state() {
    let fx = setup();
    fx.client.fund_escrow(&fx.escrow_id, &fx.buyer);
    assert_eq!(
        fx.client.try_cancel_escrow(&fx.seller, &fx.escrow_id),
        Err(Ok(ContractError::InvalidState)),
    );
}

#[test]
fn cancel_fails_in_shipped_state() {
    let fx = setup();
    fx.client.fund_escrow(&fx.escrow_id, &fx.buyer);
    ship(&fx);
    assert_eq!(
        fx.client.try_cancel_escrow(&fx.seller, &fx.escrow_id),
        Err(Ok(ContractError::InvalidState)),
    );
}

#[test]
fn cancel_fails_in_completed_state() {
    let fx = setup();
    fx.client.fund_escrow(&fx.escrow_id, &fx.buyer);
    ship(&fx);

    let escrow: EscrowData = fx
        .env
        .as_contract(&fx.contract_id, || {
            fx.env.storage().persistent().get(&DataKey::Escrow(fx.escrow_id))
        })
        .expect("escrow exists");
    fx.env.ledger().set_timestamp(escrow.dispute_deadline + 1);
    fx.client.confirm_delivery(&fx.buyer, &fx.escrow_id);

    assert_eq!(
        fx.client.try_cancel_escrow(&fx.seller, &fx.escrow_id),
        Err(Ok(ContractError::InvalidState)),
    );
}

#[test]
fn cancel_fails_in_disputed_state() {
    let fx = setup();
    fx.client.fund_escrow(&fx.escrow_id, &fx.buyer);
    ship(&fx);

    let reason = Symbol::new(&fx.env, "non_delivery");
    let description = String::from_str(&fx.env, "missing");
    let evidence = BytesN::from_array(&fx.env, &[0xab; 32]);
    fx.client.raise_dispute(&fx.buyer, &fx.escrow_id, &reason, &description, &evidence);

    assert_eq!(
        fx.client.try_cancel_escrow(&fx.seller, &fx.escrow_id),
        Err(Ok(ContractError::InvalidState)),
    );

    let _ = fx.resolver;
    let _ = fx.token_addr;
}
