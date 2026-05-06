# Changelog

All notable changes to the Synapse Agent Protocol program will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] — 2026-04-29 — Hardening Release

> **Security & trust release.** All findings from the v0.10 internal audit
> applied. Backward-compatible for *existing* on-chain accounts (PDA seeds
> unchanged) but **breaking for clients** that call `settle_calls`,
> `settle_calls_v2`, `settle_batch`, `create_escrow`, `create_escrow_v2`,
> or `add_vault_delegate`. SDK consumers must upgrade to
> `@oobe-protocol-labs/synapse-sap-sdk@0.10.0`.

### Added — Anti-Replay Settlement Receipt (C1 fix)

- New account `SettlementReceipt` (PDA `["sap_recv", escrow, key]`) created
  on every `settle_calls` / `settle_calls_v2` / `settle_batch`. Replays of
  the same `service_hash` (or batch root) are now rejected at the
  account-init layer ("account already in use").
- New errors: `InvalidReceiptProof`, `DuplicateServiceHash`,
  `PaymentTokenNotAllowed`, `AgentStakeRequired`, `DelegateExpiryInvalid`,
  `VolumeCurveNotDescending`, `StakeBelowMinimum`.

### Added — Stake Gate on Escrow Creation

- `create_escrow` / `create_escrow_v2` now require an initialized
  `AgentStake` PDA with `staked_amount >= MIN_STAKE` (0.1 SOL). Agents
  must call `init_stake` + fund collateral before accepting any client.

### Added — Payment Token Allowlist

- `create_escrow` / `create_escrow_v2` only accept `token_mint`:
  - `None` (native SOL), or
  - `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` (USDC mainnet), or
  - `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` (USDC devnet).
- Arbitrary SPL mints are rejected with `PaymentTokenNotAllowed`.

### Added — Batch Settlement Root

- `settle_batch` now takes an extra `batch_root: [u8; 32]` argument and
  verifies it equals `sha256(s_0 || s_1 || ... || s_{N-1})` of the
  in-batch service hashes. Mismatch → `InvalidReceiptProof`.
- Duplicates within the same batch → `DuplicateServiceHash`.

### Changed — Validator Hardening

- `validator::validate_volume_curve` now enforces strictly
  non-increasing `price_per_call` across breakpoints (M1).
- `add_vault_delegate` caps `expires_at - now <= 365 days` and rejects
  past timestamps (H2).
- `feedback::update_reputation` uses `checked_mul` / `checked_add` on the
  rolling-average path (H3).
- All escrow lamport math switched to `checked_add` / `checked_sub` and
  follows checks-effects-interactions ordering.

### Changed — Account Layout (additive only)

- `EscrowAccount`: no field changes — receipt PDAs are external accounts.
- `AgentStake`: unchanged (introduced in v0.1.0, now mandatory).

### Migration Notes

- Existing escrows continue to function — no migration needed for
  in-flight state.
- 8 live mainnet agents must call `init_stake` + `deposit_stake` before
  they can accept new escrows. Old escrows already opened against them
  remain settleable (the stake check fires only at *creation* time).
- SDK call sites must add `agentStake` (V1/V2 create), `settlementReceipt`
  + `systemProgram` (settle), and pass `batchRoot` (settleBatch).
- **Permanent collateral floor** (audit pass #2 — finding R1):
  `request_unstake` now requires the post-unstake balance to remain
  `>= MIN_STAKE`. To fully exit, agents will need a future `close_stake`
  instruction (planned post-v0.2.0) gated on agent deactivation + zero
  active escrows. This blocks the bypass where an agent stakes the
  minimum, opens escrows, then drains stake to zero before disputes can
  slash.

### Known limitations

- **R2 (Low):** `auto_resolve_dispute` slashes only when the agent's
  `AgentStake` PDA is supplied via `remaining_accounts`. A malicious
  caller can omit it and bypass slashing. Workaround: dispute resolvers
  (Synapse indexer, watchtowers) MUST always include the stake PDA.
  A deterministic fetch (without `remaining_accounts`) is planned for
  v0.2.1.

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
