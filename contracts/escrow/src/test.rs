#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, token, Address, Bytes, Env};

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

    (env, seller, buyer, resolver, token_admin, token_address, fee_collector)
}

fn mint_tokens(env: &Env, token: &Address, to: &Address, amount: i128) {
    let sac = token::StellarAssetClient::new(env, token);
    sac.mint(to, &amount);
}

fn get_balance(env: &Env, token: &Address, user: &Address) -> i128 {
    let tc = token::Client::new(env, token);
    tc.balance(user)
}

#[test]
fn test_create_escrow() {
    let (env, seller, _buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &200_u32, &3600_u64);
    assert_eq!(id, 1);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.seller, seller);
    assert_eq!(escrow.resolver, resolver);
    assert_eq!(escrow.token, token);
    assert_eq!(escrow.amount, 100);
    assert_eq!(escrow.fee_bps, 200);
    assert_eq!(escrow.shipping_window, 3600);
    assert_eq!(escrow.state, EscrowState::Pending);
    assert!(escrow.buyer.is_none());
}

#[test]
fn test_fund_escrow() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Funded);
    assert_eq!(escrow.buyer, Some(buyer.clone()));
    assert_eq!(escrow.funded_at, 0);

    assert_eq!(get_balance(&env, &token, &buyer), 900);
    assert_eq!(get_balance(&env, &token, &contract_id), 100);
}

#[test]
fn test_confirm_delivery() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.confirm_delivery(&id);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);
    // 2% fee on 1000 = 20 to collector, 980 to seller
    assert_eq!(get_balance(&env, &token, &seller), 980);
    assert_eq!(get_balance(&env, &token, &fee_collector), 20);
    assert_eq!(get_balance(&env, &token, &contract_id), 0);
}

#[test]
fn test_raise_and_resolve_dispute_release_to_seller() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.raise_dispute(&id, &make_evidence_hash(&env));

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Disputed);

    client.resolve_dispute(&id, &true);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);
    // 2% fee on 1000 = 20 to collector, 980 to seller
    assert_eq!(get_balance(&env, &token, &seller), 980);
    assert_eq!(get_balance(&env, &token, &fee_collector), 20);
}

#[test]
fn test_raise_and_resolve_dispute_refund_buyer() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.raise_dispute(&id, &make_evidence_hash(&env));
    client.resolve_dispute(&id, &false);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Refunded);
    // 2% fee on 1000 = 20 to collector, 980 back to buyer
    assert_eq!(get_balance(&env, &token, &buyer), 980);
    assert_eq!(get_balance(&env, &token, &fee_collector), 20);
}

#[test]
fn test_auto_release() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &resolver, &token, &1000_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);

    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

    client.auto_release(&id);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);
    // 2% fee on 1000 = 20 to collector, 980 to seller
    assert_eq!(get_balance(&env, &token, &seller), 980);
    assert_eq!(get_balance(&env, &token, &fee_collector), 20);
}

#[test]
#[should_panic(expected = "escrow not pending")]
fn test_fund_non_pending_escrow_fails() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.fund_escrow(&id, &buyer);
}

#[test]
#[should_panic(expected = "shipping window not elapsed")]
fn test_auto_release_before_window_fails() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &200_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);

    client.auto_release(&id);
}

#[test]
#[should_panic(expected = "evidence_hash must be exactly 32 bytes")]
fn test_raise_dispute_invalid_evidence_hash_rejected() {
    let (env, seller, buyer, resolver, _admin, token) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &3600_u64);
    client.fund_escrow(&id, &buyer);

    // 16-byte hash — must be rejected before any storage write
    let short_hash = Bytes::from_array(&env, &[0u8; 16]);
    client.raise_dispute(&id, &short_hash);
}

#[test]
#[should_panic(expected = "escrow not funded")]
fn test_raise_dispute_only_once() {
    let (env, seller, buyer, resolver, _admin, token) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);

    mint_tokens(&env, &token, &buyer, 1000);

    let id = client.create_escrow(&seller, &resolver, &token, &100_i128, &3600_u64);
    client.fund_escrow(&id, &buyer);

    // First dispute — succeeds, state transitions to Disputed
    client.raise_dispute(&id, &make_evidence_hash(&env));

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Disputed);

    // Second dispute on the same escrow — must panic because state is no longer Funded
    client.raise_dispute(&id, &make_evidence_hash(&env));
}

#[test]
fn test_multiple_escrows() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    mint_tokens(&env, &token, &buyer, 2000);

    let id1 = client.create_escrow(&seller, &resolver, &token, &100_i128, &200_u32, &3600_u64);
    let id2 = client.create_escrow(&seller, &resolver, &token, &200_i128, &200_u32, &7200_u64);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    client.fund_escrow(&id1, &buyer);
    client.fund_escrow(&id2, &buyer);

    assert_eq!(get_balance(&env, &token, &buyer), 1700);
}

