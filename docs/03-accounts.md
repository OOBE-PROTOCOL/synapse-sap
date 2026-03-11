# Account Types

> **Synapse Agent Protocol (SAP) v2** ... 22 On-Chain Account Structs  
> Program ID: `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`  
> Anchor 0.32.1 · Solana SVM

Every piece of on-chain state in SAP is an Anchor PDA account with a deterministic seed.
This document covers all 22 account types ... their layout, rent cost, PDA derivation, and
field-level details for the most important accounts.

---

## Table of Contents

1. [Account Summary Table](#account-summary-table)
2. [Key Accounts ... Field Reference](#key-accounts--field-reference)
3. [Enums](#enums)
4. [Helper Structs](#helper-structs)

---

## Account Summary Table

| # | Account | Space (bytes) | Rent (SOL) | Seeds | Closeable |
|---|---------|:---:|:---:|-------|:---------:|
| 1 | `GlobalRegistry` | 137 | ~0.002 | `["sap_global"]` | Yes |
| 2 | `AgentAccount` | 8,192 | ~0.060 | `["sap_agent", wallet]` | Yes |
| 3 | `AgentStats` | 106 | ~0.001 | `["sap_stats", agent]` | Yes |
| 4 | `FeedbackAccount` | 209 | ~0.002 | `["sap_feedback", agent, reviewer]` | Yes |
| 5 | `CapabilityIndex` | 3,386 | ~0.025 | `["sap_cap_idx", hash]` | Yes |
| 6 | `ProtocolIndex` | 3,386 | ~0.025 | `["sap_proto_idx", hash]` | Yes |
| 7 | `PluginSlot` | 124 | ~0.002 | `["sap_plugin", agent, type]` | Yes |
| 8 | `MemoryEntry` | 231 | ~0.003 | `["sap_memory", agent, hash]` | Yes |
| 9 | `MemoryChunk` | 978 | ~0.008 | `["sap_mem_chunk", entry, idx]` | Yes |
| 10 | `MemoryVault` | 178 | ~0.002 | `["sap_vault", agent]` | Yes |
| 11 | `SessionLedger` | 210 | ~0.003 | `["sap_session", vault, hash]` | Yes |
| 12 | `EpochPage` | 103 | ~0.002 | `["sap_epoch", session, idx]` | Yes |
| 13 | `VaultDelegate` | 122 | ~0.002 | `["sap_delegate", vault, delegate]` | Yes |
| 14 | `ToolDescriptor` | 333 | ~0.004 | `["sap_tool", agent, name_hash]` | Yes |
| 15 | `SessionCheckpoint` | 141 | ~0.002 | `["sap_checkpoint", session, idx]` | Yes |
| 16 | `EscrowAccount` | 291 | ~0.004 | `["sap_escrow", agent, depositor]` | Yes |
| 17 | `ToolCategoryIndex` | 3,255 | ~0.024 | `["sap_tool_cat", category]` | Yes |
| 18 | `AgentAttestation` | 198 | ~0.003 | `["sap_attest", agent, attester]` | Yes |
| 19 | `MemoryBuffer` | 101+ | ~0.001+ | `["sap_buffer", session, page_idx]` | Yes |
| 20 | `MemoryDigest` | 230 | ~0.002 | `["sap_digest", session]` | Yes |
| 21 | `MemoryLedger` | 4,269 | ~0.032 | `["sap_ledger", session]` | Yes |
| 22 | `LedgerPage` | 4,193 | ~0.031 | `["sap_page", ledger, page_idx]` | **No (permanent)** |

> **Note**: Accounts 7...9 and 19...20 are gated behind the `legacy-memory` feature flag and are deprecated.
> `LedgerPage` is the only account without a close instruction ... pages are write-once, permanently onchain.

---

## Key Accounts ... Field Reference

### AgentAccount

> Core identity PDA. Contains all agent metadata, reputation fields, and dynamic arrays.  
> Seeds: `["sap_agent", wallet_pubkey]` ... Space: 8,192 bytes ... Rent: ~0.060 SOL

| Field | Type | Description |
|-------|------|-------------|
| `bump` | `u8` | PDA bump seed |
| `version` | `u8` | Account layout version (currently `1`) |
| `wallet` | `Pubkey` | Owner wallet ... all mutations require this signer |
| `name` | `String` (max 64) | Human-readable agent name |
| `description` | `String` (max 256) | Agent description |
| `agent_id` | `Option<String>` (max 128) | Optional DID-style identifier |
| `agent_uri` | `Option<String>` (max 256) | `.well-known/agent.json` URL |
| `x402_endpoint` | `Option<String>` (max 256) | x402 payment endpoint (must start `https://`) |
| `is_active` | `bool` | Active/inactive state for discovery filtering |
| `created_at` | `i64` | Unix timestamp of registration |
| `updated_at` | `i64` | Unix timestamp of last update |
| `reputation_score` | `u32` | 0...10,000 (2 decimal precision, computed onchain) |
| `total_feedbacks` | `u32` | Count of active (non-revoked) feedbacks |
| `reputation_sum` | `u64` | Sum of active feedback scores (incremental calculation) |
| `total_calls_served` | `u64` | **DEPRECATED** ... use `AgentStats.total_calls_served` |
| `avg_latency_ms` | `u32` | Self-reported average response latency |
| `uptime_percent` | `u8` | Self-reported uptime (0...100) |
| `capabilities` | `Vec<Capability>` (max 10) | Agent capabilities in `"domain:action"` format |
| `pricing` | `Vec<PricingTier>` (max 5) | Pricing tiers with token type, rate limits, volume curves |
| `protocols` | `Vec<String>` (max 5×64) | Protocol affiliations |
| `active_plugins` | `Vec<PluginRef>` (max 5) | Active plugin PDAs (legacy) |

**Reputation computation**: `reputation_score = (reputation_sum × 10) / total_feedbacks`. Updated incrementally on `give_feedback`, `update_feedback`, and `revoke_feedback`.

---

### MemoryLedger

> The recommended memory system. Fixed 4 KB ring buffer plus permanent TX log events.  
> Seeds: `["sap_ledger", session_pda]` ... Space: 4,269 bytes ... Rent: ~0.032 SOL

| Field | Type | Description |
|-------|------|-------------|
| `bump` | `u8` | PDA bump seed |
| `session` | `Pubkey` | Parent `SessionLedger` PDA |
| `authority` | `Pubkey` | Wallet authorized to write/close |
| `num_entries` | `u32` | Total writes (ever, including evicted from ring) |
| `merkle_root` | `[u8; 32]` | Rolling hash: `sha256(prev_root \|\| content_hash)` |
| `latest_hash` | `[u8; 32]` | Most recent `content_hash` |
| `total_data_size` | `u64` | Cumulative bytes written (lifetime) |
| `created_at` | `i64` | Unix timestamp |
| `updated_at` | `i64` | Unix timestamp of last write |
| `num_pages` | `u32` | Sealed archive pages (permanent, immutable) |
| `ring` | `Vec<u8>` (max 4,096) | Sliding-window ring buffer |

**Ring buffer format**: Each entry is `[data_len: u16 LE][data: u8 × data_len]`. When a new write doesn't fit, oldest entries are drained from the front until there's room. Evicted entries remain permanently in TX logs.

**Read paths**:
- **Hot path**: `getAccountInfo(ledgerPDA)` → parse `ring` → latest ~10...20 messages → **free**
- **Cold path**: `getSignaturesForAddress(ledgerPDA)` + `getTransaction()` → full history

---

### EscrowAccount

> Pre-funded trustless micropayment channel between a client and an agent.  
> Seeds: `["sap_escrow", agent_pda, depositor_wallet]` ... Space: 291 bytes ... Rent: ~0.004 SOL

| Field | Type | Description |
|-------|------|-------------|
| `bump` | `u8` | PDA bump seed |
| `agent` | `Pubkey` | Provider `AgentAccount` PDA |
| `depositor` | `Pubkey` | Client wallet that funded the escrow |
| `agent_wallet` | `Pubkey` | Agent owner wallet (settlement destination) |
| `balance` | `u64` | Available balance (lamports or smallest token unit) |
| `total_deposited` | `u64` | Lifetime deposits |
| `total_settled` | `u64` | Lifetime settlements (lamports paid out) |
| `total_calls_settled` | `u64` | Lifetime calls settled |
| `price_per_call` | `u64` | Base price per call ... **immutable** after creation |
| `max_calls` | `u64` | Max calls allowed (0 = unlimited) |
| `created_at` | `i64` | Unix timestamp |
| `last_settled_at` | `i64` | Timestamp of last settlement |
| `expires_at` | `i64` | Expiration timestamp (0 = never) |
| `volume_curve` | `Vec<VolumeCurveBreakpoint>` (max 5) | Tiered pricing breakpoints |
| `token_mint` | `Option<Pubkey>` | `None` = SOL, `Some` = SPL token mint |
| `token_decimals` | `u8` | Token decimals (9=SOL, 6=USDC) |

**Settlement flow**: `settle_calls` computes per-call price considering volume curve breakpoints where `total_calls_settled` crosses threshold boundaries. Payment is transferred directly from escrow PDA to `agent_wallet`.

---

### ToolDescriptor

> Onchain tool schema registry entry. Compact metadata plus hashes of full JSON schemas inscribed in TX logs.  
> Seeds: `["sap_tool", agent_pda, tool_name_hash]` ... Space: 333 bytes ... Rent: ~0.004 SOL

| Field | Type | Description |
|-------|------|-------------|
| `bump` | `u8` | PDA bump seed |
| `agent` | `Pubkey` | Parent `AgentAccount` PDA |
| `tool_name_hash` | `[u8; 32]` | `sha256(tool_name)` ... used in PDA seed |
| `tool_name` | `String` (max 32) | Human-readable tool name (e.g., `"getQuote"`) |
| `protocol_hash` | `[u8; 32]` | `sha256(protocol_id)` ... links to `ProtocolIndex` |
| `version` | `u16` | Schema version, bumped on each `update_tool` |
| `description_hash` | `[u8; 32]` | SHA-256 of full tool description |
| `input_schema_hash` | `[u8; 32]` | SHA-256 of input JSON schema |
| `output_schema_hash` | `[u8; 32]` | SHA-256 of output JSON schema |
| `http_method` | `ToolHttpMethod` | GET, POST, PUT, DELETE, or Compound |
| `category` | `ToolCategory` | Swap, Lend, Data, etc. |
| `params_count` | `u8` | Total input parameters |
| `required_params` | `u8` | Required input parameters |
| `is_compound` | `bool` | Whether tool chains multiple HTTP calls |
| `is_active` | `bool` | Can be deactivated without closing |
| `total_invocations` | `u64` | Self-reported call counter |
| `created_at` | `i64` | Unix timestamp |
| `updated_at` | `i64` | Unix timestamp of last update |
| `previous_version` | `Pubkey` | Previous version PDA (`Pubkey::default()` if first) |

**Schema verification**: Retrieve schemas from TX logs via `inscribe_tool_schema` events. Verify `sha256(schema_data) == input_schema_hash` (or `output_schema_hash`, `description_hash`).

---

### SessionLedger

> Compact session index for Memory Vault inscriptions. Tracks counters and merkle state.  
> Seeds: `["sap_session", vault_pda, session_hash]` ... Space: 210 bytes ... Rent: ~0.003 SOL

| Field | Type | Description |
|-------|------|-------------|
| `bump` | `u8` | PDA bump seed |
| `vault` | `Pubkey` | Parent `MemoryVault` PDA |
| `session_hash` | `[u8; 32]` | SHA-256 of a deterministic session identifier |
| `sequence_counter` | `u32` | Next expected sequence number (4B max) |
| `total_bytes` | `u64` | Cumulative encrypted bytes |
| `current_epoch` | `u32` | Current epoch index |
| `total_epochs` | `u32` | Total epoch pages created |
| `created_at` | `i64` | Unix timestamp |
| `last_inscribed_at` | `i64` | Timestamp of last inscription |
| `is_closed` | `bool` | Whether session is closed (no more writes) |
| `merkle_root` | `[u8; 32]` | Rolling: `sha256(prev_root \|\| content_hash)` |
| `total_checkpoints` | `u32` | Number of checkpoints created |
| `tip_hash` | `[u8; 32]` | Last `content_hash` for O(1) change detection |

---

### GlobalRegistry

> Network-wide singleton. Aggregate counters for the entire SAP deployment.  
> Seeds: `["sap_global"]` ... Space: 137 bytes ... Rent: ~0.002 SOL

| Field | Type | Description |
|-------|------|-------------|
| `bump` | `u8` | PDA bump seed |
| `total_agents` | `u64` | Lifetime agent registrations |
| `active_agents` | `u64` | Currently active agents |
| `total_feedbacks` | `u64` | Lifetime feedbacks |
| `total_capabilities` | `u32` | Unique capability indexes created |
| `total_protocols` | `u32` | Unique protocol indexes created |
| `last_registered_at` | `i64` | Timestamp of last agent registration |
| `initialized_at` | `i64` | Protocol initialization timestamp |
| `authority` | `Pubkey` | Protocol authority |
| `total_tools` | `u32` | Total tool descriptors published |
| `total_vaults` | `u32` | Total memory vaults initialized |
| `total_escrows` | `u32` | **DEPRECATED** ... escrow no longer updates this |
| `total_attestations` | `u32` | Total attestations created |

---

## Enums

### TokenType

| Value | Name | Description |
|:-----:|------|-------------|
| 0 | `Sol` | Native SOL (9 decimals) |
| 1 | `Usdc` | USDC stablecoin (6 decimals) |
| 2 | `Spl` | Arbitrary SPL token (requires `token_mint`) |

### SettlementMode

| Value | Name | Description |
|:-----:|------|-------------|
| 0 | `Instant` | Per-call onchain transfer |
| 1 | `Escrow` | Pre-funded escrow PDA, draw per call |
| 2 | `Batched` | Offchain accumulation, periodic onchain settle |
| 3 | `X402` | HTTP x402 protocol (default, recommended) |

### ToolHttpMethod

| Value | Name | Description |
|:-----:|------|-------------|
| 0 | `Get` | HTTP GET |
| 1 | `Post` | HTTP POST |
| 2 | `Put` | HTTP PUT |
| 3 | `Delete` | HTTP DELETE |
| 4 | `Compound` | Chains multiple HTTP calls |

### ToolCategory

| Value | Name | Description |
|:-----:|------|-------------|
| 0 | `Swap` | Token swaps |
| 1 | `Lend` | Lending/borrowing |
| 2 | `Stake` | Staking/validator |
| 3 | `Nft` | NFT mint/trade |
| 4 | `Payment` | Payments/transfers |
| 5 | `Data` | Data queries/feeds |
| 6 | `Governance` | DAO/voting |
| 7 | `Bridge` | Cross-chain |
| 8 | `Analytics` | Onchain analytics |
| 9 | `Custom` | Uncategorized |

### PluginType

> **DEPRECATED** ... Gated behind `legacy-memory` feature flag.

| Value | Name | Description |
|:-----:|------|-------------|
| 0 | `Memory` | Memory layer plugin |
| 1 | `Validation` | Input validation plugin |
| 2 | `Delegation` | Delegation logic plugin |
| 3 | `Analytics` | Analytics data plugin |
| 4 | `Governance` | Governance/voting plugin |
| 5 | `Custom` | Custom extension |

---

## Helper Structs

### Capability

```rust
pub struct Capability {
    pub id: String,                     // max 64 ... e.g. "jupiter:swap"
    pub description: Option<String>,    // max 128
    pub protocol_id: Option<String>,    // max 64
    pub version: Option<String>,        // max 16 ... semver
}
```

### PricingTier

```rust
pub struct PricingTier {
    pub tier_id: String,                          // max 32 ... e.g. "standard"
    pub price_per_call: u64,                      // base price (smallest unit)
    pub min_price_per_call: Option<u64>,           // price floor
    pub max_price_per_call: Option<u64>,           // price ceiling
    pub rate_limit: u32,                           // max calls/sec
    pub max_calls_per_session: u32,                // 0 = unlimited
    pub burst_limit: Option<u32>,                  // max burst/sec
    pub token_type: TokenType,                    // Sol, Usdc, Spl
    pub token_mint: Option<Pubkey>,               // required for Spl
    pub token_decimals: Option<u8>,               // 9=SOL, 6=USDC
    pub settlement_mode: Option<SettlementMode>,  // default = X402
    pub min_escrow_deposit: Option<u64>,           // Escrow mode minimum
    pub batch_interval_sec: Option<u32>,           // Batched mode interval
    pub volume_curve: Option<Vec<VolumeCurveBreakpoint>>, // max 5
}
```

### VolumeCurveBreakpoint

```rust
pub struct VolumeCurveBreakpoint {
    pub after_calls: u32,       // cumulative calls threshold
    pub price_per_call: u64,    // price per call after threshold (smallest unit)
}
```

### Settlement

```rust
pub struct Settlement {
    pub calls_to_settle: u64,       // calls to bill
    pub service_hash: [u8; 32],     // sha256 proof of service
}
```

### PluginRef

```rust
pub struct PluginRef {
    pub plugin_type: PluginType,
    pub pda: Pubkey,
}
```

---

## Account Relationships

```
GlobalRegistry (singleton)
 └── AgentAccount ──┬── AgentStats
                    ├── FeedbackAccount (×N, one per reviewer)
                    ├── CapabilityIndex (×N, shared across agents)
                    ├── ProtocolIndex (×N, shared across agents)
                    ├── AgentAttestation (×N, one per attester)
                    ├── MemoryVault ──┬── SessionLedger ──┬── EpochPage (×N, per 1000 inscriptions)
                    │                │                   ├── SessionCheckpoint (×N)
                    │                │                   ├── MemoryLedger ── LedgerPage (×N, permanent)
                    │                │                   ├── MemoryBuffer (deprecated)
                    │                │                   └── MemoryDigest (deprecated)
                    │                └── VaultDelegate (×N, hot wallets)
                    ├── ToolDescriptor ──── ToolCategoryIndex (shared)
                    ├── EscrowAccount (×N, one per depositor)
                    └── PluginSlot (deprecated)
```

---

[Previous: 02-instructions.md](02-instructions.md) · [Next: 04-events-errors.md](04-events-errors.md)
