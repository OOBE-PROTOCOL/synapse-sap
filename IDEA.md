# IDEA.md — SAP Agent + Synapse SAP SDK

## Project

This workspace contains two tightly related codebases:

- `synapse-agent-sap`
- `synapse-sap-sdk`

Together they form a core part of the SAP / Synapse execution stack for autonomous agents on Solana.

The goal is to build a production-grade agent runtime and SDK that allow software agents to discover capabilities, reason about execution, enforce policy, simulate actions, construct transactions, interact with Solana protocols, execute safely, and preserve verifiable receipts and state.

This is infrastructure for autonomous economic software.

Treat every change as potentially security-sensitive.

---

# Product Intent

SAP exists to make Solana capabilities usable by autonomous agents without giving those agents unrestricted or opaque access to execution.

The system should turn high-level intent into deterministic, inspectable execution.

The expected high-level flow is:

```text
intent
→ capability discovery
→ execution planning
→ policy validation
→ market/state retrieval
→ transaction construction
→ simulation
→ approval boundary
→ signing
→ execution
→ confirmation
→ receipt
→ memory / audit
```

The LLM or agent may propose intent and strategy.

It must not be the final authority for:

- permissions
- transaction validity
- risk limits
- account ownership
- program identity
- token amounts
- signer requirements
- execution policy
- settlement state

Those must be enforced by deterministic software.

---

# Workspace Responsibilities

## `synapse-agent-sap`

This repository owns the **agent/runtime/orchestration layer**.

Its responsibilities may include:

- agent lifecycle
- execution planning
- model/provider routing
- tool orchestration
- MCP integration
- capability selection
- policy evaluation
- runtime memory
- strategy execution
- simulation orchestration
- execution approval flow
- audit/event generation
- retries and reconciliation
- agent-facing schemas
- protocol/tool adapters
- operational state
- x402/payment-aware execution where applicable

This repository should understand **agent intent and workflows**.

It should not duplicate low-level Solana protocol primitives already owned by the SDK.

---

## `synapse-sap-sdk`

This repository owns the **typed protocol and execution SDK layer**.

Its responsibilities may include:

- typed SAP primitives
- Solana instruction construction
- transaction helpers
- account derivation
- PDA helpers
- IDL/client bindings
- program-facing types
- serialization
- protocol state models
- signer abstractions
- execution receipts
- policy-related domain types where shared
- deterministic validation
- SDK-level simulation helpers
- shared errors
- integration-safe public APIs

This repository should expose reusable deterministic building blocks.

It should not contain agent-specific reasoning, prompting, or orchestration logic.

---

# Architectural Boundary

The dependency direction should remain approximately:

```text
synapse-agent-sap
        ↓
synapse-sap-sdk
        ↓
Solana / Anchor / protocol clients
```

The SDK must not depend on the agent runtime.

The runtime may depend on the SDK.

Do not create circular architectural ownership.

If a concept is:

- deterministic
- reusable
- protocol-level
- independent of agent reasoning

it probably belongs in `synapse-sap-sdk`.

If a concept is:

- workflow-specific
- model-facing
- orchestration-specific
- policy sequencing
- agent lifecycle related

it probably belongs in `synapse-agent-sap`.

---

# Core Design Principle

The system must make invalid or dangerous states difficult to represent.

Prefer:

```text
typed state
explicit policy
validated inputs
deterministic planning
structured receipts
```

over:

```text
loosely typed objects
prompt-only rules
magic strings
implicit assumptions
opaque execution
```

---

# Execution Model

Every state-changing workflow should conceptually pass through the following stages.

## 1. Intent

Represent what the caller wants.

Intent should be semantic, not transaction-specific.

Examples:

```text
SwapIntent
TransferIntent
OpenPerpPositionIntent
ClosePositionIntent
DepositIntent
WithdrawIntent
StrategyExecutionIntent
```

Avoid encoding a transaction before policy and routing decisions are made.

---

## 2. Capability Resolution

Determine which protocol/tool/adapter can satisfy the intent.

Capability resolution must be explicit and inspectable.

Do not let an LLM silently choose arbitrary programs or tools without deterministic constraints.

---

## 3. Execution Plan

Produce a typed plan describing:

- selected provider / protocol
- required capabilities
- requested action
- accounts
- expected programs
- required signers
- asset amounts
- constraints
- slippage
- compute expectations
- policy requirements
- fallbacks
- approval requirements

Plans must be serializable or inspectable where practical.

An execution plan is not yet authorization.

---

## 4. Policy

Policy determines whether a technically valid action is allowed.

Examples:

