# Changelog

All notable changes to the Synapse Agent Protocol program will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] ... 2026-03-11

### Added

- Identity layer: `GlobalRegistry`, `AgentAccount`, `AgentStats`, `PluginSlot`, `VaultDelegate`.
- Memory layer: `MemoryLedger` (recommended), `MemoryVault`, `MemoryBuffer`, `MemoryDigest`.
- Reputation layer: `FeedbackAccount` (0...1000 scores), `AgentAttestation` (web-of-trust).
- Commerce layer: `EscrowAccount` with volume curves, batch settlement, SPL token support.
- Tool layer: `ToolDescriptor` with typed schemas, categories, checkpoints.
- Discovery layer: `CapabilityIndex`, `ProtocolIndex`, `ToolCategoryIndex`.
- 72 instructions across 13 instruction modules.
- 22 account types with deterministic PDA derivation.
- 45 events for off-chain indexing.
- 91 custom error codes with diagnostic context.
- Deep validation engine (13 functions) for input sanitization.
- `security.txt` embedded in the program binary per OWASP Solana guidelines.
- Legacy memory systems gated behind `legacy-memory` feature flag.
- 187 integration tests across 10 suites.
- Deployed to mainnet-beta and devnet at `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`.
- IDL uploaded on-chain at `7m5Zdb3whpZbFDHSbKBEytApWSJuDmykhbyfsD8StBVe`.
- TypeScript SDK published as `@oobe-protocol-labs/synapse-sap-sdk` on npm.

[Unreleased]: https://github.com/OOBE-PROTOCOL/synapse-agent-sap/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/OOBE-PROTOCOL/synapse-agent-sap/releases/tag/v0.1.0
