#![cfg(test)]

use crate::{test_helpers::setup_contract, ContractError, EscrowClient, FeeConfig};
use soroban_sdk::Env;

#[test]
fn test_fee_updates_are_independent() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    client.set_arbitration_fee(&admin, &250_u32);
    client.set_protocol_fee(&admin, &1000_u32);

    let config = client.get_fee_config();
    assert_eq!(config.protocol_fee_bps, 1000);
    assert_eq!(config.arbitration_fee_bps, 250);

    client.set_arbitration_fee(&admin, &500_u32);
    let updated = client.get_fee_config();
    assert_eq!(updated.protocol_fee_bps, 1000);
    assert_eq!(updated.arbitration_fee_bps, 500);
}

#[test]
fn test_fee_bounds_are_enforced() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    let protocol_result = client.try_set_protocol_fee(&admin, &10_001_u32);
    assert!(matches!(protocol_result, Err(Ok(ContractError::FeeExceedsMax))));

    let arbitration_result = client.try_set_arbitration_fee(&admin, &10_001_u32);
    assert!(matches!(arbitration_result, Err(Ok(ContractError::FeeExceedsMax))));
}

#[test]
fn test_fee_config_persists_across_calls() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client, admin, _fee_collector) = setup_contract(&env);

    client.set_protocol_fee(&admin, &600_u32);
    client.set_arbitration_fee(&admin, &300_u32);

    let expected = FeeConfig {
        protocol_fee_bps: 600,
        arbitration_fee_bps: 300,
    };
    assert_eq!(client.get_fee_config(), expected);

    let fresh_client = EscrowClient::new(&env, &client.address);
    assert_eq!(fresh_client.get_fee_config(), expected);
}
