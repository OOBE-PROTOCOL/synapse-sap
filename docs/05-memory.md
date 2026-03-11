# Memory Architecture

> Four on-chain memory systems. How they differ, when to use each, and what they cost.

## Overview

SAP v2 provides four memory systems for persisting agent data on-chain. They solve the same fundamental problem ... durable, verifiable session memory ... but with radically different tradeoffs. **MemoryLedger** is the recommended default for almost every workload.

### Comparison Matrix

| Feature | MemoryLedger [recommended] | MemoryVault | MemoryBuffer | MemoryDigest |
|---------|:-:|:-:|:-:|:-:|
| Init cost | ~0.032 SOL | ~0.002 SOL | ~0.001 SOL | ~0.002 SOL |
| Per-write cost | TX fee only | TX fee only | TX fee + realloc rent | TX fee only |
| 10K writes (200B) | ~0.082 SOL | ~0.052 SOL | impossible (10KB limit) | ~0.052 SOL |
| Instant readability | Yes (ring buffer) | No (TX log parse) | Yes (getAccountInfo) | No (TX log parse) |
| Permanent storage | Yes (sealed pages) | Yes (TX logs) | No (closeable) | Yes (TX logs) |
| Encryption | No | Yes (AES-256-GCM) | No | No |
| Merkle proof | Yes | Yes | No | Yes |
| Fixed cost PDA | Yes | Yes | No (grows via realloc) | Yes |
| Protocol immutability | Yes (sealed pages) | No | No | No |
| Delegation | No | Yes | No | No |
| Max data per write | 750 bytes | 750 bytes | ~10KB total | 750 bytes |

---

## MemoryLedger ([recommended] Recommended)

The Ledger is a fixed-cost ring buffer optimized for high-frequency writes. After the one-time PDA creation (~0.032 SOL), every write costs only the Solana transaction fee (~0.000005 SOL) with **zero additional rent**. Data flows through three tiers simultaneously:

### Three-Tier Architecture

```
  ┌─────────────── HOT TIER ───────────────────────────────┐
  │              MemoryLedger PDA                           │
  │  ┌────────────────────────────────────────────────────┐ │
  │  │          4 KB Ring Buffer                          │ │
  │  │  [u16 len][data] [u16 len][data] [u16 len][data]  │ │
  │  │              ← evicts oldest when full             │ │
  │  └────────────────────────────────────────────────────┘ │
  │  merkleRoot: 0xabc...                                   │
  │  numEntries: 147                                        │
  │  numPages: 2                                            │
  │  totalDataSize: 28,450                                  │
  └────────┬────────────────────────────────────────────────┘
           │ seal_ledger()
           ▼
  ┌─────────────── PERMANENT TIER ─────────────────────────┐
  │  LedgerPage PDA (0)   ← ~0.031 SOL, write-once        │
  │  LedgerPage PDA (1)   ← no close instruction exists   │
  │  LedgerPage PDA (2)   ← truly immutable forever       │
  └────────────────────────────────────────────────────────┘

  ┌─────────────── LOG TIER ───────────────────────────────┐
  │  Every write emits a TX log event (permanent,          │
  │  retrievable via getTransaction on archival RPC)       │
  └────────────────────────────────────────────────────────┘
```

**HOT** ... A 4KB sliding-window ring buffer inside the PDA. Latest ~10...20 entries are always readable via `getAccountInfo()` on any free RPC. When the buffer fills, oldest entries are evicted to make room. Evicted data is _not lost_ ... it remains permanently in the LOG tier.

**PERMANENT** ... Sealed pages. Calling `seal_ledger()` creates a `LedgerPage` PDA containing a frozen snapshot of the current ring buffer. **No close instruction exists.** Pages are irrevocably and permanently on-chain. Even the authority cannot delete them.

**LOG** ... Every `write_ledger` call emits an Anchor event to the transaction log. The full history of all writes is permanent, immutable, and on-chain ... retrievable via `getSignaturesForAddress` + `getTransaction` on archival-capable RPCs.

### Ring Buffer Wire Format

