export const contractName = "Escrow";

export const contractAbi = {
  contractName,
  functions: [
    { name: "initialize", inputs: ["admin", "fee_collector"], output: "void" },
    { name: "pause_contract", inputs: [], output: "void" },
    { name: "unpause_contract", inputs: [], output: "void" },
    { name: "withdraw_fees", inputs: ["token", "to", "amount"], output: "void" },
    {
      name: "create_escrow",
      inputs: ["seller", "resolver", "token", "amount", "fee_bps", "shipping_window"],
      output: "u64",
    },
    { name: "fund_escrow", inputs: ["escrow_id", "buyer"], output: "void" },
    { name: "mark_shipped", inputs: ["caller", "escrow_id", "tracking_id"], output: "void" },
    { name: "confirm_delivery", inputs: ["caller", "escrow_id"], output: "void" },
    {
      name: "raise_dispute",
      inputs: ["escrow_id", "reason", "description", "evidence_hash"],
      output: "void",
    },
    { name: "resolve_dispute", inputs: ["escrow_id", "resolution"], output: "void" },
    { name: "auto_release", inputs: ["escrow_id"], output: "void" },
    { name: "get_escrow", inputs: ["escrow_id"], output: "EscrowData" },
    { name: "get_dispute", inputs: ["escrow_id"], output: "DisputeData" },
    { name: "get_fee_config", inputs: [], output: "FeeConfig" },
    { name: "set_arbitration_fee", inputs: ["caller", "fee_bps"], output: "void" },
    { name: "get_arbitration_fee", inputs: [], output: "u32" },
  ],
  types: [
    "EscrowState",
    "DisputeStatus",
    "ResolutionType",
    "ContractError",
    "FeeConfig",
    "FeesWithdrawn",
    "ContractPausedEvent",
    "ContractUnpausedEvent",
    "EscrowData",
    "DisputeData",
  ],
} as const;