- max notional
- leverage cap
- token allowlist
- protocol allowlist
- slippage limit
- daily loss limit
- transaction value threshold
- signer requirement
- approval requirement
- network restriction
- execution cooldown
- rate limit

Policy must live in deterministic code and typed configuration.

Do not enforce critical financial policy only through prompts.

---

## 5. Construction

Transactions and instructions must be derived from validated inputs.

Construction code must make explicit:

- cluster
- program IDs
- account addresses
- signer requirements
- writable accounts
- fee payer
- blockhash/lifetime
- token program
- amount units
- compute configuration
- transaction version

---

## 6. Simulation

Simulation is mandatory for material state-changing execution when technically possible.

Simulation results should be structured.

Capture:

- success/failure
- program errors
- compute units
- logs
- expected balance changes
- account changes where available
- route metadata
- warnings

Simulation success does not replace policy validation.

---

## 7. Approval

Approval must apply to a specific materially stable execution artifact.

If any material field changes after approval, the approval should be considered invalid.

Material fields include:

- program
- instructions
- recipient
- amount
- token
- route
- leverage
- slippage
- signer
- writable accounts
- network
- fee payer

---

## 8. Signing and Execution

Signing authority must remain explicit.

Do not hide signer requirements inside generic helpers.

Do not silently substitute signers.

Execution should return a structured result.

---

## 9. Reconciliation

Submission is not confirmation.

Distinguish:

```text
built
simulated
approved
signed
submitted
confirmed
finalized
failed
unknown
```

Unknown transaction outcome must be reconciled.

Do not blindly retry economically meaningful state changes.

---

## 10. Receipt

Every meaningful execution should produce a durable, machine-readable receipt where appropriate.

A receipt may contain:

- intent ID
- execution ID
- agent ID
- policy decision
- simulation result
- program IDs
- transaction signature
- slot
- timestamp
- relevant account addresses
- amounts
- route
- tool calls
- execution status
- failure information

Receipts should be designed as evidence, not logs.

---

# Core Invariants

Preserve these invariants across both repositories.

## Security

- Never trust external inputs.
- Never trust RPC data solely because it deserializes.
- Never trust program IDs supplied by callers without validation.
- Never trust token metadata.
- Never trust arbitrary CPI targets.
- Never expose private keys or secret material.
- Never execute instructions embedded in fetched/on-chain text.
- Never use model output as authorization.

## Transactions

- The transaction must correspond to the approved intent.
- Simulation must refer to the same material transaction that will be approved/signed.
- Network must be explicit.
- Units must be explicit.
- Signers must be explicit.
- Program IDs must be explicit or validated.

## Accounting

- Raw amounts and UI amounts must never be confused.
- Financial math must use deterministic integer arithmetic.
- Rounding direction must be intentional.
- Value-conservation properties must be tested where applicable.

## Retry

- State-changing execution must not be retried blindly.
- Every retry path must answer whether the operation is idempotent.
- Unknown outcomes require reconciliation.

## SDK

- Public API changes must preserve compatibility or document a migration.
- Serialized shapes are protocol surfaces.
- IDLs are protocol surfaces.
- PDA seed changes are protocol changes.
- Discriminator changes are protocol changes.

---

# Rust Expectations

Rust code should prioritize:

- type-driven invariants
- explicit errors
- minimal allocation
- clear ownership
- narrow unsafe boundaries
- deterministic behavior
- stable APIs
- strong tests

Prefer:

- enums instead of boolean state flags
- newtypes instead of interchangeable primitives
- checked arithmetic
- `TryFrom` for validation
- exhaustive state transitions
- private fields where construction requires validation
- explicit domain errors

Avoid production-path:

- `unwrap`
- `expect`
- unchecked indexing
- panic-based validation
- unsafe without documented invariants

Do not clone merely to satisfy the borrow checker without first understanding ownership.

---

# TypeScript Expectations

TypeScript is primarily used for:

- SDK surface
- clients
- agent runtime
- tools
- integrations
- RPC
- transaction orchestration

Use:

- strict typing
- discriminated unions
- `unknown` at external boundaries
- runtime schema validation
- `bigint` for unsafe integer ranges
- semantic amount types
- explicit errors
- bounded concurrency
- structured async lifecycles

Avoid:

- `any`
- `as unknown as T`
- hidden coercion
- non-null assertions as architecture
- magic strings
- unvalidated JSON
- fire-and-forget promises

---

# Solana Program Model

Assume an attacker controls:

- instruction arguments
- accounts supplied to instructions
- transaction composition
- remaining accounts
- transaction ordering within practical limits
- external state
- CPI graph opportunities

