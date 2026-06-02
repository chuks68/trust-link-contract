#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };
    use crate::{Escrow, EscrowClient};

    const DISPUTE_WINDOW: u64 = 172_800;

    fn setup_contract(env: &Env) -> (EscrowClient, Address, Address) {
        let admin = Address::generate(env);
        let fee_collector = Address::generate(env);

        let token_admin = Address::generate(env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
        let token_client = soroban_sdk::token::StellarAssetClient::new(env, &token_id);

        let contract_id = env.register(Escrow, ());
        let client = EscrowClient::new(env, &contract_id);
        client.initialize(&admin, &fee_collector, &0_i128, &0_u32);

        (client, admin, fee_collector)
    }

    /// Core regression: fee snapshotted at create_escrow time must be used
    /// at confirm_delivery even if admin raises the global fee in between.
    #[test]
    fn test_fee_change_does_not_affect_funded_escrow() {
        let env = Env::default();
        env.mock_all_auths();

        let seller   = Address::generate(&env);
        let buyer    = Address::generate(&env);
        let resolver = Address::generate(&env);
        let admin    = Address::generate(&env);
        let fee_collector = Address::generate(&env);

        let token_admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
        let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
        token_client.mint(&buyer, &1_000_000_000_i128);

        let contract_id = env.register(Escrow, ());
        let client = EscrowClient::new(&env, &contract_id);

        // Initialize with 1% fee (100 bps).
        client.initialize(&admin, &fee_collector, &0_i128, &0_u32);
        client.set_protocol_fee(&admin, &100_u32);

        let amount = 1_000_000_i128;

        // Create escrow at 1% fee — this snapshots fee_bps = 100 into EscrowData.
        let escrow_id = client.create_escrow(
            &seller,
            &buyer,
            &resolver,
            &token_id,
            &amount,
            &100_u32,
            &604_800_u64,
        );

        client.fund_escrow(&escrow_id, &buyer);

        // Admin raises fee to 3% between fund and release.
        client.set_protocol_fee(&admin, &300_u32);

        // Advance past dispute window so buyer can confirm.
        env.ledger().set_timestamp(DISPUTE_WINDOW + 1);
        client.confirm_delivery(&buyer, &escrow_id);

        let tc = soroban_sdk::token::Client::new(&env, &token_id);
        let seller_balance = tc.balance(&seller);
        let fee_collector_balance = tc.balance(&fee_collector);

        // Expected: 1% fee (snapshotted), NOT 3% (live).
        let expected_fee = amount * 100 / 10_000; // 100 bps = 1%
        let expected_net = amount - expected_fee;

        assert_eq!(
            seller_balance, expected_net,
            "seller should receive amount minus snapshotted 1% fee, not live 3% fee"
        );
        assert_eq!(
            fee_collector_balance, expected_fee,
            "fee collector should receive snapshotted 1% fee"
        );
    }

    /// Auto-release must also use the snapshotted fee, not the live global fee.
    #[test]
    fn test_fee_change_does_not_affect_auto_release() {
        let env = Env::default();
        env.mock_all_auths();

        let seller   = Address::generate(&env);
        let buyer    = Address::generate(&env);
        let resolver = Address::generate(&env);
        let admin    = Address::generate(&env);
        let fee_collector = Address::generate(&env);

        let token_admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
        let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
        token_client.mint(&buyer, &1_000_000_000_i128);

        let contract_id = env.register(Escrow, ());
        let client = EscrowClient::new(&env, &contract_id);

        client.initialize(&admin, &fee_collector, &0_i128, &0_u32);
        client.set_protocol_fee(&admin, &100_u32);

        let amount = 1_000_000_i128;
        let shipping_window: u64 = 604_800;

        let escrow_id = client.create_escrow(
            &seller,
            &buyer,
            &resolver,
            &token_id,
            &amount,
            &100_u32,
            &shipping_window,
        );

        client.fund_escrow(&escrow_id, &buyer);
        client.mark_shipped(&seller, &escrow_id, &soroban_sdk::String::from_str(&env, "TRACK-001"));
        client.record_delivery(&admin, &escrow_id);

        // Admin raises fee to 3% before auto_release is triggered.
        client.set_protocol_fee(&admin, &300_u32);

        // Advance past delivery release window.
        env.ledger().set_timestamp(shipping_window + DISPUTE_WINDOW + 1);
        client.auto_release(&escrow_id);

        let tc = soroban_sdk::token::Client::new(&env, &token_id);
        let seller_balance = tc.balance(&seller);
        let fee_collector_balance = tc.balance(&fee_collector);

        let expected_fee = amount * 100 / 10_000;
        let expected_net = amount - expected_fee;

        assert_eq!(
            seller_balance, expected_net,
            "auto_release should use snapshotted 1% fee, not live 3%"
        );
        assert_eq!(
            fee_collector_balance, expected_fee,
            "fee collector should receive snapshotted 1% fee on auto_release"
        );
    }
}