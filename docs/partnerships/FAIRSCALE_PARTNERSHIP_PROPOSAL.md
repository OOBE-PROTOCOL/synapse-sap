# FairScale × Synapse Agent Protocol (SAP) — Partnership Proposals

> **Authors:** Synapse / Oobe Protocol Labs
> **For:** FairScale team
> **Status:** Draft v1 — open for discussion
> **Date:** 2026-04-17
> **Contact:** partnerships@oobeprotocol.ai · @oobe_protocol

---

## 0. Why this document

Synapse Agent Protocol (SAP) is a Solana program (`SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`) that
manages the on-chain lifecycle of AI agents.. registration, tools, escrows,
disputes, staking, subscriptions, and reputation. We already maintain ~8
verified agents on mainnet and a public block-explorer at
[explorer.oobeprotocol.ai](https://explorer.oobeprotocol.ai).

We have **already shipped two pieces of integration on our side** as a
goodwill commitment:

| Layer | Artifact | Status |
|---|---|---|
| SDK | `@oobe-protocol-labs/synapse-sap-sdk@0.11.0` exposes `client.fairscale.*` — score, trust-gate, batch, score-AI, agent-profile, score-history, directory, leaderboard, credit, human-score, and an `aggregate(wallet, …)` helper that blends SAP on-chain reputation with FairScale via configurable weights. | live |
| Explorer | `GET /api/sap/agents/[wallet]/aggregate-reputation` returns a single 0–100 blended score and the SAP × FairScale breakdown. The agent detail page renders a "FairScale × SAP Aggregated Reputation" chip. | live |

This document outlines **two further, larger collaborations** we'd like to
co-design with you. They are independent and can be evaluated separately.

---

## 1. Proposal A — On-Chain Reputation Attestation Oracle

### 1.1 The problem
FairScale scores are authoritative but **off-chain**. Every consumer (dApps,
routers, smart contracts) has to:
- Pay a per-call fee, or
- Trust an opaque API gateway, or
- Re-run their own indexers.

A smart contract that wants to gate an action on "FairScale tier ≥ gold"
cannot do so without an on-chain oracle.

### 1.2 The proposal
SAP introduces a new instruction `attest_external_score` that records a
signed FairScale verdict on-chain at a deterministic PDA:

```
seeds = ["sap_extern_score", agent_pda, attester_pubkey]
```

**Account layout** (Anchor, Borsh-serialised, ≤200 B):

```rust
#[account]
pub struct ExternalScoreAttestation {
    pub agent: Pubkey,            // SAP agent PDA
    pub attester: Pubkey,         // FairScale's signer key
    pub provider: [u8; 16],       // "fairscale\0"
    pub score: u16,               // 0..=100
    pub tier: u8,                 // 0..4 (bronze → diamond)
    pub recommendation: u8,       // 0..3 (trusted | caution | high_risk | unverified)
    pub payload_hash: [u8; 32],   // sha256 of canonical FairScale JSON
    pub signature: [u8; 64],      // Ed25519(payload_hash) by `attester`
    pub scored_at: i64,           // unix seconds
    pub expires_at: i64,          // scored_at + 7d (configurable)
    pub revoked: bool,
    pub _reserved: [u8; 32],
}
```

The instruction:

1. Verifies the Ed25519 signature against `attester` using the
   built-in `ed25519` syscall (free at runtime).
2. Refuses if `attester` is not in a small allowlist (`["fairscale-prod", "fairscale-staging"]`)
   stored at a `ScoreOracleConfig` account whose authority is the SAP DAO.
3. Emits `ExternalScoreAttested { agent, provider, score, tier, expires_at }`.
4. Allows `revoke_external_score` from the same `attester`.

### 1.3 What FairScale needs to do

1. Pick one (or two) Ed25519 signing keys — held in your existing HSM/KMS.
2. Add the same payload-hash logic you already use for x-social-identity
   (so the canonical JSON definition stays under your control).
3. Optionally run a tiny worker that pushes attestations on score change,
   or simply expose a `GET /v1/attestation?wallet=…` that returns
   `{payload, signature}` so consumers can submit themselves.

### 1.4 What FairScale gets

- **Trustless distribution.** Every Solana smart contract can read
  FairScale scores without paying or trusting anyone — the signed payload
  is the source of truth.
- **Composability.** AMMs, lending markets, agent routers can gate at the
  instruction level (`require!(score.tier >= 2 && !score.revoked)`).
- **Brand surface.** `provider = "fairscale"` becomes a first-class
  citizen of every Solana block explorer that decodes SAP IDL — including
  ours, Solscan, SolanaFM.
- **Anti-spoof.** Because the oracle config is a permissioned allowlist
  controlled by the SAP DAO, no third party can fake "fairscale-signed"
  attestations.

### 1.5 Security & ops

| Concern | Mitigation |
|---|---|
| Key compromise | Allowlist multiple `attester` pubkeys; rotation = single SAP DAO multisig tx. |
| Stale scores | `expires_at` is enforced by every consumer via Anchor constraint. |
| Replay | `payload_hash` includes `scored_at` and `wallet`, so the same signature can't be reused for another agent. |
| Cost | Account is rent-exempt at ~0.0017 SOL per agent; SAP can subsidise via vault. |

### 1.6 Headline metrics we expect

- ~8 active SAP agents today → 8 attestations on day one.
- ~200 expected by Q3 → ~0.34 SOL total rent, paid by SAP.
- 0 ongoing cost to FairScale beyond the worker that signs payloads.

---

## 2. Proposal B — Bidirectional Verified-Agent Directory Feed

### 2.1 The problem
Both projects maintain an agent registry. Both registries are valuable to
the other:

- **FairScale's `/v1/directory`** has rich signals (verifications, recommendation,
  pillars) but only sees agents that have been scored.
