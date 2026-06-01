#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Bytes, Env, String as SorobanString, Symbol,
};

fn make_evidence_hash(env: &Env) -> Bytes {
    Bytes::from_array(env, &[0u8; 32])
}

fn setup_env() -> (Env, Address, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(token_admin.clone());

    (
        env,
        seller,
        buyer,
        resolver,
        token_admin,
        token_address,
        fee_collector,
    )
}

fn mint_tokens(env: &Env, token: &Address, to: &Address, amount: i128) {
    let sac = token::StellarAssetClient::new(env, token);
    sac.mint(to, &amount);
}

fn get_balance(env: &Env, token: &Address, user: &Address) -> i128 {
    let tc = token::Client::new(env, token);
    tc.balance(user)
}

fn create_and_fund(
    _env: &Env,
    client: &super::EscrowClient,
    seller: &Address,
    resolver: &Address,
    token: &Address,
    buyer: &Address,
) -> u64 {
    let id = client.create_escrow(seller, resolver, token, &100_i128, &0_u32, &3600_u64);
    client.fund_escrow(&id, buyer);
    id
}

#[test]
fn test_create_escrow() {
    let (env, seller, _buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &200_u32, &3600_u64);
    assert_eq!(id, 1u64);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.seller, seller);
    assert_eq!(escrow.resolver, resolver);
    assert_eq!(escrow.token, token);
    assert_eq!(escrow.amount, 100);
    assert_eq!(escrow.fee_bps, 200);
    assert_eq!(escrow.shipping_window, 3600);
    assert_eq!(escrow.state, EscrowState::Pending);
}

#[test]
fn test_fund_escrow() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Funded);
    assert_eq!(get_balance(&env, &token, &buyer), 900);
    assert_eq!(get_balance(&env, &token, &contract_id), 100);
}

#[test]
fn test_confirm_delivery() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);
    client.set_protocol_fee(&admin, &200_u32);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-010"));

    let escrow = client.get_escrow(&id);
    env.ledger().set_timestamp(escrow.dispute_deadline + 1);
    client.confirm_delivery(&buyer, &id);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);
    // 2% fee on 1000 = 20 routed to the fee collector, 980 to seller
    assert_eq!(get_balance(&env, &token, &seller), 980);
    assert_eq!(get_balance(&env, &token, &fee_collector), 20);
    assert_eq!(get_balance(&env, &token, &contract_id), 0);
}

#[test]
fn test_raise_and_resolve_dispute_release_to_seller() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-DISPUTE-1"));
    client.raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "reason"),
        &SorobanString::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );

    client.resolve_dispute(&resolver, &id, &ResolutionType::Release);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);
    assert_eq!(get_balance(&env, &token, &seller), 980);
    assert_eq!(get_balance(&env, &token, &contract_id), 20);
}

#[test]
fn test_raise_and_resolve_dispute_refund_buyer() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-DISPUTE-2"));
    client.raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "reason"),
        &SorobanString::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );
    client.resolve_dispute(&resolver, &id, &ResolutionType::Refund);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Refunded);
    assert_eq!(get_balance(&env, &token, &buyer), 980);
    assert_eq!(get_balance(&env, &token, &contract_id), 20);
}

// Note: fee_collector = 0 in dispute resolution paths since fees stay in vault
// and are swept by admin via withdraw_fees.

#[test]
fn test_auto_release() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);
    client.set_protocol_fee(&admin, &200_u32);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-AUTO-1"));
    env.ledger().set_timestamp(1_700_000_000);
    client.record_delivery(&admin, &id);

    let escrow = client.get_escrow(&id);
    env.ledger().set_timestamp(escrow.delivered_at + 172_801);
    client.auto_release(&id);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);
    assert_eq!(get_balance(&env, &token, &seller), 980);
    assert_eq!(get_balance(&env, &token, &fee_collector), 20);
    assert_eq!(get_balance(&env, &token, &contract_id), 0);
}

#[test]
fn test_fund_non_pending_escrow_fails() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);
    mint_tokens(&env, &token, &buyer, 1000);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    let res = client.try_fund_escrow(&id, &buyer);
    assert!(matches!(res, Err(Ok(ContractError::InvalidState))));
}

#[test]
fn test_auto_release_before_window_fails() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);
    client.set_protocol_fee(&admin, &200_u32);
    mint_tokens(&env, &token, &buyer, 1000);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-AUTO-2"));
    env.ledger().set_timestamp(1_700_000_000);
    client.record_delivery(&admin, &id);
    let escrow = client.get_escrow(&id);
    env.ledger().set_timestamp(escrow.delivered_at + 1);
    let res = client.try_auto_release(&id);
    assert!(matches!(res, Err(Ok(ContractError::ShippingWindowNotElapsed))));
}

#[test]
fn test_raise_dispute_only_once() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);
    mint_tokens(&env, &token, &buyer, 1000);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &0_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-DISPUTE-3"));
    client.raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "reason"),
        &SorobanString::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );
    let res = client.try_raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "reason"),
        &SorobanString::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );
    assert!(matches!(res, Err(Ok(ContractError::InvalidState))));
}

