# Oracle Trust Model — Why the Backend Signer Is Needed

## Overview

TrustLink is a non-custodial P2P escrow on Stellar/Soroban. Most state transitions
are governed entirely by on-chain signatures from the involved parties (seller, buyer).
Two flows, however, require a trusted off-chain actor:

1. **Dispute resolution** — a designated `resolver` address adjudicates whether funds
   go to the seller or back to the buyer.
2. **Auto-release triggering** — any caller (including a backend service) may invoke
   `auto_release` once the shipping window has elapsed.

This document focuses on the resolver as the primary oracle and explains why it exists,
what trust it requires, and how to manage its key safely.

---

## Why a Resolver Is Necessary

Soroban contracts have no access to real-world shipping data, courier APIs, or legal
dispute processes. When buyer and seller disagree on delivery, the contract itself
cannot adjudicate. A neutral third party — the `resolver` — is therefore embedded in
each escrow at creation time.

The resolver is an account (EOA or multisig smart-contract) whose `require_auth()` call
in `resolve_dispute` is the only way to move an escrow out of `Disputed` state. This is
the single centralized trust assumption in the protocol.

```
Disputed ──[resolver signs resolve_dispute]──> Completed | Refunded
```

---

## Trust Assumptions

| Assumption | Where it appears | Risk if violated |
|---|---|---|
| Resolver will not collude with one party | `resolve_dispute` requires resolver auth | Funds stolen or wrongly withheld |
| Resolver key is not compromised | Resolver signs every dispute settlement | Attacker can drain any disputed escrow |
| Resolver is liveness-available | Disputes can only be resolved by the resolver | Funds locked indefinitely if resolver goes offline |
| Auto-release caller has no privileged access | `auto_release` is permissionless | None — caller cannot redirect funds |

---

## Cryptographic Requirements

- The resolver must use an Ed25519 keypair (Stellar native) or a custom Soroban account
  contract implementing `__check_auth` (for multisig or hardware-key setups).
- `resolve_dispute` calls `escrow.resolver.require_auth()`, which enforces that the
  transaction is signed by the resolver's keypair and that no other address can
  substitute for it — auth is anchored to the specific address stored at escrow creation.
- Evidence is committed as a 32-byte hash (SHA-256 of the off-chain evidence package,
  e.g. an IPFS CID). The contract does not verify the evidence content — the resolver is
  trusted to interpret it correctly.

---

## Security Risks

### 1. Resolver Key Compromise
If an attacker obtains the resolver's private key they can:
- Call `resolve_dispute` on any escrow in `Disputed` state.
- Choose any outcome (release to seller or refund buyer), effectively stealing funds.

**Mitigation**: Use a hardware security module (HSM) or multisig account contract as the
resolver. Soroban's `require_auth` accepts any address including custom account
contracts, so a 2-of-3 multisig resolver is fully supported.

### 2. Resolver Censorship / Liveness Failure
A resolver that stops responding (key lost, service down, legal injunction) will leave
all disputed escrows permanently locked — neither party can retrieve funds.

**Mitigation**:
- Introduce a dispute-timeout after which the buyer can reclaim funds unilaterally
  (future improvement — not yet in contract).
- Operate resolver infrastructure with high availability and monitoring.
- Store the resolver private key in a geographically distributed secret-management
  system with recovery procedures.

### 3. Single Point of Trust
Each escrow hard-codes its resolver at creation (`create_escrow(resolver: Address)`).
There is no way to update the resolver after creation. A compromised resolver affects
every escrow that named it.

**Mitigation**: Use a multisig contract as resolver so that compromise of one key is
insufficient to forge a signature.

### 4. Evidence Integrity
The 32-byte `evidence_hash` is committed on-chain but its content is off-chain. The
resolver is trusted to verify that the hash corresponds to legitimate evidence before
signing a resolution.

**Mitigation**: Resolver infrastructure should re-derive the hash from the original
evidence package before signing, and refuse to sign if they cannot verify the source.

---

## Backup Strategies

### Resolver Key Backup
1. Generate the resolver keypair in a hardware wallet (Ledger, YubiHSM).
2. Store the mnemonic seed phrase in at minimum two physically separated, encrypted
   offline vaults.
3. For service-operated resolvers, use a KMS (AWS KMS, GCP Cloud KMS, HashiCorp Vault)
   with key material that never leaves the secure boundary.
4. Maintain a documented runbook for key recovery that has been tested at least annually.

### Multisig Rotation
Because `create_escrow` accepts any Soroban address as resolver, a multisig account
contract can be passed directly. When the underlying signers change:
- Deploy a new multisig account contract with the updated signer set.
- Going forward, reference the new multisig address in new escrows.
- Existing escrows retain the old resolver — coordinate resolution of in-flight disputes
  before rotating away from the old multisig.

---

## Fee Collector Key Rotation

Unlike the resolver (which is per-escrow and immutable), the protocol-wide fee collector
can be rotated by calling `update_fee_collector(new_collector)`. This requires
authorization from the **current** fee collector, enabling a safe handoff:

```text
1. Generate or prepare the new collector address (can be a multisig contract).
2. Have the current fee collector sign a transaction calling update_fee_collector.
3. Verify get_fee_config() returns the new address.
4. Revoke or archive the old key.
```

This rotation path means the fee collector should itself be a multisig account for any
production deployment, so that key rotation is governed by a quorum rather than a
single signer.

---

## Auto-Release and the Backend Signer

`auto_release` is **permissionless**: any account — including an automated backend
service — can call it once `ledger.timestamp() >= funded_at + shipping_window`. The
caller cannot redirect funds; they always go to the seller minus the protocol fee.

The backend signer for auto-release therefore carries **no privileged trust** — its
compromise cannot steal funds, only trigger an early payout that the shipping window
was designed to allow anyway. Nevertheless, the service should be monitored to ensure
timely release and to prevent spamming the network with premature calls.

---

## Summary

| Component | Trust level | Rotation path |
|---|---|---|
| `resolver` | High — single point of dispute finality | Deploy new multisig; update in future escrows |
| `fee_collector` | Medium — controls protocol revenue | `update_fee_collector` (current collector must sign) |
| Auto-release caller | None — permissionless, outcome is deterministic | N/A |