// ---------------------------------------------------------------------------
// Multi-asset / non-USDC SEP-41 token tests
// ---------------------------------------------------------------------------

/// Register a second, independent SEP-41 token (simulates any non-USDC asset).
/// Returns (token_address, token_admin).
fn register_alt_token(env: &Env) -> (Address, Address) {
    let admin = Address::generate(env);
    let token_address = env.register_stellar_asset_contract(admin.clone());
    (token_address, admin)
}

/// Verify that `create_escrow` accepts an arbitrary non-USDC token address and
/// stores it correctly in contract state.
#[test]
fn test_create_escrow_with_non_usdc_token() {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let resolver = Address::generate(&env);
    let (alt_token, _alt_admin) = register_alt_token(&env);

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);

    let id = client.create_escrow(&seller, &resolver, &alt_token, &500_i128, &7200_u64);
    assert_eq!(id, 1);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.token, alt_token);
    assert_eq!(escrow.amount, 500);
    assert_eq!(escrow.shipping_window, 7200);
    assert_eq!(escrow.state, EscrowState::Pending);
    assert!(escrow.buyer.is_none());
}

/// Full happy-path (fund → confirm delivery) using a non-USDC SEP-41 token.
/// Verifies that token transfers and storage updates work end-to-end without
/// any hardcoded token address assumptions.
#[test]
fn test_fund_and_confirm_delivery_with_non_usdc_token() {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let (alt_token, _alt_admin) = register_alt_token(&env);

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);

    mint_tokens(&env, &alt_token, &buyer, 1_000);

    let id = client.create_escrow(&seller, &resolver, &alt_token, &300_i128, &3600_u64);
    client.fund_escrow(&id, &buyer);

    // Buyer balance reduced; contract holds the funds.
    assert_eq!(get_balance(&env, &alt_token, &buyer), 700);
    assert_eq!(get_balance(&env, &alt_token, &contract_id), 300);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Funded);
    assert_eq!(escrow.token, alt_token);

    client.confirm_delivery(&id);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);
    // Funds released to seller; contract balance zeroed.
    assert_eq!(get_balance(&env, &alt_token, &seller), 300);
    assert_eq!(get_balance(&env, &alt_token, &contract_id), 0);
}

/// Dispute raised and resolved in favour of the seller using a non-USDC token.
#[test]
fn test_dispute_resolved_to_seller_with_non_usdc_token() {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let (alt_token, _alt_admin) = register_alt_token(&env);

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);

    mint_tokens(&env, &alt_token, &buyer, 1_000);

    let id = client.create_escrow(&seller, &resolver, &alt_token, &400_i128, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.raise_dispute(&id, &make_evidence_hash(&env));

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Disputed);

    client.resolve_dispute(&id, &true);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);
    assert_eq!(get_balance(&env, &alt_token, &seller), 400);
    assert_eq!(get_balance(&env, &alt_token, &contract_id), 0);
}

/// Dispute raised and resolved in favour of the buyer (refund) using a non-USDC token.
#[test]
fn test_dispute_refunded_to_buyer_with_non_usdc_token() {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let (alt_token, _alt_admin) = register_alt_token(&env);

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);

    mint_tokens(&env, &alt_token, &buyer, 1_000);

    let id = client.create_escrow(&seller, &resolver, &alt_token, &400_i128, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.raise_dispute(&id, &make_evidence_hash(&env));
    client.resolve_dispute(&id, &false);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Refunded);
    // Buyer gets full refund; seller and contract receive nothing.
    assert_eq!(get_balance(&env, &alt_token, &buyer), 1_000);
    assert_eq!(get_balance(&env, &alt_token, &seller), 0);
    assert_eq!(get_balance(&env, &alt_token, &contract_id), 0);
}

/// Auto-release after shipping window elapses using a non-USDC token.
#[test]
fn test_auto_release_with_non_usdc_token() {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let (alt_token, _alt_admin) = register_alt_token(&env);

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);

    mint_tokens(&env, &alt_token, &buyer, 1_000);

    let shipping_window: u64 = 86_400; // 24 hours
    let id = client.create_escrow(&seller, &resolver, &alt_token, &250_i128, &shipping_window);
    client.fund_escrow(&id, &buyer);

    // Advance ledger time past the shipping window.
    env.ledger().set_timestamp(env.ledger().timestamp() + shipping_window + 1);

    client.auto_release(&id);

    let escrow = client.get_escrow(&id);
    assert_eq!(escrow.state, EscrowState::Completed);
    assert_eq!(get_balance(&env, &alt_token, &seller), 250);
    assert_eq!(get_balance(&env, &alt_token, &contract_id), 0);
}

