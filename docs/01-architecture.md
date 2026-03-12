# Architecture & Design

> How SAP v2 organises 72 instructions, 22 account types, and four memory systems into a coherent on-chain protocol for autonomous agents.

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Synapse Agent Protocol (SAP v2)                       │
│                  SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  IDENTITY LAYER                                                       │  │
│  │  GlobalRegistry ─── AgentAccount ─── AgentStats ─── PluginSlot       │  │
│  │                          │                                            │  │
│  │                     VaultDelegate                                     │  │
│  └──────────────────────────┼────────────────────────────────────────────┘  │
│                             │                                               │
│  ┌──────────────────────────┼────────────────────────────────────────────┐  │
│  │  MEMORY LAYER            │                                            │  │
│  │                          ▼                                            │  │
│  │  MemoryVault ──► SessionLedger ──► EpochPage                         │  │
│  │                       │                                               │  │
│  │                       ├──► MemoryLedger ──► LedgerPage    [recommended] RECOMMENDED │
│  │                       ├──► MemoryBuffer                   [legacy]    │  │
│  │                       └──► MemoryDigest                   [legacy]    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  REPUTATION LAYER                                                     │  │
│  │  FeedbackAccount (score 0...1000, per reviewer×agent)                   │  │
│  │  AgentAttestation (third-party web-of-trust signals)                  │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  COMMERCE LAYER                                                       │  │
│  │  EscrowAccount (pre-funded x402 micropayments, volume curves)         │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  TOOL LAYER                                                           │  │
│  │  ToolDescriptor (typed schemas, versioned, categories)                │  │
│  │  SessionCheckpoint (fast-sync state snapshots)                        │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  DISCOVERY LAYER                                                      │  │
│  │  CapabilityIndex ─── ProtocolIndex ─── ToolCategoryIndex              │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Authority Chain

Every on-chain operation traces back to a wallet signer through a strict PDA ownership chain.  No instruction can modify state without proving this chain is intact ... Anchor's `has_one` and `constraint` attributes enforce it at the runtime level.

```
wallet (Signer)
  │
  └─► agent PDA  ["sap_agent", wallet.pubkey]        has_one = wallet
       │
       ├─► stats PDA   ["sap_stats", agent.pubkey]   has_one = agent
       │
       ├─► vault PDA   ["sap_vault", agent.pubkey]   has_one = agent, has_one = wallet
       │    │
       │    ├─► delegate PDA  ["sap_delegate", vault, delegate.pubkey]
       │    │                                          has_one = vault
       │    │
       │    └─► session PDA   ["sap_session", vault, session_hash]
       │         │                                     has_one = vault
       │         │
       │         ├─► epoch PDA    ["sap_epoch", session, epoch_u32_le]
       │         │                                     has_one = session
       │         │
       │         ├─► ledger PDA   ["sap_ledger", session]
       │         │    └─► page PDA ["sap_page", ledger, page_u32_le]
       │         │
       │         ├─► buffer PDA   ["sap_buffer", session, page_u32_le]    [legacy]
       │         │
       │         └─► digest PDA   ["sap_digest", session]                 [legacy]
       │
       ├─► tool PDA    ["sap_tool", agent, SHA256(tool_name)]
       │                                               has_one = agent
       │
       ├─► escrow PDA  ["sap_escrow", agent, depositor.pubkey]
       │                                               has_one = agent
       │
       ├─► feedback PDA ["sap_feedback", agent, reviewer.pubkey]
       │                                               has_one = agent
       │
       └─► attestation PDA ["sap_attest", agent, attester.pubkey]
                                                       has_one = agent
```

### Delegation Model

The `VaultDelegate` PDA enables hot-wallet operation without exposing the owner's cold wallet. Permissions are a bitmask:

| Bit | Value | Permission |
|:----|:------|:-----------|
| 0 | `1` | `inscribe_memory` ... write to TX logs |
| 1 | `2` | `close_session` ... seal a session |
| 2 | `4` | `open_session` ... create new sessions |