Each entry in the ring buffer is encoded as:

```
[data_len: u16 LE][data: u8 × data_len]
```

Entries are packed contiguously within the 4096-byte ring. On write, if the new entry doesn't fit, the oldest entries are drained from the front until there's room. The `RING_CAPACITY` constant is 4096 bytes.

### Merkle Integrity

Every write updates a rolling Merkle hash:

```
merkle_root = sha256(prev_merkle_root || content_hash)
```

This forms a tamper-evident chain across all writes. Third parties can verify data integrity by replaying the hash chain and comparing against the on-chain `merkle_root`.

### Complete Workflow

```
  init_ledger → write_ledger (×N) → read (ring buffer)
                      ↓
                seal_ledger → read (sealed pages)
                      ↓
                close_ledger (reclaim ~0.032 SOL)
```

### Code Examples

#### Initialize

```typescript
// Creates a MemoryLedger PDA with 4 KB ring buffer
// Cost: ~0.032 SOL rent (one-time, reclaimable on close)
await client.ledger.init(sessionPda);
```

#### Write

```typescript
import { sha256, hashToArray } from "@synapse-sap/sdk/utils";

const data = Buffer.from("User requested SOL→USDC swap");
const contentHash = hashToArray(sha256(data));

// Cost: ~0.000005 SOL (TX fee only ... ZERO additional rent)
await client.ledger.write(sessionPda, data, contentHash);
```

Each write simultaneously:
1. Appends to the ring buffer (evicts oldest entry if full)
2. Emits a TX log event (permanent, archival)
3. Updates the rolling Merkle root

#### Seal to Permanent Archive

```typescript
// Freezes ring buffer contents into a permanent LedgerPage PDA
// Cost: ~0.031 SOL rent (write-once, never-delete)
await client.ledger.seal(sessionPda);
```

Sealing creates a new `LedgerPage` PDA, resets the ring buffer, and increments `numPages`. Pages are **immutable** ... no close instruction exists in the program.

#### Read ... Ring Buffer (Hot Path)

```typescript
// Fetch the ledger account (FREE ... any RPC, no archival needed)
const ledger = await client.ledger.fetchLedger(sessionPda);

// Decode ring buffer into individual entries
const entries = client.ledger.decodeRingBuffer(ledger.ring);
entries.forEach((entry) => {
  console.log(Buffer.from(entry).toString("utf-8"));
});
```

#### Read ... Sealed Pages

```typescript
// Fetch a specific sealed page by index
const page = await client.ledger.fetchPage(ledgerPda, 0);

// Pages contain frozen ring buffer data in the same wire format
const pageEntries = client.ledger.decodeRingBuffer(page.data);
```

#### Close (Reclaim Rent)

```typescript
// Closes the MemoryLedger PDA, reclaims ~0.032 SOL
// NOTE: Sealed LedgerPages are NOT closed ... they're permanent by design
await client.ledger.close(sessionPda);
```

### PDA Derivation

```typescript
import { deriveLedger, deriveLedgerPage } from "@synapse-sap/sdk/pda";

const [ledgerPda] = deriveLedger(sessionPda);          // ["sap_ledger", session]
const [pagePda]   = deriveLedgerPage(ledgerPda, 0);    // ["sap_page", ledger, 0_u32_le]
```

---

## MemoryVault

The Vault provides encrypted, session-scoped memory with epoch-based pagination. Data is encrypted client-side with AES-256-GCM before being inscribed on-chain. The vault also supports hot-wallet delegation, allowing authorized delegates to inscribe data without the owner's key.

> **When to use Vault:** When you need client-side encryption or hot-wallet delegation. For everything else, use the Ledger.

### Encryption Model

- **Algorithm:** AES-256-GCM (client-side, before submission)
- **Key derivation:** PBKDF2 from a user-supplied secret + the on-chain `vault_nonce` (32 bytes, public)
- **Nonce rotation:** `rotate_nonce` instruction updates the salt; clients re-derive keys
- **On-chain data:** Only ciphertext + nonce are stored. The program never sees plaintext.

