#![cfg(test)]
//! Integration test for the 48-hour auto-release path (#16):
//! create -> fund -> ship -> (advance time past the dispute window) -> auto_release.
//!
//! Verifies auto_release fails before the dispute window has elapsed and
//! succeeds afterward, and that funds end up at the seller with the escrow
//! advanced to Completed.

use crate::{ContractError, Escrow, EscrowClient, EscrowState};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, Env, String as SorobanString,
};

const DISPUTE_WINDOW_SECS: u64 = 172_800; // 48h, matches the contract constant.

struct Fx {
    env: Env,
    client: EscrowClient<'static>,
    escrow_id: u64,
    seller: Address,
    funded_at: u64,
    token_addr: Address,
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
    client.initialize(&admin, &fee_collector, &0_i128);

    let amount: i128 = 1_000;
    // shipping_window=0 isolates the dispute-window assertion the issue cares about.
    let escrow_id = client.create_escrow(&seller, &resolver, &token_addr, &amount, &0_u32, &0_u64);
    token::StellarAssetClient::new(&env, &token_addr).mint(&buyer, &amount);
    client.fund_escrow(&escrow_id, &buyer);
    client.mark_shipped(&escrow_id, &soroban_sdk::String::from_str(&env, "TRACK001"));

    // The contract sets dispute_deadline = funded_at + DISPUTE_WINDOW; pin
    // funded_at so the assertions below are unambiguous.
    use crate::{DataKey, EscrowData};
    let data: EscrowData = env
        .as_contract(&client.address, || env.storage().persistent().get(&DataKey::Escrow(escrow_id)))
        .expect("escrow exists");
    Fx { env, client, escrow_id, seller, funded_at: data.funded_at, token_addr }
}

#[test]
fn auto_release_before_48_hours_is_rejected() {
    let fx = setup_funded_and_shipped();
    // One second before the 48h window closes.
    fx.env.ledger().with_mut(|li| li.timestamp = fx.funded_at + DISPUTE_WINDOW_SECS - 1);

    assert_eq!(
        fx.client.try_auto_release(&fx.escrow_id),
        Err(Ok(ContractError::DisputeWindowClosed)),
        "auto_release must be rejected while the dispute window is still open",
    );
}

#[test]
fn auto_release_after_48_hours_succeeds_and_pays_the_seller() {
    let fx = setup_funded_and_shipped();
    // One second past the 48h window.
    fx.env.ledger().with_mut(|li| li.timestamp = fx.funded_at + DISPUTE_WINDOW_SECS + 1);

    fx.client.auto_release(&fx.escrow_id);

    // Seller received the full amount (fee_bps was 0).
    let token_client = token::TokenClient::new(&fx.env, &fx.token_addr);
    assert_eq!(token_client.balance(&fx.seller), 1_000);

    // State advanced to Completed.
    use crate::{DataKey, EscrowData};
    let after: EscrowData = fx.env
        .as_contract(&fx.client.address, || {
            fx.env.storage().persistent().get(&DataKey::Escrow(fx.escrow_id))
        })
        .expect("escrow exists");
    assert_eq!(after.state, EscrowState::Completed);
}