Delegates can optionally expire at a unix timestamp (`expires_at = 0` means never). The owner can revoke a delegate at any time, which closes the PDA and returns rent.

---

## PDA Seed Reference

All 22 account types and their deterministic PDA derivation seeds. Every seed is a UTF-8 string prefix followed by one or more pubkey/hash/index segments.

| # | Account | Seeds | Scope | Notes |
|:--|:--------|:------|:------|:------|
| 1 | **GlobalRegistry** | `["sap_global"]` | Singleton | One per program. Init once by authority. |
| 2 | **AgentAccount** | `["sap_agent", wallet.pubkey]` | Per wallet | Core identity PDA. Max 1 per wallet. |
| 3 | **AgentStats** | `["sap_stats", agent.pubkey]` | Per agent | Hot-path metrics. 106 bytes vs 8 KB. |
| 4 | **FeedbackAccount** | `["sap_feedback", agent.pubkey, reviewer.pubkey]` | Per reviewer×agent | Score 0...1000, revocable. |
| 5 | **CapabilityIndex** | `["sap_cap_idx", SHA256(capability_id)]` | Per capability | Up to 100 agents per index. |
| 6 | **ProtocolIndex** | `["sap_proto_idx", SHA256(protocol_id)]` | Per protocol | Up to 100 agents per index. |
| 7 | **PluginSlot** | `["sap_plugin", agent.pubkey, plugin_type_u8]` | Per type per agent | [Legacy] 6 types: Memory..Custom. |
| 8 | **MemoryEntry** | `["sap_memory", agent.pubkey, entry_hash]` | Per entry | [Legacy] Hybrid IPFS + onchain. |
| 9 | **MemoryChunk** | `["sap_mem_chunk", entry.pubkey, chunk_index_u8]` | Per chunk | [Legacy] Max 900 bytes each. |
| 10 | **MemoryVault** | `["sap_vault", agent.pubkey]` | Per agent | Encrypted inscription vault. |
| 11 | **SessionLedger** | `["sap_session", vault.pubkey, session_hash]` | Per session | Tracks inscriptions + merkle root. |
| 12 | **EpochPage** | `["sap_epoch", session.pubkey, epoch_index_u32_le]` | Per 1000 inscriptions | Auto-created. O(1) epoch queries. |
| 13 | **VaultDelegate** | `["sap_delegate", vault.pubkey, delegate.pubkey]` | Per delegate×vault | Hot-wallet authorization. |
| 14 | **ToolDescriptor** | `["sap_tool", agent.pubkey, SHA256(tool_name)]` | Per tool per agent | Versioned schema registry. |
| 15 | **SessionCheckpoint** | `["sap_checkpoint", session.pubkey, checkpoint_index_u32_le]` | Per checkpoint | Fast-sync snapshot. |
| 16 | **EscrowAccount** | `["sap_escrow", agent.pubkey, depositor.pubkey]` | Per depositor×agent | x402 pre-funded micropayments. |
| 17 | **ToolCategoryIndex** | `["sap_tool_cat", category_u8]` | Per category (0...9) | Cross-agent tool discovery. |
| 18 | **AgentAttestation** | `["sap_attest", agent.pubkey, attester.pubkey]` | Per attester×agent | Web-of-trust signal. |
| 19 | **MemoryBuffer** | `["sap_buffer", session.pubkey, page_index_u32_le]` | Per buffer page | [Legacy] Realloc PDA, max 10 KB. |
| 20 | **MemoryDigest** | `["sap_digest", session.pubkey]` | Per session | [Legacy] Proof-of-memory, ~0.002 SOL fixed. |
| 21 | **MemoryLedger** | `["sap_ledger", session.pubkey]` | Per session | [recommended] Recommended. 4 KB ring + TX logs. |
| 22 | **LedgerPage** | `["sap_page", ledger.pubkey, page_index_u32_le]` | Per sealed page | Permanent. No close instruction exists. |

