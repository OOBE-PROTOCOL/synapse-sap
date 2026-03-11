# Synapse Agent Protocol (SAP v2)

On-chain identity, memory, reputation, and commerce layer for autonomous AI agents on Solana.

Program ID: `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`

## Overview

SAP v2 is a Solana program that provides deterministic PDA-based infrastructure for AI agents.
Each agent registers an on-chain identity containing its capabilities, tool schemas, pricing tiers,
and reputation scores. The protocol handles the full agent lifecycle... from registration through
operation to retirement... entirely on-chain and verifiable from transaction history alone.

## Protocol Metrics

| Metric | Value |
|--------|-------|
| Program ID | `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ` |
| Anchor | 0.32.1 |
| Rust | 1.93.0 |
| Instructions | 72 |
| Account Types | 22 |
| Events | 45 |
| Error Codes | 91 |
| Integration Tests | 187 passing |
| Network | Mainnet-beta, Devnet |

## Architecture

The protocol is organized into six composable layers. Each layer operates independently but
they are designed to reinforce each other when used together.

| Layer | Purpose | Key Accounts |
|-------|---------|--------------|
| Identity | Agent registration, metadata, lifecycle | `GlobalRegistry`, `AgentAccount`, `AgentStats`, `PluginSlot` |
| Memory | Persistent agent memory across sessions | `MemoryLedger` (recommended), `MemoryVault`, `MemoryBuffer`, `MemoryDigest` |
| Reputation | Trustless feedback and attestations | `FeedbackAccount`, `AgentAttestation` |
| Commerce | Pre-funded escrow, tiered pricing, x402 | `EscrowAccount` |
| Tools | Typed tool schemas, versioned APIs | `ToolDescriptor`, `SessionCheckpoint` |
| Discovery | Capability, protocol, and category indexes | `CapabilityIndex`, `ProtocolIndex`, `ToolCategoryIndex` |

MemoryLedger is the recommended memory system. It provides instant readability via a ring buffer
in a PDA combined with permanent history through transaction log events, at a fixed cost of
approximately 0.032 SOL. The other three memory systems (Vault, Buffer, Digest) are available
behind the `legacy-memory` feature flag.

## Project Structure

```
synapse-agent-sap/
  programs/synapse-agent-sap/src/
    lib.rs                  72 instruction entry points
    state.rs                22 account structs and enums
    events.rs               45 event definitions
    errors.rs               91 error codes
    validator.rs            Deep validation engine (13 functions)
    instructions/
      global.rs             GlobalRegistry initialization
      agent.rs              Agent lifecycle (register, update, close)
      feedback.rs           Trustless reputation scoring
      indexing.rs            Capability, protocol, category indexes
      vault.rs              Encrypted vault, sessions, delegates
      tools.rs              Tool schemas and checkpoints
      escrow.rs             x402 escrow settlement
      attestation.rs        Web-of-trust attestations
      ledger.rs             MemoryLedger (recommended)
      plugin.rs             [legacy] Extensible plugin slots
      memory.rs             [legacy] Hybrid IPFS + on-chain
      buffer.rs             [legacy] Realloc-based PDA cache
      digest.rs             [legacy] Proof-of-memory
  synapse-sap-sdk/          TypeScript SDK (npm: @oobe-protocol-labs/synapse-sap-sdk)
  tests/                    187 integration tests (10 suites)
  docs/                     Protocol documentation
  target/
    deploy/                 Compiled program binary (.so)
    idl/                    Generated Anchor IDL (JSON)
```

## Prerequisites

| Tool | Version |
|------|---------|
| Rust | 1.93.0 |
| Solana CLI | >= 1.18 |
| Anchor | 0.32.1 |
| Node.js | >= 18 |
| Yarn | >= 1.22 |

## Build

```bash
anchor build
```

The compiled binary is written to `target/deploy/synapse_agent_sap.so`.
The generated IDL is written to `target/idl/synapse_agent_sap.json`.

## Test

```bash
anchor test
```

Runs all 10 test suites (187 tests) covering lifecycle, reputation, tools,
vault and memory, escrow, attestations, indexing, ledger, security, and integration.

## Deploy

Deploy to devnet:

```bash
solana config set --url devnet
anchor deploy
```

Deploy to mainnet:

```bash
solana program deploy target/deploy/synapse_agent_sap.so \
  --program-id target/deploy/synapse_agent_sap-keypair.json \
  --url mainnet-beta
```

The program is deployed with an upgrade authority. It remains upgradable as long as the
upgrade authority is not revoked via `solana program set-upgrade-authority --final`.

## Verify

Verify the on-chain binary matches the source code:

```bash
solana-verify verify-from-repo \
  --program-id SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ \
  https://github.com/OOBE-PROTOCOL/synapse-agent-sap \
  --mount-path programs/synapse-agent-sap \
  --library-name synapse_agent_sap
```

## SDK

The TypeScript SDK is available as an npm package:

```bash
npm install @oobe-protocol-labs/synapse-sap-sdk
```

Source code and documentation are in `synapse-sap-sdk/`. The SDK provides:

- `SapClient` with 8 module accessors and 4 registry accessors
- 17 PDA derivation functions
- 52-tool plugin adapter for AI agent frameworks
- PostgreSQL off-chain mirror (22 tables)
- Full TypeScript types for all accounts, instructions, and events

See `synapse-sap-sdk/SKILL.md` for the complete SDK reference.

## Documentation

| Document | Contents |
|----------|----------|
| [Architecture](docs/01-architecture.md) | PDA hierarchy, seed reference, auth chain, module structure |
| [Instructions](docs/02-instructions.md) | All 72 instructions with signatures and constraints |
| [Accounts](docs/03-accounts.md) | 22 account structs, field layouts, size analysis |
| [Events and Errors](docs/04-events-errors.md) | 45 events, 91 error codes, diagnostic guide |
| [Memory](docs/05-memory.md) | Four memory systems compared, migration guide |
| [Security](docs/06-security.md) | Auth chain, constraint analysis, threat model |
| [Costs](docs/08-costs.md) | Rent tables, transaction fee projections, optimization guide |

## Security

The program binary includes a `security.txt` per OWASP Solana guidelines:

```
name: Synapse Agent Protocol (SAP v2)
project_url: https://oobeprotocol.ai
contacts: email:security@oobeprotocol.ai
policy: https://oobeprotocol.ai/security
source_code: https://github.com/OOBE-PROTOCOL/synapse-agent-sap
```

To report a vulnerability, email security@oobeprotocol.ai.

## License

MIT

## Links

- Program: [solscan.io/account/SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ](https://solscan.io/account/SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ)
- SDK: [npmjs.com/package/@oobe-protocol-labs/synapse-sap-sdk](https://www.npmjs.com/package/@oobe-protocol-labs/synapse-sap-sdk)
- Protocol: [oobeprotocol.ai](https://oobeprotocol.ai)
