#![no_std]
use soroban_sdk::{contract, contractimpl, token, Address, Env, String, Symbol, BytesN, Vec};

pub mod errors;
pub mod events;
pub mod helpers;
pub mod types;
pub use crate::errors::ContractError;
pub use crate::events::{
    AdminRotated, AutoReleased, ContractPausedEvent, ContractUnpausedEvent, DeliveryRecorded,
    DisputeRaised, DisputeResolved, EscrowCancelled, EscrowCompleted, EscrowCreated,
    EscrowFunded, EscrowShipped, FeeUpdated, FeesWithdrawn, ArbitrationFeeUpdated,
    ProtocolFeeUpdated,
    emit_admin_rotated, emit_auto_released, emit_contract_paused, emit_contract_unpaused,
    emit_delivery_recorded, emit_dispute_raised, emit_dispute_resolved, emit_escrow_cancelled,
    emit_escrow_completed, emit_escrow_created, emit_escrow_funded, emit_escrow_shipped,
    emit_fee_updated, emit_fees_withdrawn, emit_arbitration_fee_updated,
    emit_protocol_fee_updated,
};
pub use crate::types::{
    ContractConfig, ContractStats, DataKey, DisputeData, DisputeStatus, EscrowData, EscrowState,
    FeeConfig, ResolutionType,
};

/// Maximum protocol fee in basis points (300 = 3%).
const MAX_FEE_BPS: u32 = 300;

/// Minimum escrow amount in stroops (1 USDC = 1,000,000 stroops).
/// Prevents dust transactions and ensures meaningful escrow values.
pub const MIN_ESCROW_AMOUNT: i128 = 1_000_000;

