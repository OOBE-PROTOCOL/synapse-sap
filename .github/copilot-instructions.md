# Synapse Agent SAP — Copilot Instructions

## Project Overview
Solana Agent Protocol (SAP) — an Anchor 0.32 on-chain program (`SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`) that manages decentralized AI agent lifecycle: registration, reputation, tools, vaults, escrows, disputes, staking, subscriptions, and indexing. Already live on mainnet with ~8 registered agents.

## Architecture
- **Framework**: Anchor 0.32.1, Solana 2.x
- **Program**: `programs/synapse-agent-sap/src/` — `lib.rs` (dispatch), `state.rs` (accounts), `errors.rs`, `events.rs`, `validator.rs`, `instructions/` (modules)
- **SDK**: `synapse-sap-sdk/` — TypeScript client, CLI, plugin system
- **Tests**: `tests/` — Mocha/Anchor integration tests

## Code Style
- Rust strict mode, no `unsafe` unless justified
- Anchor constraints over manual checks
- Checks-effects-interactions ordering
- PDA seeds: `["sap_<type>", ...keys]`
- Events on every state change
- All SDK calls server-side only (see Explorer instructions)

## Skills
This project has 9 specialized skills in `.github/skills/`:

| Skill | Use When |
|-------|----------|
| `anchor-architect` | Designing instructions, accounts, PDA schemas |
| `anchor-zero-copy` | Large accounts, compute-sensitive paths |
| `solana-security-review` | Pre-deploy audits, code review, trust verification |
| `solana-compute-optimizer` | Reducing CU, hot-path optimization |
| `solana-pda-designer` | PDA seed design, collision analysis |
| `solana-cpi-designer` | Cross-program invocations, authority propagation |
| `solana-test-author` | Writing meaningful tests (not smoke tests) |
| `solana-rpc-infra` | Account queryability, indexing, Geyser integration |
| `simplify-before-final` | Meta-skill: simplification pass on every output |

### Skill Pipeline
For any code change, apply skills in order:
1. **Design**: `anchor-architect` + `solana-pda-designer`
2. **Implement**: `solana-cpi-designer` + `solana-compute-optimizer`
3. **Review**: `solana-security-review`
4. **Test**: `solana-test-author`
5. **Simplify**: `simplify-before-final` (always last)

## Critical Constraints (Mainnet)
- **8 live agents** — migrations must be backward-compatible
- **No breaking PDA changes** — existing accounts must remain derivable
- **Upgrade authority** — program is upgradeable, use `anchor upgrade`
- **IDL compatibility** — new instructions are additive only
- **Fund safety** — escrow/vault lamport math must be exact
