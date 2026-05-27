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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ResolutionType {
    Release = 0,
    Refund = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    Active,
    Resolved,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeInput {
    pub escrow_id: u64,
    pub reason: soroban_sdk::Symbol,
    pub description: soroban_sdk::String,
    pub evidence_hash: soroban_sdk::BytesN<32>,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeData {
    pub escrow_id: u64,
    pub reason: soroban_sdk::Symbol,
    pub description: soroban_sdk::String,
    pub evidence_hash: soroban_sdk::BytesN<32>,
    pub status: DisputeStatus,
    pub raised_at: u64,
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
    pub dispute_deadline: u64,
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    DisputeWindowClosed,
}

#[contract]
pub struct Escrow;

fn deduct_and_transfer(env: &Env, token_addr: &Address, recipient: &Address, amount: i128, fee_bps: u32) {
    let fee = amount
        .checked_mul(fee_bps as i128)
        .expect("fee overflow")
        / 10_000i128;
    let net = amount.checked_sub(fee).expect("fee underflow");

    let token_client = token::Client::new(env, token_addr);

    // Protocol fees are kept in the contract balance.
    // The admin can later withdraw them using the `withdraw_fees` function.
    // We only transfer the net amount to the recipient.
    token_client.transfer(&env.current_contract_address(), recipient, &net);
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
    }

    pub fn withdraw_fees(env: Env, token: Address, to: Address, amount: i128) -> Result<(), ContractError> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("not initialized");
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
