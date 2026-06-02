#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events as _},
    token, Address, Env, String as SorobanString, Symbol, TryFromVal, Val,
};
use trustlink_escrow::{
    Escrow, EscrowClient, EscrowCreated, EscrowFunded, EscrowShipped, EscrowCompleted,
    EscrowState,
};

struct Fx {
    env: Env,
    client: EscrowClient<'static>,
    contract_id: Address,
    seller: Address,
    buyer: Address,
    resolver: Address,
    fee_collector: Address,
    token_addr: Address,
}

fn setup() -> Fx {
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
    client.initialize(&admin, &fee_collector, &100_u32); // 100 bps = 1% fee

    Fx {
        env,
        client,
        contract_id,
        seller,
        buyer,
        resolver,
        token_addr,
        fee_collector,
    }
}

fn has_event<T, F>(env: &Env, contract_id: &Address, topic: &str, predicate: F) -> bool
where
    T: TryFromVal<Env, Val>,
    F: Fn(&T) -> bool,
{
    let expected_topic = Symbol::new(env, topic);
    env.events()
        .all()
        .filter_by_contract(contract_id)
        .events()
        .iter()
        .any(|event| match &event.body {
            soroban_sdk::xdr::ContractEventBody::V0(v0) => {
                let Some(topic) = v0.topics.iter().next() else {
                    return false;
                };

                let Ok(topic) = Symbol::try_from_val(env, topic) else {
                    return false;
                };
                if topic != expected_topic {
                    return false;
                }

                let Ok(data) = Val::try_from_val(env, &v0.data) else {
                    return false;
                };

                T::try_from_val(env, &data)
                    .map(|event| predicate(&event))
                    .unwrap_or(false)
            }
            _ => false,
        })
}

#[test]
fn test_happy_path_escrow_lifecycle() {
    let fx = setup();
    let amount: i128 = 10_000;
    
    // Mint token balance to buyer.
    token::StellarAssetClient::new(&fx.env, &fx.token_addr).mint(&fx.buyer, &amount);
    assert_eq!(token::Client::new(&fx.env, &fx.token_addr).balance(&fx.buyer), amount);

    // 1. Create Escrow
    let escrow_id = fx.client.create_escrow(
        &fx.seller,
        &None::<soroban_sdk::Address>,
        &fx.resolver,
        &fx.token_addr,
        &amount,
        &100_u32, // 1% escrow fee
        &3600_u64, // shipping window
    );

    let escrow_before = fx.client.get_escrow(&escrow_id);
    assert_eq!(escrow_before.state, EscrowState::Pending);
    assert_eq!(escrow_before.amount, amount);
    assert!(has_event::<EscrowCreated, _>(&fx.env, &fx.contract_id, "escrow_created", |e| {
        e.escrow_id == escrow_id && e.seller == fx.seller && e.amount == amount
    }));

    // 2. Fund Escrow
    fx.client.fund_escrow(&escrow_id, &fx.buyer);
    let escrow_funded = fx.client.get_escrow(&escrow_id);
    assert_eq!(escrow_funded.state, EscrowState::Funded);
    assert_eq!(token::Client::new(&fx.env, &fx.token_addr).balance(&fx.buyer), 0);
    assert_eq!(token::Client::new(&fx.env, &fx.token_addr).balance(&fx.contract_id), amount);
    assert!(has_event::<EscrowFunded, _>(&fx.env, &fx.contract_id, "escrow_funded", |e| {
        e.escrow_id == escrow_id && e.buyer == fx.buyer && e.amount == amount
    }));

    // 3. Mark Shipped
    let tracking = SorobanString::from_str(&fx.env, "TRK-HAPPY-001");
    fx.client.mark_shipped(&fx.seller, &escrow_id, &tracking);
    let escrow_shipped = fx.client.get_escrow(&escrow_id);
    assert_eq!(escrow_shipped.state, EscrowState::Shipped);
    assert!(has_event::<EscrowShipped, _>(&fx.env, &fx.contract_id, "escrow_shipped", |e| {
        e.escrow_id == escrow_id && e.seller == fx.seller && e.tracking_id == tracking
    }));

    // 4. Confirm Delivery
    fx.client.confirm_delivery(&fx.buyer, &escrow_id);
    let escrow_completed = fx.client.get_escrow(&escrow_id);
    assert_eq!(escrow_completed.state, EscrowState::Completed);

    // 5. Assert Payout and Fee Allocation
    // Amount = 10,000. Fee = 1% = 100. Seller gets 9,900. Fee collector gets 100.
    let seller_balance = token::Client::new(&fx.env, &fx.token_addr).balance(&fx.seller);
    let fee_collector_balance = token::Client::new(&fx.env, &fx.token_addr).balance(&fx.fee_collector);
    let contract_balance = token::Client::new(&fx.env, &fx.token_addr).balance(&fx.contract_id);

    assert_eq!(seller_balance, 9_900);
    assert_eq!(fee_collector_balance, 100);
    assert_eq!(contract_balance, 0);

    assert!(has_event::<EscrowCompleted, _>(&fx.env, &fx.contract_id, "escrow_completed", |e| {
        e.escrow_id == escrow_id && e.recipient == fx.seller && e.amount == amount
    }));
}
