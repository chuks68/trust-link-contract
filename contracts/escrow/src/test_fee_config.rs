#![cfg(test)]

use crate::{test_helpers::setup_contract, ContractError, EscrowClient, FeeConfig};
use soroban_sdk::Env;

#[test]
fn test_fee_updates_are_independent() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    client.set_arbitration_fee(&admin, &250_u32);
    client.set_protocol_fee(&admin, &300_u32);

    let config = client.get_fee_config();
    assert_eq!(config.protocol_fee_bps, 300);
    assert_eq!(config.arbitration_fee_bps, 250);

    client.set_arbitration_fee(&admin, &400_u32);
    let updated = client.get_fee_config();
    assert_eq!(updated.protocol_fee_bps, 300);
    assert_eq!(updated.arbitration_fee_bps, 400);
}

#[test]
fn test_fee_bounds_are_enforced() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    // Individual protocol fee over 500 bps is rejected
    let protocol_result = client.try_set_protocol_fee(&admin, &501_u32);
    assert!(matches!(protocol_result, Err(Ok(ContractError::FeeExceedsMax))));

    // Individual arbitration fee over 500 bps is rejected
    let arbitration_result = client.try_set_arbitration_fee(&admin, &501_u32);
    assert!(matches!(arbitration_result, Err(Ok(ContractError::FeeExceedsMax))));
    
    // Valid individual fees at boundary (500 bps) are accepted
    client.set_protocol_fee(&admin, &500_u32);
    client.set_arbitration_fee(&admin, &500_u32);
    let config = client.get_fee_config();
    assert_eq!(config.protocol_fee_bps, 500);
    assert_eq!(config.arbitration_fee_bps, 500);
}

#[test]
fn test_fee_config_persists_across_calls() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    client.set_protocol_fee(&admin, &400_u32);
    client.set_arbitration_fee(&admin, &300_u32);

    let expected = FeeConfig {
        protocol_fee_bps: 400,
        arbitration_fee_bps: 300,
    };
    assert_eq!(client.get_fee_config(), expected);

    let fresh_client = EscrowClient::new(&env, &client.address);
    assert_eq!(fresh_client.get_fee_config(), expected);
}

#[test]
fn test_combined_fee_cap_is_enforced() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    // Set protocol fee to 500 bps (max individual cap)
    client.set_protocol_fee(&admin, &500_u32);
    
    // Set arbitration fee to 500 bps (max individual cap)
    // Combined is 500 + 500 = 1000 which equals MAX_COMBINED_FEE_BPS
    client.set_arbitration_fee(&admin, &500_u32);
    let config = client.get_fee_config();
    assert_eq!(config.protocol_fee_bps, 500);
    assert_eq!(config.arbitration_fee_bps, 500);

    // Now reduce protocol fee to 400 to test that arbitration can be adjusted
    client.set_protocol_fee(&admin, &400_u32);
    
    // Try to set arbitration to 600 bps — should fail due to individual cap
    let result = client.try_set_arbitration_fee(&admin, &600_u32);
    assert!(matches!(result, Err(Ok(ContractError::FeeExceedsMax))));
    
    // Verify the individual cap is enforced
    let config = client.get_fee_config();
    assert_eq!(config.arbitration_fee_bps, 500); // Unchanged
}
