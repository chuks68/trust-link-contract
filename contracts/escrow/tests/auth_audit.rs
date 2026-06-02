#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _},
    token, Address, Env,
};
use trustlink_escrow::{
    ContractError, Escrow, EscrowClient, EscrowData, EscrowState,
};

#[test]
fn test_unauthorized_attacker_cannot_fund_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let legitimate_buyer = Address::generate(&env);
    let attacker = Address::generate(&env);
    let resolver = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_addr = sac.address();

    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    client.initialize(&admin, &fee_collector, &100_u32);

    let amount = 1000;
    
    // Create the escrow.
    let escrow_id = client.create_escrow(&seller, &None::<Address>, &resolver, &token_addr, &amount, &100_u32, &3600_u64);

    // Pre-assign the legitimate buyer in contract storage.
    use trustlink_escrow::types::DataKey;
    let mut escrow: EscrowData = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&DataKey::Escrow(escrow_id))
    })
    .expect("escrow exists");
    
    escrow.buyer = Some(legitimate_buyer.clone());
    
    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&DataKey::Escrow(escrow_id), &escrow);
    });

    // Mint tokens to the attacker and approve the contract.
    token::StellarAssetClient::new(&env, &token_addr).mint(&attacker, &amount);
    
    // Attacker attempts to fund the escrow using their own address.
    let result = client.try_fund_escrow(&escrow_id, &attacker);

    // It must return NotAuthorized.
    assert!(
        matches!(result, Err(Ok(ContractError::NotAuthorized))),
        "Expected try_fund_escrow to return Err(Ok(ContractError::NotAuthorized)) but got {:?}",
        result
    );

    // The escrow must remain Pending.
    let escrow_after = client.get_escrow(&escrow_id);
    assert_eq!(escrow_after.state, EscrowState::Pending);
    assert_eq!(escrow_after.buyer, Some(legitimate_buyer));

    // Attacker balance must remain unchanged (no tokens debited).
    let attacker_balance = token::Client::new(&env, &token_addr).balance(&attacker);
    assert_eq!(attacker_balance, amount);
}
