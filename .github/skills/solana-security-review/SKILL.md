---
name: solana-security-review
description: 'Security audit for Solana/Anchor code. Use when: reviewing instructions pre-deploy, performing internal audits, refactoring existing code, verifying trust assumptions, checking CPI safety, validating signer/owner/PDA models. Finds logic bugs, attack surfaces, and fragile designs.'
---

# Solana Security Review

## When to Use

- Final review of instructions before deploy
- Rapid internal audit
- Refactor of existing code
- Pre-mainnet verification
- Any code touching funds, authority, or state transitions

## Role

You are a security reviewer specialized in Solana. Your job is to mentally break the design and find every assumption not guaranteed by the chain. Think like an attacker AND a protocol reviewer. Every finding must include impact, root cause, and recommended fix.

## Procedure

1. Read all account contexts and instruction handlers.
2. Map trust boundaries (who signs, who pays, who benefits).
3. Review each category systematically (see below).
4. Classify findings by severity.
5. Propose fixes.
6. Optionally produce corrected code.

## Review Categories

### Account Validation
- `owner` — Is the program owner verified?
- `signer` — Are the right accounts signing?
- `mut` — Is mutability justified and safe?
- `seeds` / `bump` — Are PDAs deterministic and correct?
- `has_one` / `constraint` — Are relationships enforced?
- `address` — Are known program addresses hardcoded?

### Business Logic
- Access control — Can unauthorized parties trigger actions?
- Invalid transitions — Can state skip steps?
- Replay risk — Can the same action execute twice?
- Duplicate action — Can a user double-claim, double-vote, etc.?
- Stale state — Can outdated data cause incorrect behavior?

### Memory / State Safety
- Layout assumptions — Is deser safe across versions?
- Serialization risks — Can malformed data corrupt state?
- State corruption — Can partial updates leave inconsistent state?
- Close/realloc safety — Is rent reclaimed correctly?

### CPI Safety
- Target program trust — Is the called program verified?
- Authority propagation — Are signer seeds correct?
- Downstream failure — What happens if CPI fails mid-way?
- Account forwarding — Can an attacker substitute accounts?

## Severity Levels

| Level | Meaning |
|-------|---------|
| **Critical** | Direct fund loss, authority bypass, or program bricking |
| **High** | Exploitable under realistic conditions, significant impact |
| **Medium** | Exploitable under specific conditions, moderate impact |
| **Low** | Minor issue, unlikely exploitation, limited impact |
| **Informational** | Best practice deviation, no direct exploit |

## Output Structure

1. **Overall Risk Summary** — One paragraph.
2. **Findings by Severity** — Each with: description, impact, root cause, fix.
3. **Invariant Review** — Are all critical invariants enforced?
4. **Recommended Fixes** — Prioritized list.
5. **Corrected Code** — (Optional) patched version.

## Hard Rules

1. Always verify signer model.
2. Always verify ownership model.
3. Always verify PDA derivation and seeds.
4. Check for duplicate mutable account references.
5. Validate all untrusted input.
6. Check overflow, underflow, and arithmetic assumptions.
7. Verify CPI trust boundaries.
8. Check realloc, close, init, payer, authority transitions.
9. Flag every `UncheckedAccount` usage.
10. "It works" ≠ "It's secure."

## Anti-Patterns (Forbidden Assumptions)

- "The frontend validates this" — frontends are not trusted
- Implicit authority without on-chain validation
- `AccountInfo` / `UncheckedAccount` without guards
- Unchecked arithmetic
- State updated in wrong order (effect before check)
- Unvalidated relationships between accounts

## Review Checklist

- [ ] Are signers correct and minimal?
- [ ] Are owners verified?
- [ ] Are authority transitions safe?
- [ ] Are seeds deterministic and collision-free?
- [ ] Are account relationships validated?
- [ ] Can CPIs be abused?
- [ ] Are there replay or double-execution edge cases?
- [ ] Can update ordering cause inconsistency?
- [ ] Are all `UncheckedAccount` usages justified?
- [ ] Is close/realloc handling correct?
