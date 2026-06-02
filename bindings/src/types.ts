export type AddressLike = string;
export type ContractSymbol = string;
export type Bytes32 = Uint8Array;
export type Result<T> = T | null;

export enum EscrowState {
  Pending = "Pending",
  Funded = "Funded",
  Shipped = "Shipped",
  Completed = "Completed",
  Disputed = "Disputed",
  Refunded = "Refunded",
  Canceled = "Canceled",
}

export enum DisputeStatus {
  Active = "Active",
  Resolved = "Resolved",
}

export enum ResolutionType {
  Release = "Release",
  Refund = "Refund",
}

export enum ContractError {
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
  InvalidTrackingId = 13,
}

export interface FeeConfig {
  collector: AddressLike;
  max_fee_bps: number;
}

export interface FeesWithdrawn {
  token: AddressLike;
  to: AddressLike;
  amount: bigint;
  timestamp: bigint;
}

export interface ContractPausedEvent {
  admin: AddressLike;
  timestamp: bigint;
}

export interface ContractUnpausedEvent {
  admin: AddressLike;
  timestamp: bigint;
}

export interface EscrowData {
  seller: AddressLike;
  buyer: AddressLike | null;
  resolver: AddressLike;
  token: AddressLike;
  amount: bigint;
  fee_bps: number;
  shipping_window: bigint;
  funded_at: bigint;
  dispute_deadline: bigint;
  state: EscrowState;
  shipped_at: bigint;
  tracking_id: string | null;
  delivered_at: bigint;
}

export interface DisputeData {
  escrow_id: bigint;
  reason: ContractSymbol;
  description: string;
  evidence_hash: Bytes32;
  status: DisputeStatus;
  disputed_at: bigint;
}
