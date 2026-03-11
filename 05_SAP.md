# 05 ... SAP: Solana Agent Protocol

> **Program ID**: `SAPTU7aUXk2AaAdktexae1iuxXpokxzNDBAYYhaVyQL`  
> **Anchor Version**: `0.32.1`  
> **Rust Toolchain**: `1.93.0`  
> **Binary Size**: `1.4 MB` (~10.2 SOL deploy cost on mainnet)  
> **Instructions**: `72` | **Accounts**: `22` | **Events**: `45` | **Errors**: `91`  
> **Tests**: `187 passing`

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Constants & Program IDs](#constants--program-ids)
4. [PDA Seed Reference](#pda-seed-reference)
5. [Instruction Reference (72 instructions)](#instruction-reference)
   - [Global Registry](#1-global-registry-1-instruction)
   - [Agent Lifecycle](#2-agent-lifecycle-7-instructions)
   - [Feedback System](#3-feedback-system-4-instructions)
   - [Discovery Indexing](#4-discovery-indexing-12-instructions)
   - [Plugin System](#5-plugin-system-2-instructions)
   - [Legacy Memory](#6-legacy-memory-4-instructions)
   - [Memory Vault](#7-memory-vault-12-instructions)
   - [Tool Registry](#8-tool-registry--checkpoints-9-instructions)
   - [x402 Escrow Settlement](#9-x402-escrow-settlement-6-instructions)
   - [Agent Attestation](#10-agent-attestation-web-of-trust-3-instructions)
   - [Memory Buffer](#11-memory-buffer-3-instructions)
   - [Memory Digest](#12-memory-digest-5-instructions)
   - [Memory Ledger](#13-memory-ledger-recommended-4-instructions)
6. [Account Structs](#account-structs-22-types)
7. [Events Reference](#events-reference-45-events)
8. [Error Codes](#error-codes-91-errors)
9. [Memory Architecture ... Comparison & Recommendations](#memory-architecture)
10. [SDK Constants & Utilities](#sdk-constants--utilities)
11. [Client SDK Methods & Integration Patterns](#client-sdk-methods--integration-patterns)
12. [Cost Analysis](#cost-analysis)
13. [Testnet Deployment Guide](#testnet-deployment-guide)
14. [Security Model](#security-model)
15. [Best Practices](#best-practices)

---

## Overview

SAP (Solana Agent Protocol) is a comprehensive **on-chain identity, memory, reputation, and commerce layer** for AI agents on Solana. Every agent registers a PDA containing its identity, capabilities, tool schemas, pricing tiers, and reputation ... fully verifiable and discoverable without any centralized registry.

### What SAP Provides

| Layer | Purpose | Key Instructions |
|-------|---------|-----------------|
| **Identity** | Agent registration, capabilities, pricing | `register_agent`, `update_agent` |
| **Memory** | 4 memory systems (Ledger recommended) | `write_ledger`, `seal_ledger` |
| **Reputation** | Trustless feedback, attestations | `give_feedback`, `create_attestation` |
| **Commerce** | x402 escrow, micropayments | `create_escrow`, `settle_calls` |
| **Tools** | On-chain tool schemas w/ versioning | `publish_tool`, `inscribe_tool_schema` |
| **Discovery** | Capability/protocol/category indexes | `init_capability_index`, `add_to_*` |

---

## Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                         SAP Program On-Chain                          │
│                                                                        │
│  IDENTITY LAYER                    MEMORY LAYER (4 systems)           │
│  ┌─ GlobalRegistry PDA            ┌─ MemoryLedger [recommended] RECOMMENDED      │
│  ├─ AgentAccount PDA               │  ├─ Ring Buffer (4KB, hot read)  │
│  ├─ PluginSlot PDA(s)              │  ├─ TX Log Events (permanent)    │
│  └─ VaultDelegate PDA(s)           │  └─ LedgerPage PDA(s) (sealed)  │
│                                     ├─ MemoryVault (encrypted)         │
│  REPUTATION LAYER                   │  ├─ SessionLedger PDA            │
│  ├─ FeedbackAccount PDA(s)         │  └─ EpochPage PDA(s)             │
│  └─ AgentAttestation PDA(s)       ├─ MemoryBuffer (realloc PDA)       │
│                                     └─ MemoryDigest (proof-only)       │
│  COMMERCE LAYER                                                        │
│  └─ EscrowAccount PDA(s)          TOOL LAYER                          │
│                                     ├─ ToolDescriptor PDA(s)           │
│  DISCOVERY LAYER                    └─ SessionCheckpoint PDA(s)        │
│  ├─ CapabilityIndex PDA(s)                                             │
│  ├─ ProtocolIndex PDA(s)                                               │
│  └─ ToolCategoryIndex PDA(s)                                           │
└────────────────────────────────────────────────────────────────────────┘
```

### Auth Chain

Most instructions follow this authorization pattern:

```
wallet (Signer)
  └─ agent PDA  ["sap_agent", wallet]        has_one = wallet
       └─ vault PDA  ["sap_vault", agent]     bump verified
            └─ session PDA  ["sap_session", vault, hash]  has_one = vault
                 └─ ledger/buffer/digest PDA   constraint checks
```

---

## Constants & Program IDs

```typescript
// ═══════════════════════════════════════════════════════════
//  SAP Program Constants ... use these in your SDK/client
// ═══════════════════════════════════════════════════════════

/** SAP Program ID (same on devnet, testnet, mainnet after deploy) */
export const SAP_PROGRAM_ID = "SAPTU7aUXk2AaAdktexae1iuxXpokxzNDBAYYhaVyQL";

/** Anchor discriminator size (always 8 bytes) */
export const ANCHOR_DISCRIMINATOR = 8;

// ── PDA Seeds ──
export const SEED_GLOBAL           = "sap_global";
export const SEED_AGENT            = "sap_agent";
export const SEED_FEEDBACK         = "sap_feedback";
export const SEED_CAPABILITY_INDEX = "sap_cap_idx";
export const SEED_PROTOCOL_INDEX   = "sap_proto_idx";
export const SEED_PLUGIN           = "sap_plugin";
export const SEED_MEMORY           = "sap_memory";
export const SEED_MEMORY_CHUNK     = "sap_mem_chunk";
export const SEED_VAULT            = "sap_vault";
export const SEED_SESSION          = "sap_session";
export const SEED_EPOCH            = "sap_epoch";
export const SEED_DELEGATE         = "sap_delegate";
export const SEED_TOOL             = "sap_tool";
export const SEED_CHECKPOINT       = "sap_checkpoint";
export const SEED_ESCROW           = "sap_escrow";
export const SEED_TOOL_CATEGORY    = "sap_tool_cat";
export const SEED_ATTESTATION      = "sap_attest";
export const SEED_STATS            = "sap_stats";
export const SEED_BUFFER           = "sap_buffer";
export const SEED_DIGEST           = "sap_digest";
export const SEED_LEDGER           = "sap_ledger";
export const SEED_PAGE             = "sap_page";

// ── Account Sizes (bytes) ──
export const AGENT_ACCOUNT_SPACE          = 8192;
export const FEEDBACK_ACCOUNT_SPACE       = 209;
export const CAPABILITY_INDEX_SPACE       = 3386;
export const PROTOCOL_INDEX_SPACE         = 3386;
export const GLOBAL_REGISTRY_SPACE        = 137;
export const PLUGIN_SLOT_SPACE            = 124;
export const MEMORY_ENTRY_SPACE           = 231;
export const MEMORY_CHUNK_SPACE           = 978;
export const MEMORY_VAULT_SPACE           = 178;
export const SESSION_LEDGER_SPACE         = 210;
export const EPOCH_PAGE_SPACE             = 103;
export const VAULT_DELEGATE_SPACE         = 122;
export const TOOL_DESCRIPTOR_SPACE        = 333;
export const SESSION_CHECKPOINT_SPACE     = 141;
export const ESCROW_ACCOUNT_SPACE         = 291;
export const TOOL_CATEGORY_INDEX_SPACE    = 3255;
export const AGENT_ATTESTATION_SPACE      = 198;
export const AGENT_STATS_SPACE            = 106;
export const MEMORY_BUFFER_HEADER_SPACE   = 101;  // dynamic realloc
export const MEMORY_DIGEST_SPACE          = 230;
export const MEMORY_LEDGER_SPACE          = 4269; // 173 header + 4096 ring
export const LEDGER_PAGE_SPACE            = 4193; // 97 header + 4096 data

// ── Limits ──
export const MAX_AGENT_NAME_LEN        = 64;
export const MAX_AGENT_DESCRIPTION_LEN = 256;
export const MAX_AGENT_URI_LEN         = 256;
export const MAX_AGENT_ID_LEN          = 128;
export const MAX_CAPABILITIES          = 10;
export const MAX_PRICING_TIERS         = 5;
export const MAX_PROTOCOLS             = 5;
export const MAX_PLUGINS               = 5;
export const MAX_INDEX_ENTRIES         = 100;
export const MAX_INSCRIPTION_SIZE      = 750;  // bytes per write
export const MAX_TOOL_NAME_LEN         = 32;
export const RING_BUFFER_CAPACITY      = 4096; // bytes
export const EPOCH_INTERVAL            = 1000; // inscriptions per epoch

// ── Memory Ledger ──
export const LEDGER_MAX_WRITE_SIZE     = 750;  // max bytes per write_ledger
export const LEDGER_RING_CAPACITY      = 4096; // ring buffer capacity

// ── Enums ──
export enum TokenType       { Sol = 0, Usdc = 1, Spl = 2 }
export enum SettlementMode  { Instant = 0, Escrow = 1, Batched = 2, X402 = 3 }
export enum ToolHttpMethod  { Get = 0, Post = 1, Put = 2, Delete = 3, Compound = 4 }
export enum ToolCategory    {
  Swap = 0, Lend = 1, Stake = 2, Nft = 3, Payment = 4,
  Data = 5, Governance = 6, Bridge = 7, Analytics = 8, Custom = 9,
}
export enum PluginType {
  Memory = 0, Validation = 1, Delegation = 2,
  Analytics = 3, Governance = 4, Custom = 5,
}
```

---

## PDA Seed Reference

All PDAs are derived with `PublicKey.findProgramAddressSync(seeds, SAP_PROGRAM_ID)`.

| PDA | Seeds | Notes |
|-----|-------|-------|
| `GlobalRegistry` | `["sap_global"]` | Singleton, one per program |
| `AgentAccount` | `["sap_agent", wallet.pubkey]` | One per wallet |
| `AgentStats` | `["sap_stats", agent.pubkey]` | One per agent (hot-path metrics) |
| `FeedbackAccount` | `["sap_feedback", agent.pubkey, reviewer.pubkey]` | One per reviewer×agent |
| `CapabilityIndex` | `["sap_cap_idx", capability_hash]` | sha256 of capability name |
| `ProtocolIndex` | `["sap_proto_idx", protocol_hash]` | sha256 of protocol name |
| `PluginSlot` | `["sap_plugin", agent.pubkey, plugin_type_u8]` | One per type per agent |
| `MemoryEntry` | `["sap_memory", agent.pubkey, entry_hash]` | Legacy memory |
| `MemoryChunk` | `["sap_mem_chunk", entry.pubkey, chunk_index_u8]` | Legacy memory chunk |
| `MemoryVault` | `["sap_vault", agent.pubkey]` | One per agent |
| `SessionLedger` | `["sap_session", vault.pubkey, session_hash]` | One per session |
| `EpochPage` | `["sap_epoch", session.pubkey, epoch_index_u32_le]` | Per 1000 inscriptions |
| `VaultDelegate` | `["sap_delegate", vault.pubkey, delegate.pubkey]` | Delegation access |
| `ToolDescriptor` | `["sap_tool", agent.pubkey, tool_name_hash]` | sha256 of tool name |
| `SessionCheckpoint` | `["sap_checkpoint", session.pubkey, checkpoint_index_u32_le]` | State snapshot |
| `EscrowAccount` | `["sap_escrow", agent.pubkey, depositor.pubkey]` | Per depositor×agent |
| `ToolCategoryIndex` | `["sap_tool_cat", category_u8]` | One per category |
| `AgentAttestation` | `["sap_attest", agent.pubkey, attester.pubkey]` | Per attester×agent |
| `MemoryBuffer` | `["sap_buffer", session.pubkey, page_index_u32_le]` | Realloc PDA |
| `MemoryDigest` | `["sap_digest", session.pubkey]` | Proof-of-memory PDA |
| `MemoryLedger` | `["sap_ledger", session.pubkey]` | [recommended] Recommended memory |
| `LedgerPage` | `["sap_page", ledger.pubkey, page_index_u32_le]` | Sealed archive (permanent) |

---

## Instruction Reference

### 1. Global Registry (1 instruction)

| # | Instruction | Description |
|---|------------|-------------|
| 1 | `initialize_global()` | Create singleton GlobalRegistry PDA. Must be called once before any agent registers. Sets the deployer as authority. |

**Accounts**: `wallet` (signer, mut), `global` (init), `system_program`

---

### 2. Agent Lifecycle (7 instructions)

| # | Instruction | Args | Description |
|---|------------|------|-------------|
| 2 | `register_agent` | name, description, capabilities[], pricing[], protocols[], agent_id, agent_uri, x402_endpoint | Register an agent with full identity |
| 3 | `update_agent` | name?, description?, capabilities?, pricing?, protocols?, agent_id?, agent_uri?, x402_endpoint? | Update any agent field (all optional) |
| 4 | `deactivate_agent` | ... | Mark agent inactive (not discoverable) |
| 5 | `reactivate_agent` | ... | Re-activate a deactivated agent |
| 6 | `close_agent` | ... | Close agent PDA, reclaim rent |
| 7 | `report_calls` | calls_served: u64 | Self-report call count |
| 8 | `update_reputation` | avg_latency_ms: u32, uptime_percent: u8 | Self-report performance metrics |

**Auth chain**: `wallet → agent PDA → global PDA`

**Validation** (deep validation engine in `validator.rs`):
- Name: 1-64 chars, no control characters
- Description: 1-256 chars
- Capabilities: max 10, format `domain:action` valid
- Pricing: max 5 tiers, no duplicate IDs, valid volume curves
- URI: max 256 chars
- x402 endpoint: must start with `https://`
- Uptime: 0-100%

```typescript
// SDK Example: Register an agent
const agentPda = deriveAgentPDA(wallet.publicKey);
const globalPda = deriveGlobalPDA();

await program.methods.registerAgent(
  "MyAgent",                           // name (max 64)
  "AI agent for DeFi analytics",       // description (max 256)
  [{ domain: "defi", action: "analyze", version: "1.0", description: "DeFi protocol analysis" }],
  [{                                    // pricing tier
    tierId: "standard",
    basePrice: new BN(1000),           // lamports per call
    tokenType: { sol: {} },
    tokenMint: null,
    minPrice: new BN(500),
    maxPrice: new BN(5000),
    rateLimit: 100,
    rateLimitWindow: 60,
    settlementMode: { instant: {} },
    volumeCurve: [{ threshold: new BN(100), priceFactor: 80 }],
    isActive: true,
    validFrom: null,
    validUntil: null,
    metadata: null,
  }],
  ["A2A", "MCP"],                      // protocols
  "agent-001",                          // agent_id
  "https://myagent.ai",                // agent_uri
  "https://myagent.ai/.well-known/x402", // x402 endpoint
)
.accountsPartial({
  wallet: wallet.publicKey,
  agent: agentPda,
  global: globalPda,
  systemProgram: SystemProgram.programId,
})
.signers([wallet])
.rpc();
```

---

### 3. Feedback System (4 instructions)

| # | Instruction | Args | Description |
|---|------------|------|-------------|
| 9 | `give_feedback` | score: u16 (0-1000), tag: String, comment_hash?: [u8;32] | Leave feedback on an agent |
| 10 | `update_feedback` | new_score: u16, new_tag?: String, comment_hash?: [u8;32] | Update existing feedback |
| 11 | `revoke_feedback` | ... | Mark feedback as revoked |
| 12 | `close_feedback` | ... | Close feedback PDA (must be revoked first) |

**Auth**: Reviewer signs. Self-review not allowed.  
**Score range**: 0-1000 (multiply by 0.1 for display: 0.0 ... 100.0)

---

### 4. Discovery Indexing (12 instructions)

Three index types, each with init/add/remove/close:

| Index Type | Seed | Max Entries | Purpose |
|-----------|------|-------------|---------|
| `CapabilityIndex` | `sap_cap_idx` | 100 | Find agents by capability (e.g., "defi:swap") |
| `ProtocolIndex` | `sap_proto_idx` | 100 | Find agents by protocol (e.g., "MCP", "A2A") |
| `ToolCategoryIndex` | `sap_tool_cat` | 100 | Find tools by category (Swap, Lending, etc.) |

| # | Instruction | Description |
|---|------------|-------------|
| 13 | `init_capability_index(capability_id, capability_hash)` | Create index for a capability |
| 14 | `add_to_capability_index(capability_hash)` | Add agent to index |
| 15 | `remove_from_capability_index(capability_hash)` | Remove agent from index |
| 16 | `init_protocol_index(protocol_id, protocol_hash)` | Create index for a protocol |
| 17 | `add_to_protocol_index(protocol_hash)` | Add agent to index |
| 18 | `remove_from_protocol_index(protocol_hash)` | Remove agent from index |
| 19 | `close_capability_index(capability_hash)` | Close (must be empty) |
| 20 | `close_protocol_index(protocol_hash)` | Close (must be empty) |
| 21 | `init_tool_category_index(category: u8)` | Create category index |
| 22 | `add_to_tool_category(category: u8)` | Add tool to category |
| 23 | `remove_from_tool_category(category: u8)` | Remove tool from category |
| 24 | `close_tool_category_index(category: u8)` | Close (must be empty) |

---

### 5. Plugin System (2 instructions)

| # | Instruction | Args | Description |
|---|------------|------|-------------|
| 25 | `register_plugin` | plugin_type: u8 | Register a plugin slot (validator, logger, etc.) |
| 26 | `close_plugin` | ... | Close plugin PDA, reclaim rent |

**Plugin types**: `Memory(0)`, `Validation(1)`, `Delegation(2)`, `Analytics(3)`, `Governance(4)`, `Custom(5)`

---

### 6. Legacy Memory (4 instructions)

> ⚠️ **Deprecated** ... Use MemoryLedger instead. Kept for backward compatibility.

| # | Instruction | Description |
|---|------------|-------------|
| 27 | `store_memory` | Create a MemoryEntry PDA with content hash |
| 28 | `append_memory_chunk` | Store data chunk (up to 900 bytes) |
| 29 | `close_memory_entry` | Close entry PDA |
| 30 | `close_memory_chunk` | Close chunk PDA |

**Cost**: ~6.96 SOL per MB. Very expensive for large data.

---

### 7. Memory Vault (12 instructions)

Encrypted conversation memory with TX log inscriptions, epoch-based indexing, and delegation.

| # | Instruction | Args | Description |
|---|------------|------|-------------|
| 31 | `init_vault` | vault_nonce: [u8;32] | Create MemoryVault with PBKDF2 nonce |
| 32 | `open_session` | session_hash: [u8;32] | Open a named conversation session |
| 33 | `inscribe_memory` | sequence, encrypted_data, nonce, content_hash, total_fragments, fragment_index, compression, epoch_index | Inscribe encrypted data to TX log |
| 34 | `close_session` | ... | Mark session closed (no more writes) |
| 35 | `close_vault` | ... | Close vault (all sessions must be closed first) |
| 36 | `close_session_pda` | ... | Reclaim session PDA rent (must be closed first) |
| 37 | `close_epoch_page` | epoch_index: u32 | Close an epoch page PDA |
| 38 | `rotate_vault_nonce` | new_nonce: [u8;32] | Rotate encryption nonce |
| 39 | `add_vault_delegate` | permissions: u8, expires_at: i64 | Grant write access to another wallet |
| 40 | `revoke_vault_delegate` | ... | Revoke delegate access |
| 41 | `inscribe_memory_delegated` | (same as inscribe_memory) | Delegate writes to vault |
| 42 | `compact_inscribe` | sequence, encrypted_data, nonce, content_hash | Simplified inscription (single fragment, no compression) |

**Client-side encryption**: AES-256-GCM with PBKDF2 key derivation from password + vault_nonce.  
**Compression**: `deflate(1)`, `gzip(2)`, `brotli(3)` applied BEFORE encryption.

---

### 8. Tool Registry + Checkpoints (9 instructions)

On-chain tool schema registry with versioning, usage tracking, and session checkpoints.

| # | Instruction | Args | Description |
|---|------------|------|-------------|
| 43 | `publish_tool` | tool_name, name_hash, protocol_hash, description_hash, input/output_schema_hash, http_method, category, params_count, required_params, is_compound | Publish a tool definition |
| 44 | `inscribe_tool_schema` | schema_type: u8, schema_data, schema_hash, compression: u8 | Inscribe full schema to TX log |
| 45 | `update_tool` | description_hash?, input_schema_hash?, output_schema_hash?, http_method?, category?, params_count?, required_params? | Update tool (bumps version) |
| 46 | `deactivate_tool` | ... | Deactivate tool |
| 47 | `reactivate_tool` | ... | Re-activate tool |
| 48 | `close_tool` | ... | Close tool PDA, reclaim rent |
| 49 | `report_tool_invocations` | invocations: u64 | Self-report usage count |
| 50 | `create_session_checkpoint` | checkpoint_index: u32 | Snapshot session state at a point in time |
| 51 | `close_checkpoint` | checkpoint_index: u32 | Close checkpoint PDA |

---

### 9. x402 Escrow Settlement (6 instructions)

Pre-funded escrow for micropayments between agents. Supports x402 payment protocol.

| # | Instruction | Args | Description |
|---|------------|------|-------------|
| 52 | `create_escrow` | price_per_call, max_calls, initial_deposit, expires_at, volume_curve, token_mint?, token_decimals | Create escrow with deposit |
| 53 | `deposit_escrow` | amount: u64 | Add funds to escrow |
| 54 | `settle_calls` | calls_to_settle: u64, service_hash: [u8;32] | Agent claims payment for served calls |
| 55 | `withdraw_escrow` | amount: u64 | Depositor withdraws unused funds |
| 56 | `close_escrow` | ... | Close escrow (must be empty) |
| 57 | `settle_batch` | settlements: Vec\<Settlement\> | Batch settlement ... up to 10 settlements in one TX |

**Flow**: Depositor creates escrow → Agent serves calls → Agent calls `settle_calls` → SOL transferred from escrow → Depositor withdraws remainder → Close.

---

### 10. Agent Attestation / Web of Trust (3 instructions)

Agents attest to each other's capabilities, creating an on-chain web of trust.

| # | Instruction | Args | Description |
|---|------------|------|-------------|
| 58 | `create_attestation` | attestation_type: String, metadata_hash: [u8;32], expires_at: i64 | Attest to another agent |
| 59 | `revoke_attestation` | ... | Revoke an attestation |
| 60 | `close_attestation` | ... | Close attestation PDA (must be revoked first) |

**Constraint**: Self-attestation not allowed. Expired attestations are rejected.

---

### 11. Memory Buffer (3 instructions)

> ⚠️ **Consider MemoryLedger instead** ... Buffer grows via realloc and costs ~50× more than Ledger for large data.

| # | Instruction | Args | Description |
|---|------------|------|-------------|
| 61 | `create_buffer` | page_index: u32 | Create MemoryBuffer PDA (starts small) |
| 62 | `append_buffer` | page_index: u32, data: Vec\<u8\> | Append data (reallocs up to ~10KB) |
| 63 | `close_buffer` | page_index: u32 | Close buffer PDA, reclaim rent |

**Max size per page**: ~10KB. Data directly readable via `getAccountInfo()`.

---

### 12. Memory Digest (5 instructions)

Proof-of-memory system. Fixed-size PDA (~230 bytes) that NEVER grows. Stores merkle root + counters only.

| # | Instruction | Args | Description |
|---|------------|------|-------------|
| 64 | `init_digest` | ... | Create fixed-size digest PDA (~0.002 SOL) |
| 65 | `post_digest` | content_hash: [u8;32], data_size: u32 | Record hash-only proof (no data) |
| 66 | `inscribe_to_digest` | data: Vec\<u8\>, content_hash: [u8;32] | Inscribe data to TX log + update proof |
| 67 | `update_digest_storage` | storage_ref: [u8;32], storage_type: u8 | Set off-chain storage pointer |
| 68 | `close_digest` | ... | Close digest PDA, reclaim rent |

---

### 13. Memory Ledger ... [recommended] RECOMMENDED (4 instructions)

The **unified on-chain memory system**. Combines the best of every approach into one fixed-cost PDA with a 4KB ring buffer and permanent sealed archive pages.

| # | Instruction | Args | Description |
|---|------------|------|-------------|
| 69 | `init_ledger` | ... | Create ledger PDA with 4KB ring buffer (~0.032 SOL) |
| 70 | `write_ledger` | data: Vec\<u8\>, content_hash: [u8;32] | Write to ring + TX log + update merkle (TX fee only) |
| 71 | `seal_ledger` | ... | Freeze ring contents into permanent LedgerPage (~0.031 SOL) |
| 72 | `close_ledger` | ... | Close ledger PDA, reclaim ~0.032 SOL. Sealed pages survive. |

#### Three Tiers of Durability

| Tier | Storage | Readability | Persistence | Cost |
|------|---------|-------------|-------------|------|
| **HOT** | Ring buffer in PDA | `getAccountInfo()` → instant, FREE | Evicted when full | Fixed ~0.032 SOL |
| **PERMANENT** | Sealed LedgerPages | `getAccountInfo()` → instant, FREE | **FOREVER** (no close exists) | ~0.031 SOL/page |
| **LOG** | TX log events | `getSignaturesForAddress()` | On-chain, complementary | TX fee only |

#### Ring Buffer Wire Format

```
Entry: [data_len: u16 LE][data: u8 × data_len]
       └── 2 bytes ────┘└── variable ────────┘

Ring capacity: 4096 bytes → fits ~5-38 entries depending on size.
When full: oldest entries drained from front. Evicted entries remain in TX logs.
```

#### Complete Workflow

```typescript
// 1. Init ledger (~0.032 SOL, reclaimable)
await program.methods.initLedger()
  .accountsPartial({ wallet, agent, vault, session, ledger, systemProgram })
  .rpc();

// 2. Write entries (TX fee only ~0.000005 SOL each)
const data = Buffer.from(JSON.stringify({ role: "user", content: "Hello" }));
const contentHash = crypto.createHash("sha256").update(data).digest();
await program.methods.writeLedger(data, Array.from(contentHash))
  .accountsPartial({ wallet, agent, vault, session, ledger })
  .rpc();

// 3. Seal ring → permanent page that NOBODY can delete (~0.031 SOL)
await program.methods.sealLedger()
  .accountsPartial({ wallet, agent, vault, session, ledger, page, systemProgram })
  .rpc();

// 4. Read HOT path ... latest entries from ring buffer (FREE)
const lg = await program.account.memoryLedger.fetch(ledgerPda);
const entries = parseRingBuffer(lg.ring);

// 5. Read COLD path ... all entries from TX logs (FREE)
const sigs = await connection.getSignaturesForAddress(ledgerPda);
// Parse LedgerEntryEvent from each transaction

// 6. Read SEALED pages ... permanent archive (FREE)
const page = await program.account.ledgerPage.fetch(pagePda);
const archivedEntries = parseRingBuffer(page.data);

// 7. Close ledger, reclaim rent. Sealed pages survive FOREVER.
await program.methods.closeLedger()
  .accountsPartial({ wallet, agent, vault, session, ledger })
  .rpc();
```

---

## Account Structs (22 types)

| # | Account | Space (bytes) | Rent (SOL) | Seeds | Closeable |
|---|---------|---------------|-----------|-------|-----------|
| 1 | `GlobalRegistry` | 137 | ~0.002 | `["sap_global"]` | ✅ |
| 2 | `AgentAccount` | 8,192 | ~0.060 | `["sap_agent", wallet]` | ✅ |
| 3 | `AgentStats` | 106 | ~0.001 | `["sap_stats", agent]` | ✅ |
| 4 | `FeedbackAccount` | 209 | ~0.002 | `["sap_feedback", agent, reviewer]` | ✅ |
| 5 | `CapabilityIndex` | 3,386 | ~0.025 | `["sap_cap_idx", hash]` | ✅ |
| 6 | `ProtocolIndex` | 3,386 | ~0.025 | `["sap_proto_idx", hash]` | ✅ |
| 7 | `PluginSlot` | 124 | ~0.002 | `["sap_plugin", agent, type]` | ✅ |
| 8 | `MemoryEntry` | 231 | ~0.003 | `["sap_memory", agent, hash]` | ✅ |
| 9 | `MemoryChunk` | 978 | ~0.008 | `["sap_mem_chunk", entry, idx]` | ✅ |
| 10 | `MemoryVault` | 178 | ~0.002 | `["sap_vault", agent]` | ✅ |
| 11 | `SessionLedger` | 210 | ~0.003 | `["sap_session", vault, hash]` | ✅ |
| 12 | `EpochPage` | 103 | ~0.002 | `["sap_epoch", session, idx]` | ✅ |
| 13 | `VaultDelegate` | 122 | ~0.002 | `["sap_delegate", vault, delegate]` | ✅ |
| 14 | `ToolDescriptor` | 333 | ~0.004 | `["sap_tool", agent, name_hash]` | ✅ |
| 15 | `SessionCheckpoint` | 141 | ~0.002 | `["sap_checkpoint", session, idx]` | ✅ |
| 16 | `EscrowAccount` | 291 | ~0.004 | `["sap_escrow", agent, depositor]` | ✅ |
| 17 | `ToolCategoryIndex` | 3,255 | ~0.024 | `["sap_tool_cat", category]` | ✅ |
| 18 | `AgentAttestation` | 198 | ~0.003 | `["sap_attest", agent, attester]` | ✅ |
| 19 | `MemoryBuffer` | 101+ | ~0.001+ | `["sap_buffer", session, page_idx]` | ✅ |
| 20 | `MemoryDigest` | 230 | ~0.002 | `["sap_digest", session]` | ✅ |
| 21 | `MemoryLedger` | 4,269 | ~0.032 | `["sap_ledger", session]` | ✅ |
| 22 | `LedgerPage` | 4,193 | ~0.031 | `["sap_page", ledger, page_idx]` | ❌ **Permanent** |

---

## Events Reference (45 events)

### Agent Events
| Event | Fields | Emitted By |
|-------|--------|-----------|
| `RegisteredEvent` | agent, wallet, name, capabilities, timestamp | `register_agent` |
| `UpdatedEvent` | agent, wallet, updated_fields, timestamp | `update_agent` |
| `DeactivatedEvent` | agent, wallet, timestamp | `deactivate_agent` |
| `ReactivatedEvent` | agent, wallet, timestamp | `reactivate_agent` |
| `ClosedEvent` | agent, wallet, timestamp | `close_agent` |
| `CallsReportedEvent` | agent, wallet, calls_reported, total_calls_served, timestamp | `report_calls` |
| `ReputationUpdatedEvent` | agent, wallet, avg_latency_ms, uptime_percent, timestamp | `update_reputation` |

### Feedback Events
| Event | Emitted By |
|-------|-----------|
| `FeedbackEvent` | `give_feedback` |
| `FeedbackUpdatedEvent` | `update_feedback` |
| `FeedbackRevokedEvent` | `revoke_feedback` |

### Vault Events
| Event | Emitted By |
|-------|-----------|
| `VaultInitializedEvent` | `init_vault` |
| `SessionOpenedEvent` | `open_session` |
| `MemoryInscribedEvent` | `inscribe_memory`, `inscribe_memory_delegated`, `compact_inscribe` |
| `EpochOpenedEvent` | `inscribe_memory` (auto when crossing epoch boundary) |
| `SessionClosedEvent` | `close_session` |
| `VaultClosedEvent` | `close_vault` |
| `SessionPdaClosedEvent` | `close_session_pda` |
| `EpochPageClosedEvent` | `close_epoch_page` |
| `VaultNonceRotatedEvent` | `rotate_vault_nonce` |
| `DelegateAddedEvent` | `add_vault_delegate` |
| `DelegateRevokedEvent` | `revoke_vault_delegate` |

### Tool Events
| Event | Emitted By |
|-------|-----------|
| `ToolPublishedEvent` | `publish_tool` |
| `ToolSchemaInscribedEvent` | `inscribe_tool_schema` |
| `ToolUpdatedEvent` | `update_tool` |
| `ToolDeactivatedEvent` | `deactivate_tool` |
| `ToolReactivatedEvent` | `reactivate_tool` |
| `ToolClosedEvent` | `close_tool` |
| `ToolInvocationReportedEvent` | `report_tool_invocations` |
| `CheckpointCreatedEvent` | `create_session_checkpoint` |

### Escrow Events
| Event | Emitted By |
|-------|-----------|
| `EscrowCreatedEvent` | `create_escrow` |
| `EscrowDepositedEvent` | `deposit_escrow` |
| `PaymentSettledEvent` | `settle_calls` |
| `EscrowWithdrawnEvent` | `withdraw_escrow` |
| `BatchSettledEvent` | `settle_batch` |

### Attestation Events
| Event | Emitted By |
|-------|-----------|
| `AttestationCreatedEvent` | `create_attestation` |
| `AttestationRevokedEvent` | `revoke_attestation` |

### Memory Events
| Event | Emitted By |
|-------|-----------|
| `MemoryStoredEvent` | `store_memory` |
| `BufferCreatedEvent` | `create_buffer` |
| `BufferAppendedEvent` | `append_buffer` |
| `DigestPostedEvent` | `post_digest` |
| `DigestInscribedEvent` | `inscribe_to_digest` |
| `StorageRefUpdatedEvent` | `update_digest_storage` |
| `LedgerEntryEvent` | `write_ledger` |
| `LedgerSealedEvent` | `seal_ledger` |

---

## Error Codes (91 errors)

<details>
<summary>Click to expand full error list</summary>

### Agent Validation
| Error | Message |
|-------|---------|
| `NameTooLong` | Agent name exceeds max length |
| `DescriptionTooLong` | Agent description exceeds max length |
| `UriTooLong` | Agent URI exceeds max length |
| `TooManyCapabilities` | Too many capabilities (max 10) |
| `TooManyPricingTiers` | Too many pricing tiers (max 5) |
| `TooManyProtocols` | Too many protocols (max 5) |
| `TooManyPlugins` | Too many plugins (max 5) |
| `AlreadyActive` | Agent is already active |
| `AlreadyInactive` | Agent is already inactive |
| `AgentInactive` | Agent must be active for this operation |

### Deep Validation
| Error | Message |
|-------|---------|
| `EmptyName` | Name cannot be empty |
| `ControlCharInName` | Name contains control characters |
| `EmptyDescription` | Description cannot be empty |
| `AgentIdTooLong` | Agent ID too long |
| `InvalidCapabilityFormat` | Capability must be "domain:action" |
| `DuplicateCapability` | Duplicate capability |
| `EmptyTierId` | Pricing tier ID is empty |
| `DuplicateTierId` | Duplicate tier ID |
| `InvalidRateLimit` | Rate limit must be > 0 |
| `SplRequiresTokenMint` | SPL token requires token_mint |
| `InvalidX402Endpoint` | Must start with https:// |
| `InvalidVolumeCurve` | Volume curve thresholds must increase |
| `TooManyVolumeCurvePoints` | Max 5 points |
| `MinPriceExceedsMax` | min_price > max_price |
| `InvalidUptimePercent` | Must be 0-100 |

### Feedback
| Error | Message |
|-------|---------|
| `InvalidFeedbackScore` | Score must be 0-1000 |
| `TagTooLong` | Tag too long |
| `SelfReviewNotAllowed` | Cannot review yourself |
| `FeedbackAlreadyRevoked` | Already revoked |
| `FeedbackNotRevoked` | Must revoke before close |

### Indexing
| Error | Message |
|-------|---------|
| `CapabilityIndexFull` | Index at 100 capacity |
| `ProtocolIndexFull` | Index at 100 capacity |
| `AgentNotInIndex` | Agent not found in index |
| `InvalidCapabilityHash` | Hash mismatch |
| `InvalidProtocolHash` | Hash mismatch |

### Vault
| Error | Message |
|-------|---------|
| `SessionClosed` | Session is closed |
| `InvalidSequence` | Wrong sequence number |
| `InvalidFragmentIndex` | Fragment index out of range |
| `InscriptionTooLarge` | Data > 750 bytes |
| `EmptyInscription` | Data cannot be empty |
| `InvalidTotalFragments` | Fragment count invalid |
| `EpochMismatch` | Wrong epoch index |
| `VaultNotClosed` | Vault must be closed first |
| `SessionNotClosed` | Session must be closed first |
| `SessionStillOpen` | Session is still open |

### Delegation
| Error | Message |
|-------|---------|
| `DelegateExpired` | Delegate access expired |
| `InvalidDelegate` | Invalid delegate |

### Tools
| Error | Message |
|-------|---------|
| `ToolNameTooLong` | Tool name > 32 chars |
| `EmptyToolName` | Tool name empty |
| `InvalidToolNameHash` | Hash mismatch |
| `InvalidToolHttpMethod` | Invalid HTTP method |
| `InvalidToolCategory` | Invalid category |
| `ToolAlreadyInactive` | Already inactive |
| `ToolAlreadyActive` | Already active |
| `InvalidSchemaHash` | Schema hash mismatch |
| `InvalidSchemaType` | Invalid schema type |
| `InvalidCheckpointIndex` | Wrong checkpoint index |

### Escrow
| Error | Message |
|-------|---------|
| `InsufficientEscrowBalance` | Not enough funds |
| `EscrowMaxCallsExceeded` | Max calls reached |
| `EscrowEmpty` | Escrow is empty |
| `EscrowNotEmpty` | Escrow has funds (can't close) |
| `InvalidSettlementCalls` | Invalid call count |
| `EscrowExpired` | Escrow has expired |

### Attestation
| Error | Message |
|-------|---------|
| `AttestationTypeTooLong` | Type string too long |
| `EmptyAttestationType` | Type cannot be empty |
| `SelfAttestationNotAllowed` | Cannot attest yourself |
| `AttestationAlreadyRevoked` | Already revoked |
| `AttestationNotRevoked` | Must revoke before close |
| `AttestationExpired` | Attestation has expired |

### Memory Systems
| Error | Message |
|-------|---------|
| `ChunkDataTooLarge` | Chunk > 900 bytes |
| `ContentTypeTooLong` | Content type too long |
| `IpfsCidTooLong` | IPFS CID too long |
| `BufferFull` | Buffer at max capacity |
| `BufferDataTooLarge` | Buffer write too large |
| `Unauthorized` | Not the authority |
| `InvalidSession` | Wrong session |
| `EmptyDigestHash` | Content hash is all zeros |
| `LedgerDataTooLarge` | Write > 750 bytes |
| `LedgerRingEmpty` | Ring empty, nothing to seal |

### Tool Category Index
| Error | Message |
|-------|---------|
| `ToolCategoryIndexFull` | Category index at 100 capacity |
| `ToolNotInCategoryIndex` | Tool not found in category index |
| `ToolCategoryMismatch` | Tool category doesn't match index |

### Batch Settlement
| Error | Message |
|-------|---------|
| `BatchEmpty` | Batch is empty |
| `BatchTooLarge` | Batch exceeds max 10 settlements |

### SPL Token Escrow
| Error | Message |
|-------|---------|
| `SplTokenRequired` | SPL token accounts required |
| `InvalidTokenAccount` | Invalid token account |
| `InvalidTokenProgram` | Invalid token program |

### Safety
| Error | Message |
|-------|---------|
| `ArithmeticOverflow` | Numeric overflow |
| `NoFieldsToUpdate` | No fields specified |
| `IndexNotEmpty` | Index not empty (can't close) |
| `InvalidPluginType` | Invalid plugin type |

</details>

---

## Memory Architecture

### Comparison Matrix

| | MemoryLedger [recommended] | MemoryVault | MemoryBuffer | MemoryDigest |
|---|:---:|:---:|:---:|:---:|
| **Init cost** | ~0.032 SOL | ~0.002 SOL | ~0.001 SOL | ~0.002 SOL |
| **Per-write cost** | TX fee only | TX fee only | TX fee + realloc rent | TX fee only |
| **10K writes (200B)** | ~0.082 SOL | ~0.052 SOL | impossible (10KB limit) | ~0.052 SOL |
| **Instant readability** | ✅ ring buffer | ❌ TX log parse | ✅ getAccountInfo | ❌ TX log parse |
| **Permanent storage** | ✅ sealed pages | ✅ TX logs | ❌ closeable | ✅ TX logs |
| **Encryption** | ❌ | ✅ AES-256-GCM | ❌ | ❌ |
| **Merkle proof** | ✅ | ✅ | ❌ | ✅ |
| **Fixed cost PDA** | ✅ (never grows) | ✅ | ❌ (grows via realloc) | ✅ |
| **Protocol-level immutability** | ✅ sealed pages | ❌ | ❌ | ❌ |
| **Delegation** | ❌ | ✅ | ❌ | ❌ |
| **Max data per write** | 750 bytes | 750 bytes | ~10KB total | 750 bytes |

### Recommendation

| Use Case | System |
|----------|--------|
| **General agent memory** | [recommended] MemoryLedger |
| **Sensitive/encrypted conversations** | MemoryVault |
| **Small config/state cache** | MemoryBuffer |
| **Proof of computation (hash-only)** | MemoryDigest |
| **Long-term permanent archive** | MemoryLedger + `seal_ledger()` |

---

## SDK Constants & Utilities

### PDA Derivation Helpers

```typescript
import { PublicKey } from "@solana/web3.js";
import * as crypto from "crypto";

const PROGRAM_ID = new PublicKey("SAPTU7aUXk2AaAdktexae1iuxXpokxzNDBAYYhaVyQL");

/** Derive agent PDA from wallet public key */
function deriveAgentPDA(wallet: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_agent"), wallet.toBuffer()],
    PROGRAM_ID,
  );
}

/** Derive vault PDA from agent PDA */
function deriveVaultPDA(agent: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_vault"), agent.toBuffer()],
    PROGRAM_ID,
  );
}

/** Derive session PDA from vault + session ID string */
function deriveSessionPDA(vault: PublicKey, sessionId: string): [PublicKey, number] {
  const hash = crypto.createHash("sha256").update(sessionId).digest();
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_session"), vault.toBuffer(), hash],
    PROGRAM_ID,
  );
}

/** Derive ledger PDA from session PDA */
function deriveLedgerPDA(session: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_ledger"), session.toBuffer()],
    PROGRAM_ID,
  );
}

/** Derive sealed page PDA from ledger + page index */
function deriveLedgerPagePDA(ledger: PublicKey, pageIndex: number): [PublicKey, number] {
  const buf = Buffer.alloc(4);
  buf.writeUInt32LE(pageIndex);
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_page"), ledger.toBuffer(), buf],
    PROGRAM_ID,
  );
}

/** Derive global registry PDA */
function deriveGlobalPDA(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_global")],
    PROGRAM_ID,
  );
}

/** Derive feedback PDA */
function deriveFeedbackPDA(agent: PublicKey, reviewer: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_feedback"), agent.toBuffer(), reviewer.toBuffer()],
    PROGRAM_ID,
  );
}

/** Derive escrow PDA */
function deriveEscrowPDA(agent: PublicKey, depositor: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_escrow"), agent.toBuffer(), depositor.toBuffer()],
    PROGRAM_ID,
  );
}

/** Derive tool PDA from agent + tool name */
function deriveToolPDA(agent: PublicKey, toolName: string): [PublicKey, number] {
  const hash = crypto.createHash("sha256").update(toolName).digest();
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_tool"), agent.toBuffer(), hash],
    PROGRAM_ID,
  );
}

/** Derive attestation PDA */
function deriveAttestationPDA(agent: PublicKey, attester: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_attest"), agent.toBuffer(), attester.toBuffer()],
    PROGRAM_ID,
  );
}

/** Derive digest PDA from session */
function deriveDigestPDA(session: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_digest"), session.toBuffer()],
    PROGRAM_ID,
  );
}

/** Derive buffer PDA from session + page index */
function deriveBufferPDA(session: PublicKey, pageIndex: number): [PublicKey, number] {
  const buf = Buffer.alloc(4);
  buf.writeUInt32LE(pageIndex);
  return PublicKey.findProgramAddressSync(
    [Buffer.from("sap_buffer"), session.toBuffer(), buf],
    PROGRAM_ID,
  );
}
```

### Ring Buffer Parser

```typescript
/**
 * Parse a MemoryLedger ring buffer into individual entries.
 * Wire format: [data_len: u16 LE][data: u8 × data_len] repeated.
 */
function parseRingBuffer(ring: Buffer | number[]): Buffer[] {
  const buf = Buffer.from(ring);
  const entries: Buffer[] = [];
  let pos = 0;
  while (pos + 2 <= buf.length) {
    const len = buf.readUInt16LE(pos);
    if (len === 0 || pos + 2 + len > buf.length) break;
    entries.push(buf.subarray(pos + 2, pos + 2 + len));
    pos += 2 + len;
  }
  return entries;
}
```

### Merkle Root Replay

```typescript
/**
 * Replay the merkle chain from a list of content hashes.
 * Used to verify the on-chain merkle root matches.
 */
function replayMerkleRoot(contentHashes: Buffer[]): Buffer {
  let root = Buffer.alloc(32);
  for (const hash of contentHashes) {
    const hasher = crypto.createHash("sha256");
    hasher.update(root);
    hasher.update(hash);
    root = hasher.digest();
  }
  return root;
}
```

### Content Hash Helper

```typescript
/**
 * Compute SHA-256 content hash of data.
 * Pass this as `content_hash` arg to write_ledger / inscribe_to_digest.
 */
function computeContentHash(data: Buffer | string): number[] {
  const hash = crypto.createHash("sha256")
    .update(typeof data === "string" ? Buffer.from(data) : data)
    .digest();
  return Array.from(hash);
}
```

---

## Client SDK Methods & Integration Patterns

### Full Agent Lifecycle

```typescript
import { Program, AnchorProvider } from "@coral-xyz/anchor";
import { SynapseAgentSap } from "./target/types/synapse_agent_sap";
import { PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";

const program = anchor.workspace.synapseAgentSap as Program<SynapseAgentSap>;
const wallet = provider.wallet;

// ── Step 1: Initialize global (once, by deployer) ──
const [globalPda] = deriveGlobalPDA();
await program.methods.initializeGlobal()
  .accountsPartial({ wallet: wallet.publicKey, global: globalPda, systemProgram: SystemProgram.programId })
  .rpc();

// ── Step 2: Register agent ──
const [agentPda] = deriveAgentPDA(wallet.publicKey);
await program.methods.registerAgent("MyAgent", "Description", capabilities, pricing, protocols, "id", "uri", "x402")
  .accountsPartial({ wallet: wallet.publicKey, agent: agentPda, global: globalPda, systemProgram: SystemProgram.programId })
  .rpc();

// ── Step 3: Init vault ──
const [vaultPda] = deriveVaultPDA(agentPda);
const vaultNonce = crypto.randomBytes(32);
await program.methods.initVault(Array.from(vaultNonce))
  .accountsPartial({ wallet: wallet.publicKey, agent: agentPda, vault: vaultPda, systemProgram: SystemProgram.programId })
  .rpc();

// ── Step 4: Open session ──
const sessionId = "task:my-conversation:001";
const sessionHash = crypto.createHash("sha256").update(sessionId).digest();
const [sessionPda] = deriveSessionPDA(vaultPda, sessionId);
await program.methods.openSession(Array.from(sessionHash))
  .accountsPartial({ wallet: wallet.publicKey, agent: agentPda, vault: vaultPda, session: sessionPda, systemProgram: SystemProgram.programId })
  .rpc();

// ── Step 5: Init ledger ──
const [ledgerPda] = deriveLedgerPDA(sessionPda);
await program.methods.initLedger()
  .accountsPartial({ wallet: wallet.publicKey, agent: agentPda, vault: vaultPda, session: sessionPda, ledger: ledgerPda, systemProgram: SystemProgram.programId })
  .rpc();

// ── Step 6: Write messages ──
async function writeMessage(message: string) {
  const data = Buffer.from(message);
  const contentHash = computeContentHash(data);
  await program.methods.writeLedger(data, contentHash)
    .accountsPartial({ wallet: wallet.publicKey, agent: agentPda, vault: vaultPda, session: sessionPda, ledger: ledgerPda })
    .rpc();
}

await writeMessage(JSON.stringify({ role: "user", content: "Hello agent!" }));
await writeMessage(JSON.stringify({ role: "assistant", content: "Hello! How can I help?" }));

// ── Step 7: Read latest messages (HOT path ... FREE) ──
const ledger = await program.account.memoryLedger.fetch(ledgerPda);
const messages = parseRingBuffer(ledger.ring);
messages.forEach(m => console.log(JSON.parse(m.toString())));

// ── Step 8: Seal important memory (PERMANENT) ──
const [pagePda] = deriveLedgerPagePDA(ledgerPda, ledger.numPages);
await program.methods.sealLedger()
  .accountsPartial({ wallet: wallet.publicKey, agent: agentPda, vault: vaultPda, session: sessionPda, ledger: ledgerPda, page: pagePda, systemProgram: SystemProgram.programId })
  .rpc();
```

### Fetching All Agent Data

```typescript
// ── Fetch agent identity ──
const agent = await program.account.agentAccount.fetch(agentPda);
console.log(agent.name, agent.capabilities, agent.pricing);

// ── Fetch all agents (discovery) ──
const allAgents = await program.account.agentAccount.all();
const activeAgents = allAgents.filter(a => a.account.isActive);

// ── Fetch agents by capability ──
const capHash = crypto.createHash("sha256").update("defi:swap").digest();
const [capIndexPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("sap_cap_idx"), capHash], PROGRAM_ID,
);
const capIndex = await program.account.capabilityIndex.fetch(capIndexPda);
// capIndex.agents = PublicKey[] of agents with this capability

// ── Fetch feedback for an agent ──
const allFeedback = await program.account.feedbackAccount.all([
  { memcmp: { offset: 8 + 1, bytes: agentPda.toBase58() } },
]);

// ── Fetch all tools for an agent ──
const agentTools = await program.account.toolDescriptor.all([
  { memcmp: { offset: 8 + 1, bytes: agentPda.toBase58() } },
]);
```

### Event Parsing

```typescript
import { EventParser } from "@coral-xyz/anchor";

// Parse events from confirmed transactions
const parser = new EventParser(program.programId, program.coder);

const txSigs = await connection.getSignaturesForAddress(ledgerPda);
for (const sig of txSigs) {
  const tx = await connection.getTransaction(sig.signature, { maxSupportedTransactionVersion: 0 });
  if (!tx?.meta?.logMessages) continue;
  
  const events = [];
  parser.parseLogs(tx.meta.logMessages, (event) => events.push(event));
  
  for (const event of events) {
    if (event.name === "ledgerEntryEvent") {
      const data = Buffer.from(event.data.data);
      console.log("Entry:", data.toString());
    }
  }
}
```

---

## Cost Analysis

### Program Deployment

| Item | Cost |
|------|------|
| Binary size | 1,469,280 bytes (1.4 MB) |
| Program account rent | ~10.2 SOL (rent-exempt) |
| Buffer account for deploy | ~10.2 SOL (reclaimed after deploy) |
| **Total deploy cost** | **~10.2 SOL** |

### Per-Operation Costs

| Operation | Cost | Recoverable |
|-----------|------|-------------|
| `register_agent` | ~0.060 SOL | ✅ via `close_agent` |
| `init_vault` | ~0.002 SOL | ✅ via `close_vault` |
| `open_session` | ~0.003 SOL | ✅ via `close_session_pda` |
| `init_ledger` | ~0.032 SOL | ✅ via `close_ledger` |
| `write_ledger` | ~0.000005 SOL | ❌ (TX fee only) |
| `seal_ledger` | ~0.031 SOL | ❌ **Permanent** (by design) |
| `close_ledger` | reclaim ~0.032 SOL | ← recovery |
| `publish_tool` | ~0.004 SOL | ✅ via `close_tool` |
| `create_escrow` | ~0.003 SOL + deposit | ✅ via `close_escrow` |
| `give_feedback` | ~0.002 SOL | ✅ via `close_feedback` |
| `create_attestation` | ~0.003 SOL | ✅ via `close_attestation` |

### Memory Cost at Scale

| Scenario | MemoryLedger | MemoryVault | Raw PDA |
|----------|-------------|-------------|---------|
| **100 entries × 200B** | ~0.033 SOL | ~0.003 SOL | ~0.14 SOL |
| **1K entries × 200B** | ~0.037 SOL | ~0.007 SOL | ~1.39 SOL |
| **10K entries × 200B** | ~0.082 SOL | ~0.052 SOL | ~13.9 SOL |
| **100K entries × 200B** | ~0.532 SOL | ~0.502 SOL | ~139 SOL |
| **+ 10 sealed pages** | +0.31 SOL | N/A | N/A |

---

## Testnet Deployment Guide

### Prerequisites

```bash
# Install Solana CLI
sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"

# Install Anchor CLI
cargo install --git https://github.com/coral-xyz/anchor avm --force
avm install 0.32.1
avm use 0.32.1

# Set to devnet (or testnet)
solana config set --url devnet
```

### Step-by-step Deploy

```bash
# 1. Generate a deploy keypair (or use existing)
solana-keygen new -o keys/live/deployer.json --no-bip39-passphrase

# 2. Airdrop SOL for deploy (devnet only)
solana airdrop 15 keys/live/deployer.json --url devnet
# You need ~10.2 SOL for the program + ~1 SOL for transactions

# 3. Update Anchor.toml for devnet
# Change:
#   [provider]
#   cluster = "devnet"
#   wallet = "keys/live/deployer.json"
#
#   [programs.devnet]
#   synapse_agent_sap = "SAPTU7aUXk2AaAdktexae1iuxXpokxzNDBAYYhaVyQL"

# 4. Build
anchor build

# 5. Verify program ID matches keypair
solana-keygen pubkey target/deploy/synapse_agent_sap-keypair.json
# Should output: SAPTU7aUXk2AaAdktexae1iuxXpokxzNDBAYYhaVyQL

# 6. Deploy to devnet
anchor deploy --provider.cluster devnet --provider.wallet keys/live/deployer.json

# 7. Verify deployment
solana program show SAPTU7aUXk2AaAdktexae1iuxXpokxzNDBAYYhaVyQL --url devnet

# 8. Run tests against devnet
anchor test --provider.cluster devnet --skip-local-validator
```

### Post-Deploy Checklist

- [ ] Verify program ID on-chain: `solana program show <PROGRAM_ID>`
- [ ] Initialize GlobalRegistry: call `initialize_global()`
- [ ] Test agent registration on devnet
- [ ] Test memory ledger write + seal + read cycle
- [ ] Verify IDL uploaded: `anchor idl init` or `anchor idl upgrade`
- [ ] Consider making program non-upgradeable for immutability guarantee:
  ```bash
  solana program set-upgrade-authority <PROGRAM_ID> --final
  ```
  > ⚠️ This is IRREVERSIBLE. Once non-upgradeable, sealed pages are guaranteed permanent.

---

## Security Model

### Authorization Matrix

| Operation | Who Can Execute | Auth Check |
|-----------|----------------|------------|
| `register_agent` | Any wallet | wallet = signer |
| `update_agent` | Agent owner | `has_one = wallet` |
| `write_ledger` | Agent owner | `authority == wallet` constraint |
| `seal_ledger` | Agent owner | `authority == wallet` constraint |
| `close_*` (any) | Account owner | `has_one = wallet` or `authority == wallet` |
| `give_feedback` | Any wallet (not self) | `reviewer != agent.wallet` |
| `create_attestation` | Any agent (not self) | `attester != agent_to_attest` |
| `settle_calls` | Agent owner (provider) | `has_one = wallet` on agent |
| `inscribe_memory_delegated` | Delegate | delegate PDA verified, not expired |

### On-Chain Guarantees

1. **All arithmetic is checked** ... every `+`, `-`, `*` uses `.checked_*().ok_or(ArithmeticOverflow)`
2. **All PDAs verified by Anchor** ... seeds + bump constraints validated at runtime
3. **Escrow expiry enforced** ... expired escrows reject all settlement calls
4. **Attestation expiry enforced** ... reverts if `expires_at` < current timestamp
5. **Agent active guard** ... deactivated agents can't publish tools, create escrows
6. **Session closed guard** ... no writes to closed sessions
7. **Sealed pages are irrevocable** ... no close instruction exists in the program

### Trust Assumptions

| Aspect | Trust Level | Notes |
|--------|-------------|-------|
| Agent identity | On-chain verified | PDA derived from wallet, verifiable |
| Memory integrity | Merkle proof | Tamper-evident chain of all writes |
| Memory permanence | Protocol-guaranteed | Sealed pages have no close instruction |
| Reputation metrics | Self-reported | `report_calls`, `update_reputation` are self-attested |
| Feedback | Trustless | On-chain, reviewer verified, revocable |
| Escrow | Trustless | Funds held in PDA, not custodied by agent |

---

## Best Practices

### For SDK Implementors

1. **Always use the `SAP_PROGRAM_ID` constant** ... never hardcode the program ID string in multiple places.

2. **Use `deriveXxxPDA()` helpers** ... wrap `findProgramAddressSync` in typed functions that enforce correct seeds.

3. **Validate before sending** ... call `validate_registration()` / `validate_update()` client-side before submitting TX. The on-chain validator will reject invalid data, but client-side validation gives better UX.

4. **Parse ring buffer with `parseRingBuffer()`** ... the wire format is `[u16 LE len][data]` repeated. Don't manually parse.

5. **Replay merkle root client-side** ... store content hashes and verify against on-chain root for tamper detection.

6. **Use `EventParser` for TX log readback** ... Anchor's `EventParser` handles log decoding correctly.

7. **Batch close operations** ... when tearing down, close in order: ledger/buffer/digest → session_pda → epoch_pages → vault → agent.

### For Agent Developers

8. **Use MemoryLedger for general memory** ... it's the unified system combining readability + permanence + low cost.

9. **Seal important conversations** ... call `seal_ledger()` after key interactions to create permanent, immutable archives.

10. **Use MemoryVault only for encrypted data** ... if you need AES-256-GCM encryption with delegation, use the Vault system.

11. **Register tools with schemas** ... call `publish_tool` + `inscribe_tool_schema` to make your tools discoverable on-chain.

12. **Use capability indexes for discovery** ... register in relevant `CapabilityIndex` and `ProtocolIndex` PDAs so other agents can find you.

13. **Create escrows for paid services** ... use `create_escrow` + `settle_calls` for trustless micropayments.

14. **Monitor escrow expiry** ... set reasonable `expires_at` timestamps and withdraw unused funds before expiry.

15. **Close unused PDAs** ... always close PDAs you no longer need to reclaim rent. Every close returns SOL to the wallet.

### For Protocol Designers

16. **Make the program non-upgradeable on mainnet** ... this gives the strongest immutability guarantee for sealed pages.

17. **Consider PDA buffer accounts for large schemas** ... tool schemas > 750 bytes should use multi-fragment inscription.

18. **Index pagination** ... each index holds 100 agents. For popular capabilities, consider off-chain aggregation or L2 indexing.

19. **Epoch pages for high-throughput vaults** ... each epoch covers 1000 inscriptions. Plan epoch page cleanup for long-running sessions.

20. **Content hash verification** ... SDKs should ALWAYS compute `sha256(data)` and pass it as `content_hash`. The on-chain program trusts the caller's hash but emits it in events for off-chain verification.

---

## Statistics

| Metric | Value |
|--------|-------|
| Total instructions | **72** |
| Account types | **22** |
| Events | **45** |
| Error codes | **91** |
| Instruction modules | **13** |
| Validation functions | **13** |
| Rust LOC (src/) | ~4,930 |
| Binary size | 1.4 MB |
| Deploy cost (mainnet) | ~10.2 SOL |
| Tests passing | **187** |
