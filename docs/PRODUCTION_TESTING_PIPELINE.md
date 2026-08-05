# SAP Production Testing Pipeline

This document is the release gate for SAP program and SDK changes. The goal is
to prevent stale IDLs, broken SDK account layouts, hidden legacy paths, and
unverified economic flows from reaching devnet or mainnet.

## Release Rule

Mainnet is a **NO-GO** unless all mandatory gates pass:

1. Local artifact gates
2. Program and SDK tests
3. Devnet runtime smoke tests
4. On-chain IDL / Program Metadata checks
5. Manual upgrade authority and treasury review

Do not treat an SDK publish, local build, or successful deploy as sufficient by
itself. SAP has multiple public surfaces: binary, Program Metadata IDL, legacy
Anchor IDL accounts, embedded SDK IDLs, skills, docs, and CLI recipes.

## GitHub Actions Gates

The repository exposes these release gates in GitHub Actions:

- `SAP Program CI`: runs on PRs, pushes, and release tags. It installs the
  pinned Agave/Anchor toolchain, verifies IDLs, builds the program, starts a
  local `solana-test-validator`, and runs the full Anchor TypeScript suite.
- `SAP Program Release Gate`: runs manually or on release tags. It builds the
  program and uploads the `.so`, IDL, generated types, and SHA256 checksums.
- `SAP Devnet Preflight`: manual read-only workflow. It audits devnet Program
  Metadata and ProgramData without requiring any wallet secret.
- `SAP Mainnet Preflight`: manual read-only workflow. It builds the local
  binary, checks ProgramData, authority, rent reserve, SDK version, and mainnet
  Program Metadata IDL.

GitHub Actions must not store SAP signer keypairs. Devnet SOL/USDC smoke tests
remain local/manual release gates because they sign real transactions:

```bash
yarn smoke:devnet:sol
yarn smoke:devnet:usdc
```

The `synapse-sap-sdk` subrepo has its own Actions:

- `SAP SDK CI`: lint, tests, build, CLI build, tarball pack, clean consumer
  CJS/ESM import checks, and secret-pattern checks.
- `SAP SDK Release Pack`: manual or tag-triggered tarball generation for SDK
  and CLI, with SHA256 checksums. It does not publish to npm.

## Toolchain

Pinned production stack:

- Anchor `1.x` for the SAP program and generated IDL.
- Agave / Solana CLI `3.1.10` for Anchor 1 production deployment tooling.
- Anchor `0.32.1` only for pre-1.0 legacy IDL migration/closure tasks.
- TypeScript SDK with embedded IDL as the client source of truth.

Do not rewrite live SAP program modules in Pinocchio, Steel, or native Rust
until after the current migration is stable. Those frameworks can be evaluated
later for small hot-path modules after CU benchmarks prove the value.

## Mandatory Local Gates

Run from the repository root:

```bash
yarn verify:idl
cargo fmt --check
yarn lint
anchor build
yarn run ts-mocha -p ./tsconfig.json -t 1000000 "tests/**/*.ts"
```

Run from `synapse-sap-sdk/`:

```bash
npm run lint
npm test -- --run
npm run build
npm run verify:release
```

`yarn verify:idl` checks every local IDL artifact:

- `target/idl/synapse_agent_sap.json`
- `idl/synapse_agent_sap.json`
- `synapse-sap-sdk/idl/synapse_agent_sap.json`
- `synapse-sap-sdk/src/idl/synapse_agent_sap.json`
- `synapse-sap-sdk/src/idl.json`

The gate fails if any path diverges or if critical layouts regress.

## Devnet Runtime Smoke Tests

After a devnet program upgrade and IDL publication attempt, run:

```bash
yarn smoke:devnet:sol
yarn smoke:devnet:usdc
```

The SOL smoke covers:

- `registerAgent` treasury fee
- `initStake`
- `migratePricingMenu`
- `createEscrowV2`
- `settleCallsV2`
- treasury settlement fee
- `withdrawEscrowV2`
- `closeEscrowV2`
- `closeAgent` returning/closing stake PDA

