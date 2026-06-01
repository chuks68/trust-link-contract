#![no_std]
use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, String, Symbol, Vec};

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

/// Maximum escrow fee in basis points (300 = 3%).
///
/// This applies to the per-escrow `fee_bps` value supplied at creation time,
/// and to the legacy `set_fee` helper that persists `DefaultFeeBps`.
const MAX_ESCROW_FEE_BPS: u32 = 300;

/// Maximum configurable protocol/arbitration fee in basis points.
///
/// Protocol fee and arbitration fee are stored in `FeeConfig`, which the
/// dispute-resolution and payout paths read separately from the per-escrow fee.
const MAX_CONFIG_FEE_BPS: u32 = 10_000;

/// Minimum escrow amount in stroops.
/// Keeps the contract from accepting zero or negative escrows.
pub const MIN_ESCROW_AMOUNT: i128 = 1;

const DISPUTE_WINDOW: u64 = 172_800;
const DELIVERY_RELEASE_WINDOW: u64 = 172_800;
const DEFAULT_TTL_EXTENSION: u32 = 120_960;

/// Maximum length for user-supplied string fields.
/// - `tracking_id`: 64 characters
/// - `description` in `raise_dispute`: 256 characters
pub const MAX_TRACKING_ID_LEN: u32 = 64;
pub const MAX_DESCRIPTION_LEN: u32 = 256;

/// Maximum escrow amount intentionally capped to
/// preserve arithmetic safety for fee calculations
/// and aggregate accounting operations.
pub const MAX_ESCROW_AMOUNT: i128 = i128::MAX / 10_000;

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

fn validate_escrow_fee_bps(fee_bps: u32) -> Result<(), ContractError> {
    if fee_bps > MAX_ESCROW_FEE_BPS {
        return Err(ContractError::FeeExceedsMax);
    }
    Ok(())
}

fn validate_config_fee_bps(fee_bps: u32) -> Result<(), ContractError> {
    if fee_bps > MAX_CONFIG_FEE_BPS {
        return Err(ContractError::FeeExceedsMax);
    }
    Ok(())
}

fn update_default_fee(env: &Env, caller: &Address, fee_bps: u32) -> Result<u32, ContractError> {
    caller.require_auth();
    let admin = require_admin(env);
    if caller != &admin {
        return Err(ContractError::NotAuthorized);
    }
    validate_escrow_fee_bps(fee_bps)?;
    let old_fee: u32 = env
        .storage()
        .instance()
        .get(&DataKey::DefaultFeeBps)
        .unwrap_or(0);
    env.storage().instance().set(&DataKey::DefaultFeeBps, &fee_bps);
    Ok(old_fee)
}

