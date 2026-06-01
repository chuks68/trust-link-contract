use soroban_sdk::contracterror;

/// Stable contract error codes for the escrow lifecycle.
///
/// The numeric values are part of the public ABI and must not be renumbered.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    /// Returned when an amount is zero, negative, or otherwise invalid for the requested operation.
    InvalidAmount = 1,
    /// Returned when the contract does not hold enough tokens for the requested transfer or withdrawal.
    InsufficientBalance = 2,
    /// Returned when the requested escrow id does not exist in storage.
    EscrowNotFound = 3,
    /// Returned when the escrow is in a state that does not permit the requested action.
    InvalidState = 4,
    /// Returned when the caller is not authorized to perform the requested privileged action.
    NotAuthorized = 5,
    /// Returned when `initialize` is called after the contract has already been initialized.
    AlreadyInitialized = 6,
    /// Returned when a fee basis-point value exceeds the configured hard cap.
    FeeExceedsMax = 7,
    /// Returned when an action requires an assigned buyer but the escrow has none yet.
    EscrowHasNoBuyer = 8,
    /// Returned when auto-release is attempted before the configured shipping window has elapsed.
    ShippingWindowNotElapsed = 9,
    /// Returned when dispute evidence fails validation.
    InvalidEvidenceHash = 10,
    /// Returned when a dispute record is missing for the requested escrow.
    DisputeNotFound = 11,
    /// Returned when internal checked arithmetic fails while computing a payout or fee.
    ArithmeticError = 12,
    /// Returned when the dispute window has closed for the requested escrow action.
    DisputeWindowClosed = 13,
    /// Returned when a contract action is blocked because the contract is paused.
    ContractPaused = 14,
    /// Returned when checked arithmetic overflows in helper payout calculations.
    ArithmeticOverflow = 15,
    /// Returned when a state transition is not part of the approved lifecycle matrix.
    InvalidStateTransition = 16,
    /// Returned when a supplied string or payload exceeds the supported length.
    InputTooLong = 17,
    /// Returned when an address argument is invalid for its role (e.g. admin and
    /// fee_collector must be distinct keys).
    InvalidAddress = 18,
    /// Returned when an update is a no-op because the new value equals the
    /// current one (e.g. rotating admin to the same address).
    SameAddress = 19,
    /// Returned when an escrow amount exceeds the maximum allowed limit.
    AmountExceedsMaximum = 20,
    /// Returned when a tracking ID is empty or otherwise invalid for shipment.
    InvalidTrackingId = 21,
    /// Returned when auto-release is attempted before delivery has been recorded.
    DeliveryNotRecorded = 22,
}
