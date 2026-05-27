#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env};

/// Maximum protocol fee in basis points (300 = 3%).
const MAX_FEE_BPS: u32 = 300;
const DISPUTE_WINDOW: u64 = 172_800;
const DEFAULT_TTL_EXTENSION: u32 = 120_960;

/// Storage keys for persisting escrow data and the global escrow counter.
#[contracttype]
pub enum DataKey {
    Admin,
    Escrow(u64),
    EscrowCount,
    EscrowCounter,
    FeeCollector,
    Dispute(u64),
    Paused,
    DefaultFeeBps,
    TtlExtensionLedgers,
    ArbitrationFee,
    TotalArbitrationFees(Address),
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
    ArithmeticError = 12,
    DisputeWindowClosed = 13,
    ContractPaused = 14,
}

/// Lifecycle states of an escrow transaction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowState {
    /// Escrow created but not yet funded by a buyer.
    Pending,
    /// Escrow funded and awaiting delivery confirmation or dispute.
    Funded,
    /// Seller has marked the order as shipped.
    Shipped,
    /// Escrow successfully completed with funds released to the seller.
    Completed,
    /// Escrow in dispute, awaiting resolver decision.
    Disputed,
    /// Escrow refunded to the buyer after dispute resolution.
    Refunded,
}

/// Complete escrow record containing all transaction details and current state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowData {
    /// Address of the seller who will receive funds upon successful completion.
    pub seller: Address,
    /// Address of the buyer who funds the escrow. None until the escrow is funded.
    pub buyer: Option<Address>,
    /// Address of the trusted third-party resolver who can mediate disputes.
    pub resolver: Address,
    /// Address of the token contract (SEP-41 compliant) used for the escrow.
    pub token: Address,
    /// Amount of tokens locked in the escrow.
    pub amount: i128,
    /// Protocol fee in basis points (100 = 1%).
    pub fee_bps: u32,
    /// Time window in seconds after funding during which auto-release is not allowed.
    pub shipping_window: u64,
    /// Ledger timestamp when the escrow was funded. Zero if not yet funded.
    pub funded_at: u64,
    pub dispute_deadline: u64,
    pub state: EscrowState,
    /// Ledger timestamp recorded by the admin oracle when delivery is confirmed. Zero until set.
    pub delivered_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeesWithdrawn {
    pub token: Address,
    pub to: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Protocol fee configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeConfig {
    /// Address that receives protocol fees.
    pub collector: Address,
    /// Maximum allowed fee in basis points.
    pub max_fee_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminRotated {
    pub old_admin: Address,
    pub new_admin: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryRecorded {
    pub escrow_id: u64,
    pub delivered_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractPausedEvent {
    pub admin: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractUnpausedEvent {
    pub admin: Address,
    pub timestamp: u64,
}

#[contract]
pub struct Escrow;

fn ensure_not_paused(env: &Env) {
    let paused: bool = env.storage().instance().get(&DataKey::Paused).unwrap_or(false);
    if paused {
        panic!("contract paused");
    }
}

fn require_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).expect("not initialized")
}

fn get_ttl_extension(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::TtlExtensionLedgers)
        .unwrap_or(DEFAULT_TTL_EXTENSION)
}

fn save_escrow(env: &Env, id: u64, escrow: &EscrowData) {
    let key = DataKey::Escrow(id);
    let ext = get_ttl_extension(env);
    env.storage().persistent().set(&key, escrow);
    env.storage().persistent().extend_ttl(&key, ext / 2, ext);
}

fn load_escrow(env: &Env, id: u64) -> Result<EscrowData, ContractError> {
    let key = DataKey::Escrow(id);
    let escrow: EscrowData = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::EscrowNotFound)?;
    let ext = get_ttl_extension(env);
    env.storage().persistent().extend_ttl(&key, ext / 2, ext);
    Ok(escrow)
}

fn save_dispute(env: &Env, id: u64, dispute: &DisputeData) {
    let key = DataKey::Dispute(id);
    let ext = get_ttl_extension(env);
    env.storage().persistent().set(&key, dispute);
    env.storage().persistent().extend_ttl(&key, ext / 2, ext);
}

fn load_dispute(env: &Env, id: u64) -> Result<DisputeData, ContractError> {
    let key = DataKey::Dispute(id);
    let dispute: DisputeData = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::DisputeNotFound)?;
    let ext = get_ttl_extension(env);
    env.storage().persistent().extend_ttl(&key, ext / 2, ext);
    Ok(dispute)
}

fn deduct_and_transfer(env: &Env, token_addr: &Address, recipient: &Address, amount: i128, fee_bps: u32) -> Result<(), ContractError> {
    if amount < 0 {
        return Err(ContractError::InvalidAmount);
    }

    // Split calculation to avoid overflow for large amounts
    let part1 = (amount / 10_000)
        .checked_mul(fee_bps as i128)
        .ok_or(ContractError::ArithmeticError)?;
    let part2 = (amount % 10_000)
        .checked_mul(fee_bps as i128)
        .ok_or(ContractError::ArithmeticError)?
        / 10_000;

    let fee = part1.checked_add(part2).ok_or(ContractError::ArithmeticError)?;
    let net = amount.checked_sub(fee).ok_or(ContractError::ArithmeticError)?;

    token::Client::new(env, token_addr).transfer(&env.current_contract_address(), recipient, &net);
    Ok(())
}