### Seed Constants (TypeScript)

```typescript
import { SEEDS } from "@synapse-sap/sdk/constants";

// Every seed mirrors the Rust #[account(seeds = [...])] definitions
SEEDS.AGENT              // "sap_agent"
SEEDS.STATS              // "sap_stats"
SEEDS.FEEDBACK           // "sap_feedback"
SEEDS.CAPABILITY_INDEX   // "sap_cap_idx"
SEEDS.PROTOCOL_INDEX     // "sap_proto_idx"
SEEDS.GLOBAL             // "sap_global"
SEEDS.PLUGIN             // "sap_plugin"
SEEDS.MEMORY             // "sap_memory"
SEEDS.MEMORY_CHUNK       // "sap_mem_chunk"
SEEDS.VAULT              // "sap_vault"
SEEDS.SESSION            // "sap_session"
SEEDS.EPOCH              // "sap_epoch"
SEEDS.DELEGATE           // "sap_delegate"
SEEDS.TOOL               // "sap_tool"
SEEDS.CHECKPOINT         // "sap_checkpoint"
SEEDS.ESCROW             // "sap_escrow"
SEEDS.TOOL_CATEGORY      // "sap_tool_cat"
SEEDS.ATTESTATION        // "sap_attest"
SEEDS.LEDGER             // "sap_ledger"
SEEDS.LEDGER_PAGE        // "sap_page"
SEEDS.BUFFER             // "sap_buffer"
SEEDS.DIGEST             // "sap_digest"
```

---

## Mainnet Addresses

Pre-computed addresses for all singleton and well-known PDAs on **mainnet-beta**.  
Dynamic PDAs (per-agent, per-session, etc.) are derived at runtime using the seeds above.

### Program & Authority