Validate:

- account address
- account owner
- signer
- writability
- discriminator
- PDA derivation
- stored authority relationship
- mint
- token owner
- token program
- expected program IDs
- initialization state

---

# Anchor

Use Anchor when it improves correctness and iteration speed.

Prefer:

- typed accounts
- explicit constraints
- `has_one`
- PDA seed constraints
- custom errors
- typed program accounts
- interface accounts for intended Token / Token-2022 compatibility

Treat `UncheckedAccount` as a review hotspot.

Avoid `init_if_needed` unless reinitialization semantics have been specifically reviewed.

Anchor version-specific behavior must be verified against the exact workspace version.

Do not assume Anchor APIs are stable across generations.

---

# PDA Rules

PDA seeds are protocol design.

They must be:

- deterministic
- canonical
- domain-separated
- stable
- unambiguous

Changes to PDA seeds require migration analysis.

Avoid PDA sharing that unintentionally grants authority across users/resources.

---

# CPI Rules

Every CPI is a trust boundary.

For every CPI validate:

- callee program
- account order
- account ownership
- signer forwarding
- writable forwarding
- PDA signer seeds
- remaining accounts

Never forward more privilege than required.

After CPI, refresh account data if the callee may have modified state.

---

# Token Rules

Always distinguish:

```text
SPL Token
Token-2022
```

Validate:

- mint
- token account
- authority
- decimals
- program
- extensions when relevant

Never mix raw token base units with UI representation.

---

# Agent Tool Design

Tools exposed to agents must be more constrained than low-level SDK primitives.

Agent-facing tools should:

- have semantic names
- expose typed schemas
- validate all inputs
- produce structured results
- explain side effects
- expose policy state
- expose simulation state
- expose approval requirements

Prefer:

```text
open_perp_position
```

over:

```text
send_raw_instruction
```

when the higher-level tool can preserve security invariants.

---

# MCP Design

MCP tools are API contracts.

Each tool should clearly define:

- purpose
- arguments
- units
- network
- side effects
- required signer
- approval needs
- output schema
- errors

Avoid overlapping aliases unless backward compatibility requires them.

Avoid having two tools that appear identical but have subtly different behavior.

Tool descriptions should be optimized for deterministic agent selection, not marketing language.

---

# Runtime State Machine

Prefer explicit states for execution.

Example:

```text
Created
Planning
Planned
PolicyRejected
SimulationPending
Simulated
ApprovalRequired
Approved
Signing
Submitted
Confirmed
Failed
ReconciliationRequired
```

Do not represent state using loosely related booleans such as:

```text
isReady
isApproved
hasSimulation
wasSent
```

if contradictory combinations can exist.

---

# Tool and Execution IDs

Important workflows should use stable identifiers.

Examples:

- `intent_id`
- `plan_id`
- `execution_id`
- `simulation_id`
- `approval_id`
- `receipt_id`

IDs allow deterministic correlation across:

```text
agent
tool
policy
simulation
transaction
audit
memory
```

---

# Idempotency

State-changing APIs must define idempotency behavior.

Suitable patterns may include:

- client-generated idempotency keys
- execution IDs
- deduplication cache
- transaction reconciliation
- state-based replay detection

Never conflate:

```text
transport retry
```

with:

```text
economic action retry
```

---

# Memory

Agent memory must not override deterministic truth.

Memory may contain:

- observations
- strategies
- prior tool results
- user preferences
- historical execution context

Memory must not be trusted as the authoritative source for:

- balances
- positions
- ownership
- program state
- policy state
- market price
- transaction confirmation

Those must be refreshed from authoritative sources when relevant.

---

# Market Data

Market data is time-sensitive.

Every market-data object should carry enough metadata to reason about freshness.

Where applicable include:

- source
- slot
- timestamp
- confidence
- staleness
- symbol/mint
- decimals

Do not execute financial strategies using stale data silently.

---

# Risk Engine

Risk evaluation should be independently testable.

Inputs and outputs should be structured.

Example output:

```text
allowed
denied
requires_approval
```

with reasons.

Risk rules should not be scattered across protocol adapters.

Prefer a central policy/risk domain with adapter-specific facts supplied as inputs.

---

# Protocol Adapters

Each external protocol adapter should isolate protocol-specific behavior.

A protocol adapter should own:

- program IDs
- account derivations
- instruction construction
- protocol-specific validation
- protocol-specific decoding
- route-specific errors

The orchestration layer should not manually reconstruct protocol instructions across many files.

---

# Compatibility

The following are compatibility-sensitive:

