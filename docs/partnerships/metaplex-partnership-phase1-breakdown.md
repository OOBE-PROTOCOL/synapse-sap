# SAP × Metaplex Core — Phase 1 Technical Breakdown

> **Status:** Phase 1 implemented in `synapse-sap-sdk` `v0.9.0`.
> **Verified against:** `@metaplex-foundation/mpl-core` `>=1.9.0` ([PR #258](https://github.com/metaplex-foundation/mpl-core/pull/258)) and the [`AgentIdentity`](https://mpl-core-js-docs.vercel.app/types/AgentIdentity.html) external plugin.
> **Companion docs:** [`synapse-sap-sdk/skills/metaplex-bridge.md`](../synapse-sap-sdk/skills/metaplex-bridge.md), [`synapse-sap-sdk/docs/11-metaplex-bridge.md`](../synapse-sap-sdk/docs/11-metaplex-bridge.md)

---

## 0. Why we rewrote our own first attempt

Our first prototype assumed `mpl-core` exposed an "executive registry" with
`addExecutive` / `delegateExecutionV1` instructions and tried to perform an
**atomic dual delegation** (one tx writes to SAP, a paired tx writes to MPL).

After verifying directly against the merged `mpl-core` source we found:

| Assumption | Reality |
|---|---|
| `addExecutive(asset, executive, perms)` exists | ❌ Function does not exist. |
| `delegateExecutionV1` exists | ❌ Function does not exist. |
| `AgentIdentity` stores executives on chain | ❌ It stores **only** `{ uri: string }`. |
| Executives / capabilities live in MPL Core program state | ❌ They live in the **EIP-8004 JSON** at the URI. |
| The plugin can be attached to a Collection | ❌ Asset-only (`validate_create` rejects collections). |
| Multiple `AgentIdentity` plugins per asset | ❌ One per asset (`add_external_plugin_adapter`). |

So Phase 1 was redesigned around the **real** mpl-core surface. The
corrected design is also strictly **more efficient** (fewer transactions,
zero SAP program changes).

---

## 1. The verified primitive

`AgentIdentity` is an **asset-only external plugin adapter** with one field:

```rust
pub struct AgentIdentity { pub uri: String }
```

attached via the on-chain instruction:

```
addExternalPluginAdapterV1 {
  asset, collection?, payer, authority, systemProgram, logWrapper?,
  initInfo: ExternalPluginAdapterInitInfo::AgentIdentity(BaseAgentIdentityInitInfoArgs {
    initPluginAuthority,
    lifecycleChecks: Vec<(HookableLifecycleEvent, ExternalCheckResult)>,
    uri,
  }),
}
```

The plugin hooks the `Execute` lifecycle event, allowing the URI authority
to gate execution operations on the asset (this is the cross-protocol hook
that motivates the entire feature).

The **URI must point to an EIP-8004 registration JSON** — the Ethereum
trustless-agent spec being adopted by Metaplex for cross-chain agent
discovery.

---

## 2. The corrected SAP × MPL bridge

```
                ┌──────────────────────────────────────┐
                │   MPL Core Asset (transferable NFT)  │
                │   AgentIdentity.uri ─────┐           │
                └──────────────────────────┼───────────┘
                                           ▼
        https://api.synapse.xyz/agents/<sapAgentPda>/eip-8004.json
                                           │
                                           ▼
                     SAP indexer  ◀──reads──  AgentAccount + VaultDelegate*
```

- **One MPL transaction** (`addExternalPluginAdapterV1` with `AgentIdentity`)
  links the asset to a SAP agent.
- The URI is deterministic: `<base>/agents/<sapAgentPda>/eip-8004.json`.
- The JSON is rendered **live** from on-chain SAP state by the SAP host.
- Every SAP write propagates automatically — no MPL transaction required
  after the initial linking.

---

## 3. SDK surface (`synapse-sap-sdk@0.9.0`)

Module: `client.metaplex` ([`MetaplexBridge`](../synapse-sap-sdk/src/registries/metaplex-bridge.ts))

| Method | Description |
|---|---|
| `deriveRegistrationUrl(pda, baseUrl)` | Pure URL helper |
| `buildEip8004Registration({ sapAgentOwner, services, extra })` | Server-side EIP-8004 JSON builder |
| `buildAttachAgentIdentityIx(opts)` | Single MPL ix that links an asset to a SAP agent |
| `buildUpdateAgentIdentityUriIx(opts)` | Re-point the URI when migrating registry hosts |
| `getUnifiedProfile({ wallet?, asset?, rpcUrl })` | Merged read (SAP `AgentAccount` + MPL asset + EIP-8004 JSON) |
| `verifyLink({ asset, sapAgentPda, rpcUrl })` | Bidirectional cryptographic link check |

Removed from the prototype (because they were built on a fictional API):
`delegateBoth`, `revokeBoth`, `isDelegatedOnBoth`, `MplExecutive`,
`DelegateBothOptions`.

---

## 4. Efficiency comparison

| Operation | Naive dual-on-chain | This bridge |
|---|---|---|
| Initial linking | 2 tx (SAP + MPL) | **1 tx** (MPL only) |
| Add a vault delegate | 2 tx | **1 tx** (SAP only) |
| Revoke a delegate | 2 tx | **1 tx** (SAP only) |
| Capability or x402 tier change | 2 tx | **1 tx** (SAP only) |
| Reads per profile | 2 RPC + 2 deserialisations | 1 RPC + 1 cached fetch |
| MPL programs touched after init | every change | **never** (until host migration) |
| Required SAP program changes | new instructions / fields | **zero** |
| Mainnet migration risk for the 8 live agents | requires migration | **none** |

Every recurring operation drops from 2 tx to 1 tx.

---

## 5. Compatibility & safety

- **No SAP program changes.** The 8 live mainnet agents keep working
  unmodified. The bridge is pure SDK + indexer endpoint.
- **No new on-chain accounts.** Linking is expressed entirely as a single
  MPL plugin URI plus the host-served JSON.
- **Optional peer deps.** `@metaplex-foundation/mpl-core` and `umi-bundle-defaults`
  are `peerDependenciesMeta.optional = true` — only consumers calling
  `client.metaplex.*` install them.
- **Forward compatible.** If we later add `mpl_asset: Pubkey` to
  `AgentAccount` (Phase 3), the SDK shape and `linked` heuristic do not
  break.

---

## 6. EIP-8004 envelope produced by `buildEip8004Registration`

```jsonc
{
  "schema":      "eip-8004/agent-registration/1",
  "synapseAgent": "<sapAgentPda base58>",
  "owner":       "<owner base58>",
  "name":        "...",
  "description": "...",
  "capabilities": ["..."],
  "executives": [
    { "address": "<delegate>", "permissions": 7, "expiresAt": 1234567890 }
  ],
  "services": [
    { "id": "x402-default", "type": "x402-endpoint", "url": "..." }
  ],
  "x402":        { "tiers": [...] },
  "reputation":  { "score": 9740, "feedbacks": 12 },
  "issuedAt":    1700000000,
  "version":     "0.9.0"
}
```

Generated server-side from on-chain state. The `synapseAgent` field is what
makes the link **bidirectional and cryptographic** without an on-chain
SAP-side write.

---

## 7. Roadmap

| Phase | Scope | Status |
|---|---|---|
| **1** | Off-chain bridge: `AgentIdentity` plugin + EIP-8004 host endpoint | ✅ Implemented in SDK 0.9.0 |
| **2** | Marketplace UX: explorer renders the unified profile, "transfer = sell agent" UX | Planned |
| **3 (optional)** | On-chain `mpl_asset: Pubkey` field on `AgentAccount` for fully on-chain link verification | Optional, backwards compatible |

Phase 1 alone already gives Metaplex a verifiable bridge into a live
production agent registry without touching either program's bytecode.

---

## 8. References

- [mpl-core PR #258 — AgentIdentity external plugin](https://github.com/metaplex-foundation/mpl-core/pull/258)
- [`AgentIdentity` type](https://mpl-core-js-docs.vercel.app/types/AgentIdentity.html)
- [`addExternalPluginAdapterV1`](https://mpl-core-js-docs.vercel.app/functions/addExternalPluginAdapterV1.html)
- [EIP-8004 — Trustless Agents](https://eips.ethereum.org/EIPS/eip-8004)
- [SAP SDK — Metaplex Bridge skill](../synapse-sap-sdk/skills/metaplex-bridge.md)
- [SAP SDK — Metaplex Bridge docs](../synapse-sap-sdk/docs/11-metaplex-bridge.md)