/// Storage keys for persisting escrow data and the global escrow counter.
#[contracttype]
pub enum DataKey {
    /// Stores the admin address with permission to manage contract operations.
    Admin,
    /// Stores individual escrow data by escrow ID.
    Escrow(u64),
    /// Legacy key; use EscrowCounter instead.
    EscrowCount,
    /// Stores the current escrow counter for generating unique escrow IDs.
    EscrowCounter,
    /// Stores the fee collector address that receives protocol and arbitration fees.
    FeeCollector,
    /// Stores dispute data for a specific escrow ID.
    Dispute(u64),
    /// Stores the global arbitration fee in basis points.
    ArbitrationFee,
    /// Stores the total arbitration fees accumulated by a specific address.
    TotalArbitrationFees(Address),
    /// Stores the contract pause state (true = paused, false = active).
    IsPaused,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ResolutionType {
    Release = 0,
    Refund = 1,
}

const DISPUTE_WINDOW: u64 = 172_800;
const DEFAULT_TTL_EXTENSION: u32 = 120_960;

/// Maximum length for user-supplied string fields.
/// - `tracking_id`: 64 characters
/// - `description` in `raise_dispute`: 256 characters
pub const MAX_TRACKING_ID_LEN: u32 = 64;
pub const MAX_DESCRIPTION_LEN: u32 = 256;

/// Validity matrix for escrow state transitions (#9).
///
/// Returns `Ok(())` if the move from `from` to `to` is legal under the
/// escrow lifecycle, `Err(InvalidStateTransition)` otherwise. Provided as a
/// pure helper alongside the existing inline guards so reviewers can audit
/// every legal edge in one place.
pub fn transition_state(
    from: &EscrowState,
    to: &EscrowState,
) -> Result<(), ContractError> {
    use EscrowState::*;
    let allowed = matches!(
        (from, to),
        (Pending, Funded)
            | (Pending, Canceled)
            | (Funded, Shipped)
            | (Funded, Disputed)
            | (Funded, Completed)
            | (Funded, Refunded)
            | (Shipped, Completed)
            | (Shipped, Disputed)
            | (Shipped, Refunded)
            | (Disputed, Completed)
            | (Disputed, Refunded)
    );
    if allowed {
        Ok(())
    } else {
        Err(ContractError::InvalidStateTransition)
    }
}

#[contract]
pub struct Escrow;

fn ensure_not_paused(env: &Env) -> Result<(), ContractError> {
    let paused: bool = env.storage().instance().get(&DataKey::Paused).unwrap_or(false);
    if paused {
        return Err(ContractError::ContractPaused);
    }
    Ok(())
}

fn require_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("not initialized")
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
    Paused = 14,
fn default_fee_config() -> FeeConfig {
    FeeConfig {
        protocol_fee_bps: 0,
        arbitration_fee_bps: 0,
    }
}

fn read_fee_config(env: &Env) -> FeeConfig {
    env.storage()
        .instance()
        .get(&DataKey::FeeConfig)
        .unwrap_or_else(|| default_fee_config())
}

fn write_fee_config(env: &Env, fee_config: &FeeConfig) {
    env.storage().instance().set(&DataKey::FeeConfig, fee_config);
}

fn validate_fee_bps(fee_bps: u32) -> Result<(), ContractError> {
    if fee_bps > MAX_FEE_BPS {
        return Err(ContractError::FeeExceedsMax);
    }
    Ok(())
}

fn update_protocol_fee(env: &Env, caller: &Address, fee_bps: u32) -> Result<u32, ContractError> {
    caller.require_auth();
    let admin = require_admin(env);
    if caller != &admin {
        return Err(ContractError::NotAuthorized);
    }
    validate_fee_bps(fee_bps)?;
    let mut config = read_fee_config(env);
    let old_fee = config.protocol_fee_bps;
    config.protocol_fee_bps = fee_bps;
    write_fee_config(env, &config);
    Ok(old_fee)
}

fn update_arbitration_fee(env: &Env, caller: &Address, fee_bps: u32) -> Result<u32, ContractError> {
    caller.require_auth();
    let admin = require_admin(env);
    if caller != &admin {
        return Err(ContractError::NotAuthorized);
    }
    validate_fee_bps(fee_bps)?;
    let mut config = read_fee_config(env);
    let old_fee = config.arbitration_fee_bps;
    config.arbitration_fee_bps = fee_bps;
    write_fee_config(env, &config);
    Ok(old_fee)
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

fn increment_counter(env: &Env, key: &DataKey) {
    let current: u64 = env.storage().instance().get(key).unwrap_or(0);
    env.storage().instance().set(key, &(current + 1));
}

#[contractimpl]
#[allow(deprecated)]
impl Escrow {
    /// Sets the protocol fee collector, admin address, and arbitration fee. Must be called once.
    pub fn initialize(
        env: Env,
        admin: Address,
        fee_collector: Address,
        arbitration_fee_bps: u32,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        // admin and fee_collector must be distinct keys: sharing one address
        // means compromising the admin key also compromises all fee revenue.
        if admin == fee_collector {
            return Err(ContractError::InvalidAddress);
        }
        validate_fee_bps(arbitration_fee_bps)?;

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::FeeCollector, &fee_collector);
        write_fee_config(
            &env,
            &FeeConfig {
                protocol_fee_bps: 0,
                arbitration_fee_bps,
            },
        );
        env.storage().instance().set(&DataKey::EscrowCounter, &1u64);
        env.storage().instance().set(&DataKey::Paused, &false);
        Ok(())
    }

    pub fn pause_contract(env: Env, caller: Address) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        let admin = require_admin(&env);
        if caller != admin {
            return Err(ContractError::NotAuthorized);
        }

        env.storage().instance().set(&DataKey::Paused, &true);
        emit_contract_paused(&env, admin);
        Ok(())
    }

    pub fn unpause_contract(env: Env, caller: Address) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        let admin = require_admin(&env);
        if caller != admin {
            return Err(ContractError::NotAuthorized);
        }

