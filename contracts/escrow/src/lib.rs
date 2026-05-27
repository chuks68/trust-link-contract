#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env};

/// Maximum protocol fee in basis points (300 = 3%).
const MAX_FEE_BPS: u32 = 300;

/// Storage keys for persisting escrow data and the global escrow counter.
#[contracttype]
pub enum DataKey {
    Admin,
    Escrow(u64),
    EscrowCount,
    EscrowCounter,
    FeeCollector,
    Dispute(u64),
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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeesWithdrawn {
    pub token: Address,
    pub to: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Lifecycle states of an escrow transaction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowState {
    /// Escrow created but not yet funded by a buyer.
    Pending,
    /// Escrow funded and awaiting delivery confirmation or dispute.
    Funded,
    /// Escrow successfully completed with funds released to the seller.
    Completed,
    /// Escrow in dispute, awaiting resolver decision.
    Disputed,
    /// Escrow refunded to the buyer after dispute resolution.
    Refunded,
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

#[contract]
pub struct Escrow;

fn deduct_and_transfer(env: &Env, token_addr: &Address, recipient: &Address, amount: i128, fee_bps: u32) -> Result<(), ContractError> {
    if amount < 0 {
        return Err(ContractError::InvalidAmount);
    }

    // Use split calculation to avoid overflow for large amounts
    // fee = (amount / 10000) * fee_bps + (amount % 10000) * fee_bps / 10000
    let part1 = (amount / 10_000)
        .checked_mul(fee_bps as i128)
        .ok_or(ContractError::ArithmeticError)?;
    let part2 = (amount % 10_000)
        .checked_mul(fee_bps as i128)
        .ok_or(ContractError::ArithmeticError)?
        / 10_000;

    let fee = part1.checked_add(part2).ok_or(ContractError::ArithmeticError)?;
    let net = amount.checked_sub(fee).ok_or(ContractError::ArithmeticError)?;

    let token_client = token::Client::new(env, token_addr);

    // Protocol fees are kept in the contract balance.
    // The admin can later withdraw them using the `withdraw_fees` function.
    // We only transfer the net amount to the recipient.
    token_client.transfer(&env.current_contract_address(), recipient, &net);
    Ok(())
}

#[contractimpl]
#[allow(deprecated)]
impl Escrow {
    /// Sets the protocol fee collector and admin address. Must be called once.
    pub fn initialize(env: Env, admin: Address, fee_collector: Address, arbitration_fee: i128) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::FeeCollector, &fee_collector);
        env.storage().instance().set(&DataKey::ArbitrationFee, &arbitration_fee);
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

    /// Creates a new escrow transaction in the Pending state.
    ///
    /// This function initializes an escrow with the specified parameters and assigns it
    /// a unique sequential ID. The escrow remains in the Pending state until a buyer
    /// funds it via `fund_escrow`.
    ///
    /// # Arguments
    ///
    /// * `env` - The Soroban environment providing access to ledger state and storage.
    /// * `seller` - The address that will receive funds upon successful completion.
    /// * `resolver` - The address authorized to resolve disputes if they arise.
    /// * `token` - The address of the SEP-41 token contract to be used for payment.
    /// * `amount` - The quantity of tokens to be locked in escrow (must be positive).
    /// * `fee_bps` - Protocol fee in basis points (100 = 1%, max 300 = 3%).
    /// * `shipping_window` - Duration in seconds after funding before auto-release is permitted.
    ///
    /// # Returns
    ///
    /// Returns the unique escrow ID (u32) assigned to this escrow. IDs start at 1 and
    /// increment sequentially.
    ///
    /// # Errors
    ///
    /// This function panics if:
    /// - The seller address fails authentication (does not sign the transaction).
    /// - The `fee_bps` exceeds the maximum allowed fee (300 basis points).
    /// - Storage operations fail (extremely rare in normal operation).
    ///
    /// # Auth
    ///
    /// Requires authorization from the `seller` address. The seller must sign this
    /// transaction to prove they are creating the escrow.
    pub fn create_escrow(
        env: Env,
        seller: Address,
        resolver: Address,
        token: Address,
        amount: i128,
        fee_bps: u32,
        shipping_window: u64,
    ) -> Result<u64, ContractError> {
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
        };

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        env.events().publish(("create_escrow",), escrow_id);
        Ok(escrow_id)
    }

    pub fn fund_escrow(env: Env, escrow_id: u64, buyer: Address) -> Result<(), ContractError> {
        buyer.require_auth();

        let mut escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.state != EscrowState::Pending {
            return Err(ContractError::InvalidState);
        }

        escrow.buyer = Some(buyer.clone());
        escrow.state = EscrowState::Funded;
        escrow.funded_at = env.ledger().timestamp();
        escrow.dispute_deadline = escrow.funded_at + 172800;

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(&buyer, env.current_contract_address(), &escrow.amount);

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &escrow);
        env.events().publish(("fund_escrow",), escrow_id);
        Ok(())
    }

    pub fn confirm_delivery(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.state != EscrowState::Funded {
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

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
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
        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.state != EscrowState::Funded {
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

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.storage()
            .instance()
            .set(&DataKey::Dispute(escrow_id), &dispute_data);

        env.events()
            .publish(("raise_dispute",), (escrow_id,));
        Ok(())
    }

    pub fn resolve_dispute(env: Env, escrow_id: u64, resolution: ResolutionType) -> Result<(), ContractError> {
        let mut escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

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

        // Deduct arbitration fee first
        escrow.amount = escrow.amount.checked_sub(arbitration_fee).ok_or(ContractError::ArithmeticError)?;

        // Update total arbitration fees tracking
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

        let mut dispute_data: DisputeData = env
            .storage()
            .instance()
            .get(&DataKey::Dispute(escrow_id))
            .ok_or(ContractError::DisputeNotFound)?;
        dispute_data.status = DisputeStatus::Resolved;

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.storage()
            .instance()
            .set(&DataKey::Dispute(escrow_id), &dispute_data);

        env.events()
            .publish(("resolve_dispute",), (escrow_id, resolution));
        Ok(())
    }

    pub fn auto_release(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let escrow: EscrowData = env
            .storage()
            .instance()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.state != EscrowState::Funded {
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

        env.storage()
            .instance()
            .set(&DataKey::Escrow(escrow_id), &updated);
        env.events().publish(("auto_release",), escrow_id);
        Ok(())
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
}

mod test;
mod test_withdraw_fees;
mod test_dispute;
mod test_escrow_id;
mod test_resolution;
mod test_overflow;
mod test_arbitration_fee;