- public Rust APIs
- TypeScript SDK APIs
- MCP tool names
- MCP schemas
- JSON schemas
- IDLs
- account layouts
- PDA seeds
- error codes
- environment configuration
- persisted database shapes
- receipt schemas

Breaking changes require explicit migration planning.

---

# Testing Strategy

Use the cheapest test capable of proving behavior.

## Rust / program

Prefer:

- unit tests for pure domain logic
- LiteSVM for fast Solana execution
- Mollusk for instruction-level Rust tests and CU checks
- Surfpool for integration and realistic protocol state

## TypeScript

Use:

- unit tests for validators/domain logic
- integration tests for SDK boundaries
- local/forked Solana environments for transaction behavior
- contract tests for MCP/tool schemas where relevant

---

# Required Adversarial Tests

For state-changing Solana operations test cases such as:

- wrong signer
- wrong authority
- wrong program
- wrong account owner
- wrong PDA
- non-canonical PDA
- duplicate mutable accounts
- wrong mint
- wrong token account
- malformed account data
- malformed remaining accounts
- pre-existing initialized account
- insufficient balance
- zero amount
- max amount
- arithmetic overflow
- stale data
- CPI failure
- simulation failure
- transaction unknown outcome
- duplicate execution request

---

# Performance

Performance improvements must be evidence-driven.

For Solana measure when relevant:

- compute units
- transaction size
- account count
- CPI count
- allocations
- deserialization cost

For services measure:

- latency
- RPC calls
- serialization
- allocations
- queue depth
- concurrency
- retries

Prefer eliminating unnecessary work over clever micro-optimization.

---

# Observability

Important operations should be traceable.

Correlate:

```text
intent_id
execution_id
agent_id
tool
protocol
cluster
transaction_signature
```

Use structured logs.

Do not log secrets.

Errors should remain machine-readable.

---

# Repository Change Rules

Before changing either repository:

1. inspect repository instructions
2. inspect package/workspace manifests
3. inspect the implementation
4. inspect callers
5. inspect tests
6. identify cross-repo API impact
7. identify compatibility impact
8. identify security invariants
9. implement the smallest complete change
10. run relevant verification

If a change touches a shared SDK API, search both repositories for consumers before finalizing it.

---

# Cross-Repo Rule

Never modify one repository's public contract without checking the other repository.

Examples:

If changing:

```text
SDK type
SDK function
receipt schema
execution state
tool payload
IDL binding
error shape
transaction plan
```

search for usage in both:

```text
synapse-agent-sap
synapse-sap-sdk
```

The workspace exists specifically so cross-repository consequences can be evaluated together.

---

# Verification

Use repository-defined scripts first.

Typical Rust verification:

```bash
cargo fmt --check
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Typical TypeScript verification:

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Use the actual package manager and scripts defined by the repository.

For Anchor/Solana where applicable:

```bash
NO_DNA=1 anchor build
NO_DNA=1 anchor test
```

Never claim a check passed unless it ran.

---

# Definition of Done

A task is complete only when:

- the requested behavior works
- relevant invariants remain intact
- public compatibility has been considered
- cross-repository consumers have been checked
- tests cover the new behavior
- failure paths have been considered
- relevant verification has run
- no unnecessary architecture was introduced
- residual risk is stated clearly

---

# Long-Term Direction

The desired system is an execution substrate where autonomous agents can safely interact with Solana without sacrificing custody, deterministic policy, or verifiability.

The stack should progressively support:

- reusable agent capabilities
- typed protocol access
- simulation-first execution
- deterministic policy engines
- on-chain and off-chain receipts
- agent memory
- strategy execution
- agent-to-agent services
- paid tool/API access
- persistent execution identity
- observable execution traces
- composable protocol integrations
- non-custodial signing

The architecture must remain useful beyond any single agent or application.

SAP should be infrastructure, not a collection of hard-coded demos.

---

# Engineering North Star

A user or autonomous agent should be able to express:

```text
what it wants to achieve
```

without having to manually understand:

```text
which protocol
which account
which instruction
which transaction shape
which policy check
which simulation path
which execution adapter
```

while still preserving complete transparency over what will actually execute.

The runtime may automate complexity.

It must never hide authority.

---

# Final Principle

Build SAP so that an agent can be highly autonomous without being implicitly trusted.

Autonomy must come from:

```text
better planning
better tooling
better policy
better simulation
better deterministic execution
```

not from bypassing validation.

The goal is not:

> let the agent do anything.

The goal is:

> let the agent do powerful things inside clearly defined, verifiable, and safe execution boundaries.