/// Two concurrent escrows each using a *different* non-USDC SEP-41 token.
/// Verifies that the contract tracks per-escrow token addresses independently
/// and that transfers are isolated — no cross-token contamination.
#[test]
fn test_multi_asset_concurrent_escrows_different_tokens() {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);
    let resolver = Address::generate(&env);

    // Two completely independent SEP-41 tokens.
    let (token_a, _admin_a) = register_alt_token(&env);
    let (token_b, _admin_b) = register_alt_token(&env);

    // Sanity: the two token addresses must differ.
    assert_ne!(token_a, token_b);

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);

    mint_tokens(&env, &token_a, &buyer_a, 1_000);
    mint_tokens(&env, &token_b, &buyer_b, 2_000);

    // Escrow 1: token_a, amount 150
    let id1 = client.create_escrow(&seller, &resolver, &token_a, &150_i128, &3600_u64);
    // Escrow 2: token_b, amount 500
    let id2 = client.create_escrow(&seller, &resolver, &token_b, &500_i128, &3600_u64);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    client.fund_escrow(&id1, &buyer_a);
    client.fund_escrow(&id2, &buyer_b);

    // Intermediate balance checks — each token is tracked independently.
    assert_eq!(get_balance(&env, &token_a, &buyer_a), 850);
    assert_eq!(get_balance(&env, &token_b, &buyer_b), 1_500);
    assert_eq!(get_balance(&env, &token_a, &contract_id), 150);
    assert_eq!(get_balance(&env, &token_b, &contract_id), 500);

    // Settle escrow 1 via confirm_delivery.
    client.confirm_delivery(&id1);
    // Settle escrow 2 via dispute → refund to buyer.
    client.raise_dispute(&id2, &make_evidence_hash(&env));
    client.resolve_dispute(&id2, &false);

    // Final state assertions.
    let escrow1 = client.get_escrow(&id1);
    let escrow2 = client.get_escrow(&id2);
    assert_eq!(escrow1.state, EscrowState::Completed);
    assert_eq!(escrow2.state, EscrowState::Refunded);

    // token_a: seller received 150; contract zeroed.
    assert_eq!(get_balance(&env, &token_a, &seller), 150);
    assert_eq!(get_balance(&env, &token_a, &contract_id), 0);

    // token_b: buyer_b refunded in full; seller received nothing from token_b.
    assert_eq!(get_balance(&env, &token_b, &buyer_b), 2_000);
    assert_eq!(get_balance(&env, &token_b, &seller), 0);
    assert_eq!(get_balance(&env, &token_b, &contract_id), 0);
}

/// Sequential escrows reusing the same non-USDC token verify that the escrow
/// counter increments correctly and storage slots remain independent.
#[test]
fn test_sequential_escrows_same_non_usdc_token() {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let resolver = Address::generate(&env);
    let (alt_token, _alt_admin) = register_alt_token(&env);

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);

    mint_tokens(&env, &alt_token, &buyer, 5_000);

    // Create and fully settle three escrows in sequence.
    for (i, amount) in [100_i128, 200_i128, 300_i128].iter().enumerate() {
        let expected_id = (i as u32) + 1;
        let id = client.create_escrow(&seller, &resolver, &alt_token, amount, &3600_u64);
        assert_eq!(id, expected_id);

        client.fund_escrow(&id, &buyer);
        client.confirm_delivery(&id);

        let escrow = client.get_escrow(&id);
        assert_eq!(escrow.state, EscrowState::Completed);
        assert_eq!(escrow.token, alt_token);
    }

    // Seller received 100 + 200 + 300 = 600 tokens total.
    assert_eq!(get_balance(&env, &alt_token, &seller), 600);
    // Buyer spent exactly 600 tokens.
    assert_eq!(get_balance(&env, &alt_token, &buyer), 4_400);
    // Contract holds nothing after all settlements.
    assert_eq!(get_balance(&env, &alt_token, &contract_id), 0);
#[test]
fn test_zero_fee_no_collector_transfer() {
    let (env, seller, buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    mint_tokens(&env, &token, &buyer, 1000);

    // 0 bps fee — entire amount goes to seller
    let id = client.create_escrow(&seller, &resolver, &token, &1000_i128, &0_u32, &3600_u64);
    client.fund_escrow(&id, &buyer);
    client.confirm_delivery(&id);

    assert_eq!(get_balance(&env, &token, &seller), 1000);
    assert_eq!(get_balance(&env, &token, &fee_collector), 0);
}

#[test]
fn test_get_fee_config() {
    let (env, _seller, _buyer, _resolver, _admin, _token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    let config = client.get_fee_config();
    assert_eq!(config.collector, fee_collector);
    assert_eq!(config.max_fee_bps, 300);
}

#[test]
#[should_panic(expected = "fee exceeds maximum")]
fn test_fee_exceeds_max_bps_fails() {
    let (env, seller, _buyer, resolver, _admin, token, fee_collector) = setup_env();

    let contract_id = env.register(Escrow, ());
    let client = super::EscrowClient::new(&env, &contract_id);
    client.initialize(&fee_collector);

    // 301 bps exceeds MAX_FEE_BPS (300)
    client.create_escrow(&seller, &resolver, &token, &1000_i128, &301_u32, &3600_u64);
}