### Session & Epoch Architecture

Each vault holds multiple sessions. Sessions are scoped by a 32-byte SHA-256 hash of a developer-chosen identifier (e.g., `sha256("conversation-456")`). Inscriptions within a session are grouped into **epochs** of 1000 entries each.

```
  ┌─────────────────────────────────────────────────────┐
  │                 MemoryVault PDA                      │
  │  owner, nonce, session count, protocol_version       │
  └─────────────┬───────────────────────────────────────┘
                │
      ┌─────────┼──────────┐
      ▼         ▼          ▼
  ┌──────────┐ ┌────────┐ ┌────────┐
  │ Session A│ │Session B│ │Session C│
  │ (hash)   │ │        │ │        │
  └────┬─────┘ └────────┘ └────────┘
       │
  ┌────┼────┬────┐
  ▼    ▼    ▼    ▼
┌────┐┌────┐┌────┐
│Ep 0││Ep 1││Ep 2│  ← EpochPage PDAs (1000 inscriptions each)
└────┘└────┘└────┘
```

Each `EpochPage` PDA acts as a scan target ... `getSignaturesForAddress(epochPagePDA)` returns only the transactions for that epoch, enabling O(1) random access to any range of inscriptions.

### Delegation (Hot Wallets)

Vaults support delegated access via `VaultDelegate` PDAs. Delegates are secondary wallets (e.g., a backend service) authorized to inscribe data on behalf of the vault owner.

```typescript
const hotWallet = new PublicKey("...");

// Authorize a delegate with permissions bitmask and optional expiry
// Permissions: bit 0 = inscribe, bit 1 = close_session, bit 2 = open_session
await client.vault.addDelegate(
  hotWallet,
  0x07,                                     // all permissions
  Math.floor(Date.now() / 1000) + 86400,    // expires in 24 hours
);

// Delegate can now inscribe on behalf of the owner
await client.vault.inscribeDelegated(hotWallet, vaultPda, sessionPda, epochPagePda, args);

// Revoke when no longer needed
await client.vault.revokeDelegate(hotWallet);
```

The program verifies the delegate PDA on every delegated call. Expired delegates are automatically rejected.

### Code Examples

#### Initialize Vault

```typescript
// 32-byte random salt for PBKDF2 key derivation
const vaultNonce = Array.from(crypto.getRandomValues(new Uint8Array(32)));
await client.vault.initVault(vaultNonce);
```

#### Open Session

```typescript
import { sha256, hashToArray } from "@synapse-sap/sdk/utils";

const sessionHash = hashToArray(sha256("conversation-456"));
await client.vault.openSession(sessionHash);
```

#### Inscribe (Full & Compact)

```typescript
// Full inscription ... 8 args, supports fragmentation and epoch control
await client.vault.inscribeWithAccounts(sessionPda, epochPagePda, vaultPda, {
  sequence: 1,
  encryptedData: Buffer.from(ciphertext),
  nonce: encryptionNonce,
  contentHash: contentHashArray,
  totalFragments: 1,
  fragmentIndex: 0,
  compression: 0,       // 0 = none, 1 = zlib, 2 = zstd
  epochIndex: 0,
});

// Compact inscription ... 4 args, single-fragment convenience
await client.vault.compactInscribe(sessionPda, vaultPda, {
  sequence: 1,
  encryptedData: Buffer.from(ciphertext),
  nonce: encryptionNonce,
  contentHash: contentHashArray,
});
```

#### Close Session & Vault

```typescript
await client.vault.closeSession(vaultPda, sessionPda);     // no more inscriptions
await client.vault.closeEpochPage(sessionPda, 0);           // reclaim epoch rent
await client.vault.closeVault();                             // reclaim vault rent
```

### PDA Derivation

```typescript
import { deriveVault, deriveSession, deriveEpochPage, deriveVaultDelegate } from "@synapse-sap/sdk/pda";

const [vaultPda]    = deriveVault(agentPda);                         // ["sap_vault", agent]
const [sessionPda]  = deriveSession(vaultPda, sessionHash);          // ["sap_session", vault, hash]
const [epochPda]    = deriveEpochPage(sessionPda, 0);                // ["sap_epoch", session, 0]
const [delegatePda] = deriveVaultDelegate(vaultPda, delegateWallet); // ["sap_delegate", vault, delegate]
```

