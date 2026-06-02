#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Symbol};

/// Storage keys for persisting escrow data and the global escrow counter.
#[contracttype]
pub enum DataKey {
    Admin,
    Escrow(u64),
    EscrowCounter,
    FeeCollector,
    Dispute(u64),
    Paused,
    DefaultFeeBps,
    TtlExtensionLedgers,
    ArbitrationFee,
    TotalArbitrationFees(Address),
    AccumulatedFees(Address),
    TotalCreated,
    TotalCompleted,
    TotalDisputed,
    TotalRefunded,
    FeeConfig,
    BuyerEscrowIndex(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    Active,
    Resolved,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeData {
    pub escrow_id: u64,
    pub reason: Symbol,
    pub description: String,
    pub evidence_hash: BytesN<32>,
    pub status: DisputeStatus,
    pub disputed_at: u64,
    pub tracking_id: Option<String>,
}

/// Resolution direction for `resolve_dispute`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolutionType {
    Release,
    Refund,
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
    ArithmeticOverflow = 15,
    InvalidStateTransition = 16,
    InputTooLong = 17,
    InvalidAddress = 18,
    SameAddress = 19,
    AmountExceedsMaximum = 20,
    InvalidTrackingId = 21,
    DeliveryNotRecorded = 22,
}

/// Lifecycle states of an escrow transaction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowState {
    Pending,
    Funded,
    Shipped,
    Completed,
    Disputed,
    Refunded,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    DefaultFeeBps,
    EscrowCounter,
    Escrow(u32),
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
    pub fee_bps: u32, // Snapshot parameter tracking slot
    pub funded_at: u64,
    pub shipped_at: u64,
    pub created_at: u64,
    pub state: EscrowState,
}
