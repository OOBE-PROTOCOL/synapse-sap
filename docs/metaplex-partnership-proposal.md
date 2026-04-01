# Synapse SAP × Metaplex — Technical Partnership Proposal

> **Date**: March 2026
> **From**: OOBE Protocol Labs (Synapse Agent Protocol)
> **To**: Metaplex Foundation
> **Status**: Draft — Technical Proposal

---

## Executive Summary

Metaplex Agent Kit provides **NFT-based identity** for autonomous agents on Solana — a "passport" that answers *who is this agent and where can I reach it*. Synapse Agent Protocol (SAP) provides the **operational infrastructure** — payments, memory, reputation, tool schemas, and discovery indexing — that answers *how do I pay this agent, can I trust it, what tools does it expose, and what's our conversation history*.

These two protocols are **complementary, not competing**. Together they create the most complete agent infrastructure stack on Solana: Metaplex handles identity and asset ownership, SAP handles everything that happens after discovery.

This document outlines the technical architecture differences and five concrete integration proposals.

---

## Protocol Comparison

### Metaplex Agent Kit (MPL Agent Registry)

| Layer | Implementation | Details |
|-------|---------------|---------|
| **Identity** | MPL Core Asset + `AgentIdentity` plugin | PDA derived from asset pubkey; one identity per asset |
| **Metadata** | Off-chain ERC-8004 JSON (Arweave) | Name, description, services, endpoints (A2A/MCP/web) |
| **Execution** | Executive delegation via lifecycle hooks | `ExecutiveProfile` PDA per wallet; `ExecutionDelegateRecord` per (agent, executive) pair |
| **Wallet** | Asset Signer PDA (`findAssetSignerPda`) | No private key — can hold SOL/tokens; signs via Core `Execute` instruction |
| **Discovery** | Collection enumeration | No on-chain capability/protocol/category indexes |
| **Trust** | Declarative `supportedTrust` field in JSON | No on-chain feedback, no computed reputation score |
| **Payments** | None | No escrow, no micropayment protocol |
| **Memory** | None | No conversation storage, no encrypted vault |
| **Tool Registry** | None | No on-chain tool descriptors or schemas |

### Synapse Agent Protocol (SAP v2)

| Layer | Implementation | Details |
|-------|---------------|---------|
| **Identity** | `AgentAccount` PDA derived from wallet | Capabilities, pricing, protocols — all on-chain |
| **Metadata** | 100% on-chain | Endpoint descriptors, manifests, tool schemas in TX logs |
| **Execution** | Direct wallet signing + `VaultDelegate` | Permission bitmask (inscribe/close/open), expiry, auth chain |
| **Wallet** | Owner wallet signs directly | Standard Solana keypair |
| **Discovery** | Full on-chain indexing | `CapabilityIndex`, `ProtocolIndex`, `ToolCategoryIndex` PDAs; `DiscoveryRegistry` with multi-param queries |
| **Trust** | Trustless on-chain feedback + attestation web-of-trust | Reputation formula: `(sum × 10) / count` → 0–10000; attestations from any wallet (self-attestation blocked) |
| **Payments** | x402 escrow with volume pricing curves | SOL + SPL token; single and batch settlement; priority fees |
| **Memory** | Vault (encrypted TX logs, epoch system) + Ledger (4 KB ring buffer) | Merkle accumulator, multi-fragment, nonce rotation, sealed immutable pages |
| **Tool Registry** | Full on-chain descriptors + schema inscription | JSON Schema stored permanently in TX logs (zero rent); input/output/description types |

## Integration Proposals

### Proposal 1 — MPL Core Asset as SAP Identity Anchor

**Problem**: SAP agents are wallet-based.. functional but not tradeable or composable as NFTs. Metaplex agents are NFTs but lack operational infrastructure.

**Solution**: Link an `AgentAccount` (SAP) to an MPL Core Asset (Metaplex) bidirectionally.

```
MPL Core Asset                          SAP AgentAccount
┌─────────────────────┐               ┌─────────────────────┐
│ AgentIdentity plugin│──────────────→│ mpl_asset: Pubkey   │
│ uri: registration   │               │ (new field linking  │
│   JSON              │               │  to Metaplex asset) │
│                     │               │                     │
│ services:           │               │ capabilities: [...] │
│   - A2A endpoint    │               │ pricing: [...]      │
│   - MCP endpoint    │               │ tools: [...]        │
│   - x402 endpoint ──┼──────────────→│ escrow, memory,     │
│                     │               │ reputation, schemas │
└─────────────────────┘               └─────────────────────┘
```

**Implementation**:
1. SAP adds an optional `mpl_asset` field to `AgentAccount` (32 bytes, `Pubkey::default()` if unused)
2. Registration accepts an MPL Core asset pubkey and verifies the signer owns it
3. The Metaplex `agentRegistrationUri` JSON includes a `services` entry pointing to the SAP agent's x402 endpoint
4. A helper function derives both the SAP `AgentAccount` PDA and the Metaplex `AgentIdentity` PDA from the same identity, enabling cross-protocol lookups

