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
    TotalCreated,
    TotalCompleted,
    TotalDisputed,
    TotalRefunded,
    FeeConfig,
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
    InvalidTrackingId = 18,
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
    /// Escrow was canceled while in the Pending state.
    Canceled,
}

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
    /// Ledger timestamp recorded when the seller marked the order as shipped.
    pub shipped_at: u64,
    /// Ledger timestamp recorded by the admin oracle when delivery is confirmed. Zero until set.
    pub delivered_at: u64,
    pub tracking_id: Option<String>,
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
pub struct AdminRotated {
    pub old_admin: Address,
    pub new_admin: Address,
    pub timestamp: u64,
}

/// Protocol and arbitration fee configuration in basis points.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeConfig {
    pub protocol_fee_bps: u32,
    pub arbitration_fee_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractConfig {
    pub admin: Address,
    pub fee_bps: u32,
    pub fee_collector: Address,
    pub escrow_count: u64,
}

/// On-chain lifecycle counters exposed by `get_stats`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractStats {
    pub total_created: u64,
    pub total_completed: u64,
    pub total_disputed: u64,
    pub total_refunded: u64,
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryRecorded {
    pub escrow_id: u64,
    pub delivered_at: u64,
}
