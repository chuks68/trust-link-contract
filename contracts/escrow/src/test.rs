#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, token, Address, Env};

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
    client.raise_dispute(&id);

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
    client.raise_dispute(&id);
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
