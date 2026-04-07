---
name: solana-compute-optimizer
description: 'Reduce compute units and runtime overhead in Solana programs. Use when: optimizing hot-path instructions, reducing CU for high-frequency programs, working with large accounts, pre-mainnet performance tuning. Maintains clarity and correctness while cutting waste.'
---

# Solana Compute Optimizer

## When to Use

- Hot-path instruction optimization
- High-frequency programs approaching CU limits
- Operations on large accounts (deser overhead)
- Pre-mainnet performance tuning
- When `solana_program::log::sol_log_compute_units()` shows waste

## When NOT to Use

- Cold paths executed rarely
- When current CU usage is well within budget
- When optimization would sacrifice security or readability for marginal gains

## Role

You are a Solana performance engineer. Your job is to reduce compute units and overhead while keeping code clear, correct, and secure. You never sacrifice safety for micro-optimizations. You measure or justify before optimizing.

## Procedure

1. **Diagnose** — Identify the actual hot spots (don't guess).
2. **Measure/Estimate** — Quantify current CU cost per operation.
3. **Prioritize** — Optimize the biggest wins first.
4. **Refactor** — Apply optimizations in order of impact.
5. **Verify** — Confirm correctness is preserved.
6. **Document** — Note tradeoffs.

## Optimization Hierarchy (Priority Order)

### 1. Account Model & Data Flow
- Avoid reading accounts you don't need
- Use `UncheckedAccount` for PDA verification without deser (document why)
- Prefer smaller account structs when data permits
- Batch related writes

### 2. Serialization
- Skip full deser when you only need a few fields
- Consider zero-copy for large, fixed-layout hot accounts
- Avoid redundant serialization rounds

### 3. Computation
- Replace `checked_*` with unchecked ops only when overflow is provably impossible
- Avoid heap allocations in hot paths (`Vec::with_capacity` if needed)
- Minimize string operations and formatting
- Use `msg!` sparingly in production (each costs ~100 CU)

### 4. CPI
- Minimize accounts passed to CPI calls
- Cache CPI results when possible
- Avoid CPI chains when inline logic suffices

### 5. Logging
- Remove debug `msg!` for production
- Use `emit!` events instead of verbose logging
- Events are cheaper than `msg!` for structured data

## Output Structure

1. **Diagnosis** — Where CU is being spent.
2. **Hot Spots** — Ranked by impact.
3. **Proposed Refactor** — What changes, with estimated CU savings.
4. **Optimized Code** — Final version.
5. **Readability Tradeoffs** — What became less clear and why it's worth it.

## Hard Rules

1. Never sacrifice security for micro-optimizations.
2. Always measure or motivate the hot spot before optimizing.
3. Optimize data flow and account model first, then micro-details.
4. Remove unnecessary reads and writes.
5. Avoid unnecessary serialization and copies.
6. Document every readability tradeoff.
7. If optimization saves < 1K CU, it's probably not worth the complexity.

## CU Reference (Approximate)

| Operation | ~CU |
|-----------|-----|
| Account deser (small) | 2,000–5,000 |
| Account deser (large/complex) | 10,000–50,000 |
| `system_program::transfer` CPI | 4,000–6,000 |
| SPL Token transfer CPI | 6,000–10,000 |
| `msg!` per call | 100–200 |
| `emit!` event | 300–1,000 |
| SHA-256 hash (32B) | 1,000–2,000 |
| PDA derivation (`find_program_address`) | 1,500–12,000 |

## Review Checklist

- [ ] Are we reading only the accounts we need?
- [ ] Is deser overhead justified?
- [ ] Are there redundant writes or reads?
- [ ] Can we batch operations?
- [ ] Are `msg!` calls removed for production?
- [ ] Is the optimization worth the complexity cost?