- **SAP's on-chain registry** has provable identity, escrow capacity, tools,
  and live revenue — but no behavioural score.

Today, neither catalogue knows about the other, so end-users see a
fragmented picture.

### 2.2 The proposal — two unidirectional feeds, signed and rate-limited

#### 2.2.1 SAP → FairScale
SAP exposes a public, no-key endpoint:

```
GET https://explorer.oobeprotocol.ai/api/public/v1/sap-agents/verified
```

Response (paginated, 200/page, `Cache-Control: s-maxage=300`):

```jsonc
{
  "version": 1,
  "generated_at": "2026-04-17T00:00:00Z",
  "total": 8,
  "page": 1,
  "results": [
    {
      "wallet": "4xq...",
      "agent_pda": "9hG...",
      "name": "agent.example",
      "description": "…",
      "metadata_uri": "ar://…",
      "is_active": true,
      "stake_lamports": "100000000",
      "tools_count": 4,
      "escrows_settled": 132,
      "reputation_score_sap": 78,
      "verifications": {
        "sap_registered": true,
        "metaplex_attached": true,
        "eip8004_registered": false
      },
      "first_seen_slot": 234567890,
      "explorer_url": "https://explorer.oobeprotocol.ai/agents/4xq..."
    }
  ]
}
```

We propose FairScale ingest this feed and surface a new `source: "sap"`
filter in `/v1/directory` (analogous to `said | erc8004 | sati`).

#### 2.2.2 FairScale → SAP
SAP indexes the public `/v1/directory` (top 100/min_score=60) into our
discovery layer so that agents with a strong FairScale signal appear in our
"Discover" tab and are surfaced to users searching by capability.

We commit to:
- Caching the feed at most 5 min (no thundering-herd against your API).
- Always linking back to the canonical FairScale agent profile.
- Honouring `verifications` and `recommendation` exactly as you publish them.

### 2.3 What FairScale gets

- **Onboarding pipeline.** Every SAP agent already has on-chain identity,
  tools, and revenue — they're high-quality candidates for FairScale
  scoring. We'd start with the ~8 mainnet agents and grow as SAP expands.
- **Cross-link traffic.** SAP's explorer publicly links to the FairScale
  profile for every scored agent.
- **Source diversity.** A new `source: "sap"` tag complements your existing
  `said | erc8004 | sati` taxonomy and signals "Solana-native, on-chain
  verified".

### 2.4 What SAP gets

- **Behavioural signal** that we can't compute on-chain alone.
- **Discoverability** for our agents inside the FairScale directory.
- **A unified merchant-readiness score** in our explorer UI (already shipped).

### 2.5 Operational details

| Item | Proposal |
|---|---|
| Auth | Both feeds public; we honour `If-None-Match` / `Last-Modified` to keep your egress low. |
| Rate limit | 1 req/min on our side; we'll respect FairScale's free-tier 1 000 req/day quota. |
| SLA | Best-effort 99% monthly; downtime ≤ 4 h notice via shared Telegram/Discord channel. |
| Versioning | URL path version (`/v1/`); breaking changes ship as `/v2/` with 60-day overlap. |
| Schema source of truth | Both teams co-own a JSON-Schema file in a shared GitHub gist. |

---

## 3. Commercial Terms (proposed)

For both proposals combined, we suggest a fully reciprocal arrangement:

| Item | SAP commitment | FairScale commitment |
|---|---|---|
| API access | Free, unmetered access to the SAP feed for FairScale's prod + staging | Free `Builder`-tier-equivalent access (≥ 20 000 req/day) for the SAP indexer |
| Attribution | "Reputation by FairScale" badge + link on every agent page | "Verified by SAP" badge + link on every `source=sap` directory entry |
| Co-marketing | Joint announcement blog + thread when Proposal A hits mainnet | Same |
| Support | Dedicated Telegram channel; max 1 business day reply | Same |
| Term | 12 months auto-renew; either side may exit with 30-day notice | Same |

No money exchanges hands. The deal is value-for-value and easy to unwind.

---

## 4. Suggested Timeline

| Week | Milestone |
|---|---|
| 0 | This doc reviewed; FairScale signals interest |
| 1 | Joint working session — finalise oracle payload schema + feed JSON Schema |
| 2 | FairScale generates Ed25519 signer key; SAP deploys `attest_external_score` to devnet |
| 3 | End-to-end devnet demo: FairScale signs → SAP records → Explorer renders |
| 4 | Mainnet deploy of Proposal A (SAP upgrade authority) |
| 5 | Public launch + joint announcement; Proposal B feeds go live |
| 8 | First retro; tune feed frequency, oracle TTL, and weights |

---

## 5. Next Steps

1. **FairScale:** review, comment, propose changes inline.
2. **Synapse:** schedule a 30-min call to align on the oracle account
   layout and the JSON-Schema for the directory feed.
3. **Both:** sign a one-page MoU referencing this document.

If anything in this proposal seems oversized — we are happy to ship Proposal
B alone first, and treat Proposal A as a phase-2 milestone. The two are
designed to be independent.

---

*Thank you for building FairScale. We think the Solana agent stack will be
materially stronger if SAP and FairScale interoperate at the protocol layer
rather than at the marketing layer.*