The USDC smoke covers:

- USDC pricing tier
- depositor ATA to escrow ATA deposit
- escrow ATA to agent ATA settlement
- escrow ATA to treasury ATA fee
- withdraw/close cleanup

The smoke scripts use `SAP_RPC` and `ANCHOR_WALLET` if provided. Never commit
private RPC keys, wallet paths, or generated keypairs.

## On-chain IDL Gates

Run:

```bash
yarn audit:onchain-idl -- --network devnet --strict
yarn audit:onchain-idl -- --network mainnet --strict
```

The audit intentionally checks two different surfaces:

- **Legacy Anchor JS IDL** via `@coral-xyz/anchor` `Program.fetchIdl`
- **Anchor 1.x Program Metadata IDL** via `anchor idl fetch`

Anchor 1.0 removed legacy IDL instructions. A stale legacy IDL account can
remain readable by older clients, but it is not an acceptable source of truth
for SAP 1.x. If legacy IDL and Program Metadata disagree, document it and make
the SDK use embedded IDL paths only.

Mainnet is blocked if Program Metadata is not fetchable or does not match the
current SDK/program layout.

## Next Frameworks To Add

Add these only after the current Anchor 1 migration is stable:

1. **Mollusk** for deterministic instruction-level Rust tests and CU budgets.
2. **LiteSVM** for fast stateful tests without a validator process.
3. **Surfpool** for forked legacy-state rehearsal before mainnet.
4. **Trident + Proptest** for fuzzing and economic invariants.
5. **Kani** for pure Rust math invariants such as fee math, settlement caps,
   stake coverage, and volume curve behavior.

The first useful non-Anchor addition should be Mollusk tests for:

- `register_agent`
- `migrate_pricing_menu`
- `close_agent`
- `create_escrow_v2`
- `settle_calls_v2`
- dispute window settlement/finalization

## Mainnet Checklist

Before mainnet:

- Run the read-only mainnet gate:

```bash
yarn preflight:mainnet
```

This checks the program account, ProgramData authority, ProgramData rent
reserve, local build hash, SDK release version, and Program Metadata IDL. It
must return `GO` after the upgrade and metadata write. Before the upgrade it is
expected to return `NO-GO` while mainnet still points at an older IDL/binary.

- Confirm ProgramData balance is rent reserve, not withdrawable funds.
- Confirm upgrade authority and wallet balance.
- Confirm treasury wallet and treasury USDC ATA.
- Confirm no program buffers are open.
- Confirm Program Metadata IDL fetches and matches local critical layouts.
- Confirm SDK embedded IDL paths match local program IDL.
- Run devnet SOL and USDC smoke tests after the exact devnet upgrade.
- Record devnet signatures and slot.
- Prepare rollback plan and legacy-user support message.

If any gate fails, stop. Do not compensate by publishing another SDK version or
telling integrators to guess account metas manually.

## Mainnet Upgrade Sequence

Use this order for Anchor 1.x upgrades:

1. Run `yarn preflight:mainnet` and save the `NO-GO` output as the pre-upgrade
   baseline.
2. Close or explicitly retire stale legacy Anchor IDL state before upgrading
   the binary when the current deployed binary still supports legacy IDL
   management instructions. Anchor 1.x removes those instructions.
3. Dump the current mainnet binary for rollback.
4. Deploy the devnet-proven binary.
5. Write the canonical Program Metadata IDL with the same exported-transaction
   flow used on devnet if one-shot `anchor idl init/upgrade` is unreliable.
6. Run `yarn audit:onchain-idl -- --network mainnet --strict`.
7. Run `yarn preflight:mainnet`; it must return `GO`.
8. Run the safest possible mainnet smoke against fresh test accounts and tiny
   amounts, then monitor failures for at least one hour.

Never withdraw the ProgramData lamports of the live program. That balance is
the executable program rent reserve, not treasury revenue.
