# Instruction Reference

> **Synapse Agent Protocol (SAP) v2** ... 72 On-Chain Instructions  
> Program ID: `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`  
> Anchor 0.32.1 · Solana SVM

This document is the complete reference for every instruction the SAP program exposes.
Instructions are grouped by domain; each section includes a summary table, auth constraints,
validation rules, and a minimal TypeScript example using the Anchor client.

---

## Table of Contents

1. [Global Registry (1)](#1-global-registry)
2. [Agent Lifecycle (7)](#2-agent-lifecycle)
3. [Feedback (4)](#3-feedback)
4. [Discovery Indexing (12)](#4-discovery-indexing)
5. [Plugin System (2)](#5-plugin-system)
6. [Legacy Memory (4)](#6-legacy-memory)
7. [Memory Vault (12)](#7-memory-vault)
8. [Tool Registry + Checkpoints (9)](#8-tool-registry--checkpoints)
9. [x402 Escrow (6)](#9-x402-escrow)
10. [Attestation (3)](#10-attestation)
11. [Memory Buffer (3)](#11-memory-buffer)
12. [Memory Digest (5)](#12-memory-digest)
13. [Memory Ledger (4)](#13-memory-ledger)

---

## 1. Global Registry

Single instruction to bootstrap the protocol. Must be called exactly once before any other SAP operation.

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 1 | `initialize_global` | ... | Creates the `GlobalRegistry` singleton PDA. Zeroed counters, records `authority` and `initialized_at`. |

**Auth chain**: Any signer can call this, but the PDA seed `["sap_global"]` ensures a single instance. The signer becomes `authority`.

**Validation**: None beyond Anchor account init constraints. Fails if the PDA already exists.

```typescript
await program.methods
  .initializeGlobal()
  .accounts({
    globalRegistry: globalRegistryPda,
    authority: wallet.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

---

## 2. Agent Lifecycle

Seven instructions manage the full agent lifecycle: create → update → deactivate/reactivate → report metrics → close.

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 2 | `register_agent` | `name`, `description`, `capabilities[]`, `pricing[]`, `protocols[]`, `agent_id?`, `agent_uri?`, `x402_endpoint?` | Creates an `AgentAccount` PDA (8,192 B) and an `AgentStats` PDA (106 B). Bumps `GlobalRegistry` counters. |
| 3 | `update_agent` | same fields, all `Option` | Partial update ... `None` fields are left unchanged. Runs deep validation on any supplied field. |
| 4 | `deactivate_agent` | ... | Sets `is_active = false` on both `AgentAccount` and `AgentStats`. Discovery indexes filter on read. |
| 5 | `reactivate_agent` | ... | Sets `is_active = true`. Fails if already active. |
| 6 | `close_agent` | ... | Closes `AgentAccount` + `AgentStats` PDAs. Rent returned to wallet. Decrements global counters. |
| 7 | `report_calls` | `calls_served: u64` | Increments `AgentStats.total_calls_served`. No reputation effect. |
| 8 | `update_reputation` | `avg_latency_ms: u32`, `uptime_percent: u8` | Self-reports latency and uptime. Updates `AgentAccount` fields directly. |

**Auth chain**: All instructions require `wallet` as signer, and the `AgentAccount` PDA must be seeded with `["sap_agent", wallet]`. The program asserts `agent.wallet == wallet.key()` on every mutation.

**Key validation rules**:
- `deactivate_agent` → errors `AlreadyInactive` if already inactive.
- `reactivate_agent` → errors `AlreadyActive` if already active.
- `close_agent` → decrements `GlobalRegistry.active_agents` only if the agent was active.
- `report_calls` → protected by `ArithmeticOverflow` on counter addition.
- `update_reputation` → validated via `validate_uptime_percent()` (0...100).

### Deep Validation Engine (`register_agent` / `update_agent`)

Both instructions call `validate_registration()` or `validate_update()` from the onchain validator module.
The rules are enforced at the BPF level ... invalid payloads are rejected before any state mutation.

| Field | Rule | Error |
|-------|------|-------|
| `name` | 1...64 bytes, no control chars (`< 0x20`) | `EmptyName`, `NameTooLong`, `ControlCharInName` |
| `description` | 1...256 bytes | `EmptyDescription`, `DescriptionTooLong` |
| `agent_id` | ≤ 128 bytes (optional) | `AgentIdTooLong` |
| `capabilities` | Max 10 items, each `"domain:action"` format, no duplicates | `TooManyCapabilities`, `InvalidCapabilityFormat`, `DuplicateCapability` |
| `pricing` | Max 5 tiers, non-empty `tier_id`, `rate_limit > 0`, no duplicate tier IDs | `TooManyPricingTiers`, `EmptyTierId`, `DuplicateTierId`, `InvalidRateLimit` |
| `pricing[].token_type == Spl` | Requires `token_mint.is_some()` | `SplRequiresTokenMint` |
| `pricing[].min/max_price` | `min ≤ max` when both present | `MinPriceExceedsMax` |
| `pricing[].volume_curve` | Max 5 breakpoints, `after_calls` strictly ascending | `TooManyVolumeCurvePoints`, `InvalidVolumeCurve` |
| `protocols` | Max 5 entries | `TooManyProtocols` |
| `agent_uri` | ≤ 256 bytes | `UriTooLong` |
| `x402_endpoint` | Must start with `https://`, ≤ 256 bytes | `InvalidX402Endpoint`, `UriTooLong` |
| `uptime_percent` | 0...100 | `InvalidUptimePercent` |

```typescript
await program.methods
  .registerAgent(
    "jupiter-swap-agent",
    "Production swap agent backed by Jupiter Aggregator v6",
    [{ id: "jupiter:swap", description: "Token swaps", protocolId: "jupiter", version: "1.0.0" }],
    [{
      tierId: "standard",
      pricePerCall: new BN(100_000),        // 0.0001 SOL
      minPricePerCall: null,
      maxPricePerCall: null,
      rateLimit: 100,
      maxCallsPerSession: 0,
      burstLimit: null,
      tokenType: { sol: {} },
      tokenMint: null,
      tokenDecimals: 9,
      settlementMode: { x402: {} },
      minEscrowDeposit: null,
      batchIntervalSec: null,
      volumeCurve: null,
    }],
    ["jupiter"],
    null,                                   // agent_id
    "https://agent.example.com/.well-known/agent.json",
    "https://agent.example.com/x402",
  )
  .accounts({
    agentAccount: agentPda,
    agentStats: agentStatsPda,
    globalRegistry: globalRegistryPda,
    wallet: wallet.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

---

## 3. Feedback

Trustless on-chain reputation. One feedback PDA per `(agent, reviewer)` pair.

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 9 | `give_feedback` | `score: u16`, `tag: String`, `comment_hash?: [u8;32]` | Creates `FeedbackAccount` PDA. Score 0...1000. Updates `AgentAccount.reputation_score` incrementally. |
| 10 | `update_feedback` | `new_score: u16`, `new_tag?: String`, `comment_hash?: [u8;32]` | Updates an existing review. Adjusts reputation sum atomically. |
| 11 | `revoke_feedback` | ... | Marks feedback as revoked. Subtracts score from reputation sum. |
| 12 | `close_feedback` | ... | Closes a revoked feedback PDA. Rent → reviewer. Errors `FeedbackNotRevoked` if still active. |

**Auth chain**: `give_feedback` requires the `reviewer` signer. All mutations require `reviewer == feedback.reviewer`. Self-review blocked via `SelfReviewNotAllowed` (`reviewer != agent.wallet`).

**Validation**:
- `score` must be 0...1000 (`InvalidFeedbackScore`).
- `tag` must be ≤ 32 bytes (`TagTooLong`).
- `close_feedback` requires `is_revoked == true` (`FeedbackNotRevoked`).
- `revoke_feedback` fails on already revoked feedback (`FeedbackAlreadyRevoked`).

```typescript
await program.methods
  .giveFeedback(850, "reliability", commentHash)
  .accounts({
    feedbackAccount: feedbackPda,
    agentAccount: agentPda,
    reviewer: reviewer.publicKey,
    globalRegistry: globalRegistryPda,
    systemProgram: SystemProgram.programId,
  })
  .signers([reviewer])
  .rpc();
```

---

## 4. Discovery Indexing

Twelve instructions manage capability, protocol, and tool category indexes. Each index is a PDA holding up to 100 agent or tool pubkeys.

### Capability Index

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 13 | `init_capability_index` | `capability_id: String`, `capability_hash: [u8;32]` | Creates a `CapabilityIndex` PDA and adds the caller's agent as the first entry. |
| 14 | `add_to_capability_index` | `capability_hash: [u8;32]` | Adds the caller's agent to an existing index. |
| 15 | `remove_from_capability_index` | `capability_hash: [u8;32]` | Removes the caller's agent. Errors `AgentNotInIndex` if absent. |
| 16 | `close_capability_index` | `capability_hash: [u8;32]` | Closes an empty index PDA. Errors `IndexNotEmpty` if agents remain. |

### Protocol Index

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 17 | `init_protocol_index` | `protocol_id: String`, `protocol_hash: [u8;32]` | Creates `ProtocolIndex` PDA and registers the first agent. |
| 18 | `add_to_protocol_index` | `protocol_hash: [u8;32]` | Adds the caller's agent. |
| 19 | `remove_from_protocol_index` | `protocol_hash: [u8;32]` | Removes the caller's agent. |
| 20 | `close_protocol_index` | `protocol_hash: [u8;32]` | Closes empty index PDA. |

### Tool Category Index

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 21 | `init_tool_category_index` | `category: u8` | Creates `ToolCategoryIndex` PDA for a `ToolCategory` enum value (0...9). |
| 22 | `add_to_tool_category` | `category: u8` | Adds a `ToolDescriptor` PDA. Verifies `tool.category` matches. |
| 23 | `remove_from_tool_category` | `category: u8` | Removes a tool from the index. |
| 24 | `close_tool_category_index` | `category: u8` | Closes empty category index. |

**Auth chain**: All index mutations require agent/tool ownership ... the signer must own the `AgentAccount` or `ToolDescriptor` being indexed.

**Validation**:
- Hash arguments are used as PDA seeds and must match the SHA-256 of the ID string. Mismatch → `InvalidCapabilityHash` / `InvalidProtocolHash`.
- Indexes are capped at 100 entries → `CapabilityIndexFull` / `ProtocolIndexFull` / `ToolCategoryIndexFull`.
- Remove operations error `AgentNotInIndex` / `ToolNotInCategoryIndex` if the key is absent.
- Close operations error `IndexNotEmpty` if the agents/tools vector is non-empty.
- `add_to_tool_category` verifies `tool.category == category` → `ToolCategoryMismatch` on mismatch.

```typescript
const capabilityHash = sha256(Buffer.from("jupiter:swap"));

await program.methods
  .initCapabilityIndex("jupiter:swap", Array.from(capabilityHash))
  .accounts({
    capabilityIndex: capIndexPda,
    agentAccount: agentPda,
    wallet: wallet.publicKey,
    globalRegistry: globalRegistryPda,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

---

## 5. Plugin System

> **DEPRECATED** ... Gated behind the `legacy-memory` feature flag. Retained for backward compatibility.

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 25 | `register_plugin` | `plugin_type: u8` | Creates a `PluginSlot` PDA (124 B). Adds a `PluginRef` to the agent's `active_plugins`. |
| 26 | `close_plugin` | ... | Closes `PluginSlot` PDA. Removes from `active_plugins`. |

**Auth chain**: Requires agent owner signature. Plugin type must be 0...5 (`InvalidPluginType`). Max 5 plugins per agent (`TooManyPlugins`).

```typescript
// plugin_type: 0=Memory, 1=Validation, 2=Delegation, 3=Analytics, 4=Governance, 5=Custom
await program.methods
  .registerPlugin(0)
  .accounts({
    pluginSlot: pluginPda,
    agentAccount: agentPda,
    wallet: wallet.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

---

## 6. Legacy Memory

> **DEPRECATED** ... Gated behind the `legacy-memory` feature flag. Use Memory Ledger (§13) or Memory Vault (§7) instead.

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 27 | `store_memory` | `entry_hash: [u8;32]`, `content_type: String`, `ipfs_cid?: String`, `total_size: u32` | Creates a `MemoryEntry` PDA with IPFS pointer. |
| 28 | `append_memory_chunk` | `chunk_index: u8`, `data: Vec<u8>` | Appends an onchain `MemoryChunk` (≤ 900 B). |
| 29 | `close_memory_entry` | ... | Closes entry PDA. Rent → wallet. |
| 30 | `close_memory_chunk` | ... | Closes chunk PDA. Rent → wallet. |

**Auth chain**: Agent owner signs all operations.

**Validation**:
- Chunk data ≤ 900 bytes (`ChunkDataTooLarge`).
- Content type ≤ 32 bytes (`ContentTypeTooLong`).
- IPFS CID ≤ 64 bytes (`IpfsCidTooLong`).

---

## 7. Memory Vault

Twelve instructions for encrypted transaction-log inscriptions. Data lives permanently in TX logs at zero rent; only lightweight index PDAs pay rent.

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 31 | `init_vault` | `vault_nonce: [u8;32]` | Creates `MemoryVault` PDA. Nonce is PBKDF2 salt for client-side key derivation (never decrypted onchain). |
| 32 | `open_session` | `session_hash: [u8;32]` | Creates `SessionLedger` PDA. Hash is SHA-256 of a deterministic session ID. |
| 33 | `inscribe_memory` | `sequence`, `encrypted_data`, `nonce`, `content_hash`, `total_fragments`, `fragment_index`, `compression`, `epoch_index` | AES-256-GCM ciphertext inscription to TX log. Auto-creates `EpochPage` PDA at epoch boundaries. |
| 34 | `close_session` | ... | Marks session `is_closed = true`. No further inscriptions allowed. |
| 35 | `close_vault` | ... | Closes `MemoryVault` PDA. All sessions must be closed first. |
| 36 | `close_session_pda` | ... | Closes a closed `SessionLedger` PDA. Rent returned. |
| 37 | `close_epoch_page` | `epoch_index: u32` | Closes an `EpochPage` PDA. Rent returned. |
| 38 | `rotate_vault_nonce` | `new_nonce: [u8;32]` | Rotates the PBKDF2 salt. Old nonce emitted in event for historical decryption. Increments `nonce_version`. |
| 39 | `add_vault_delegate` | `permissions: u8`, `expires_at: i64` | Creates `VaultDelegate` PDA for a hot wallet. Bitmask: `1=inscribe`, `2=close`, `4=open`. |
| 40 | `revoke_vault_delegate` | ... | Closes `VaultDelegate` PDA. Rent returned. |
| 41 | `inscribe_memory_delegated` | same as `inscribe_memory` | Delegation variant ... delegate signs instead of owner. Checks `VaultDelegate` permissions + expiry. |
| 42 | `compact_inscribe` | `sequence`, `encrypted_data`, `nonce`, `content_hash` | DX-first: 4 args vs 8. Assumes single fragment, no compression, current epoch. |

**Auth chain**:
- `init_vault`, `close_vault`, `rotate_vault_nonce`, `add_vault_delegate`, `revoke_vault_delegate` → vault owner (`vault.wallet == signer`).
- `open_session`, `inscribe_memory`, `close_session`, `compact_inscribe` → vault owner.
- `inscribe_memory_delegated` → authorized delegate (checked against `VaultDelegate` PDA).

**Validation**:
- `encrypted_data` ≤ 750 bytes (`InscriptionTooLarge`), ≥ 1 byte (`EmptyInscription`).
- `sequence` must match `session.sequence_counter` (`InvalidSequence`).
- `fragment_index < total_fragments` (`InvalidFragmentIndex`), `total_fragments ≥ 1` (`InvalidTotalFragments`).
- Session must not be closed (`SessionClosed`).
- Vault must not have open sessions to close (`SessionNotClosed` on `close_vault`).
- Delegate must not be expired (`DelegateExpired`) and must have correct permission bit (`InvalidDelegate`).
- `epoch_index` must match `session.current_epoch` (`EpochMismatch`).

```typescript
// Compact inscription ... simplest write path
await program.methods
  .compactInscribe(
    0,                                  // sequence
    Buffer.from(encryptedData),         // AES-256-GCM ciphertext
    Array.from(nonce),                  // 12-byte nonce
    Array.from(contentHash),            // sha256 of plaintext
  )
  .accounts({
    vault: vaultPda,
    session: sessionPda,
    epochPage: epochPagePda,
    wallet: wallet.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

---

## 8. Tool Registry + Checkpoints

Nine instructions for the onchain tool schema registry and session fast-sync checkpoints.

### Tool Registry

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 43 | `publish_tool` | `tool_name`, `tool_name_hash`, `protocol_hash`, `description_hash`, `input_schema_hash`, `output_schema_hash`, `http_method`, `category`, `params_count`, `required_params`, `is_compound` | Creates `ToolDescriptor` PDA (333 B). Hashes reference schemas inscribed via TX log. |
| 44 | `inscribe_tool_schema` | `schema_type: u8`, `schema_data: Vec<u8>`, `schema_hash: [u8;32]`, `compression: u8` | Inscribes full JSON schema to TX log. `schema_type`: 0=input, 1=output, 2=description. Verify: `sha256(data) == hash`. |
| 45 | `update_tool` | `description_hash?`, `input_schema_hash?`, `output_schema_hash?`, `http_method?`, `category?`, `params_count?`, `required_params?` | Partial update. Bumps `version`. `None` = unchanged. |
| 46 | `deactivate_tool` | ... | Sets `is_active = false`. Tool remains discoverable but marked unavailable. |
| 47 | `reactivate_tool` | ... | Sets `is_active = true`. |
| 48 | `close_tool` | ... | Closes `ToolDescriptor` PDA. Rent → wallet. |
| 49 | `report_tool_invocations` | `invocations: u64` | Self-reported invocation counter. |

### Session Checkpoints

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 50 | `create_session_checkpoint` | `checkpoint_index: u32` | Snapshots `merkle_root` + counters from `SessionLedger` into a `SessionCheckpoint` PDA (141 B). |
| 51 | `close_checkpoint` | `checkpoint_index: u32` | Closes checkpoint PDA. Rent → wallet. |

**Auth chain**: All tool mutations require agent owner (`agent.wallet == signer`). Checkpoint operations require vault/session owner.

**Validation**:
- `tool_name` ≤ 32 bytes (`ToolNameTooLong`), not empty (`EmptyToolName`).
- `tool_name_hash` must equal `sha256(tool_name)` (`InvalidToolNameHash`).
- `http_method` must be 0...4 (`InvalidToolHttpMethod`).
- `category` must be 0...9 (`InvalidToolCategory`).
- `deactivate_tool` errors `ToolAlreadyInactive` if already inactive.
- `reactivate_tool` errors `ToolAlreadyActive` if already active.
- `update_tool` errors `NoFieldsToUpdate` if all fields are `None`.
- `inscribe_tool_schema` validates `schema_type` (0...2) → `InvalidSchemaType`, and hash → `InvalidSchemaHash`.
- `checkpoint_index` must match `session.total_checkpoints` → `InvalidCheckpointIndex`.

```typescript
await program.methods
  .publishTool(
    "getQuote",
    Array.from(sha256("getQuote")),
    Array.from(protocolHash),
    Array.from(descriptionHash),
    Array.from(inputSchemaHash),
    Array.from(outputSchemaHash),
    1,       // POST
    0,       // Swap
    5,       // params_count
    3,       // required_params
    false,   // is_compound
  )
  .accounts({
    toolDescriptor: toolPda,
    agentAccount: agentPda,
    globalRegistry: globalRegistryPda,
    wallet: wallet.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

---

## 9. x402 Escrow

Six instructions for pre-funded trustless micropayments between clients and agents. Supports SOL and SPL tokens.

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 52 | `create_escrow` | `price_per_call`, `max_calls`, `initial_deposit`, `expires_at`, `volume_curve[]`, `token_mint?`, `token_decimals` | Creates `EscrowAccount` PDA (291 B). Initial deposit transferred in same TX. Price per call is immutable. |
| 53 | `deposit_escrow` | `amount: u64` | Adds funds to existing escrow. |
| 54 | `settle_calls` | `calls_to_settle: u64`, `service_hash: [u8;32]` | Agent claims payment. Computes amount via base price + volume curve. Transfers funds to agent wallet. Emits `PaymentSettledEvent` as permanent receipt. |
| 55 | `withdraw_escrow` | `amount: u64` | Client withdraws. Takes `min(amount, balance)`. |
| 56 | `close_escrow` | ... | Closes empty escrow PDA. Balance must be zero. Rent → depositor. |
| 57 | `settle_batch` | `settlements: Vec<Settlement>` | Batch settle up to 10 settlements in one TX. Volume curve spans entire batch. |

**Auth chain**:
- `create_escrow`, `deposit_escrow`, `withdraw_escrow`, `close_escrow` → depositor (client) signs.
- `settle_calls`, `settle_batch` → agent wallet signs.

**Validation**:
- `settle_calls`: balance must cover `calls × effective_price` (`InsufficientEscrowBalance`).
- `settle_calls`: `total_calls_settled + calls ≤ max_calls` (if max_calls > 0) (`EscrowMaxCallsExceeded`).
- `settle_calls`: `calls_to_settle ≥ 1` (`InvalidSettlementCalls`).
- `close_escrow`: balance must be zero (`EscrowNotEmpty`).
- `withdraw_escrow`: balance must be > 0 (`EscrowEmpty`).
- `settle_batch`: 1...10 settlements (`BatchEmpty`, `BatchTooLarge`).
- Expiry enforced: `expires_at == 0 || Clock::get().unix_timestamp < expires_at` (`EscrowExpired`).
- Agent must be active (`AgentInactive`).
- SPL token escrows require token accounts and token program in remaining accounts (`SplTokenRequired`, `InvalidTokenAccount`, `InvalidTokenProgram`).

```typescript
await program.methods
  .createEscrow(
    new BN(100_000),      // price_per_call (lamports)
    new BN(1000),         // max_calls
    new BN(10_000_000),   // initial_deposit
    new BN(0),            // expires_at (0 = never)
    [],                   // volume_curve
    null,                 // token_mint (null = SOL)
    9,                    // token_decimals
  )
  .accounts({
    escrowAccount: escrowPda,
    agentAccount: agentPda,
    agentStats: agentStatsPda,
    depositor: client.publicKey,
    agentWallet: agentWallet,
    systemProgram: SystemProgram.programId,
  })
  .signers([client])
  .rpc();
```

---

## 10. Attestation

Web-of-trust attestations. Any wallet can vouch for any agent (one attestation per `(agent, attester)` pair). Trust derives from *who* is attesting, not the attestation itself.

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 58 | `create_attestation` | `attestation_type: String`, `metadata_hash: [u8;32]`, `expires_at: i64` | Creates `AgentAttestation` PDA (198 B). Type examples: `"verified"`, `"audited"`, `"partner"`. |
| 59 | `revoke_attestation` | ... | Marks `is_active = false`. Original attester only. |
| 60 | `close_attestation` | ... | Closes revoked attestation PDA. Errors `AttestationNotRevoked` if still active. |

**Auth chain**: `attester` wallet signs all operations. Self-attestation blocked (`SelfAttestationNotAllowed`).

**Validation**:
- `attestation_type` ≤ 32 chars (`AttestationTypeTooLong`), not empty (`EmptyAttestationType`).
- `revoke_attestation` errors `AttestationAlreadyRevoked` on double revoke.
- `close_attestation` requires revoked state (`AttestationNotRevoked`).
- Expiry enforced on read: `expires_at == 0 || now < expires_at` (`AttestationExpired`).

```typescript
await program.methods
  .createAttestation("audited", Array.from(metadataHash), new BN(0))
  .accounts({
    attestation: attestPda,
    agentAccount: agentPda,
    attester: auditor.publicKey,
    globalRegistry: globalRegistryPda,
    systemProgram: SystemProgram.programId,
  })
  .signers([auditor])
  .rpc();
```

---

## 11. Memory Buffer

> **DEPRECATED** ... Gated behind `legacy-memory`. Use Memory Ledger (§13) instead.

Onchain readable session cache using dynamic realloc. Data accessible via `getAccountInfo()` on any free RPC.

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 61 | `create_buffer` | `page_index: u32` | Creates `MemoryBuffer` PDA (~101 B initial, ≈0.001 SOL). |
| 62 | `append_buffer` | `page_index: u32`, `data: Vec<u8>` | Appends ≤ 750 B per call. Uses `realloc` to grow the PDA. Max 10 KB total. |
| 63 | `close_buffer` | `page_index: u32` | Closes buffer PDA. Reclaims all accumulated rent. |

**Auth chain**: Session authority signs all operations.

**Validation**:
- Data per append ≤ 750 bytes (`BufferDataTooLarge`).
- Total data ≤ 10,000 bytes (`BufferFull`).
- Session must be valid (`InvalidSession`).
- Signer must be session authority (`Unauthorized`).

---

## 12. Memory Digest

> **DEPRECATED** ... Gated behind `legacy-memory`. Use Memory Ledger (§13) instead.

Fixed-size PDA (~0.002 SOL, never grows) with rolling merkle root. Data posted to TX logs.

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 64 | `init_digest` | ... | Creates `MemoryDigest` PDA (230 B). |
| 65 | `post_digest` | `content_hash: [u8;32]`, `data_size: u32` | Posts hash-only proof. Zero additional rent. |
| 66 | `inscribe_to_digest` | `data: Vec<u8>`, `content_hash: [u8;32]` | Inscribes data to TX log + updates PDA proof. Primary write path. |
| 67 | `update_digest_storage` | `storage_ref: [u8;32]`, `storage_type: u8` | Sets offchain storage pointer. Types: 0=None, 1=IPFS, 2=Arweave, 3=ShadowDrive, 4=HTTP/S, 5=Filecoin. |
| 68 | `close_digest` | ... | Closes digest PDA. Reclaims all rent. |

**Auth chain**: Session authority signs all operations.

**Validation**:
- `content_hash` must not be all zeros (`EmptyDigestHash`).
- `data` in `inscribe_to_digest` ≤ 750 bytes (`LedgerDataTooLarge`).

---

## 13. Memory Ledger

> **[recommended] RECOMMENDED** ... The unified memory system. Replaces Vault, Buffer, and Digest for most use cases.

Fixed 4 KB ring buffer PDA (~0.032 SOL) plus permanent TX log events. Two read paths: hot (free `getAccountInfo`) and cold (TX history).

| # | Instruction | Args | Description |
|---|-------------|------|-------------|
| 69 | `init_ledger` | ... | Creates `MemoryLedger` PDA (4,269 B, ~0.032 SOL). Fixed cost, never grows. |
| 70 | `write_ledger` | `data: Vec<u8>`, `content_hash: [u8;32]` | Writes to TX log (permanent) + ring buffer (instant read). Cost = TX fee only (~0.000005 SOL). Evicts oldest entries if ring is full. |
| 71 | `seal_ledger` | ... | Freezes current ring buffer into a permanent `LedgerPage` PDA (~0.031 SOL). Write-once, no close exists. |
| 72 | `close_ledger` | ... | Closes `MemoryLedger` PDA. Reclaims ~0.032 SOL. Sealed pages remain permanent. |

**Auth chain**: Session authority signs all operations.

**Validation**:
- `data` ≤ 750 bytes (`LedgerDataTooLarge`).
- Ring must have entries to seal (`LedgerRingEmpty`).
- Session must be valid (`InvalidSession`).
- Signer must be authority (`Unauthorized`).

**Cost model**:
| Writes | Total Cost |
|--------|-----------|
| Init | ~0.032 SOL |
| 1,000 writes | ~0.037 SOL (init + TX fees) |
| 10,000 writes | ~0.082 SOL (init + TX fees) |
| `close_ledger` | reclaim ~0.032 SOL |

```typescript
// Init
await program.methods
  .initLedger()
  .accounts({
    ledger: ledgerPda,
    session: sessionPda,
    wallet: wallet.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();

// Write
await program.methods
  .writeLedger(Buffer.from(messageData), Array.from(contentHash))
  .accounts({
    ledger: ledgerPda,
    session: sessionPda,
    wallet: wallet.publicKey,
  })
  .rpc();

// Read (free, any RPC)
const ledgerAccount = await program.account.memoryLedger.fetch(ledgerPda);
const ring = ledgerAccount.ring; // latest ~10-20 messages
```

---

## Instruction Index (Quick Reference)

| # | Instruction | Domain |
|---|-------------|--------|
| 1 | `initialize_global` | Global Registry |
| 2 | `register_agent` | Agent Lifecycle |
| 3 | `update_agent` | Agent Lifecycle |
| 4 | `deactivate_agent` | Agent Lifecycle |
| 5 | `reactivate_agent` | Agent Lifecycle |
| 6 | `close_agent` | Agent Lifecycle |
| 7 | `report_calls` | Agent Lifecycle |
| 8 | `update_reputation` | Agent Lifecycle |
| 9 | `give_feedback` | Feedback |
| 10 | `update_feedback` | Feedback |
| 11 | `revoke_feedback` | Feedback |
| 12 | `close_feedback` | Feedback |
| 13 | `init_capability_index` | Discovery Indexing |
| 14 | `add_to_capability_index` | Discovery Indexing |
| 15 | `remove_from_capability_index` | Discovery Indexing |
| 16 | `close_capability_index` | Discovery Indexing |
| 17 | `init_protocol_index` | Discovery Indexing |
| 18 | `add_to_protocol_index` | Discovery Indexing |
| 19 | `remove_from_protocol_index` | Discovery Indexing |
| 20 | `close_protocol_index` | Discovery Indexing |
| 21 | `init_tool_category_index` | Discovery Indexing |
| 22 | `add_to_tool_category` | Discovery Indexing |
| 23 | `remove_from_tool_category` | Discovery Indexing |
| 24 | `close_tool_category_index` | Discovery Indexing |
| 25 | `register_plugin` | Plugin System (deprecated) |
| 26 | `close_plugin` | Plugin System (deprecated) |
| 27 | `store_memory` | Legacy Memory (deprecated) |
| 28 | `append_memory_chunk` | Legacy Memory (deprecated) |
| 29 | `close_memory_entry` | Legacy Memory (deprecated) |
| 30 | `close_memory_chunk` | Legacy Memory (deprecated) |
| 31 | `init_vault` | Memory Vault |
| 32 | `open_session` | Memory Vault |
| 33 | `inscribe_memory` | Memory Vault |
| 34 | `close_session` | Memory Vault |
| 35 | `close_vault` | Memory Vault |
| 36 | `close_session_pda` | Memory Vault |
| 37 | `close_epoch_page` | Memory Vault |
| 38 | `rotate_vault_nonce` | Memory Vault |
| 39 | `add_vault_delegate` | Memory Vault |
| 40 | `revoke_vault_delegate` | Memory Vault |
| 41 | `inscribe_memory_delegated` | Memory Vault |
| 42 | `compact_inscribe` | Memory Vault |
| 43 | `publish_tool` | Tool Registry |
| 44 | `inscribe_tool_schema` | Tool Registry |
| 45 | `update_tool` | Tool Registry |
| 46 | `deactivate_tool` | Tool Registry |
| 47 | `reactivate_tool` | Tool Registry |
| 48 | `close_tool` | Tool Registry |
| 49 | `report_tool_invocations` | Tool Registry |
| 50 | `create_session_checkpoint` | Checkpoints |
| 51 | `close_checkpoint` | Checkpoints |
| 52 | `create_escrow` | x402 Escrow |
| 53 | `deposit_escrow` | x402 Escrow |
| 54 | `settle_calls` | x402 Escrow |
| 55 | `withdraw_escrow` | x402 Escrow |
| 56 | `close_escrow` | x402 Escrow |
| 57 | `settle_batch` | x402 Escrow |
| 58 | `create_attestation` | Attestation |
| 59 | `revoke_attestation` | Attestation |
| 60 | `close_attestation` | Attestation |
| 61 | `create_buffer` | Memory Buffer (deprecated) |
| 62 | `append_buffer` | Memory Buffer (deprecated) |
| 63 | `close_buffer` | Memory Buffer (deprecated) |
| 64 | `init_digest` | Memory Digest (deprecated) |
| 65 | `post_digest` | Memory Digest (deprecated) |
| 66 | `inscribe_to_digest` | Memory Digest (deprecated) |
| 67 | `update_digest_storage` | Memory Digest (deprecated) |
| 68 | `close_digest` | Memory Digest (deprecated) |
| 69 | `init_ledger` | Memory Ledger [recommended] |
| 70 | `write_ledger` | Memory Ledger [recommended] |
| 71 | `seal_ledger` | Memory Ledger [recommended] |
| 72 | `close_ledger` | Memory Ledger [recommended] |

---

[Previous: 01-architecture.md](01-architecture.md) · [Next: 03-accounts.md](03-accounts.md)
