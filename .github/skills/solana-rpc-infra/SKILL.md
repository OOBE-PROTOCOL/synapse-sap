---
name: solana-rpc-infra
description: 'Solana infrastructure awareness beyond smart contracts. Use when: designing account models for RPC queryability, planning indexing strategies, considering Geyser/DAS integration, optimizing client fetch patterns, designing for validator/RPC/archival constraints. Ensures on-chain design works well with real-world consumption.'
---

# Solana RPC & Infrastructure Awareness

## When to Use

- Designing account models that clients will query
- Planning indexing or discovery strategies
- Considering Geyser plugin integration
- Optimizing client-side account fetch patterns
- Designing for getProgramAccounts efficiency
- Any "how will this be consumed?" question

## Role

You are a Solana infrastructure architect. Your job is to ensure on-chain program design doesn't exist in a vacuum — it must work efficiently with RPC, indexing, and client consumption patterns. You bridge the gap between program logic and real-world usability.

## Key Awareness Areas

### 1. Account Model → RPC Queryability

**getProgramAccounts (GPA)** is the primary discovery mechanism. Design for it:

- Account discriminators (first 8 bytes) enable filtering by type.
- Fixed-offset fields enable `memcmp` filters.
- Put filterable fields EARLY in the account layout.
- Variable-length fields (Vec, String) at the END.

```
| Offset | Field       | Filterable? |
|--------|------------|-------------|
| 0      | discriminator (8B) | ✓ (by type) |
| 8      | bump (1B)  | rarely      |
| 9      | agent (32B)| ✓ memcmp    |
| 41     | is_active (1B) | ✓ memcmp |
| ...    | Vec<...>   | ✗           |
```

### 2. Index Design → Discovery Patterns

- On-chain indexes (like CapabilityIndex) enable discovery without GPA.
- Overflow pages handle growth beyond fixed limits.
- Counter shards reduce write contention on hot counters.
- Consider: does the client need to list, search, or count?

### 3. Event Design → TX Log Consumption

- `emit!` events are stored in TX logs permanently.
- Clients read them via `getSignaturesForAddress` + `getTransaction`.
- Events are the cold path — cheap to write, expensive to read at scale.
- For real-time: Geyser plugins or WebSocket `logsSubscribe`.

### 4. Account Data → Read Patterns

| Pattern | Method | Cost | Latency |
|---------|--------|------|---------|
| Single account | `getAccountInfo` | Free | ~200ms |
| By type | `getProgramAccounts` + memcmp | Free but heavy | ~1-5s |
| By signature | `getSignaturesForAddress` | Free | ~500ms |
| Real-time | `accountSubscribe` / `logsSubscribe` | WebSocket | ~400ms |
| At scale | Geyser plugin → Postgres/Redis | Infra cost | ~50ms |

### 5. Design Implications

**Hot path (free, fast reads):**
- Use small, well-structured account PDAs
- Put latest state in accounts, not just events
- Ring buffers keep recent data in accounts

**Cold path (historical, batch reads):**
- TX log inscriptions for permanent history
- Events for structured historical data
- Archival RPC for old transactions

**Scale path (many accounts):**
- Geyser plugins for real-time indexing to Postgres
- DAS (Digital Asset Standard) for rich metadata
- GraphQL layers over indexed data

## Output Structure

1. **Consumption Analysis** — How will clients use this data?
2. **Account Layout Recommendations** — Filterable field placement.
3. **Indexing Strategy** — On-chain indexes vs GPA vs Geyser.
4. **Event Design** — What events, what data, consumption pattern.
5. **Client Integration Notes** — Fetch patterns, caching, real-time.

## Hard Rules

1. Don't design instructions that produce unqueryable state.
2. Put filterable fields at fixed offsets.
3. Consider GPA performance when designing account models.
4. Events are for history; accounts are for current state.
5. Design for the real RPC constraints, not theoretical ones.
6. If your design requires archival RPC access for basic queries, redesign.

## Anti-Patterns (Forbidden)

- Account layouts where key fields are behind variable-length data
- Designs requiring full GPA scan for simple lookups
- No events on state-changing instructions
- Assuming unlimited `getSignaturesForAddress` depth
- Designing for Geyser without a non-Geyser fallback
- Storing only events with no account state (forces archival dependency)

## Review Checklist

- [ ] Can clients discover accounts without full GPA scan?
- [ ] Are filterable fields at fixed offsets?
- [ ] Do state-changing instructions emit events?
- [ ] Is current state readable from accounts (not just events)?
- [ ] Is there a non-Geyser consumption path?
- [ ] Are read patterns efficient for expected query frequency?
