#![cfg(test)]

use crate::test_helpers::setup_contract;
use crate::{DataKey, DisputeData, DisputeStatus, EscrowData, EscrowState, ResolutionType, ContractError};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol, String as SorobanString, BytesN};

#[test]
fn test_resolve_dispute_refund_no_buyer_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    // Construct an escrow with `buyer = None` and state = Disputed
    let seller = Address::generate(&env);
    let resolver = Address::generate(&env);
    let token = Address::generate(&env);
    let id: u64 = 42;

    let escrow = EscrowData {
        seller: seller.clone(),
        buyer: None,
        resolver: resolver.clone(),
        token: token.clone(),
        amount: 1_000_000,
        fee_bps: 0,
        shipping_window: 0,
        funded_at: env.ledger().timestamp(),
        dispute_deadline: env.ledger().timestamp() + 1000,
        state: EscrowState::Disputed,
        shipped_at: 0,
        delivered_at: None,
        tracking_id: None,
    };

    env.storage().persistent().set(&DataKey::Escrow(id), &escrow);

    // Insert an active dispute record so resolve_dispute proceeds to resolution logic.
    let dispute = DisputeData {
        escrow_id: id,
        reason: Symbol::new(&env, "no-buyer"),
        description: SorobanString::from_str(&env, "no buyer set"),
        evidence_hash: BytesN::from_array(&env, &[0u8; 32]),
        status: DisputeStatus::Active,
        disputed_at: env.ledger().timestamp(),
        tracking_id: None,
    };
    env.storage().persistent().set(&DataKey::Dispute(id), &dispute);

    // Admin attempts to resolve with Refund — should return EscrowHasNoBuyer error,
    // not panic/trap.
    let res = client.try_resolve_dispute(&admin, &id, &ResolutionType::Refund);
    assert!(matches!(res, Err(Ok(ContractError::EscrowHasNoBuyer))));
}
