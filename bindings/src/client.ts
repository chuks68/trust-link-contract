import {
  type AddressLike,
  type ContractSymbol,
  type DisputeData,
  type EscrowData,
  type FeeConfig,
  type ResolutionType,
} from "./types.js";

export interface ContractTransport {
  invoke<TReturn>(method: string, args: readonly unknown[]): TReturn | Promise<TReturn>;
}

export class EscrowClient {
  constructor(private readonly transport: ContractTransport) {}

  initialize(admin: AddressLike, feeCollector: AddressLike): void | Promise<void> {
    return this.transport.invoke("initialize", [admin, feeCollector]);
  }

  pause_contract(): void | Promise<void> {
    return this.transport.invoke("pause_contract", []);
  }

  unpause_contract(): void | Promise<void> {
    return this.transport.invoke("unpause_contract", []);
  }

  withdraw_fees(token: AddressLike, to: AddressLike, amount: bigint): void | Promise<void> {
    return this.transport.invoke("withdraw_fees", [token, to, amount]);
  }

  create_escrow(
    seller: AddressLike,
    resolver: AddressLike,
    token: AddressLike,
    amount: bigint,
    feeBps: number,
    shippingWindow: bigint,
  ): bigint | Promise<bigint> {
    return this.transport.invoke("create_escrow", [seller, resolver, token, amount, feeBps, shippingWindow]);
  }

  fund_escrow(escrowId: bigint, buyer: AddressLike): void | Promise<void> {
    return this.transport.invoke("fund_escrow", [escrowId, buyer]);
  }

  mark_shipped(caller: AddressLike, escrowId: bigint, trackingId: string): void | Promise<void> {
    return this.transport.invoke("mark_shipped", [caller, escrowId, trackingId]);
  }

  confirm_delivery(caller: AddressLike, escrowId: bigint): void | Promise<void> {
    return this.transport.invoke("confirm_delivery", [caller, escrowId]);
  }

  raise_dispute(
    escrowId: bigint,
    reason: ContractSymbol,
    description: string,
    evidenceHash: Uint8Array,
  ): void | Promise<void> {
    return this.transport.invoke("raise_dispute", [escrowId, reason, description, evidenceHash]);
  }

  resolve_dispute(escrowId: bigint, resolution: ResolutionType): void | Promise<void> {
    return this.transport.invoke("resolve_dispute", [escrowId, resolution]);
  }

  auto_release(escrowId: bigint): void | Promise<void> {
    return this.transport.invoke("auto_release", [escrowId]);
  }

  get_escrow(escrowId: bigint): EscrowData | Promise<EscrowData> {
    return this.transport.invoke("get_escrow", [escrowId]);
  }

  get_dispute(escrowId: bigint): DisputeData | null | Promise<DisputeData | null> {
    return this.transport.invoke("get_dispute", [escrowId]);
  }

  get_fee_config(): FeeConfig | Promise<FeeConfig> {
    return this.transport.invoke("get_fee_config", []);
  }

  set_arbitration_fee(caller: AddressLike, feeBps: number): void | Promise<void> {
    return this.transport.invoke("set_arbitration_fee", [caller, feeBps]);
  }

  get_arbitration_fee(): number | Promise<number> {
    return this.transport.invoke("get_arbitration_fee", []);
  }

  rotate_resolver(caller: AddressLike, escrowId: bigint, newResolver: AddressLike): void | Promise<void> {
    return this.transport.invoke("rotate_resolver", [caller, escrowId, newResolver]);
  }
}
