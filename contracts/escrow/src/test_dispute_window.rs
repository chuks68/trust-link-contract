#![cfg(test)]
//! Regression tests for dispute handling after shipping and for the
//! admin-triggered auto-release path (#4).

use crate::{ContractError, DataKey, DisputeData, DisputeStatus, Escrow, EscrowClient, EscrowData, EscrowState};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, BytesN, Env, String, Symbol,
};

struct Fx {
    env: Env,
    client: EscrowClient<'static>,
    contract_id: Address,
    admin: Address,
    buyer: Address,
    seller: Address,
    escrow_id: u64,
    delivered_at: u64,
}

fn setup_funded_and_shipped() -> Fx {
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
    env.ledger().set_timestamp(1_700_000_000);
    client.fund_escrow(&escrow_id, &buyer);
    client.mark_shipped(&seller, &escrow_id, &String::from_str(&env, "TRK-001"));

    let delivered_at = env.ledger().timestamp();
    env.ledger().set_timestamp(delivered_at);
    client.record_delivery(&admin, &escrow_id);

    Fx { env, client, contract_id, admin, buyer, seller, escrow_id, delivered_at }
}

#[test]
fn dispute_can_be_opened_while_shipped() {
    let fx = setup_funded_and_shipped();
    // Stay within the dispute window (dispute_deadline = 1_700_000_000 + 172_800)
    fx.env.ledger().set_timestamp(1_700_000_010);

    let reason = Symbol::new(&fx.env, "non_delivery");
    let description = String::from_str(&fx.env, "missing");
    let evidence = BytesN::from_array(&fx.env, &[0xab; 32]);

    fx.client.raise_dispute(&fx.buyer, &fx.escrow_id, &reason, &description, &evidence);

    let dispute: DisputeData = fx
        .env
        .as_contract(&fx.contract_id, || fx.env.storage().persistent().get(&DataKey::Dispute(fx.escrow_id)))
        .expect("dispute exists");
    assert_eq!(dispute.status, DisputeStatus::Active);
    assert_eq!(dispute.evidence_hash, evidence);

    let escrow: EscrowData = fx
        .env
        .as_contract(&fx.contract_id, || fx.env.storage().persistent().get(&DataKey::Escrow(fx.escrow_id)))
        .expect("escrow exists");
    assert_eq!(escrow.state, EscrowState::Disputed);
}

#[test]
fn auto_release_rejects_when_dispute_exists() {
    let fx = setup_funded_and_shipped();
    // Stay within the dispute window (dispute_deadline = 1_700_000_000 + 172_800)
    fx.env.ledger().set_timestamp(1_700_000_010);
    let reason = Symbol::new(&fx.env, "non_delivery");
    let description = String::from_str(&fx.env, "missing");
    let evidence = BytesN::from_array(&fx.env, &[0xab; 32]);

    fx.client.raise_dispute(&fx.buyer, &fx.escrow_id, &reason, &description, &evidence);

    fx.env.ledger().set_timestamp(fx.delivered_at + 172_801);
    assert_eq!(
        fx.client.try_auto_release(&fx.escrow_id),
        Err(Ok(ContractError::InvalidState)),
    );

    let _ = fx.admin;
    let _ = fx.seller;
}
