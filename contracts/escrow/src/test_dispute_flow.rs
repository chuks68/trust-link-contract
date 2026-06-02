#![cfg(test)]
//! Integration test for the full dispute → vendor-wins resolution flow (#17).
//!
//! Covers: create → fund → ship → raise_dispute → resolve_dispute(Release).
//! After resolution the escrow must be in `Completed`, the seller must
//! receive `amount - arbitration_fee`, the buyer must not be refunded, and
//! the on-chain dispute record must be marked `Resolved`.

use crate::{DataKey, DisputeData, DisputeStatus, Escrow, EscrowClient, EscrowData, EscrowState, ResolutionType};
use soroban_sdk::{
    testutils::Address as _,
    token, Address, BytesN, Env, String, Symbol,
};

#[test]
fn full_dispute_release_to_vendor() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    // SAC token used to fund the buyer + receive the seller payout.
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_address = sac.address();

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);

    let arbitration_fee: u32 = 50;
    client.initialize(&admin, &fee_collector, &arbitration_fee);

    let amount: i128 = 1_000;
    // shipping_window = 0 so `mark_shipped` is permitted immediately. The
    // dispute window is enforced separately on raise_dispute.
    // fee_bps = 0 isolates the arbitration-fee accounting the issue specifies
    // (a non-zero protocol fee would further reduce the seller's payout).
    let escrow_id = client.create_escrow(&seller, &None::<Address>, &resolver, &token_address, &amount, &0_u32, &0_u64);

    // Fund the buyer and the escrow.
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);
    token_admin_client.mint(&buyer, &amount);
    client.fund_escrow(&escrow_id, &buyer);

    // Seller marks shipped. mark_shipped now takes (caller, escrow_id, tracking_id).
    let tracking_id = String::from_str(&env, "TRK-001");
    client.mark_shipped(&seller, &escrow_id, &tracking_id);

    // Buyer raises a dispute.
    let reason = Symbol::new(&env, "non_delivery");
    let description = String::from_str(&env, "Item never arrived");
    let evidence = BytesN::from_array(&env, &[0xab; 32]);
    client.raise_dispute(&buyer, &escrow_id, &reason, &description, &evidence);

    // Sanity: state is now Disputed before resolution.
    let mid: EscrowData = env
        .as_contract(&contract_id, || env.storage().persistent().get(&DataKey::Escrow(escrow_id)))
        .expect("escrow exists");
    assert_eq!(mid.state, EscrowState::Disputed);

    // Resolver decides in favour of the vendor.
    client.resolve_dispute(&resolver, &escrow_id, &ResolutionType::Release);

    // ── Post-resolution assertions ─────────────────────────────────────────
    let token_client = token::TokenClient::new(&env, &token_address);

    // Vendor received the net amount (face value minus the arbitration fee).
    assert_eq!(
        token_client.balance(&seller),
        amount - 5,
        "seller should receive amount minus arbitration fee on Release",
    );

    // Buyer received no refund.
    assert_eq!(
        token_client.balance(&buyer),
        0,
        "buyer should not be refunded on a vendor-wins resolution",
    );

    // Escrow state advanced to Completed.
    let after: EscrowData = env
        .as_contract(&contract_id, || env.storage().persistent().get(&DataKey::Escrow(escrow_id)))
        .expect("escrow exists");
    assert_eq!(after.state, EscrowState::Completed);

    // Dispute record is marked Resolved.
    let dispute: DisputeData = env
        .as_contract(&contract_id, || env.storage().persistent().get(&DataKey::Dispute(escrow_id)))
        .expect("dispute exists");
    assert_eq!(dispute.status, DisputeStatus::Resolved);
}