---

## MemoryBuffer

A realloc-based PDA that stores data directly in account state. Readable via `getAccountInfo()` on any free RPC ... no archival access needed.

**Tradeoff:** Simple and instant readability, but expensive at scale. The PDA grows with each write, and rent accumulates proportionally. At ~50× the cost of Ledger for sustained writes, Buffer is best reserved for small, short-lived state.

### Cost Model

| Size | Rent |
|------|------|
| Empty (101 bytes) | ~0.001 SOL |
| 500 bytes | ~0.005 SOL |
| 2.5 KB | ~0.021 SOL |
| 10 KB (max) | ~0.073 SOL |

All rent is reclaimable via `close_buffer`.

### Workflow

```
  create_buffer → append_buffer (×N) → getAccountInfo() (free reads) → close_buffer
```

- Max write size per call: 750 bytes
- Max total data per buffer page: 10,000 bytes (~10 KB)
- No Merkle proof, no TX log backup, no permanent storage
- PDA grows via `realloc` ... pays incremental rent per append

### When to Use

Small configuration, ephemeral state, or data that needs to be instantly readable without archival RPC access. Not suitable for conversation history or high-frequency writes.

---

## MemoryDigest

A proof-of-memory protocol. The PDA is fixed-size (~230 bytes, ~0.002 SOL) and **never grows**. Stores only a rolling Merkle root, a latest hash, and aggregate counters ... no actual data.

### Verification Flow

```
  1. Fetch data from off-chain storage (IPFS, Arweave, S3, etc.)
  2. sha256(data) → must match content_hash in TX log event
  3. Replay merkle chain → must match on-chain merkle_root
```

### Storage Types

| Type | ID | `storage_ref` semantics |
|------|----|------------------------|
| None | 0 | Not set |
| IPFS | 1 | SHA-256 of CID |
| Arweave | 2 | 32-byte TX ID |
| Shadow Drive | 3 | SHA-256 of URL |
| HTTP/S | 4 | SHA-256 of URL |
| Filecoin | 5 | Deal CID hash |
| Custom | 6...255 | Developer-defined |

### When to Use

When you need verifiable proof that computation or data existed, but don't need to store the data itself on-chain. Pairs naturally with IPFS or Arweave for content-addressable off-chain storage.

---

## Choosing the Right System

| Use Case | System | Why |
|----------|--------|-----|
| General agent memory | [recommended] **MemoryLedger** | Best cost/feature ratio. Instant reads + permanent archive. |
| Sensitive / encrypted data | **MemoryVault** | Client-side AES-256-GCM encryption. Hot-wallet delegation. |
| Small config / ephemeral state | **MemoryBuffer** | Instant readability via `getAccountInfo()`. Closeable. |
| Proof of computation | **MemoryDigest** | Fixed-size PDA. Off-chain data, on-chain proof. |
| Permanent archive | **MemoryLedger** + seal | Sealed pages have no close instruction. Irrevocable. |
| High-frequency writes | [recommended] **MemoryLedger** | TX fee only per write. No rent growth. |
| Hot-wallet delegation | **MemoryVault** | VaultDelegate PDA with permissions + expiry. |

### Migration Path

If you have an existing Vault setup and want to move to Ledger, the `SessionManager` handles both. New sessions use the Ledger automatically; old Vault sessions remain readable:

```typescript
// New sessions use the Ledger
const ctx = await client.session.start("new-conversation");
await client.session.write(ctx, "This writes to the Ledger");

// Old Vault sessions are still accessible
const oldSession = await client.vault.fetchSession(vaultPda, oldSessionHash);
```

---

**Previous**: [Memory Systems (SDK)](../synapse-sap-sdk/docs/04-memory-systems.md) · **Next**: [Security Model →](./06-security.md)
