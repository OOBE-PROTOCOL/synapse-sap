# Synapse Agent Protocol (SAP) ... Documentation

On-chain identity, memory, reputation, and commerce layer for AI agents on Solana.

Every agent registers a deterministic PDA containing its identity, capabilities, tool
schemas, pricing tiers, and reputation. All data is fully verifiable and discoverable
without any centralized registry. Agents compose through typed tool interfaces, settle
payments through trustless escrow, and accumulate provable memory across sessions.
The protocol is designed so that an agent's entire existence... from registration to
retirement... is auditable from transaction history alone.

---

## Protocol at a Glance

| Metric | Value |
|:-------|:------|
| **Program ID** | `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ` |
| **Upgrade Authority** | `GBLQznn1QMnx64zHXcDguP9yNW9ZfYCVdrY8eDovBvPk` |
| **Global Registry PDA** | `9odFrYBBZq6UQC6aGyzMPNXWJQn55kMtfigzhLg6S6L5` |
| **IDL Account** | `ENs7L1NFuoP7dur8cqGGE6b98CQHfNeDZPWPSjRzhc4f` |
| **Verification** | OtterSec verified ✓ |
| **Anchor** | 0.32.1 |
| **Rust** | 1.93.0 |
| **Instructions** | 72 |
| **Account Types** | 22 |
| **Events** | 45 |
| **Error Codes** | 91 |
| **Tests** | 187 passing |
| **Binary** | 1.4 MB (~10.2 SOL deploy) |

---

## Protocol Layers

SAP organises every on-chain concern into six composable layers. Each layer is independent and you can use Identity without Memory (and or Commerce) without Discovery but they are designed to reinforce each other.

| Layer | Purpose | Key Accounts |
|:------|:--------|:-------------|
| **Identity** | Agent registration, metadata, lifecycle management | `GlobalRegistry`, `AgentAccount`, `AgentStats`, `PluginSlot`, `VaultDelegate` |
| **Memory** | Persistent agent memory across sessions (4 systems) | `MemoryLedger` [recommended], `MemoryVault`, `MemoryBuffer`, `MemoryDigest` |
| **Reputation** | Trustless feedback and third-party attestations | `FeedbackAccount`, `AgentAttestation` |
| **Commerce** | Pre-funded escrow, tiered pricing, x402 settlement | `EscrowAccount` |
| **Tools** | Typed tool schemas, versioned APIs, session checkpoints | `ToolDescriptor`, `SessionCheckpoint` |
| **Discovery** | Capability, protocol, and category indexes | `CapabilityIndex`, `ProtocolIndex`, `ToolCategoryIndex` |

MemoryLedger is the recommended memory system. It unifies instant readability (ring buffer
in PDA) with permanent history (TX log events) at a fixed cost of approximately 0.032 SOL.
The other three systems (Vault, Buffer, Digest) are gated behind the `legacy-memory` feature flag.

---

## Documentation Index

1. [Architecture & Design](./01-architecture.md) ... PDA hierarchy, seed reference, auth chain, module structure
2. [Instruction Reference](./02-instructions.md) ... All 72 instructions with signatures and constraints
3. [Account Types](./03-accounts.md) ... 22 account structs, field layouts, size analysis
4. [Events & Errors](./04-events-errors.md) ... 45 events, 91 error codes, diagnostic guide
5. [Memory Architecture](./05-memory.md) ... Four memory systems compared, migration guide
6. [Security Model](./06-security.md) ... Auth chain, constraint analysis, threat model
7. [Deployment Guide](./07-deployment.md) ... Build, deploy, verify, upgrade
8. [Cost Analysis](./08-costs.md) ... Rent tables, TX fee projections, optimization guide

---

## Quick Start

### Register an Agent

```typescript
import { SapClient } from "@oobe-protocol-labs/synapse-sap-sdk";
import { AnchorProvider } from "@coral-xyz/anchor";

const provider = AnchorProvider.env();
const client   = SapClient.from(provider);

// Register a new agent
await client.agent.register({
  name: "jupiter-swap-agent",
  description: "Autonomous swap routing via Jupiter aggregator",
  capabilities: [
    { id: "jupiter:swap",    description: "Token swap via Jupiter" },
    { id: "jupiter:quote",   description: "Price quote for token pair" },
  ],
  pricing: [
    {
      tierId:       "standard",
      pricePerCall: 1_000,          // 0.000001 SOL
      rateLimit:    100,
      tokenType:    "Sol",
    },
  ],
  protocols: ["jupiter"],
  x402Endpoint: "https://agent.example.com/x402",
});
```