fn update_protocol_fee(env: &Env, caller: &Address, fee_bps: u32) -> Result<u32, ContractError> {
    caller.require_auth();
    let admin = require_admin(env);
    if caller != &admin {
        return Err(ContractError::NotAuthorized);
    }
    validate_config_fee_bps(fee_bps)?;
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
    validate_config_fee_bps(fee_bps)?;
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

/// Deducts the protocol fee from `amount` and transfers the net to `recipient`,
/// leaving the fee in the contract vault for the admin to sweep via
/// `withdraw_fees`.
///
/// Rounding policy: **floor** — `fee = floor(amount * fee_bps / 10_000)` and
/// `net = amount - fee`, so the truncated remainder accrues to `recipient` and
/// the invariant `net + fee == amount` always holds (no stranded dust). This
/// mirrors [`helpers::payout::calculate_fee`]; the two implementations must stay
/// in sync. The calculation is split to avoid overflowing `i128` for large
/// amounts. Overflow surfaces as `ArithmeticError` (distinct from the helper's
/// `ArithmeticOverflow`).
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

fn transfer_with_protocol_fee(
    env: &Env,
    token_addr: &Address,
    recipient: &Address,
    fee_collector: &Address,
    amount: i128,
    fee_bps: u32,
) -> Result<(i128, i128), ContractError> {
    let (fee, net) = crate::helpers::payout::calculate_protocol_fee(amount, fee_bps)?;
    let token_client = token::Client::new(env, token_addr);
    let contract_addr = env.current_contract_address();

    if net > 0 {
        token_client.transfer(&contract_addr, recipient, &net);
    }

    if fee > 0 {
        token_client.transfer(&contract_addr, fee_collector, &fee);
    }

    Ok((fee, net))
}

fn increment_counter(env: &Env, key: &DataKey) -> Result<(), ContractError> {
    let current: u64 = env.storage().instance().get(key).unwrap_or(0);
    let next = current.checked_add(1).ok_or(ContractError::ArithmeticError)?;
    env.storage().instance().set(key, &next);
    Ok(())
}

#[contractimpl]
#[allow(deprecated)]
impl Escrow {
    /// Sets the protocol fee collector, admin address, and arbitration fee. Must be called once.
    ///
    /// Returns `Err(ContractError::InvalidAddress)` if `admin` or `fee_collector` is the
    /// all-zero/empty Stellar account address (#55). Returning early on validation failure
    /// guarantees no storage entries (`Admin`, `FeeCollector`, `ArbitrationFee`,
    /// `EscrowCounter`, `Paused`) are written, leaving the contract uninitialized.
    pub fn initialize(
        env: Env,
        admin: Address,
        fee_collector: Address,
        arbitration_fee: i128,
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
        validate_config_fee_bps(arbitration_fee_bps)?;

        let zero = Address::from_string(&String::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
        if admin == zero || fee_collector == zero {
            return Err(ContractError::InvalidAddress);
        }

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
        let old_fee_bps = update_default_fee(&env, &caller, fee_bps)?;
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
        buyer: Address,
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
        if amount > MAX_ESCROW_AMOUNT {
            return Err(ContractError::AmountExceedsMaximum);
        }

        if amount < MIN_ESCROW_AMOUNT {
            return Err(ContractError::InvalidAmount);
        }

        validate_escrow_fee_bps(fee_bps)?;

        let escrow_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .expect("counter initialized");
        let next_id = escrow_id.checked_add(1).ok_or(ContractError::ArithmeticError)?;
        env.storage()
            .instance()
            .set(&DataKey::EscrowCounter, &next_id);

        let escrow = EscrowData {
            seller,
            buyer: Some(buyer),
            resolver,
            token,
            amount,
            fee_bps,
            shipping_window,
            funded_at: 0,
            dispute_deadline: 0,
            state: EscrowState::Pending,
            shipped_at: 0,
            delivered_at: 0,
            tracking_id: None,
        };

        save_escrow(&env, escrow_id, &escrow);
        increment_counter(&env, &DataKey::TotalCreated)?;
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

        let buyer = escrow.buyer.clone();
        if escrow.seller != caller && buyer.as_ref() != Some(&caller) {
            return Err(ContractError::NotAuthorized);
        }

        if escrow.state != EscrowState::Pending {
            return Err(ContractError::InvalidState);
        }

        // Allow either the seller or the named buyer to cancel a pending escrow.
        // In Soroban the transaction is signed by exactly one invoker, so we
        // check which party is authorising and require auth from that party.
        if let Some(ref buyer) = escrow.buyer {
            buyer.clone().require_auth();
        } else {
            escrow.seller.clone().require_auth();
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

        // 1. First, retrieve the designated buyer already registered inside the stored escrow state
        let designated_buyer = escrow.buyer.as_ref().ok_or(ContractError::NotAuthorized)?;

        // 2. Enforce authorization check BEFORE mutating any struct fields
        if &buyer != designated_buyer {
            return Err(ContractError::NotAuthorized);
        }

        // 3. Mark the ledger updates safely
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
        let mut escrow = load_escrow(&env, escrow_id)?;

        if escrow.seller != caller {
            return Err(ContractError::NotAuthorized);
        }

        if escrow.state != EscrowState::Funded {
            return Err(ContractError::InvalidState);
        }

        if tracking_id.len() == 0 {
            return Err(ContractError::InvalidTrackingId);
        }
        if tracking_id.len() > MAX_TRACKING_ID_LEN {
            return Err(ContractError::InputTooLong);
        }

        let shipped_at = env.ledger().timestamp();
        escrow.state = EscrowState::Shipped;
        escrow.shipped_at = shipped_at;
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

    pub fn confirm_delivery(
        env: Env,
        caller: Address,
        escrow_id: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        ensure_not_paused(&env)?;
        let mut escrow = load_escrow(&env, escrow_id)?;

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

        let fee_config = read_fee_config(&env);
        let fee_collector: Address = env
            .storage()
            .instance()
            .get(&DataKey::FeeCollector)
            .expect("not initialized");

        transfer_with_protocol_fee(
            &env,
            &escrow.token,
            &escrow.seller,
            &fee_collector,
            escrow.amount,
            fee_config.protocol_fee_bps,
        )?;

        escrow.state = EscrowState::Completed;
        save_escrow(&env, escrow_id, &escrow);
        increment_counter(&env, &DataKey::TotalCompleted)?;
        emit_escrow_completed(
            &env,
            escrow_id,
            escrow.seller.clone(),
            escrow.amount,
            fee_config.protocol_fee_bps,
        );
        Ok(())
    }

    pub fn raise_dispute(
        env: Env,
        caller: Address,
        escrow_id: u64,
        reason: Symbol,
        description: String,
        evidence_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        ensure_not_paused(&env)?;
        let mut escrow = load_escrow(&env, escrow_id)?;

        let buyer = escrow.buyer.clone().ok_or(ContractError::EscrowHasNoBuyer)?;
        if caller != buyer {
            return Err(ContractError::NotAuthorized);
        }

        if escrow.state != EscrowState::Shipped {
            return Err(ContractError::InvalidState);
        }

        if env.ledger().timestamp() >= escrow.dispute_deadline {
            return Err(ContractError::DisputeWindowClosed);
        }

        if description.len() > MAX_DESCRIPTION_LEN {
            return Err(ContractError::InputTooLong);
        }

        escrow.state = EscrowState::Disputed;

        let dispute_data = DisputeData {
            escrow_id,
            reason,
            description,
            evidence_hash,
            status: DisputeStatus::Active,
            disputed_at: env.ledger().timestamp(),
            tracking_id: escrow.tracking_id.clone(),
        };

        save_escrow(&env, escrow_id, &escrow);
        save_dispute(&env, escrow_id, &dispute_data);
        increment_counter(&env, &DataKey::TotalDisputed)?;
        emit_dispute_raised(
            &env,
            escrow_id,
            buyer,
            dispute_data.reason.clone(),
            dispute_data.description.clone(),
            dispute_data.evidence_hash.clone(),
        );
        Ok(())
    }

    pub fn resolve_dispute(env: Env, caller: Address, escrow_id: u64, resolution: ResolutionType) -> Result<(), ContractError> {
        // SECURITY:
        // Authenticate before any state reads.
        caller.require_auth();

        ensure_not_paused(&env)?;
        let mut escrow = load_escrow(&env, escrow_id)?;
        let admin = require_admin(&env);

        if caller != escrow.resolver && caller != admin {
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
        let next_total = current_total.checked_add(arbitration_fee).ok_or(ContractError::ArithmeticError)?;
        env.storage().instance().set(&total_key, &next_total);

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
            ResolutionType::Release => increment_counter(&env, &DataKey::TotalCompleted)?,
            ResolutionType::Refund => increment_counter(&env, &DataKey::TotalRefunded)?,
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
        let mut escrow = load_escrow(&env, escrow_id)?;

        if escrow.state != EscrowState::Shipped {
            return Err(ContractError::InvalidState);
        }

        if escrow.delivered_at == 0 {
            return Err(ContractError::InvalidState);
        }

        let eligible_at = escrow
            .delivered_at
            .checked_add(DELIVERY_RELEASE_WINDOW)
            .ok_or(ContractError::ArithmeticOverflow)?;
        if env.ledger().timestamp() < eligible_at {
            return Err(ContractError::ShippingWindowNotElapsed);
        }

        if load_dispute(&env, escrow_id).is_ok() {
            return Err(ContractError::InvalidState);
        }

        let fee_config = read_fee_config(&env);
        let fee_collector: Address = env
            .storage()
            .instance()
            .get(&DataKey::FeeCollector)
            .expect("not initialized");

        transfer_with_protocol_fee(
            &env,
            &escrow.token,
            &escrow.seller,
            &fee_collector,
            escrow.amount,
            fee_config.protocol_fee_bps,
        )?;

        escrow.state = EscrowState::Completed;
        save_escrow(&env, escrow_id, &escrow);
        increment_counter(&env, &DataKey::TotalCompleted)?;
        emit_auto_released(
            &env,
            escrow_id,
            escrow.seller.clone(),
            escrow.amount,
            fee_config.protocol_fee_bps,
        );
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
        let counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(1);
        for id in 1..counter {
            if let Ok(escrow) = load_escrow(&env, id) {
                if escrow.buyer.as_ref() == Some(&buyer) {
                    result.push_back(id);
                }
            }
        }
        result
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

    pub fn get_fee_config(env: Env) -> FeeConfig {
        read_fee_config(&env)
    }

    pub fn get_contract_config(env: Env) -> ContractConfig {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("not initialized");
        let fee_collector: Address = env
            .storage()
            .instance()
            .get(&DataKey::FeeCollector)
            .expect("not initialized");
        let fee_config = read_fee_config(&env);
        let escrow_count: u64 = env.storage().instance().get(&DataKey::EscrowCounter).unwrap_or(1) - 1;
        ContractConfig {
            admin,
            fee_bps: fee_config.protocol_fee_bps,
            fee_collector,
            escrow_count,
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
mod test_initialize_zero_admin;
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
mod test_concurrent_vendor_escrows;
