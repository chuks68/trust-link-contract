#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env};

#[contracttype]
pub enum DataKey {
    Escrow(u32),
    EscrowCount,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowData {
    pub seller: Address,
    pub buyer: Option<Address>,
    pub resolver: Address,
    pub token: Address,
    pub amount: i128,
    pub shipping_window: u64,
    pub funded_at: u64,
    pub state: EscrowState,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowState {
    Pending,
    Funded,
    Completed,
    Disputed,
    Refunded,
}

#[contract]
pub struct Escrow;

#[contractimpl]
#[allow(deprecated)]
impl Escrow {
    pub fn create_escrow(
        env: Env,
        seller: Address,
        resolver: Address,
        token: Address,
        amount: i128,
        shipping_window: u64,
    ) -> u32 {
        seller.require_auth();

        let mut count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCount)
            .unwrap_or(0);
        count += 1;

        let escrow = EscrowData {
            seller,
            buyer: None,
            resolver,
            token,
            amount,
            shipping_window,
            funded_at: 0,
            state: EscrowState::Pending,
        };

        env.storage()
            .instance()
            .set(&DataKey::Escrow(count), &escrow);
        env.storage()
            .instance()
            .set(&DataKey::EscrowCount, &count);

        env.events().publish(("create_escrow",), count);
        count
    }

    pub fn fund_escrow(env: Env, escrow_id: u32, buyer: Address) {
        buyer.require_auth();

        let mut escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found");

        assert!(escrow.state == EscrowState::Pending, "escrow not pending");

        escrow.buyer = Some(buyer.clone());
        escrow.state = EscrowState::Funded;
        escrow.funded_at = env.ledger().timestamp();

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(&buyer, &env.current_contract_address(), &escrow.amount);

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &escrow);
        env.events().publish(("fund_escrow",), escrow_id);
    }

    pub fn confirm_delivery(env: Env, escrow_id: u32) {
        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found");

        assert!(escrow.state == EscrowState::Funded, "escrow not funded");

        let buyer = escrow.buyer.clone().expect("escrow has no buyer");
        buyer.require_auth();

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &escrow.amount,
        );

        let mut updated = escrow;
        updated.state = EscrowState::Completed;

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.events().publish(("confirm_delivery",), escrow_id);
    }

    pub fn raise_dispute(env: Env, escrow_id: u32, evidence_hash: soroban_sdk::Bytes) {
        assert!(
            evidence_hash.len() == 32,
            "evidence_hash must be exactly 32 bytes"
        );

        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found");

        assert!(escrow.state == EscrowState::Funded, "escrow not funded");

        let buyer = escrow.buyer.clone().expect("escrow has no buyer");
        buyer.require_auth();

        let mut updated = escrow;
        updated.state = EscrowState::Disputed;

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.events()
            .publish(("raise_dispute",), (escrow_id, evidence_hash));
    }

    pub fn resolve_dispute(env: Env, escrow_id: u32, release_to_seller: bool) {
        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found");

        assert!(escrow.state == EscrowState::Disputed, "escrow not disputed");

        escrow.resolver.require_auth();

        let token_client = token::Client::new(&env, &escrow.token);
        if release_to_seller {
            token_client.transfer(
                &env.current_contract_address(),
                &escrow.seller,
                &escrow.amount,
            );
        } else {
            token_client.transfer(
                &env.current_contract_address(),
                &escrow.buyer.clone().expect("escrow has no buyer"),
                &escrow.amount,
            );
        }

        let mut updated = escrow;
        updated.state = if release_to_seller {
            EscrowState::Completed
        } else {
            EscrowState::Refunded
        };

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.events()
            .publish(("resolve_dispute",), (escrow_id, release_to_seller));
    }

    pub fn auto_release(env: Env, escrow_id: u32) {
        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found");

        assert!(escrow.state == EscrowState::Funded, "escrow not funded");
        assert!(
            env.ledger().timestamp() >= escrow.funded_at + escrow.shipping_window,
            "shipping window not elapsed"
        );

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &escrow.amount,
        );

        let mut updated = escrow;
        updated.state = EscrowState::Completed;

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.events().publish(("auto_release",), escrow_id);
    }

    pub fn get_escrow(env: Env, escrow_id: u32) -> EscrowData {
        env.storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found")
    }
}

mod test;
