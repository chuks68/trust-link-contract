#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol};

const MAX_FEE_BPS: u32 = 300;

const TOPIC_CREATE_ESCROW: &str = "create_escrow";
const TOPIC_FUND_ESCROW: &str = "fund_escrow";
const TOPIC_CONFIRM_DELIVERY: &str = "confirm_delivery";
const TOPIC_RAISE_DISPUTE: &str = "raise_dispute";
const TOPIC_RESOLVE_DISPUTE: &str = "resolve_dispute";
const TOPIC_CANCEL_ESCROW: &str = "cancel_escrow";
const TOPIC_AUTO_RELEASE: &str = "auto_release";

#[contracttype]
pub enum DataKey {
    Escrow(u32),
    EscrowCount,
    FeeCollector,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowData {
    pub seller: Address,
    pub buyer: Option<Address>,
    pub resolver: Address,
    pub token: Address,
    pub amount: i128,
    pub fee_bps: u32,
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeConfig {
    pub collector: Address,
    pub max_fee_bps: u32,
}

#[contract]
pub struct Escrow;

/// Calculates the protocol fee, transfers it to the fee collector, and sends
/// the remainder to the designated recipient. This is the single source of
/// truth for all outbound escrow disbursements.
fn deduct_and_transfer(env: &Env, token_addr: &Address, recipient: &Address, amount: i128, fee_bps: u32) {
    let fee = amount
        .checked_mul(fee_bps as i128)
        .expect("fee overflow")
        / 10_000i128;
    let net = amount.checked_sub(fee).expect("fee underflow");

    let token_client = token::Client::new(env, token_addr);

    if fee > 0 {
        let collector: Address = env
            .storage()
            .instance()
            .get(&DataKey::FeeCollector)
            .expect("fee collector not set");
        token_client.transfer(&env.current_contract_address(), &collector, &fee);
    }

    token_client.transfer(&env.current_contract_address(), recipient, &net);
}

#[contractimpl]
#[allow(deprecated)]
impl Escrow {
    /// Sets the protocol fee collector address. Must be called once before any
    /// escrow settlement can occur.
    pub fn initialize(env: Env, fee_collector: Address) {
        if env
            .storage()
            .instance()
            .has(&DataKey::FeeCollector)
        {
            panic!("already initialized");
        }
        env.storage()
            .instance()
            .set(&DataKey::FeeCollector, &fee_collector);
    }

    pub fn create_escrow(
        env: Env,
        seller: Address,
        resolver: Address,
        token: Address,
        amount: i128,
        fee_bps: u32,
        shipping_window: u64,
    ) -> u32 {
        seller.require_auth();
        assert!(fee_bps <= MAX_FEE_BPS, "fee exceeds maximum");

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
            fee_bps,
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

        env.events().publish((Symbol::new(&env, TOPIC_CREATE_ESCROW),), count);
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
        env.events().publish((Symbol::new(&env, TOPIC_FUND_ESCROW),), escrow_id);
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

        deduct_and_transfer(&env, &escrow.token, &escrow.seller, escrow.amount, escrow.fee_bps);

        let mut updated = escrow;
        updated.state = EscrowState::Completed;

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.events().publish((Symbol::new(&env, TOPIC_CONFIRM_DELIVERY),), escrow_id);
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
            .publish((Symbol::new(&env, TOPIC_RAISE_DISPUTE),), (escrow_id, evidence_hash));
    }

    pub fn resolve_dispute(env: Env, escrow_id: u32, release_to_seller: bool) {
        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found");

        assert!(escrow.state == EscrowState::Disputed, "escrow not disputed");

        escrow.resolver.require_auth();

        let recipient = if release_to_seller {
            escrow.seller.clone()
        } else {
            escrow.buyer.clone().expect("escrow has no buyer")
        };

        deduct_and_transfer(&env, &escrow.token, &recipient, escrow.amount, escrow.fee_bps);

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
            .publish((Symbol::new(&env, TOPIC_RESOLVE_DISPUTE),), (escrow_id, release_to_seller));
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

        deduct_and_transfer(&env, &escrow.token, &escrow.seller, escrow.amount, escrow.fee_bps);

        let mut updated = escrow;
        updated.state = EscrowState::Completed;

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.events().publish((Symbol::new(&env, TOPIC_AUTO_RELEASE),), escrow_id);
    }

    pub fn get_escrow(env: Env, escrow_id: u32) -> EscrowData {
        env.storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found")
    }

    /// Returns the current protocol fee configuration as a read-only view.
    pub fn get_fee_config(env: Env) -> FeeConfig {
        let collector: Address = env
            .storage()
            .instance()
            .get(&DataKey::FeeCollector)
            .expect("fee collector not set");

        FeeConfig {
            collector,
            max_fee_bps: MAX_FEE_BPS,
        }
    }
}

mod test;
