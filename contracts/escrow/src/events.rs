use soroban_sdk::{contracttype, Address, BytesN, Env, String, Symbol};

use crate::ResolutionType;

/// Event topic/data schemas used by the escrow contract.
///
/// Each emitter publishes a single-symbol topic and a structured data payload.
/// The topic symbol is the canonical event name and the payload is the data XDR
/// stored by the Soroban event log.

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeUpdated {
    pub old_fee_bps: u32,
    pub new_fee_bps: u32,
    pub timestamp: u64,
}

/// Topic: `(\"fee_updated\",)`, data: `FeeUpdated`.
pub fn emit_fee_updated(env: &Env, old_fee_bps: u32, new_fee_bps: u32) {
    env.events().publish(
        (Symbol::new(env, "fee_updated"),),
        FeeUpdated {
            old_fee_bps,
            new_fee_bps,
            timestamp: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolFeeUpdated {
    pub old_fee_bps: u32,
    pub new_fee_bps: u32,
    pub timestamp: u64,
}

/// Topic: `(\"protocol_fee_updated\",)`, data: `ProtocolFeeUpdated`.
pub fn emit_protocol_fee_updated(env: &Env, old_fee_bps: u32, new_fee_bps: u32) {
    env.events().publish(
        (Symbol::new(env, "protocol_fee_updated"),),
        ProtocolFeeUpdated {
            old_fee_bps,
            new_fee_bps,
            timestamp: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbitrationFeeUpdated {
    pub old_fee_bps: u32,
    pub new_fee_bps: u32,
    pub timestamp: u64,
}

/// Topic: `(\"arbitration_fee_updated\",)`, data: `ArbitrationFeeUpdated`.
pub fn emit_arbitration_fee_updated(env: &Env, old_fee_bps: u32, new_fee_bps: u32) {
    env.events().publish(
        (Symbol::new(env, "arbitration_fee_updated"),),
        ArbitrationFeeUpdated {
            old_fee_bps,
            new_fee_bps,
            timestamp: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminRotated {
    pub old_admin: Address,
    pub new_admin: Address,
    pub timestamp: u64,
}

/// Topic: `(\"admin_rotated\",)`, data: `AdminRotated`.
pub fn emit_admin_rotated(env: &Env, old_admin: Address, new_admin: Address) {
    env.events().publish(
        (Symbol::new(env, "admin_rotated"),),
        AdminRotated {
            old_admin,
            new_admin,
            timestamp: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractPausedEvent {
    pub admin: Address,
    pub timestamp: u64,
}

/// Topic: `(\"contract_paused\",)`, data: `ContractPausedEvent`.
pub fn emit_contract_paused(env: &Env, admin: Address) {
    env.events().publish(
        (Symbol::new(env, "contract_paused"),),
        ContractPausedEvent {
            admin,
            timestamp: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractUnpausedEvent {
    pub admin: Address,
    pub timestamp: u64,
}

/// Topic: `(\"contract_unpaused\",)`, data: `ContractUnpausedEvent`.
pub fn emit_contract_unpaused(env: &Env, admin: Address) {
    env.events().publish(
        (Symbol::new(env, "contract_unpaused"),),
        ContractUnpausedEvent {
            admin,
            timestamp: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeesWithdrawn {
    pub token: Address,
    pub to: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Topic: `(\"fees_withdrawn\",)`, data: `FeesWithdrawn`.
pub fn emit_fees_withdrawn(env: &Env, token: Address, to: Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "fees_withdrawn"),),
        FeesWithdrawn {
            token,
            to,
            amount,
            timestamp: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowCreated {
    pub escrow_id: u64,
    pub seller: Address,
    pub resolver: Address,
    pub token: Address,
    pub amount: i128,
    pub fee_bps: u32,
    pub shipping_window: u64,
    pub timestamp: u64,
}

/// Topic: `(\"escrow_created\",)`, data: `EscrowCreated`.
pub fn emit_escrow_created(
    env: &Env,
    escrow_id: u64,
    seller: Address,
    resolver: Address,
    token: Address,
    amount: i128,
    fee_bps: u32,
    shipping_window: u64,
) {
    env.events().publish(
        (Symbol::new(env, "escrow_created"),),
        EscrowCreated {
            escrow_id,
            seller,
            resolver,
            token,
            amount,
            fee_bps,
            shipping_window,
            timestamp: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowFunded {
    pub escrow_id: u64,
    pub buyer: Address,
    pub amount: i128,
    pub funded_at: u64,
}

/// Topic: `(\"escrow_funded\",)`, data: `EscrowFunded`.
pub fn emit_escrow_funded(env: &Env, escrow_id: u64, buyer: Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "escrow_funded"),),
        EscrowFunded {
            escrow_id,
            buyer,
            amount,
            funded_at: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowShipped {
    pub escrow_id: u64,
    pub seller: Address,
    pub tracking_id: String,
    pub shipped_at: u64,
}

/// Topic: `(\"escrow_shipped\",)`, data: `EscrowShipped`.
pub fn emit_escrow_shipped(env: &Env, escrow_id: u64, seller: Address, tracking_id: String) {
    env.events().publish(
        (Symbol::new(env, "escrow_shipped"),),
        EscrowShipped {
            escrow_id,
            seller,
            tracking_id,
            shipped_at: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryRecorded {
    pub escrow_id: u64,
    pub delivered_at: u64,
}

/// Topic: `(\"delivery_recorded\",)`, data: `DeliveryRecorded`.
pub fn emit_delivery_recorded(env: &Env, escrow_id: u64, delivered_at: u64) {
    env.events().publish(
        (Symbol::new(env, "delivery_recorded"),),
        DeliveryRecorded { escrow_id, delivered_at },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowCompleted {
    pub escrow_id: u64,
    pub recipient: Address,
    pub amount: i128,
    pub fee_bps: u32,
    pub completed_at: u64,
}

/// Topic: `(\"escrow_completed\",)`, data: `EscrowCompleted`.
pub fn emit_escrow_completed(
    env: &Env,
    escrow_id: u64,
    recipient: Address,
    amount: i128,
    fee_bps: u32,
) {
    env.events().publish(
        (Symbol::new(env, "escrow_completed"),),
        EscrowCompleted {
            escrow_id,
            recipient,
            amount,
            fee_bps,
            completed_at: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeRaised {
    pub escrow_id: u64,
    pub buyer: Address,
    pub reason: Symbol,
    pub description: String,
    pub evidence_hash: BytesN<32>,
    pub disputed_at: u64,
}

/// Topic: `(\"dispute_raised\",)`, data: `DisputeRaised`.
pub fn emit_dispute_raised(
    env: &Env,
    escrow_id: u64,
    buyer: Address,
    reason: Symbol,
    description: String,
    evidence_hash: BytesN<32>,
) {
    env.events().publish(
        (Symbol::new(env, "dispute_raised"),),
        DisputeRaised {
            escrow_id,
            buyer,
            reason,
            description,
            evidence_hash,
            disputed_at: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeResolved {
    pub escrow_id: u64,
    pub resolver: Address,
    pub resolution: ResolutionType,
    pub recipient: Address,
    pub amount: i128,
    pub arbitration_fee: i128,
    pub resolved_at: u64,
}

/// Topic: `(\"dispute_resolved\",)`, data: `DisputeResolved`.
pub fn emit_dispute_resolved(
    env: &Env,
    escrow_id: u64,
    resolver: Address,
    resolution: ResolutionType,
    recipient: Address,
    amount: i128,
    arbitration_fee: i128,
) {
    env.events().publish(
        (Symbol::new(env, "dispute_resolved"),),
        DisputeResolved {
            escrow_id,
            resolver,
            resolution,
            recipient,
            amount,
            arbitration_fee,
            resolved_at: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoReleased {
    pub escrow_id: u64,
    pub seller: Address,
    pub amount: i128,
    pub fee_bps: u32,
    pub released_at: u64,
}

/// Topic: `(\"auto_released\",)`, data: `AutoReleased`.
pub fn emit_auto_released(
    env: &Env,
    escrow_id: u64,
    seller: Address,
    amount: i128,
    fee_bps: u32,
) {
    env.events().publish(
        (Symbol::new(env, "auto_released"),),
        AutoReleased {
            escrow_id,
            seller,
            amount,
            fee_bps,
            released_at: env.ledger().timestamp(),
        },
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowCancelled {
    pub escrow_id: u64,
    pub seller: Address,
    pub cancelled_at: u64,
}

/// Topic: `(\"escrow_cancelled\",)`, data: `EscrowCancelled`.
pub fn emit_escrow_cancelled(env: &Env, escrow_id: u64, seller: Address) {
    env.events().publish(
        (Symbol::new(env, "escrow_cancelled"),),
        EscrowCancelled {
            escrow_id,
            seller,
            cancelled_at: env.ledger().timestamp(),
        },
    );
}
