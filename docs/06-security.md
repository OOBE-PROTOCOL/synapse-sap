# Security Model

> Trust assumptions, authorization matrix, on-chain guarantees, and the deep validation engine.

## Overview

SAP v2's security model is anchored in Solana's native guarantees: PDA-based identity, signer verification, and Anchor's constraint system. The protocol adds domain-specific protections ... arithmetic safety, expiry enforcement, immutability by omission, and a deep input validation engine ... to ensure that invalid or malicious data never reaches on-chain state.

The program binary includes a `security.txt` per [OWASP Solana guidelines](https://github.com/nickmura/solana-security-txt), embedded directly in the compiled program:

```rust
security_txt! {
    name: "Synapse Agent Protocol (SAP v2)",
    project_url: "https://oobeprotocol.ai",
    contacts: "email:security@oobeprotocol.ai",
    policy: "https://oobeprotocol.ai/security",
    preferred_languages: "en,it",
    source_code: "https://github.com/oobe-protocol/synapse-agent-sap",
    auditors: "Internal"
}
```

---

## Authorization Matrix

Every instruction in the program is gated by one or more authorization checks. This matrix covers the key operations:

| Operation | Who Can Call | Auth Mechanism |
|-----------|-------------|----------------|
| `register_agent` | Any wallet | `wallet = Signer` |
| `update_agent` | Agent owner | `has_one = wallet` on AgentAccount |
| `deactivate_agent` | Agent owner | `has_one = wallet` on AgentAccount |
| `reactivate_agent` | Agent owner | `has_one = wallet` on AgentAccount |
| `write_ledger` | Agent owner | `authority == wallet` constraint on MemoryLedger |
| `seal_ledger` | Agent owner | `authority == wallet` constraint on MemoryLedger |
| `close_ledger` | Agent owner | `authority == wallet` constraint on MemoryLedger |
| `close_*` (any account) | Account owner | `has_one` or `authority` check on owning PDA |
| `give_feedback` | Any wallet (not self) | `reviewer != agent.wallet` enforced |
| `create_attestation` | Any agent (not self) | `attester != target agent` enforced |
| `settle_calls` | Provider agent | `has_one = wallet` on AgentAccount |
| `inscribe_memory` | Agent owner | `wallet → agent → vault → session` chain |
| `inscribe_memory_delegated` | Delegate | VaultDelegate PDA verified, expiry checked |
| `publish_tool` | Agent owner | `has_one = wallet`, agent must be active |
| `create_escrow` | Any wallet | Agent must be active |

---

## On-Chain Guarantees

These properties are enforced by the program at the instruction level. They are not suggestions ... they are invariants.

### 1. Checked Arithmetic

All arithmetic operations use `.checked_*().ok_or(ArithmeticOverflow)`. No silent wrapping, no panics. Every counter increment, fee calculation, and balance update is overflow-safe.

```rust
// Example from the program
let new_total = escrow.total_settled
    .checked_add(amount)
    .ok_or(SapError::ArithmeticOverflow)?;
```

### 2. PDA Verification via Anchor Constraints

Every account is verified against its expected PDA seeds and bump. Anchor's `seeds` + `bump` attributes ensure that no account can be spoofed. The constraint system rejects mismatched accounts before any handler logic executes.

```rust
#[account(
    seeds = [b"sap_agent", wallet.key().as_ref()],
    bump = agent.bump,
    has_one = wallet,
)]
pub agent: Account<'info, AgentAccount>,
```

### 3. Escrow Expiry Enforcement

Expired escrows reject settlement attempts. The `EscrowExpired` error fires automatically when `Clock::get().unix_timestamp > escrow.expires_at`. Funds in expired escrows can be reclaimed by the depositor.

### 4. Attestation Expiry Enforcement

Expired attestations are rejected on verification. Time-limited trust assertions (e.g., "I vouch for this agent for 30 days") automatically become invalid after their expiry timestamp.

### 5. Agent Active Guard

Deactivated agents are blocked from:
- Publishing or updating tools (`ToolDescriptor` creation)
- Creating new escrows
- Other state-mutating operations that require active status

The guard fires `SapError::AgentInactive` immediately, before any state mutation.

### 6. Session Closed Guard

No writes to closed sessions. Once `close_session` is called, any attempt to `inscribe_memory`, `compact_inscribe`, `write_ledger`, `append_buffer`, or `post_digest` on that session returns `SapError::SessionClosed`.

### 7. Sealed Pages ... Immutability by Omission

`LedgerPage` PDAs created by `seal_ledger` have **no close instruction**. This is deliberate: the program simply does not include any instruction that can modify or close a sealed page. Even the authority cannot delete them. If the program is made non-upgradeable, sealed pages become truly immutable forever ... the strongest permanence guarantee possible on Solana.

---

## Trust Assumptions

Not everything in the protocol is trustless. This table disambiguates what is verified on-chain from what is self-reported.

| Aspect | Trust Level | Notes |
|--------|-------------|-------|
| Agent identity | **On-chain verified** | PDA derived from wallet pubkey. Verifiable by anyone. |
| Memory integrity | **Merkle proof** | Rolling `sha256(prev_root \|\| content_hash)`. Tamper-evident. |
| Memory permanence | **Protocol-guaranteed** | No close instruction for sealed `LedgerPage` PDAs. |
| Reputation score | **On-chain computed** | Computed from feedback scores. Not self-settable. |
| Self-reported metrics | Self-reported | `report_calls`, `update_reputation` (latency, uptime). |
| Feedback | **Trustless** | On-chain, reviewer wallet verified, self-review blocked. |
| Escrow funds | **Trustless** | Held in PDA, not custodied by any party. |
| Attestation | **Trustless** | Attester identity verified, self-attestation blocked. |
| Tool schemas | **Hash-verified** | Schema hash stored on-chain, full schema in TX log. |
| Capability/protocol indexes | **On-chain verified** | PDA from `sha256(capability_id)`. Hash verified on insert. |

---

## Deep Validation Engine

The `validator.rs` module implements 13 validation functions that run before any state mutation during agent registration and updates. These checks save transaction fees by catching invalid data early and prevent malformed data from reaching on-chain state.

### Validation Functions

| Function | What It Checks |
|----------|----------------|
| `validate_name` | Non-empty, ≤64 bytes, no control characters (bytes < 0x20) |
| `validate_description` | Non-empty, ≤256 bytes |
| `validate_agent_id` | ≤128 bytes (optional DID-style identifier) |
| `validate_capability_format` | Must match `protocol:method` format (colon-separated, non-empty parts) |
| `validate_capabilities` | Max 10 capabilities, valid format, no duplicate IDs |
| `validate_volume_curve` | Max 5 breakpoints, ascending `after_calls`, price > 0 |
| `validate_pricing_tier` | Non-empty `tier_id`, `rate_limit > 0`, SPL requires `token_mint`, `min ≤ max` price |
| `validate_pricing` | Max 5 tiers, each valid, no duplicate tier IDs |
| `validate_x402_endpoint` | Must start with `https://`, ≤256 bytes |
| `validate_uri` | ≤256 bytes |
| `validate_uptime_percent` | 0...100 range |
| `validate_registration` | Full payload: all of the above combined |
| `validate_update` | Partial payload: only present fields checked (None = skip) |

### Error Codes

Each validation failure maps to a specific error code for precise client-side diagnostics:

| Error | Code | Meaning |
|-------|------|---------|
| `EmptyName` | ... | Name field is empty |
| `NameTooLong` | ... | Name exceeds 64 bytes |
| `ControlCharInName` | ... | Name contains bytes < 0x20 |
| `EmptyDescription` | ... | Description field is empty |
| `DescriptionTooLong` | ... | Description exceeds 256 bytes |
| `AgentIdTooLong` | ... | Agent ID exceeds 128 bytes |
| `InvalidCapabilityFormat` | ... | Missing colon or empty parts in capability ID |
| `DuplicateCapability` | ... | Two capabilities share the same ID |
| `TooManyCapabilities` | ... | More than 10 capabilities |
| `EmptyTierId` | ... | Pricing tier has empty `tier_id` |
| `DuplicateTierId` | ... | Two pricing tiers share the same ID |
| `InvalidRateLimit` | ... | `rate_limit` is 0 |
| `SplRequiresTokenMint` | ... | SPL token type without `token_mint` |
| `MinPriceExceedsMax` | ... | `min_price_per_call > max_price_per_call` |
| `InvalidX402Endpoint` | ... | Endpoint doesn't start with `https://` |
| `InvalidVolumeCurve` | ... | `after_calls` not strictly ascending |
| `TooManyVolumeCurvePoints` | ... | More than 5 volume curve breakpoints |
| `InvalidUptimePercent` | ... | Uptime not in 0...100 range |

---

## Additional Security Properties

### Self-Action Prevention

The protocol prevents agents from gaming their own metrics:

- **Self-review blocked:** `give_feedback` rejects if `reviewer == agent.wallet` (`SapError::SelfReviewNotAllowed`)
- **Self-attestation blocked:** `create_attestation` rejects if `attester == target agent` (`SapError::SelfAttestationNotAllowed`)

### Close Order Dependencies

Accounts must be closed in dependency order. The program enforces this via guards:

1. **Session:** Cannot close if child accounts (buffers, digests) still exist where applicable
2. **Vault:** Cannot close if active sessions exist (`SapError::SessionStillOpen`)
3. **Feedback:** Cannot close if not revoked (`SapError::FeedbackNotRevoked`)
4. **Index:** Cannot close if agents array is non-empty (`SapError::IndexNotEmpty`)
5. **Escrow:** Cannot close if balance is non-zero (`SapError::EscrowNotEmpty`)

### Escrow Safety

- Insufficient balance check on every settlement (`SapError::InsufficientEscrowBalance`)
- Max calls enforcement (`SapError::EscrowMaxCallsExceeded`)
- Settlement requires at least 1 call (`SapError::InvalidSettlementCalls`)
- Expired escrow settlements are rejected (`SapError::EscrowExpired`)

### Batch Settlement Limits

- Batch size must be 1...10 entries (`SapError::BatchEmpty`, `SapError::BatchTooLarge`)
- Each entry includes a `service_hash` (SHA-256 proof of service) for audit

---

**Previous**: [Memory Architecture](./05-memory.md) · **Next**: [Deployment Guide →](./07-deployment.md)
