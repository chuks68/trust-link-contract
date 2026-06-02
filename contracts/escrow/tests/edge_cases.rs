#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _},
    token, Address, Env, String as SorobanString,
};
use trustlink_escrow::{
    ContractError, Escrow, EscrowClient, EscrowState,
};

#[test]
fn test_auto_release_before_record_delivery_reverts() {
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
    client.initialize(&admin, &fee_collector, &100_u32);

    let amount = 1000;
    token::StellarAssetClient::new(&env, &token_addr).mint(&buyer, &amount);

    // 1. Create Escrow
    let escrow_id = client.create_escrow(&seller, &None::<Address>, &resolver, &token_addr, &amount, &100_u32, &3600_u64);

    // 2. Fund Escrow
    client.fund_escrow(&escrow_id, &buyer);

    // 3. Mark Shipped
    let tracking = SorobanString::from_str(&env, "TRK-EDGE-001");
    client.mark_shipped(&seller, &escrow_id, &tracking);

    // 4. Try auto_release without recording delivery
    let result = client.try_auto_release(&escrow_id);

    // It must return DeliveryNotRecorded
    assert!(
        matches!(result, Err(Ok(ContractError::DeliveryNotRecorded))),
        "Expected try_auto_release to return DeliveryNotRecorded but got {:?}",
        result
    );

    // The state must remain Shipped
    let escrow_after = client.get_escrow(&escrow_id);
    assert_eq!(escrow_after.state, EscrowState::Shipped);
}