        env.storage().instance().set(&DataKey::Paused, &false);
        emit_contract_unpaused(&env, admin);
        Ok(())
    }

    /// Returns whether the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    /// Rotates the admin to a new address. Requires auth from the current admin.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        let old_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        old_admin.require_auth();
        // Reject no-op rotations to the same address so monitoring isn't polluted
        // with misleading AdminRotated events.
        if new_admin == old_admin {
            return Err(ContractError::SameAddress);
        }
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        emit_admin_rotated(&env, old_admin, new_admin);
        Ok(())
    }

    /// Updates the default protocol fee. Requires admin auth.
    pub fn set_fee(env: Env, caller: Address, fee_bps: u32) -> Result<(), ContractError> {
        let old_fee_bps = update_protocol_fee(&env, &caller, fee_bps)?;
        emit_fee_updated(&env, old_fee_bps, fee_bps);
        Ok(())
    }

    /// Updates the protocol fee configuration in basis points. Requires admin auth.
    pub fn set_protocol_fee(env: Env, caller: Address, fee_bps: u32) -> Result<(), ContractError> {
        let old_fee_bps = update_protocol_fee(&env, &caller, fee_bps)?;
        emit_protocol_fee_updated(&env, old_fee_bps, fee_bps);
        Ok(())
    }

    /// Configures the TTL extension (in ledgers) applied to persistent storage entries.
    pub fn set_ttl_extension(env: Env, caller: Address, ledgers: u32) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        
        if caller != admin {
            return Err(ContractError::NotAuthorized);
        }

        env.storage().instance().set(&DataKey::TtlExtensionLedgers, &ledgers);
        Ok(())
    }

    pub fn withdraw_fees(
        env: Env,
        caller: Address,
        token: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        ensure_not_paused(&env)?;
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("not initialized");
        if caller != admin {
            return Err(ContractError::NotAuthorized);
        }

        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let token_client = token::Client::new(&env, &token);
        let contract_balance = token_client.balance(&env.current_contract_address());

        if amount > contract_balance {
            return Err(ContractError::InsufficientBalance);
        }

        token_client.transfer(&env.current_contract_address(), &to, &amount);

        emit_fees_withdrawn(&env, token, to, amount);

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
        // SECURITY:
        // Authenticate before any state reads.
        seller.require_auth();

        ensure_not_paused(&env)?;

        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        if amount < MIN_ESCROW_AMOUNT {
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
            tracking_id: None,
        };

        save_escrow(&env, escrow_id, &escrow);
        increment_counter(&env, &DataKey::TotalCreated);
        emit_escrow_created(
            &env,
            escrow_id,
            escrow.seller.clone(),
            escrow.resolver.clone(),
            escrow.token.clone(),
            escrow.amount,
            escrow.fee_bps,
            escrow.shipping_window,
        );
        Ok(escrow_id)
    }

    pub fn cancel_escrow(env: Env, caller: Address, escrow_id: u64) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        ensure_not_paused(&env)?;
        let mut escrow = load_escrow(&env, escrow_id)?;

        if escrow.seller != caller {
            return Err(ContractError::NotAuthorized);
        }

        if escrow.state != EscrowState::Pending {
            return Err(ContractError::InvalidState);
        }

        escrow.state = EscrowState::Canceled;

        save_escrow(&env, escrow_id, &escrow);
        emit_escrow_cancelled(&env, escrow_id, escrow.seller);
        Ok(())
    }

    pub fn fund_escrow(env: Env, escrow_id: u64, buyer: Address) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        buyer.require_auth();

        ensure_not_paused(&env)?;

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
        emit_escrow_funded(&env, escrow_id, buyer, escrow.amount);
        Ok(())
    }

    /// Seller marks an escrow as shipped. Transitions Funded → Shipped.
    pub fn mark_shipped(env: Env, caller: Address, escrow_id: u64, tracking_id: String) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        ensure_not_paused(&env)?;
        if tracking_id.len() > MAX_TRACKING_ID_LEN {
            return Err(ContractError::InputTooLong);
        }
        let mut escrow = load_escrow(&env, escrow_id)?;

        if escrow.seller != caller {
            return Err(ContractError::NotAuthorized);
        }

        if escrow.state != EscrowState::Funded {
            return Err(ContractError::InvalidState);
        }
        
        escrow.state = EscrowState::Shipped;
        escrow.tracking_id = Some(tracking_id);
        let tracking = escrow.tracking_id.clone().expect("tracking id set");
        save_escrow(&env, escrow_id, &escrow);
        emit_escrow_shipped(&env, escrow_id, escrow.seller, tracking);
        Ok(())
    }

    /// Admin oracle records delivery timestamp. Only callable from Shipped state.
    pub fn record_delivery(env: Env, caller: Address, escrow_id: u64) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        
        if caller != admin {
            return Err(ContractError::NotAuthorized);
        }

        let mut escrow = load_escrow(&env, escrow_id)?;
        if escrow.state != EscrowState::Shipped {
            return Err(ContractError::InvalidState);
        }

        let delivered_at = env.ledger().timestamp();
        escrow.delivered_at = delivered_at;
        save_escrow(&env, escrow_id, &escrow);

        emit_delivery_recorded(&env, escrow_id, delivered_at);
        Ok(())
    }

    pub fn confirm_delivery(env: Env, caller: Address, escrow_id: u64) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        ensure_not_paused(&env)?;
        let escrow = load_escrow(&env, escrow_id)?;

        let buyer = escrow.buyer.clone().ok_or(ContractError::EscrowHasNoBuyer)?;
        if caller != buyer {
            return Err(ContractError::NotAuthorized);
        }

        if escrow.state != EscrowState::Funded && escrow.state != EscrowState::Shipped {
            return Err(ContractError::InvalidState);
        }
        if env.ledger().timestamp() < escrow.dispute_deadline {
            return Err(ContractError::DisputeWindowClosed);
        }

        deduct_and_transfer(&env, &escrow.token, &escrow.seller, escrow.amount, escrow.fee_bps)?;

        let mut updated = escrow;
        updated.state = EscrowState::Completed;

        save_escrow(&env, escrow_id, &updated);
        increment_counter(&env, &DataKey::TotalCompleted);
        emit_escrow_completed(&env, escrow_id, updated.seller.clone(), updated.amount, updated.fee_bps);
        Ok(())
    }

    pub fn raise_dispute(
        env: Env,
        caller: Address,
        escrow_id: u64,
        reason: soroban_sdk::Symbol,
        description: soroban_sdk::String,
        evidence_hash: soroban_sdk::BytesN<32>,
    ) -> Result<(), ContractError> {
        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        ensure_not_paused(&env)?;
        if description.len() > MAX_DESCRIPTION_LEN {
            return Err(ContractError::InputTooLong);
        }
        let escrow = load_escrow(&env, escrow_id)?;

        let buyer = escrow.buyer.clone().ok_or(ContractError::EscrowHasNoBuyer)?;
        if caller != buyer {
            return Err(ContractError::NotAuthorized);
        }

        if escrow.state != EscrowState::Funded && escrow.state != EscrowState::Shipped {
            return Err(ContractError::InvalidState);
        }
        if env.ledger().timestamp() >= escrow.dispute_deadline {
            return Err(ContractError::DisputeWindowClosed);
        }

        let mut updated = escrow;
        updated.state = EscrowState::Disputed;

        let reason_event = reason.clone();
        let description_event = description.clone();
        let evidence_hash_event = evidence_hash.clone();

        let dispute_data = DisputeData {
            escrow_id,
            reason,
            description,
            evidence_hash,
            status: DisputeStatus::Active,
            raised_at: env.ledger().timestamp(),
            tracking_id: updated.tracking_id.clone(),
        };

        save_escrow(&env, escrow_id, &updated);
        save_dispute(&env, escrow_id, &dispute_data);
        increment_counter(&env, &DataKey::TotalDisputed);
        emit_dispute_raised(
            &env,
            escrow_id,
            buyer,
            reason_event,
            description_event,
            evidence_hash_event,
        );
        Ok(())
    }

    pub fn resolve_dispute(env: Env, caller: Address, escrow_id: u64, resolution: ResolutionType) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        ensure_not_paused(&env)?;
        let mut escrow = load_escrow(&env, escrow_id)?;

        if caller != escrow.resolver {
            return Err(ContractError::NotAuthorized);
        }

        if escrow.state != EscrowState::Disputed {
            return Err(ContractError::InvalidState);
        }

        let arbitration_fee_bps = read_fee_config(&env).arbitration_fee_bps;
        let arbitration_fee = crate::helpers::payout::calculate_fee(escrow.amount, arbitration_fee_bps)?;

        if arbitration_fee > escrow.amount {
            return Err(ContractError::InsufficientBalance);
        }

        escrow.amount = escrow
            .amount
            .checked_sub(arbitration_fee)
            .ok_or(ContractError::ArithmeticError)?;

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

        match resolution {
            ResolutionType::Release => increment_counter(&env, &DataKey::TotalCompleted),
            ResolutionType::Refund => increment_counter(&env, &DataKey::TotalRefunded),
        };

        emit_dispute_resolved(
            &env,
            escrow_id,
            updated.resolver.clone(),
            resolution,
            recipient,
            updated.amount,
            arbitration_fee,
        );
        Ok(())
    }

    pub fn set_arbitration_fee(env: Env, caller: Address, fee_bps: u32) -> Result<(), ContractError> {
        let old_fee_bps = update_arbitration_fee(&env, &caller, fee_bps)?;
        emit_arbitration_fee_updated(&env, old_fee_bps, fee_bps);
        Ok(())
    }

    pub fn get_arbitration_fee(env: Env) -> u32 {
        read_fee_config(&env).arbitration_fee_bps
    }

    pub fn get_total_arbitration_fees(env: Env, token: Address) -> i128 {
        env.storage().instance().get(&DataKey::TotalArbitrationFees(token)).unwrap_or(0)
    }

    pub fn auto_release(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        ensure_not_paused(&env)?;
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
        increment_counter(&env, &DataKey::TotalCompleted);
        emit_auto_released(&env, escrow_id, updated.seller.clone(), updated.amount, updated.fee_bps);
        Ok(())
    }

    pub fn get_escrow(env: Env, escrow_id: u64) -> EscrowData {
        load_escrow(&env, escrow_id).expect("escrow not found")
    }

    pub fn get_dispute(env: Env, escrow_id: u64) -> Option<DisputeData> {
        load_dispute(&env, escrow_id).ok()
    }

    pub fn get_escrows_by_buyer(env: Env, buyer: Address) -> Vec<u64> {
        let mut result = Vec::new(&env);
        let current_counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(1);

        for i in 1..current_counter {
            if let Ok(escrow) = load_escrow(&env, i) {
                if let Some(b) = escrow.buyer {
                    if b == buyer {
                        result.push_back(i);
                    }
                }
            }
        }
        result
    }

    /// Returns the current fee configuration as a read-only view.
    pub fn get_fee_config(env: Env) -> FeeConfig {
        read_fee_config(&env)
    }

    /// Returns the current contract configuration as a read-only view.
    pub fn get_contract_config(env: Env) -> ContractConfig {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");

        let fee_bps = read_fee_config(&env).protocol_fee_bps;

        let fee_collector: Address = env
            .storage()
            .instance()
            .get(&DataKey::FeeCollector)
            .expect("fee collector not set");

        let current_counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(1);
        let escrow_count = current_counter.saturating_sub(1);

        ContractConfig {
            admin,
            fee_bps,
            fee_collector,
            escrow_count,
        }
    }

    /// Returns on-chain counters for escrow lifecycle events.
    pub fn get_stats(env: Env) -> ContractStats {
        ContractStats {
            total_created: env.storage().instance().get(&DataKey::TotalCreated).unwrap_or(0),
            total_completed: env.storage().instance().get(&DataKey::TotalCompleted).unwrap_or(0),
            total_disputed: env.storage().instance().get(&DataKey::TotalDisputed).unwrap_or(0),
            total_refunded: env.storage().instance().get(&DataKey::TotalRefunded).unwrap_or(0),
        }
    }

}

mod test;
mod test_edge_cases;
mod test_withdraw_fees;
mod test_dispute;
mod test_escrow_id;
mod test_resolution;
mod test_pause;
mod test_overflow;
mod test_fee_minimum;
mod test_minimum_amount_guard;
mod test_fee_calculation_accuracy;
mod test_arbitration_fee;
mod test_fee_config;
mod test_helpers;
mod test_admin;
mod test_ttl;
mod test_escrow_states;
mod test_admin_rotation;
mod test_auto_release;
mod test_initialize_twice;
mod test_contract_config;
mod test_string_length;
mod test_get_escrows_by_buyer;
mod test_delivery;
mod test_auth_ordering;
mod test_dispute_flow;
mod test_set_fee_boundary;
mod test_cancel_restrictions;
mod test_dispute_window;
mod test_unauthorized;
