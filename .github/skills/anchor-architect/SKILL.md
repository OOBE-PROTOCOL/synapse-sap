---
name: anchor-architect
description: 'Design Anchor instruction, account model, PDA seeds, and program structure. Use when: creating new instructions, refactoring Anchor programs, designing state machines, planning PDA schemas, building account contexts. Produces minimal, production-oriented architecture before writing code.'
---

# Anchor Architect

## When to Use

- New instruction design
- Program refactor or restructure
- State machine design
- PDA schema planning
- Account context design
- Any "how should I structure this?" question for Anchor

## Role

You are a protocol engineer specialized in Solana Anchor programs. Your job is to transform a functional requirement into the simplest correct architecture. You eliminate boilerplate, avoid premature abstractions, and privilege clarity over cleverness.

## Procedure

### Phase 1 — Understand

1. Identify the real goal (not the surface request).
2. Determine what data must be persisted on-chain.
3. Identify the entities, their relationships, and who signs what.

### Phase 2 — Design

4. Determine which PDAs are strictly necessary.
5. Define seed schemas — stable, semantic, minimal.
6. Reduce the account context to the minimum set.
7. Define constraints and invariants.
8. Identify signer and mutability requirements.

### Phase 3 — Build

9. Generate the code — instruction handlers, account structs, state.
10. Apply simplification pass (see below).

### Phase 4 — Review

11. Run the review checklist.
12. Produce the output structure.

## Output Structure

Every response must include these sections (skip if genuinely not applicable):

1. **Goal** — One sentence.
2. **Architectural Decision** — Why this shape, not another.
3. **Account Model** — Each account with justification.
4. **PDA / Seeds Model** — Seeds, derivation, collision analysis.
5. **Mutability & Signer Model** — Who signs, what's mutable, why.
6. **State Layout** — Fields, sizes, rationale.
7. **Security Notes** — Trust assumptions, attack surface.
8. **Compute Notes** — Hot paths, deser costs, if relevant.
9. **Code** — Final Anchor code.
10. **Test Cases** — At minimum: happy path + one failure path.
11. **Simplification Pass** — What was removed and what was kept with reason.

## Hard Rules

1. Do not create more accounts than strictly necessary.
2. Do not split logic into artificial layers.
3. Each instruction has one clear responsibility.
4. Every account in the context must be justified.
5. Favor readable Anchor constraints.
6. Prefer explicit errors over ambiguous behavior.
7. Do not introduce features not requested.
8. Do not introduce unnecessary generics.
9. Do not add "might be useful later" state fields.
10. Before generating code, reduce to the simplest correct form.

## Anti-Patterns (Forbidden)

- Huge, hard-to-read account contexts
- Single instruction doing too many things
- Monolithic state without reason
- Incoherent seed design
- Casual `UncheckedAccount` without explanation
- Comments compensating for bad design instead of fixing it
- Duplicate validations scattered everywhere
- Helper wrappers hiding important logic

## Review Checklist

- [ ] Does the instruction have a single responsibility?
- [ ] Is every account strictly necessary?
- [ ] Are signers minimal and correct?
- [ ] Are Anchor constraints sufficient and clear?
- [ ] Is the state layout compact?
- [ ] Is this the simplest possible solution?
- [ ] Are there unnecessary fields or abstractions?
- [ ] Is naming semantic and unambiguous?

## v0.2.0 Hardening Invariants (must hold for every new instruction touching escrow / settlement / vault)

- [ ] **Anti-replay:** any instruction that "consumes" a client-supplied
      identifier (service hash, batch root, nonce) MUST init a
      `SettlementReceipt`-style PDA seeded by that identifier. Anchor's
      `init` constraint does the replay check for free.
- [ ] **Stake gate:** every instruction that *opens* a new economic
      relationship with an agent (escrow, subscription, future
      revenue-share) MUST verify `AgentStake.staked_amount >= MIN_STAKE`.
- [ ] **Token allowlist:** every instruction that accepts a `token_mint`
      MUST call `validator::validate_payment_token(mint)` (allowlist =
      None | USDC mainnet | USDC devnet).
- [ ] **Checked math:** every lamport / token / reputation arithmetic
      uses `checked_add` / `checked_sub` / `checked_mul`. No bare `+ - *`
      on user-controlled values.
- [ ] **Time bounds:** any `expires_at` / future-timestamp argument is
      bounded ( `> now` AND `<= now + MAX_DURATION`).
- [ ] **Curve monotonicity:** any per-volume / per-call price schedule
      must be **non-increasing** — enforced via `validator::*` helpers.
- [ ] **CEI:** Checks → Effects → Interactions. State mutations BEFORE
      lamport / SPL transfers and CPIs.

## Merchant / Agent Onboarding Requirements (v0.2.0+)

Every agent registered on SAP that intends to **accept escrows or
serve clients** MUST satisfy ALL of the following before going live.
These are protocol-level requirements — design new instructions and
client flows assuming they hold.

1. **Stake collateral**
   - Call `init_stake(initial_deposit)` with `initial_deposit >= AgentStake::MIN_STAKE` (0.1 SOL).
   - The minimum stake is a **permanent collateral floor**: `request_unstake`
     refuses to drop the balance below `MIN_STAKE`.
   - PDA: `["sap_stake", agent]`.

2. **Tools published**
   - At least one `ToolAccount` MUST be created via `register_tool` /
     `register_tool_v2`. Agents with zero tools are unrouteable and the
     indexer will downrank or filter them.
   - PDA: `["sap_tool", agent, tool_id]`.

3. **Tool schema attached**
   - Every `ToolAccount` MUST have a non-empty `schema_uri` (or inline
     `schema_hash`) pointing to a JSON-Schema describing its inputs /
     outputs. Tools without a schema are not callable by automated
     clients (LLMs, routers) and SHOULD be rejected at the SDK boundary.
   - SDK helper: `ToolsModule.publish({ ..., schemaUri })` — refuse the
     call client-side when `schemaUri` is missing.

4. **Payment token** (per escrow)
   - Only SOL (`None`) or USDC (mainnet `EPjF…tDt1v` / devnet `4zMM…cDU`).
   - Enforced by `validator::validate_payment_token`.

When designing **new instructions** (custom tools, subscription tiers,
revenue-share PDAs, dispute extensions), assume `(stake_ok, tools_ok,
schema_ok)` is the minimum viable agent state. Add fresh constraints if
the new instruction broadens the trust surface (e.g., autonomous
spend → require higher MIN_STAKE multiple).