#[contractimpl]
#[allow(deprecated)]
impl Escrow {
    /// Sets the protocol fee collector, admin address, and arbitration fee. Must be called once.
    pub fn initialize(env: Env, admin: Address, fee_collector: Address, arbitration_fee: i128) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::FeeCollector, &fee_collector);
        env.storage().instance().set(&DataKey::ArbitrationFee, &arbitration_fee);
        env.storage().instance().set(&DataKey::EscrowCounter, &1u64);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    pub fn pause_contract(env: Env) {
        let admin = require_admin(&env);
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish(
            (soroban_sdk::Symbol::new(&env, "contract_paused"),),
            ContractPausedEvent { admin, timestamp: env.ledger().timestamp() },
        );
    }

    pub fn unpause_contract(env: Env) {
        let admin = require_admin(&env);
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish(
            (soroban_sdk::Symbol::new(&env, "contract_unpaused"),),
            ContractUnpausedEvent { admin, timestamp: env.ledger().timestamp() },
        );
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    /// Rotates the admin to a new address. Requires auth from the current admin.
    pub fn set_admin(env: Env, new_admin: Address) {
        let old_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        old_admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.events().publish(
            (soroban_sdk::Symbol::new(&env, "admin_rotated"),),
            AdminRotated {
                old_admin,
                new_admin,
                timestamp: env.ledger().timestamp(),
            },
        );
    }

    /// Updates the default protocol fee. Requires admin auth.
    pub fn set_fee(env: Env, fee_bps: u32) -> Result<(), ContractError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        if fee_bps > MAX_FEE_BPS {
            return Err(ContractError::FeeExceedsMax);
        }
        env.storage().instance().set(&DataKey::DefaultFeeBps, &fee_bps);
        Ok(())
    }

    /// Configures the TTL extension (in ledgers) applied to persistent storage entries.
    pub fn set_ttl_extension(env: Env, ledgers: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        env.storage().instance().set(&DataKey::TtlExtensionLedgers, &ledgers);
    }

    pub fn withdraw_fees(env: Env, token: Address, to: Address, amount: i128) -> Result<(), ContractError> {
        ensure_not_paused(&env);
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
    ) -> Result<u64, ContractError> {
        ensure_not_paused(&env);
        seller.require_auth();

        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        if fee_bps > MAX_FEE_BPS {
            return Err(ContractError::FeeExceedsMax);
        }

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
            delivered_at: 0,
        };

        save_escrow(&env, escrow_id, &escrow);
        env.events().publish(("create_escrow",), escrow_id);
        Ok(escrow_id)
    }

    pub fn fund_escrow(env: Env, escrow_id: u64, buyer: Address) -> Result<(), ContractError> {
        ensure_not_paused(&env);
        buyer.require_auth();

        let mut escrow = load_escrow(&env, escrow_id)?;

        if escrow.state != EscrowState::Pending {
            return Err(ContractError::InvalidState);
        }

        escrow.buyer = Some(buyer.clone());
        escrow.state = EscrowState::Funded;
        escrow.funded_at = env.ledger().timestamp();
        escrow.dispute_deadline = escrow.funded_at + DISPUTE_WINDOW;

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(&buyer, &env.current_contract_address(), &escrow.amount);

        save_escrow(&env, escrow_id, &escrow);
        env.events().publish(("fund_escrow",), escrow_id);
        Ok(())
    }

    /// Seller marks an escrow as shipped. Transitions Funded → Shipped.
    pub fn mark_shipped(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        ensure_not_paused(&env);
        let mut escrow = load_escrow(&env, escrow_id)?;
        if escrow.state != EscrowState::Funded {
            return Err(ContractError::InvalidState);
        }
        escrow.seller.clone().require_auth();
        escrow.state = EscrowState::Shipped;
        save_escrow(&env, escrow_id, &escrow);
        env.events().publish(("mark_shipped",), escrow_id);
        Ok(())
    }

    /// Admin oracle records delivery timestamp. Only callable from Shipped state.
    pub fn record_delivery(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let mut escrow = load_escrow(&env, escrow_id)?;
        if escrow.state != EscrowState::Shipped {
            return Err(ContractError::InvalidState);
        }

        let delivered_at = env.ledger().timestamp();
        escrow.delivered_at = delivered_at;
        save_escrow(&env, escrow_id, &escrow);

        env.events().publish(
            (soroban_sdk::Symbol::new(&env, "delivery_recorded"),),
            DeliveryRecorded { escrow_id, delivered_at },
        );
        Ok(())
    }

    pub fn confirm_delivery(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        ensure_not_paused(&env);
        let escrow = load_escrow(&env, escrow_id)?;

        if escrow.state != EscrowState::Funded && escrow.state != EscrowState::Shipped {
            return Err(ContractError::InvalidState);
        }
        if env.ledger().timestamp() < escrow.dispute_deadline {
            return Err(ContractError::DisputeWindowClosed);
        }

        let buyer = escrow.buyer.clone().ok_or(ContractError::EscrowHasNoBuyer)?;
        buyer.require_auth();

        deduct_and_transfer(&env, &escrow.token, &escrow.seller, escrow.amount, escrow.fee_bps)?;

        let mut updated = escrow;
        updated.state = EscrowState::Completed;

        save_escrow(&env, escrow_id, &updated);
        env.events().publish(("confirm_delivery",), escrow_id);
        Ok(())
    }

    pub fn raise_dispute(
        env: Env,
        escrow_id: u64,
        reason: soroban_sdk::Symbol,
        description: soroban_sdk::String,
        evidence_hash: soroban_sdk::BytesN<32>,
    ) -> Result<(), ContractError> {
        ensure_not_paused(&env);
        let escrow = load_escrow(&env, escrow_id)?;

        if escrow.state != EscrowState::Funded && escrow.state != EscrowState::Shipped {
            return Err(ContractError::InvalidState);
        }
        if env.ledger().timestamp() >= escrow.dispute_deadline {
            return Err(ContractError::DisputeWindowClosed);
        }

        let buyer = escrow.buyer.clone().ok_or(ContractError::EscrowHasNoBuyer)?;
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

        save_escrow(&env, escrow_id, &updated);
        save_dispute(&env, escrow_id, &dispute_data);

        env.events().publish(("raise_dispute",), (escrow_id,));
        Ok(())
    }

    pub fn resolve_dispute(env: Env, escrow_id: u64, resolution: ResolutionType) -> Result<(), ContractError> {
        ensure_not_paused(&env);
        let mut escrow = load_escrow(&env, escrow_id)?;

        if escrow.state != EscrowState::Disputed {
            return Err(ContractError::InvalidState);
        }

        escrow.resolver.require_auth();

        let arbitration_fee: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ArbitrationFee)
            .unwrap_or(0);

        if escrow.amount < arbitration_fee {
            return Err(ContractError::InsufficientBalance);
        }

        escrow.amount = escrow.amount.checked_sub(arbitration_fee).ok_or(ContractError::ArithmeticError)?;

        let total_key = DataKey::TotalArbitrationFees(escrow.token.clone());
        let current_total: i128 = env.storage().instance().get(&total_key).unwrap_or(0);
        env.storage().instance().set(&total_key, &(current_total + arbitration_fee));

        let recipient = match resolution {
            ResolutionType::Release => escrow.seller.clone(),
            ResolutionType::Refund => escrow.buyer.clone().ok_or(ContractError::EscrowHasNoBuyer)?,
        };

        deduct_and_transfer(&env, &escrow.token, &recipient, escrow.amount, escrow.fee_bps)?;

        let mut updated = escrow;
        updated.state = match resolution {
            ResolutionType::Release => EscrowState::Completed,
            ResolutionType::Refund => EscrowState::Refunded,
        };

        let mut dispute_data = load_dispute(&env, escrow_id)?;
        dispute_data.status = DisputeStatus::Resolved;

        save_escrow(&env, escrow_id, &updated);
        save_dispute(&env, escrow_id, &dispute_data);

        env.events().publish(("resolve_dispute",), (escrow_id, resolution));
        Ok(())
    }

    pub fn set_arbitration_fee(env: Env, amount: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("not initialized");
        admin.require_auth();
        env.storage().instance().set(&DataKey::ArbitrationFee, &amount);
    }

    pub fn get_arbitration_fee(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::ArbitrationFee).unwrap_or(0)
    }

    pub fn get_total_arbitration_fees(env: Env, token: Address) -> i128 {
        env.storage().instance().get(&DataKey::TotalArbitrationFees(token)).unwrap_or(0)
    }

    pub fn auto_release(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        ensure_not_paused(&env);
        let escrow = load_escrow(&env, escrow_id)?;

        if escrow.state != EscrowState::Funded && escrow.state != EscrowState::Shipped {
            return Err(ContractError::InvalidState);
        }
        if env.ledger().timestamp() < escrow.dispute_deadline {
            return Err(ContractError::DisputeWindowClosed);
        }
        if env.ledger().timestamp() < escrow.funded_at + escrow.shipping_window {
            return Err(ContractError::ShippingWindowNotElapsed);
        }

        deduct_and_transfer(&env, &escrow.token, &escrow.seller, escrow.amount, escrow.fee_bps)?;

        let mut updated = escrow;
        updated.state = EscrowState::Completed;

        save_escrow(&env, escrow_id, &updated);
        env.events().publish(("auto_release",), escrow_id);
        Ok(())
    }

    pub fn get_escrow(env: Env, escrow_id: u64) -> EscrowData {
        load_escrow(&env, escrow_id).expect("escrow not found")
    }

    pub fn get_dispute(env: Env, escrow_id: u64) -> DisputeData {
        load_dispute(&env, escrow_id).expect("dispute not found")
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
mod test_overflow;
mod test_fee_minimum;
mod test_arbitration_fee;
mod test_helpers;
mod test_admin;
mod test_ttl;
mod test_delivery;
mod test_pause;