**Benefits**:
- Metaplex agents gain payments, memory, reputation, tool schemas
- SAP agents gain NFT composability, tradeability, Asset Signer wallet
- Agent identity becomes transferable (transfer the MPL Core asset → new owner inherits the SAP agent)

**Effort**: ~1/2 week (SAP program update + SDK wrapper + CLI params)

---
### Proposal 3 — Executive ↔ VaultDelegate Bridge

**Problem**: Metaplex has Executives (operate the agent), SAP has VaultDelegates (operate the memory vault). An operator authorized on one protocol should be authorized on both.

**Solution**: Atomic delegation — a single transaction that creates both a Metaplex `ExecutionDelegateRecord` and a SAP `VaultDelegate`.

```
Agent Owner
    │
    ├── delegateExecutionV1(executive)     →  Metaplex ExecutionDelegateRecord
    │                                          PDA: ["execution_delegate", executive_profile, agent_asset]
    │
    └── addVaultDelegate(executive, 7, 0)  →  SAP VaultDelegate
                                               PDA: ["sap_delegate", vault_pda, executive_pubkey]
                                               permissions: ALL (7), expires: never
```

**Implementation**:
1. SDK helper: `delegateBoth(agentAsset, executivePubkey, permissions, expiresAt)` — builds both instructions into a single transaction
2. SDK helper: `revokeBoth(agentAsset, executivePubkey)` — revokes on both protocols atomically
3. Verification: `isDelegatedOnBoth(agentAsset, executivePubkey)` — checks both delegation records exist

**Effort**: ~2 days (SDK helper, no program changes — composable at TX level)

---

### Proposal 4 — Unified Discovery Layer

**Problem**: Metaplex has no on-chain discovery indexing. Finding agents requires enumerating entire collections. SAP has `CapabilityIndex`, `ProtocolIndex`, and `ToolCategoryIndex` but doesn't know about MPL Core Assets.

**Solution**: A unified discovery indexer that reads both protocols and provides a single query API.

```
Consumer Query: "Find agents with capability 'jupiter:swap', score > 8000, that are MPL Core Assets"

                    ┌───────────────────────────┐
                    │   Unified Discovery API    │
                    │                            │
                    │  findAgents({              │
                    │    capability: 'jupiter:swap', │
                    │    minScore: 8000,         │
                    │    requireMplAsset: true,   │
                    │  })                         │
                    └─────────┬─────────────────┘
                              │
              ┌───────────────┴───────────────┐
              │                               │
    ┌─────────▼──────────┐         ┌──────────▼──────────┐
    │   SAP Discovery     │         │  Metaplex Indexer    │
    │                     │         │                      │
    │ CapabilityIndex PDA │         │ Collection scan      │
    │ → agent PDAs with   │         │ → AgentIdentity      │
    │   capability match  │         │   plugin check       │
    │ → reputationScore   │         │ → Asset ownership    │
    │ → tools + schemas   │         │ → Executive status   │
    └─────────────────────┘         └──────────────────────┘
```

**Implementation**:
1. `UnifiedDiscoveryRegistry` class that wraps both `SAP DiscoveryRegistry` and Metaplex asset queries
2. Methods: `findAgents(filters)`, `getUnifiedProfile(wallet|asset)`, `getNetworkOverview()`
3. The unified profile merges SAP data (reputation, tools, pricing, memory stats) with Metaplex data (asset ownership, executive delegation, lifecycle hooks, collection membership)

**Effort**: ~1 week (new SDK module, no program changes)

---

## Proposed Roadmap

| Phase | Deliverable | Effort | Dependencies |
|-------|------------|--------|-------------|
| **Phase 1** | Proposal 2 — SDK helpers for dual registration | 3 days | None |
| **Phase 4** | Proposal 4 — Unified Discovery Registry | 1 week | Phase 1 |
| **Phase 5** | Proposal 1 — On-chain identity link (program update) | 1 week | Metaplex coordination |

Phases 1–4 require **zero program changes on either side** — they're pure SDK composition. Phase 5 requires a SAP program update (adding `mpl_asset` field) and Metaplex coordination.

---

## Technical Contact

- **Protocol**: Synapse Agent Protocol v2
- **Program**: `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`
- **SDK**: `@oobe-protocol-labs/synapse-sap-sdk@0.6.2` (npm)
- **SAP**: https://github.com/OOBE-PROTOCOL/synapse-sap
- **GitHub**: https://github.com/OOBE-PROTOCOL/synapse-sap-sdk
- **Docs**: https://github.com/OOBE-PROTOCOL/synapse-sap-sdk/tree/main/docs
