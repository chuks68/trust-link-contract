#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env};

const MAX_FEE_BPS: u32 = 300;

#[contracttype]
pub enum DataKey {
    Admin,
    Escrow(u64),
    EscrowCount,
    EscrowCounter,
    FeeCollector,
    Dispute(u64),
    Paused,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeesWithdrawn {
    pub token: Address,
    pub to: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractPaused {
    pub admin: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractUnpaused {
    pub admin: Address,
    pub timestamp: u64,
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

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    InvalidAmount = 1,
    InsufficientBalance = 2,
    EscrowNotFound = 3,
    InvalidState = 4,
    NotAuthorized = 5,
    AlreadyInitialized = 6,
    FeeExceedsMax = 7,
    EscrowHasNoBuyer = 8,
    ShippingWindowNotElapsed = 9,
    InvalidEvidenceHash = 10,
    DisputeNotFound = 11,
    ContractPaused = 12,
}

fn ensure_not_paused(env: &Env) {
    let paused = env
        .storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false);
    assert!(!paused, "contract paused");
}

fn require_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).expect("not initialized")
}

#[contractimpl]
#[allow(deprecated)]
impl Escrow {
    /// Sets the protocol fee collector and admin address. Must be called once.
    pub fn initialize(env: Env, admin: Address, fee_collector: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::FeeCollector, &fee_collector);
        env.storage().instance().set(&DataKey::EscrowCounter, &1u64);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    pub fn pause_contract(env: Env) {
        let admin = require_admin(&env);
        admin.require_auth();

        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish(
            (soroban_sdk::Symbol::new(&env, "contract_paused"),),
            ContractPaused {
                admin,
                timestamp: env.ledger().timestamp(),
            },
        );
    }

    pub fn unpause_contract(env: Env) {
        let admin = require_admin(&env);
        admin.require_auth();

        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish(
            (soroban_sdk::Symbol::new(&env, "contract_unpaused"),),
            ContractUnpaused {
                admin,
                timestamp: env.ledger().timestamp(),
            },
        );
    }

    pub fn withdraw_fees(env: Env, token: Address, to: Address, amount: i128) -> Result<(), ContractError> {
        ensure_not_paused(&env);

        let admin = require_admin(&env);
        admin.require_auth();

        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let token_client = token::Client::new(&env, &token);
        let contract_balance = token_client.balance(&env.current_contract_address());

        if amount > contract_balance {
            return Err(ContractError::InsufficientBalance);
        }

        token_client.transfer(&env.current_contract_address(), &to, &amount);

        env.events().publish(
            (soroban_sdk::Symbol::new(&env, "fees_withdrawn"),),
            FeesWithdrawn {
                token,
                to,
                amount,
                timestamp: env.ledger().timestamp(),
            },
        );

        Ok(())
    }

    pub fn create_escrow(
        env: Env,
        seller: Address,
        resolver: Address,
        token: Address,
        amount: i128,
        fee_bps: u32,
        shipping_window: u64,
    ) -> u64 {
        ensure_not_paused(&env);
        seller.require_auth();
        assert!(fee_bps <= MAX_FEE_BPS, "fee exceeds maximum");

        let escrow_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .expect("counter initialized");
        env.storage()
            .instance()
            .set(&DataKey::EscrowCounter, &(escrow_id + 1));

        let escrow = EscrowData {
            seller,
            buyer: None,
            resolver,
            token,
            amount,
            fee_bps,
            shipping_window,
            funded_at: 0,
            dispute_deadline: 0,
            state: EscrowState::Pending,
        };

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        env.events().publish(("create_escrow",), escrow_id);
        escrow_id
    }

    pub fn fund_escrow(env: Env, escrow_id: u64, buyer: Address) {
        ensure_not_paused(&env);
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
        escrow.dispute_deadline = escrow.funded_at + 172800;

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(&buyer, &env.current_contract_address(), &escrow.amount);

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &escrow);
        env.events().publish(("fund_escrow",), escrow_id);
    }

    pub fn confirm_delivery(env: Env, escrow_id: u64) {
        ensure_not_paused(&env);

        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found");

        assert!(escrow.state == EscrowState::Funded, "escrow not funded");
        assert!(
            env.ledger().timestamp() >= escrow.dispute_deadline,
            "dispute window not closed"
        );

        let buyer = escrow.buyer.clone().expect("escrow has no buyer");
        buyer.require_auth();

        deduct_and_transfer(&env, &escrow.token, &escrow.seller, escrow.amount, escrow.fee_bps);

        let mut updated = escrow;
        updated.state = EscrowState::Completed;

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.events().publish(("confirm_delivery",), escrow_id);
    }

    pub fn raise_dispute(
        env: Env,
        escrow_id: u64,
        reason: soroban_sdk::Symbol,
        description: soroban_sdk::String,
        evidence_hash: soroban_sdk::BytesN<32>,
    ) {
        ensure_not_paused(&env);

        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found");

        assert!(escrow.state == EscrowState::Funded, "escrow not funded");
        assert!(
            env.ledger().timestamp() >= escrow.dispute_deadline,
            "dispute window not closed"
        );

        let buyer = escrow.buyer.clone().expect("escrow has no buyer");
        buyer.require_auth();

        let mut updated = escrow;
        updated.state = EscrowState::Disputed;

        let dispute_data = DisputeData {
            escrow_id,
            reason,
            description,
            evidence_hash,
            status: DisputeStatus::Active,
            raised_at: env.ledger().timestamp(),
        };

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.storage()
            .instance()
            .set(&DataKey::Dispute(escrow_id), &dispute_data);

        env.events()
            .publish(("raise_dispute",), (escrow_id,));
    }

    pub fn resolve_dispute(env: Env, escrow_id: u64, resolution: ResolutionType) {
        ensure_not_paused(&env);

        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found");

        assert!(escrow.state == EscrowState::Disputed, "escrow not disputed");

        escrow.resolver.require_auth();

        let recipient = match resolution {
            ResolutionType::Release => escrow.seller.clone(),
            ResolutionType::Refund => escrow.buyer.clone().expect("escrow has no buyer"),
        };

        deduct_and_transfer(&env, &escrow.token, &recipient, escrow.amount, escrow.fee_bps);

        let mut updated = escrow;
        updated.state = match resolution {
            ResolutionType::Release => EscrowState::Completed,
            ResolutionType::Refund => EscrowState::Refunded,
        };

        let mut dispute_data: DisputeData = env
            .storage()
            .instance()
            .get(&DataKey::Dispute(escrow_id))
            .expect("dispute not found");
        dispute_data.status = DisputeStatus::Resolved;

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.storage()
            .instance()
            .set(&DataKey::Dispute(escrow_id), &dispute_data);

        env.events()
            .publish(("resolve_dispute",), (escrow_id, resolution));
    }

    pub fn auto_release(env: Env, escrow_id: u64) {
        ensure_not_paused(&env);

        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found");

        assert!(escrow.state == EscrowState::Funded, "escrow not funded");
        assert!(
            env.ledger().timestamp() >= escrow.dispute_deadline,
            "dispute window not closed"
        );
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
        env.events().publish(("auto_release",), escrow_id);
    }

    pub fn get_escrow(env: Env, escrow_id: u64) -> EscrowData {
        env.storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .expect("escrow not found")
    }

    pub fn get_dispute(env: Env, escrow_id: u64) -> DisputeData {
        env.storage()
            .instance()
            .get(&DataKey::Dispute(escrow_id))
            .expect("dispute not found")
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
mod test_withdraw_fees;
mod test_dispute;
mod test_escrow_id;
mod test_resolution;
mod test_pause;