#[test]
fn test_multiple_escrows() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);
    mint_tokens(&env, &token, &buyer, 2000);
    let id1 = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &100_i128, &200_u32, &3600_u64);
    let id2 = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &200_i128, &200_u32, &7200_u64);
    assert_eq!(id1, 1u64);
    assert_eq!(id2, 2u64);
}

fn register_alt_token(env: &Env) -> (Address, Address) {
    let admin = Address::generate(env);
    let token_address = env.register_stellar_asset_contract(admin.clone());
    (token_address, admin)
}

#[test]
fn test_create_escrow_with_non_usdc_token() {
    let (env, seller, _buyer, resolver, _admin, _token, fee_collector) = setup_env();
    let (alt_token, _) = register_alt_token(&env);
    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_i128);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &alt_token, &500_i128, &0_u32, &7200_u64);
    client.initialize(&admin, &fee_collector, &0_u32);
    let id = client.create_escrow(&seller, &resolver, &alt_token, &500_i128, &0_u32, &7200_u64);
    assert_eq!(id, 1u64);
}

#[test]
fn test_fund_and_confirm_delivery_with_non_usdc_token() {
    let (env, seller, buyer, resolver, _admin, _token, fee_collector) = setup_env();
    let (alt_token, _) = register_alt_token(&env);
    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);
    client.set_protocol_fee(&admin, &100_u32);
    mint_tokens(&env, &alt_token, &buyer, 1000);
    let id = client.create_escrow(
        &seller, &None::<Address>, &resolver, &alt_token, &300_i128, &100_u32, &3600_u64,
    );
    client.fund_escrow(&id, &buyer);

    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-SEP41"));

    let escrow = client.get_escrow(&id);
    env.ledger().set_timestamp(escrow.dispute_deadline + 1);
    client.confirm_delivery(&buyer, &id);
    // 1% fee on 300 = 3 routed to the fee collector, 297 to seller
    assert_eq!(get_balance(&env, &alt_token, &seller), 297);
    assert_eq!(get_balance(&env, &alt_token, &fee_collector), 3);
    assert_eq!(get_balance(&env, &alt_token, &contract_id), 0);
}

#[test]
fn test_zero_fee_no_collector_transfer() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);
    mint_tokens(&env, &token, &buyer, 1000);
    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &0_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);

    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-ZERO"));

    let escrow = client.get_escrow(&id);
    env.ledger().set_timestamp(escrow.dispute_deadline + 1);
    client.confirm_delivery(&buyer, &id);
    assert_eq!(get_balance(&env, &token, &seller), 1000);
    assert_eq!(get_balance(&env, &token, &fee_collector), 0);
    assert_eq!(get_balance(&env, &token, &contract_id), 0);
}

#[test]
fn test_get_fee_config() {
    let (env, _, _, _, _, _, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);
    let config = client.get_fee_config();
    assert_eq!(config.protocol_fee_bps, 0);
    assert_eq!(config.arbitration_fee_bps, 0);
}

#[test]
fn test_fee_exceeds_max_bps_fails() {
    let (env, seller, _, resolver, _, token, fee_collector) = setup_env();
    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_i128);
    let res = client.try_create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &301_u32, &3600_u64);
    client.initialize(&admin, &fee_collector, &0_u32);
    let res = client.try_create_escrow(&seller, &resolver, &token, &1000_i128, &301_u32, &3600_u64);
    assert!(matches!(res, Err(Ok(ContractError::FeeExceedsMax))));
}

#[test]
fn test_dispute_after_shipping_succeeds() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-DISPUTE-4"));

    // Dispute should succeed
    client.raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "reason"),
        &SorobanString::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Disputed);
}

#[test]
fn test_dispute_requires_shipped_state() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);

    let res = client.try_raise_dispute(
        &buyer,
        &id,
        &Symbol::new(&env, "reason"),
        &SorobanString::from_str(&env, "desc"),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );
    assert!(matches!(res, Err(Ok(ContractError::InvalidState))));
}

#[test]
fn test_auto_release_after_dispute_deadline() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);
    client.set_protocol_fee(&admin, &200_u32);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-AUTO-3"));
    env.ledger().set_timestamp(1_700_000_000);
    client.record_delivery(&admin, &id);

    let escrow = client.get_escrow(&id);
    let delivered_at = escrow.delivered_at;

    // Advance time past the 48h post-delivery release window.
    env.ledger().set_timestamp(delivered_at + 172_801);

    client.auto_release(&id);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);
    // 2% fee on 1000 = 20 routed to the fee collector, 980 to seller
    assert_eq!(get_balance(&env, &token, &seller), 980);
    assert_eq!(get_balance(&env, &token, &fee_collector), 20);
}

#[test]
fn test_auto_release_before_dispute_deadline_fails() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &fee_collector, &0_u32);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &None::<Address>, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.mark_shipped(&seller, &id, &SorobanString::from_str(&env, "TRACK-AUTO-4"));
    env.ledger().set_timestamp(1_700_000_000);
    client.record_delivery(&admin, &id);

    let escrow = client.get_escrow(&id);
    let delivered_at = escrow.delivered_at;

    // Advance time before the 48h post-delivery release window has elapsed.
    env.ledger().set_timestamp(delivered_at + 3600);

    let res = client.try_auto_release(&id);
    assert!(matches!(res, Err(Ok(ContractError::ShippingWindowNotElapsed))));
}
