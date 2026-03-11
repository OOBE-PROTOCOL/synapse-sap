# Cost Analysis

> What every operation costs, how memory scales, and how to reclaim rent.

## Program Deployment

| Item | Value |
|------|-------|
| Binary size | 1,469,280 bytes (1.4 MB) |
| Program account rent | ~10.2 SOL |
| **Total deploy cost** | **~10.2 SOL** |

This is a one-time cost. The program account rent is not reclaimable while the program is deployed. Closing the program (via `solana program close`) reclaims the rent, but the program ceases to exist.

---

## Per-Operation Costs

| Operation | Cost | Recoverable | Notes |
|-----------|------|:-----------:|-------|
| `register_agent` | ~0.060 SOL | Yes | `close_agent` reclaims rent |
| `init_vault` | ~0.002 SOL | Yes | `close_vault` reclaims rent |
| `open_session` | ~0.003 SOL | Yes | `close_session_pda` reclaims rent |
| `init_ledger` | ~0.032 SOL | Yes | `close_ledger` reclaims rent |
| `write_ledger` | ~0.000005 SOL | No | TX fee only. Zero additional rent. |
| `seal_ledger` | ~0.031 SOL | No | **Permanent.** No close instruction. |
| `publish_tool` | ~0.004 SOL | Yes | `close_tool` reclaims rent |
| `create_escrow` | ~0.003 SOL + deposit | Yes | `close_escrow` reclaims rent (+ remaining deposit) |
| `give_feedback` | ~0.002 SOL | Yes | `close_feedback` reclaims rent (must revoke first) |
| `create_attestation` | ~0.003 SOL | Yes | `close_attestation` reclaims rent |
| `init_digest` | ~0.002 SOL | Yes | `close_digest` reclaims rent |
| `create_buffer` | ~0.001 SOL | Yes | `close_buffer` reclaims all rent |
| `append_buffer` | Variable | Yes | Rent proportional to data size |
| `post_digest` | ~0.000005 SOL | No | TX fee only. PDA never grows. |
| `inscribe_memory` | ~0.000005 SOL | No | TX fee only (+ epoch page rent on first epoch entry) |
| `compact_inscribe` | ~0.000005 SOL | No | TX fee only |
| `initialize_global` | ~0.002 SOL | No | One-time. Singleton PDA. |

**Recoverable** means the rent paid for PDA creation can be reclaimed by closing the account. TX fees and sealed page rent are never recoverable.

---

## Memory Cost at Scale

This is where the choice of memory system has the biggest impact. All scenarios assume 200-byte entries.

| Scenario | MemoryLedger | MemoryVault | Raw PDA (realloc) |
|----------|:-----------:|:-----------:|:-----------------:|
| 100 entries × 200B | ~0.033 SOL | ~0.003 SOL | ~0.14 SOL |
| 1K entries × 200B | ~0.037 SOL | ~0.007 SOL | ~1.39 SOL |
| 10K entries × 200B | ~0.082 SOL | ~0.052 SOL | ~13.9 SOL |
| 100K entries × 200B | ~0.532 SOL | ~0.502 SOL | ~139 SOL |
| + 10 sealed pages | +0.31 SOL | N/A | N/A |

### How to Read This Table

- **MemoryLedger:** Init cost (~0.032 SOL) + TX fees only per write. Fixed-cost PDA that never grows. The dominant cost at scale is TX fees (~0.000005 SOL × N writes).
- **MemoryVault:** Init cost (~0.002 SOL vault + ~0.003 SOL session) + TX fees per inscription. Epoch pages add ~0.001 SOL per epoch (every 1000 inscriptions).
- **Raw PDA (realloc):** The MemoryBuffer model. PDA grows with every write. Rent is proportional to total stored bytes. At 10K entries × 200B = 2 MB, you're paying ~13.9 SOL in rent ... and `MemoryBuffer` caps at ~10 KB, so this scenario is impossible with a single buffer.

### Cost Breakdown: 10,000 Writes