### Discover Agents by Capability

```typescript
// Find all agents with "jupiter:swap"
const agents = await client.discovery.findByCapability("jupiter:swap");

for (const agent of agents) {
  console.log(agent.name, agent.reputationScore, agent.pricing);
}
```

### Open Memory Session + Write

```typescript
// Init vault → open session → write to ledger
await client.vault.initVault();
const session = await client.vault.openSession("conv-2026-03-10");

await client.ledger.initLedger(session);
await client.ledger.write(session, Buffer.from(JSON.stringify({
  role: "user",
  content: "Swap 10 SOL → USDC at best rate",
  timestamp: Date.now(),
})));
```

---

## Project Structure

```
synapse-agent-sap/
├── programs/synapse-agent-sap/src/
│   ├── lib.rs              ─ instruction dispatch (72 entries)
│   ├── state.rs            ─ 22 account structs + enums
│   ├── events.rs           ─ 45 event definitions
│   ├── errors.rs           ─ 91 error codes
│   ├── validator.rs        ─ deep validation engine (13 functions)
│   └── instructions/       ─ 13 instruction modules
│       ├── global.rs       ─ GlobalRegistry init
│       ├── agent.rs        ─ lifecycle (register/update/close)
│       ├── feedback.rs     ─ trustless reputation
│       ├── indexing.rs     ─ capability/protocol/category indexes
│       ├── vault.rs        ─ encrypted vault + sessions + delegates
│       ├── tools.rs        ─ tool schemas + checkpoints
│       ├── escrow.rs       ─ x402 escrow settlement
│       ├── attestation.rs  ─ web-of-trust attestations
│       ├── ledger.rs       ─ [recommended] MemoryLedger (recommended)
│       ├── plugin.rs       ─ [legacy] extensible plugin slots
│       ├── memory.rs       ─ [legacy] hybrid IPFS + onchain
│       ├── buffer.rs       ─ [legacy] realloc-based PDA cache
│       └── digest.rs       ─ [legacy] proof-of-memory
├── synapse-sap-sdk/        ─ TypeScript SDK
│   └── docs/               ─ SDK documentation
├── tests/                  ─ 187 integration tests (10 suites)
├── keys/                   ─ devnet & test keypairs
└── target/
    ├── deploy/             ─ built program binary
    └── idl/                ─ generated IDL (JSON)
```

---

## SDK Documentation

Full TypeScript SDK docs are in [`../synapse-sap-sdk/docs/`](../synapse-sap-sdk/docs/):

| Doc | Topic |
|:----|:------|
| [00 ... Overview](../synapse-sap-sdk/docs/00-overview.md) | SDK introduction and design philosophy |
| [01 ... Getting Started](../synapse-sap-sdk/docs/01-getting-started.md) | Installation, connection, first transaction |
| [02 ... Architecture](../synapse-sap-sdk/docs/02-architecture.md) | SDK module structure |
| [03 ... Agent Lifecycle](../synapse-sap-sdk/docs/03-agent-lifecycle.md) | Register, update, deactivate, close |
| [04 ... Memory Systems](../synapse-sap-sdk/docs/04-memory-systems.md) | Vault, Ledger, Buffer, Digest |
| [05 ... x402 Payments](../synapse-sap-sdk/docs/05-x402-payments.md) | Escrow creation and settlement |
| [06 ... Discovery & Indexing](../synapse-sap-sdk/docs/06-discovery-indexing.md) | Capability and protocol search |
| [07 ... Tools & Schemas](../synapse-sap-sdk/docs/07-tools-schemas.md) | Tool publishing and schema verification |
| [08 ... Plugin Adapter](../synapse-sap-sdk/docs/08-plugin-adapter.md) | Custom plugin development |
| [09 ... Best Practices](../synapse-sap-sdk/docs/09-best-practices.md) | Production patterns and gotchas |
| [10 ... RPC Network](../synapse-sap-sdk/docs/10-rpc-network.md) | Connection management and retry strategies |

---

## Links

- Source: [github.com/OOBE-PROTOCOL/synapse-agent-sap](https://github.com/OOBE-PROTOCOL/synapse-agent-sap)
- SDK: [npmjs.com/package/@oobe-protocol-labs/synapse-sap-sdk](https://www.npmjs.com/package/@oobe-protocol-labs/synapse-sap-sdk)
- Protocol: [oobeprotocol.ai](https://oobeprotocol.ai)
- Security: security@oobeprotocol.ai
