#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, BytesN, Env, String,
    };

    use crate::{
        errors::ContractError,
        types::EscrowState,
        Escrow, EscrowClient,
    };

    const SHIPPING_WINDOW: u64 = 604_800;
    const GRACE_PERIOD_SECS: u64 = 604_800;

    fn setup_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    fn setup_contract(env: &Env) -> (EscrowClient, Address, Address, Address, Address) {
        let seller   = Address::generate(env);
        let buyer    = Address::generate(env);
        let resolver = Address::generate(env);
        let admin    = Address::generate(env);
        let fee_collector = Address::generate(env);

        let token_admin = Address::generate(env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
        let token_client = soroban_sdk::token::StellarAssetClient::new(env, &token_id);
        token_client.mint(&buyer, &1_000_000_000_i128);

        let contract_id = env.register(Escrow, ());
        let client = EscrowClient::new(env, &contract_id);

        client.initialize(&admin, &fee_collector, &0_i128, &0_u32);

        (client, seller, buyer, resolver, token_id)
    }

    fn create_fund_ship(
        env: &Env,
        client: &EscrowClient,
        seller: &Address,
        buyer: &Address,
        resolver: &Address,
        token: &Address,
    ) -> u64 {
        let escrow_id = client.create_escrow(
            seller,
            buyer,
            resolver,
            token,
            &100_000_000_i128,
            &0_u32,
            &SHIPPING_WINDOW,
        );
        client.fund_escrow(&escrow_id, buyer);
        client.mark_shipped(
            seller,
            &escrow_id,
            &soroban_sdk::String::from_str(env, "TRACK-001"),
        );
        escrow_id
    }

    #[test]
    fn test_dispute_blocked_after_fallback_deadline_no_record_delivery() {
        let env = setup_env();
        let (client, seller, buyer, resolver, token) = setup_contract(&env);

        env.ledger().set_timestamp(1_000_000);
        let escrow_id = create_fund_ship(&env, &client, &seller, &buyer, &resolver, &token);

        env.ledger().set_timestamp(1_000_000 + 7_776_000);

        let evidence: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
        let result = client.try_raise_dispute(
            &buyer,
            &escrow_id,
            &soroban_sdk::Symbol::new(&env, "non_delivery"),
            &String::from_str(&env, "Item never arrived after 90 days"),
            &evidence,
        );

        assert_eq!(
            result,
            Err(Ok(ContractError::DisputeWindowClosed)),
            "raise_dispute must revert after fallback deadline"
        );
    }

    #[test]
    fn test_dispute_allowed_just_before_fallback_deadline() {
        let env = setup_env();
        let (client, seller, buyer, resolver, token) = setup_contract(&env);

        let ship_time: u64 = 1_000_000;
        env.ledger().set_timestamp(ship_time);
        let escrow_id = create_fund_ship(&env, &client, &seller, &buyer, &resolver, &token);

        let just_before = ship_time + SHIPPING_WINDOW + GRACE_PERIOD_SECS - 1;
        env.ledger().set_timestamp(just_before);

        let evidence: BytesN<32> = BytesN::from_array(&env, &[2u8; 32]);
        let result = client.try_raise_dispute(
            &buyer,
            &escrow_id,
            &soroban_sdk::Symbol::new(&env, "non_delivery"),
            &String::from_str(&env, "Item not arrived but still within window"),
            &evidence,
        );

        assert!(
            result.is_ok(),
            "raise_dispute should succeed before fallback deadline, got: {:?}",
            result
        );
    }

    #[test]
    fn test_dispute_blocked_exactly_at_fallback_deadline() {
        let env = setup_env();
        let (client, seller, buyer, resolver, token) = setup_contract(&env);

        let ship_time: u64 = 1_000_000;
        env.ledger().set_timestamp(ship_time);
        let escrow_id = create_fund_ship(&env, &client, &seller, &buyer, &resolver, &token);

        let deadline = ship_time + SHIPPING_WINDOW + GRACE_PERIOD_SECS;
        env.ledger().set_timestamp(deadline);

        let evidence: BytesN<32> = BytesN::from_array(&env, &[3u8; 32]);
        let result = client.try_raise_dispute(
            &buyer,
            &escrow_id,
            &soroban_sdk::Symbol::new(&env, "non_delivery"),
            &String::from_str(&env, "At the boundary"),
            &evidence,
        );

        assert_eq!(
            result,
            Err(Ok(ContractError::DisputeWindowClosed)),
            "raise_dispute must revert at the exact fallback deadline"
        );
    }

    #[test]
    fn test_regression_issue_infinite_dispute_window() {
        let env = setup_env();
        let (client, seller, buyer, resolver, token) = setup_contract(&env);

        env.ledger().set_timestamp(0);
        let escrow_id = create_fund_ship(&env, &client, &seller, &buyer, &resolver, &token);

        env.ledger().set_timestamp(7_776_000);

        let evidence: BytesN<32> = BytesN::from_array(&env, &[0xAAu8; 32]);
        let result = client.try_raise_dispute(
            &buyer,
            &escrow_id,
            &soroban_sdk::Symbol::new(&env, "non_delivery"),
            &String::from_str(&env, "90-day late dispute attempt"),
            &evidence,
        );

        assert_eq!(
            result,
            Err(Ok(ContractError::DisputeWindowClosed)),
            "[REGRESSION] Infinite dispute window bug must be fixed"
        );
    }
}