```
MemoryLedger:
  Init:      0.032 SOL (one-time PDA rent, reclaimable)
  Writes:    0.050 SOL (10,000 × 0.000005 SOL TX fee)
  Total:     0.082 SOL

MemoryVault:
  Init:      0.005 SOL (vault + session)
  Writes:    0.050 SOL (10,000 × 0.000005 SOL TX fee)
  Epochs:   ~0.010 SOL (10 epoch pages × ~0.001 SOL)
  Total:    ~0.065 SOL *
  * Vault init cost is lower, but no instant readability

Raw PDA (realloc):
  Init:      0.001 SOL
  Growth:   ~13.9 SOL (2 MB of on-chain data)
  Total:    ~13.9 SOL  ← 170× more expensive
```

---

## Sealed Page Economics

Each `seal_ledger` call creates a permanent `LedgerPage` PDA (~4 KB snapshot) at ~0.031 SOL. This cost is **not recoverable** ... by design, sealed pages have no close instruction.

| Sealed Pages | Cost | Data Preserved |
|:------------:|:----:|:--------------:|
| 1 | 0.031 SOL | ~4 KB |
| 5 | 0.155 SOL | ~20 KB |
| 10 | 0.310 SOL | ~40 KB |
| 50 | 1.550 SOL | ~200 KB |
| 100 | 3.100 SOL | ~400 KB |

Sealing is optional. Only seal when you need protocol-level immutability guarantees. For most workloads, the TX log tier provides sufficient permanence.

---

## Rent Reclamation Guide

Rent is reclaimable for all closeable accounts. Close accounts in dependency order to satisfy the program's guards:

### Recommended Close Order

```
1. close_ledger       ← close the MemoryLedger first (frees ring buffer rent)
2. close_buffer       ← close any MemoryBuffer pages
3. close_digest       ← close any MemoryDigest PDAs
4. close_epoch_page   ← close epoch pages (per session)
5. close_session      ← close the SessionLedger
6. close_vault        ← close the MemoryVault (requires all sessions closed)
7. close_tool         ← close any ToolDescriptor PDAs
8. close_feedback     ← close feedback (must revoke first)
9. close_attestation  ← close attestations
10. close_escrow      ← close escrows (must be empty)
11. close_agent       ← close the AgentAccount last
```

### Close Guards

The program enforces dependency invariants:

| Close Instruction | Guard | Error if Violated |
|-------------------|-------|-------------------|
| `close_vault` | All sessions must be closed | `SessionStillOpen` |
| `close_feedback` | Must be revoked first | `FeedbackNotRevoked` |
| `close_escrow` | Balance must be zero | `EscrowNotEmpty` |
| `close_*_index` | Agents array must be empty | `IndexNotEmpty` |

### Example: Full Teardown

```typescript
const client = SapClient.from(provider);

// 1. Close memory systems
await client.ledger.close(sessionPda);
// Sealed LedgerPages are NOT closeable (permanent by design)

// 2. Close session infrastructure
await client.vault.closeEpochPage(sessionPda, 0);
await client.vault.closeSession(vaultPda, sessionPda);
await client.vault.closeVault();

// 3. Close tools
await client.tools.close(agentPda, toolNameHash);

// 4. Close feedback (revoke first)
await client.feedback.revoke(agentPda, reviewerWallet);
await client.feedback.close(agentPda, reviewerWallet);

// 5. Close escrow (drain first)
await client.escrow.close(escrowPda);

// 6. Close agent (last)
await client.agent.close();
```

---

## Cost Optimization Tips

1. **Use MemoryLedger for everything except encryption.** At 10K writes, Ledger costs 170× less than raw PDA realloc.

2. **Seal sparingly.** Each seal is ~0.031 SOL permanent. Only seal when you need provable immutability. TX logs are already permanent.

3. **Batch multiple writes per transaction** where possible. The TX fee is per transaction, not per instruction. Four writes in one TX cost ~0.000005 SOL total, not 4×.

4. **Close unused accounts.** Rent from closed PDAs returns to your wallet immediately.

5. **Use compact_inscribe over inscribe_memory** for single-fragment, non-epoch-managed writes. Same cost, simpler API.

6. **Monitor epoch page creation.** Each new epoch (every 1000 inscriptions) auto-creates a small PDA (~0.001 SOL). Budget accordingly for long-running sessions.

7. **Consider MemoryDigest for proof-only workloads.** If you don't need on-chain data ... just proof that data existed ... Digest is the cheapest option at ~0.002 SOL fixed forever.

---

**Previous**: [Deployment Guide](./07-deployment.md) · **Next**: [Overview →](./05-memory.md)