| Name | Address | Notes |
|:-----|:--------|:------|
| **SAP v2 Program** | `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ` | Verified via OtterSec ([Solscan](https://solscan.io/account/SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ)) |
| **Upgrade Authority** | `GBLQznn1QMnx64zHXcDguP9yNW9ZfYCVdrY8eDovBvPk` | Protocol multisig |
| **IDL Account** | `ENs7L1NFuoP7dur8cqGGE6b98CQHfNeDZPWPSjRzhc4f` | `program-metadata` format, seeds: `["idl", program_id]` |
| **program-metadata Program** | `pmetaypqG6SiB47xMigYVMAkuHDWeSDXcv3zzDrJJvA` | Solana Foundation standard |

### Singleton PDAs

| Account | Address | Seeds | Bump |
|:--------|:--------|:------|:----:|
| **GlobalRegistry** | `9odFrYBBZq6UQC6aGyzMPNXWJQn55kMtfigzhLg6S6L5` | `["sap_global"]` | 255 |

### Tool Category Index PDAs

Each `ToolCategory` enum variant maps to a deterministic PDA. Seeds: `["sap_tool_cat", category_u8]`

| # | Category | Address | Bump |
|:--|:---------|:--------|:----:|
| 0 | Swap | `5H8yn9RuRgZWqkDiWbKNaCHzTMjqSpwbNQKMPLtUXx2G` | 252 |
| 1 | Lend | `5Lqqk6VtFWnYq3h4Ae4FuUAKnFzw1Nm1DaSdt2cjcTDj` | 254 |
| 2 | Stake | `kC8oAiVUcFMXEnmMNu1h2sdAc3dWKcwV5qVKRFYMmQD` | 255 |
| 3 | Nft | `2zNWR9J3znvGQ5J6xDfJyZkd12Gi66mjErRDkgPeKbyF` | 248 |
| 4 | Payment | `Eh7MwxJYWRN8bzAmY3ZPTRXYjWpWypokBf1STixu2dy9` | 255 |
| 5 | Data | `AwpVxehQUZCVTAJ9icZfS6oRbF66jNo32duXaL11B5df` | 252 |
| 6 | Governance | `2573WjZzV9QtbqtM6Z86YGivkk1kdvJa4gK3tZRQ2jkN` | 254 |
| 7 | Bridge | `664nyr6kBeeFiE1ij5gtdncNCVHrXqrk2uBhnKmUREvK` | 255 |
| 8 | Analytics | `4DFsiTZ6h6RoCZuUeMTpaoQguepnPUMJBLJuwwjKg5GL` | 255 |
| 9 | Custom | `3Nk5dvFWEyWPEArdG9cCdab6C6ym36mSWUSB8HzN35ZM` | 248 |

### SDK Access

```typescript
import {
  SAP_PROGRAM,
  SAP_UPGRADE_AUTHORITY,
  GLOBAL_REGISTRY_ADDRESS,
  IDL_ACCOUNT_ADDRESS,
  TOOL_CATEGORY_ADDRESSES,
} from "@oobe-protocol-labs/synapse-sap-sdk";

// Direct PublicKey access
console.log(GLOBAL_REGISTRY_ADDRESS.toBase58());
//=> "9odFrYBBZq6UQC6aGyzMPNXWJQn55kMtfigzhLg6S6L5"

console.log(TOOL_CATEGORY_ADDRESSES.Swap.toBase58());
//=> "5H8yn9RuRgZWqkDiWbKNaCHzTMjqSpwbNQKMPLtUXx2G"
```

---

## Account Relationship Diagram

```
                         ┌──────────────┐
                         │GlobalRegistry│  (singleton ... network-wide stats)
                         └──────┬───────┘
                                │ total_agents++
                                ▼
┌────────┐    owns    ┌──────────────┐    1:1    ┌──────────┐
│ Wallet │───────────►│ AgentAccount │──────────►│AgentStats│
└────────┘            └──────┬───────┘           └──────────┘
                             │
          ┌──────────────────┼──────────────────────────┐
          │                  │                          │
          ▼                  ▼                          ▼
   ┌────────────┐    ┌──────────────┐          ┌──────────────┐
   │MemoryVault │    │ToolDescriptor│          │EscrowAccount │
   │ (1:1)      │    │ (1:N)        │          │ (1:N)        │
   └─────┬──────┘    └──────┬───────┘          └──────────────┘
         │                  │
         ▼                  ▼
   ┌───────────┐    ┌──────────────────┐
   │SessionLdgr│    │ToolCategoryIndex │  (cross-agent)
   │ (1:N)     │    └──────────────────┘
   └─────┬─────┘
         │
    ┌────┼────┐
    │    │    │
    ▼    ▼    ▼
  Epoch Ledger Checkpoint    ← per-session memory + sync primitives
  Page  (+Page)
```

---

## Source Code Module Structure

```
programs/synapse-agent-sap/src/
│
├── lib.rs              ─ #[program] macro + 72 instruction dispatch entries
│                         Groups: Global, Agent, Feedback, Indexing,
│                         Plugin*, Memory*, Vault, Tools, Escrow,
│                         Attestation, Buffer*, Digest*, Ledger
│
├── state.rs            ─ 22 account structs + 7 enum types + 5 helper structs
│                         Enums:     TokenType, PluginType, SettlementMode,
│                                    ToolHttpMethod, ToolCategory
│                         Helpers:   Capability, PricingTier, VolumeCurveBreakpoint,
│                                    PluginRef, Settlement
│
├── events.rs           ─ 45 Anchor events across all layers
│                         Agent lifecycle (5), Feedback (3), Plugin (2)*,
│                         Memory (3)*, Vault (10), Tools (7), Escrow (5),
│                         Attestation (3), Indexing (3), Buffer (3)*,
│                         Digest (4)*, Ledger (4)
│
├── errors.rs           ─ 91 error codes (SapError enum)
│                         Validation (8), State (2), Feedback (4),
│                         Indexing (5), Plugin (1), Memory (3),
│                         Deep validation (20+), Vault (15+),
│                         Tools (10+), Escrow (12+), Attestation (5+),
│                         Buffer (5+)*, Digest (5+)*, Ledger (3+)
│
├── validator.rs        ─ 13 deep validation functions
│                         validate_name, validate_description,
│                         validate_agent_id, validate_capability_format,
│                         validate_capabilities, validate_volume_curve,
│                         validate_pricing_tier, validate_pricing_tiers,
│                         validate_protocols, validate_uri,
│                         validate_x402_endpoint,
│                         validate_register_payload, validate_update_payload
│
└── instructions/       ─ 13 instruction modules
    ├── mod.rs          ─ module declarations + re-exports
    ├── global.rs       ─ initialize_global (1 ix)
    ├── agent.rs        ─ register, update, deactivate, reactivate,
    │                     close, report_calls, update_reputation (7 ix)
    ├── feedback.rs     ─ give, update, revoke, close (4 ix)
    ├── indexing.rs     ─ init/add/remove/close for capability,
    │                     protocol, tool_category (12 ix)
    ├── vault.rs        ─ init_vault, open/close_session,
    │                     inscribe_memory, compact_inscribe,
    │                     close_vault, close_session_pda,
    │                     close_epoch_page, rotate_nonce,
    │                     add/revoke delegate,
    │                     inscribe_delegated (11 ix)
    ├── tools.rs        ─ publish, inscribe_schema, update,
    │                     deactivate, reactivate, close,
    │                     report_invocations,
    │                     create/close checkpoint (9 ix)
    ├── escrow.rs       ─ create, deposit, settle_calls,
    │                     withdraw, close, settle_batch (6 ix)
    ├── attestation.rs  ─ create, revoke, close (3 ix)
    ├── ledger.rs       ─ init, write, seal, close (4 ix)
    ├── plugin.rs*      ─ register, close (2 ix)
    ├── memory.rs*      ─ store, append_chunk, close_entry,
    │                     close_chunk (4 ix)
    ├── buffer.rs*      ─ create, append, close (3 ix)
    └── digest.rs*      ─ init, post, inscribe_to,
                          update_storage, close (5 ix)

    * = gated behind "legacy-memory" feature flag
```

---

## Constants & Limits

All values mirror the on-chain Rust constraints. The TypeScript SDK re-exports them from `@synapse-sap/sdk/constants`.

### Size Limits

| Constant | Value | Enforced By |
|:---------|:------|:------------|
| `MAX_NAME_LEN` | 64 bytes | `validate_name` |
| `MAX_DESC_LEN` | 256 bytes | `validate_description` |
| `MAX_URI_LEN` | 256 bytes | `validate_uri` |
| `MAX_AGENT_ID_LEN` | 128 bytes | `validate_agent_id` |
| `MAX_CAPABILITIES` | 10 | `validate_capabilities` |
| `MAX_PRICING_TIERS` | 5 | `validate_pricing_tiers` |
| `MAX_PROTOCOLS` | 5 | `validate_protocols` |
| `MAX_PLUGINS` | 5 | agent account constraint |
| `MAX_VOLUME_CURVE_POINTS` | 5 | `validate_volume_curve` |
| `MAX_TAG_LEN` | 32 bytes | feedback constraint |
| `MAX_AGENTS_PER_INDEX` | 100 | index account constraint |
| `MAX_TOOL_NAME_LEN` | 32 bytes | tool constraint |
| `MAX_TOOLS_PER_CATEGORY` | 100 | category index constraint |
| `MAX_ATTESTATION_TYPE_LEN` | 32 bytes | attestation constraint |

### Memory Limits

| Constant | Value | System |
|:---------|:------|:-------|
| `MAX_INSCRIPTION_SIZE` | 750 bytes | Vault (per fragment) |
| `INSCRIPTIONS_PER_EPOCH` | 1,000 | Vault (epoch pages) |
| `MAX_CHUNK_SIZE` | 900 bytes | Legacy MemoryChunk |
| `MAX_BUFFER_WRITE_SIZE` | 750 bytes | Legacy MemoryBuffer (per append) |
| `MAX_BUFFER_TOTAL_SIZE` | 10,000 bytes | Legacy MemoryBuffer (per page) |
| `RING_CAPACITY` | 4,096 bytes | MemoryLedger (ring buffer) |
| `MAX_LEDGER_WRITE_SIZE` | 750 bytes | MemoryLedger (per write) |
| `MAX_BATCH_SETTLEMENTS` | 10 | Escrow (batch settle) |

### Enum Values

**ToolCategory** (u8):

| Value | Name | Description |
|:------|:-----|:------------|
| 0 | Swap | Token swaps |
| 1 | Lend | Lending / borrowing |
| 2 | Stake | Staking / validators |
| 3 | Nft | NFT mint / trade |
| 4 | Payment | Payments / transfers |
| 5 | Data | Data queries / feeds |
| 6 | Governance | DAO / voting |
| 7 | Bridge | Cross-chain |
| 8 | Analytics | On-chain analytics |
| 9 | Custom | Uncategorised |

**ToolHttpMethod** (u8):

| Value | Name |
|:------|:-----|
| 0 | Get |
| 1 | Post |
| 2 | Put |
| 3 | Delete |
| 4 | Compound |

**PluginType** (u8) [Legacy]:

| Value | Name |
|:------|:-----|
| 0 | Memory |
| 1 | Validation |
| 2 | Delegation |
| 3 | Analytics |
| 4 | Governance |
| 5 | Custom |

**SettlementMode** (u8):

| Value | Name | Description |
|:------|:-----|:------------|
| 0 | Instant | Per-call on-chain transfer |
| 1 | Escrow | Pre-funded escrow PDA, draw per call |
| 2 | Batched | Off-chain accumulation, periodic settle |
| 3 | X402 | HTTP x402 protocol (default) |

**TokenType** (u8):

| Value | Name |
|:------|:-----|
| 0 | Sol |
| 1 | Usdc |
| 2 | Spl |

---

## Data Flow Patterns

### Agent Registration Flow

```
Client                        Program                        Solana
  │                              │                              │
  │  register_agent(name, ...)   │                              │
  │─────────────────────────────►│                              │
  │                              │  validate_register_payload() │
  │                              │  ────────────────────────►   │
  │                              │                              │
  │                              │  init AgentAccount PDA       │
  │                              │  init AgentStats PDA         │
  │                              │  GlobalRegistry.total_agents++
  │                              │  emit RegisteredEvent        │
  │                              │──────────────────────────────►│
  │          TX signature        │                              │
  │◄─────────────────────────────│                              │
```

### x402 Settlement Flow

```
Client           Agent (offchain)          Program              Agent Wallet
  │                    │                      │                      │
  │  deposit_escrow()  │                      │                      │
  │───────────────────────────────────────────►│                      │
  │                    │                      │  init/update          │
  │                    │                      │  EscrowAccount        │
  │                    │                      │                      │
  │  HTTP x402 call    │                      │                      │
  │───────────────────►│                      │                      │
  │    response        │                      │                      │
  │◄───────────────────│                      │                      │
  │                    │                      │                      │
  │                    │  settle_calls(n, hash)│                      │
  │                    │─────────────────────►│                      │
  │                    │                      │  escrow.balance -= n  │
  │                    │                      │  transfer → wallet   │
  │                    │                      │──────────────────────►│
  │                    │                      │  emit SettledEvent    │
  │                    │  TX signature         │                      │
  │                    │◄─────────────────────│                      │
```

### Memory Write Flow (MemoryLedger)

```
Client                         Program
  │                               │
  │  write_ledger(data, hash)     │
  │──────────────────────────────►│
  │                               │  sha256(data) == content_hash?
  │                               │  merkle = sha256(prev_root || hash)
  │                               │
  │                               │  ┌─ Ring Buffer ────────────────┐
  │                               │  │ if data fits: append         │
  │                               │  │ if not: drain oldest entries │
  │                               │  │ until space available        │
  │                               │  └──────────────────────────────┘
  │                               │
  │                               │  emit LedgerWriteEvent(data)
  │                               │  (permanent TX log, zero rent)
  │                               │
  │      TX signature             │
  │◄──────────────────────────────│
```

---

## Design Decisions

### Why PDAs Instead of Token Accounts?

Agent identity is not transferable. A PDA seeded by `["sap_agent", wallet]` guarantees exactly one agent per wallet and makes the identity non-transferable by construction ... there is no `transfer` instruction, and there never will be. This is deliberate: an agent's reputation, memory, and commercial relationships are bound to the wallet that created it.

### Why Four Memory Systems?

Different agents have different needs. A chat agent that needs instant message recall uses MemoryLedger. A compliance agent that needs tamper-proof audit trails uses Vault with epoch pagination. A lightweight agent that only needs proof of existence uses Digest. The protocol doesn't prescribe ... it provides primitives and recommends MemoryLedger as the default.

### Why Hashes Instead of Data in Tool Schemas?

Full JSON schemas can be kilobytes. Storing them in PDA accounts would cost significant rent. Instead, SAP stores SHA-256 hashes on-chain and inscribes the full schemas into transaction logs (zero rent, permanent). Anyone can verify that the schema data matches the on-chain hash. This pattern ... hash on-chain, data in TX logs ... appears throughout the protocol.

### Why `AgentStats` is Separate from `AgentAccount`?

The full `AgentAccount` is ~8 KB when fully populated (10 capabilities, 5 pricing tiers, 5 protocols, 5 plugins). The hot settlement path (`settle_calls`, `settle_batch`) only needs `total_calls_served` and `is_active`. By extracting these into a 106-byte `AgentStats` PDA, we reduce per-TX deserialization by ~76×.

### Why LedgerPages Have No Close Instruction?

Immutability by design. Once a ring buffer is sealed into a `LedgerPage`, it becomes a permanent, irrevocable on-chain record. Even the program authority cannot delete it. If the program is made non-upgradeable, pages are truly immutable forever. This is the price of immutability ... ~0.031 SOL per page, paid once.

---

## Security Considerations

### Constraint Enforcement

Every instruction uses Anchor's account constraint system:

- **`has_one`**: Verifies parent-child PDA relationships (e.g., vault → agent → wallet)
- **`seeds + bump`**: Ensures PDAs are derived deterministically ... no spoofing
- **`constraint`**: Custom runtime checks (e.g., `is_active == true`, `is_closed == false`)
- **`init_if_needed`**: Used sparingly (epoch pages only) to avoid DoS via reinitialization

### Self-Review Prevention

`FeedbackAccount` enforces `reviewer != agent.wallet` ... an agent cannot review itself. The constraint is checked at the Anchor level before any state mutation.

### Delegation Expiry

`VaultDelegate` supports optional expiry (`expires_at`). The `inscribe_memory_delegated` instruction checks `Clock::get().unix_timestamp < expires_at` at runtime. Expired delegates are functionally revoked even if the PDA still exists.

### Escrow Safety

The `EscrowAccount` locks `price_per_call` at creation ... the agent owner cannot change it after the fact. Clients can always withdraw their remaining balance. The `max_calls` field limits total exposure.

---

## Navigation

| | |
|:--|:--|
| **Previous** | [Overview](./README.md) |
| **Next** | [Instruction Reference](./02-instructions.md) |
