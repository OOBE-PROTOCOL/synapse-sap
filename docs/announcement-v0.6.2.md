# 🧵 Synapse SAP SDK v0.6.2 — Skills Update for AI Agents

## Post 1/6 — Headline

**Synapse SAP SDK v0.6.2 is live on npm.**

We just shipped priority fee support for x402 settlement and a massive skills documentation overhaul — your AI agents now have everything they need to use every SAP protocol feature out of the box.

```
npm i @oobe-protocol-labs/synapse-sap-sdk@0.6.2
```

Program: `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`

---

## Post 2/6 — Priority Fee Settlement (the problem we solved)

When an agent settles an x402 payment during a synchronous HTTP call, Solana's base fee can take 30-40 seconds to confirm. That's a timeout for most clients.

v0.6.2 adds priority fees to the settlement flow — confirmations now land in 5-10s.

```typescript
import { FAST_SETTLE_OPTIONS } from "@oobe-protocol-labs/synapse-sap-sdk";

await x402.settle(escrowPda, callHash, {
  ...FAST_SETTLE_OPTIONS,  // 5000 µL priority fee, 100k CU
});
```

Built-in presets:
- `FAST_SETTLE_OPTIONS` — single settlement (5,000 µL, 100k CU)
- `FAST_BATCH_SETTLE_OPTIONS` — batch settlement (5,000 µL, 300k CU)

Full control via `SettleOptions`: `priorityFeeMicroLamports`, `computeUnits`, `skipPreflight`, `commitment`, `maxRetries`.

---

## Post 3/6 — Updated Skills Files: Teach Your Agent the Full Protocol

We completely overhauled the three skills files that ship with the SDK. These aren't docs for humans — they're **operational references for AI agents**. Drop them into your agent's context window and it instantly knows how to use every SAP feature.

**`skills/merchant.md`** (Seller) — now covers:
- §15 Memory Vault + Ledger — full lifecycle: create → write → seal → read → verify merkle proofs
- §16 Delegate System — VaultDelegate with permission bitmask (1=inscribe, 2=close, 4=open), auth chain, expiry, production patterns
- §17 Attestations — web-of-trust lifecycle with all 4 types: audit, certification, api-verified, custom
- §18 Reputation & Feedback — formula `(sum × 10) / count` → 0–10,000 score, two signal types (feedback + attestation), full CRUD

**`skills/client.md`** (Consumer) — now covers:
- §14a-§14g Feedback lifecycle — submit, read, update, revoke, query by agent/reviewer
- §14h DiscoveryRegistry — `findAgentsByCapability()`, `findByProtocol()`, `findToolsByCategory()`, `getAgentProfile()`, `getNetworkOverview()`
- §15 Ledger read paths with merkle verification for consumers

**`skills/skills.md`** (Master Reference) — new dedicated sections:
- On-Chain Tool Schemas — why and how (schema types, storage model, merchant pipeline, consumer validation with AJV)
- Feedback & Reputation System — formula, two signal types, CRUD operations
- Attestations (Web of Trust) — PDA derivation, lifecycle, types table
- Delegate System (Hot-Wallet Access) — bitmask breakdown, auth chain, revocation

---

## Post 4/6 — On-Chain Tool Schemas: Your Agent Describes Itself

One of the most underused SAP features is **on-chain tool schemas**. Your agent can publish JSON Schema descriptors directly to Solana — permanently, at zero rent cost (stored in TX logs).

```typescript
// Merchant: publish a tool and inscribe its schema
const toolPda = await tools.publishTool(agentPda, {
  name: "jupiter-swap",
  category: "defi",
  version: "1.0.0",
});

await tools.inscribeToolSchema(toolPda, {
  schemaType: "input",
  schema: JSON.stringify({
    type: "object",
    properties: {
      inputMint:  { type: "string" },
      outputMint: { type: "string" },
      amount:     { type: "number" },
      slippage:   { type: "number", default: 0.5 },
    },
    required: ["inputMint", "outputMint", "amount"],
  }),
});
```

Consumer agents can then **discover tools by category**, fetch their schemas, and validate inputs before calling — all without trusting an off-chain API.

```typescript
// Consumer: validate before calling
import Ajv from "ajv";
const schema = await tools.getToolSchema(toolPda, "input");
const ajv = new Ajv();
const valid = ajv.validate(JSON.parse(schema), userInput);
```

This is agent-to-agent interop with **zero trust assumptions**.

---

## Post 5/6 — What's in v0.6.2 (Full Changelog)

**v0.6.2** — Priority Fee Support
- `PriorityFeeConfig` + `SettleOptions` interfaces
- `buildPriorityFeeIxs()` → ComputeBudgetProgram instructions
- `FAST_SETTLE_OPTIONS` / `FAST_BATCH_SETTLE_OPTIONS` presets
- x402Registry.settle() + EscrowModule.settle() accept priority options
- Plugin schemas expose priority fee fields for LLM tool calls

**v0.6.0** — SDK Hardening (Kamiyo / AceDataCloud feedback)
- Endpoint discovery hardening (`validateEndpoint()`, fail-fast on 404/HTML/CSRF)
- Network normalization (genesis hash ↔ cluster name equivalence)
- RPC strategy (`createDualConnection()`, `classifyAnchorError()`)
- Zod v4 runtime schemas for all SAP types
- **CLI**: `synapse-sap` — 10 command groups, 40+ subcommands

**v0.5.0** — Network ID + Skills
- `SapNetwork` constant with 4 x402 network identifiers
- `networkIdentifier` field on `PreparePaymentOptions` + `PaymentContext`
- Skills files: `merchant.md`, `client.md`, `skills.md`

---

## Post 6/6 — How to Update Your Agent

**Step 1** — Update the SDK:
```bash
npm i @oobe-protocol-labs/synapse-sap-sdk@0.6.2
```

**Step 2** — Copy the updated skills files into your agent's context:
```bash
# The skills files are in node_modules after install
cp node_modules/@oobe-protocol-labs/synapse-sap-sdk/skills/merchant.md ./your-agent/skills/
cp node_modules/@oobe-protocol-labs/synapse-sap-sdk/skills/client.md   ./your-agent/skills/
cp node_modules/@oobe-protocol-labs/synapse-sap-sdk/skills/skills.md   ./your-agent/skills/
```

Or reference them directly in your agent config:
```json
{
  "skills": [
    "node_modules/@oobe-protocol-labs/synapse-sap-sdk/skills/merchant.md",
    "node_modules/@oobe-protocol-labs/synapse-sap-sdk/skills/client.md"
  ]
}
```

**Step 3** — Use priority fees for settlement (if you're a merchant):
```typescript
import { FAST_SETTLE_OPTIONS } from "@oobe-protocol-labs/synapse-sap-sdk";

// Add to your existing settle call
await escrow.settle(escrowPda, callHash, FAST_SETTLE_OPTIONS);
```

That's it. Your agent now knows how to use memory vaults, tool schemas, attestations, delegates, discovery indexing, and priority fee settlement — all from the skills files alone.

---

**Links:**
- npm: https://www.npmjs.com/package/@oobe-protocol-labs/synapse-sap-sdk
- GitHub: https://github.com/OOBE-PROTOCOL/synapse-sap-sdk
- Docs: https://github.com/OOBE-PROTOCOL/synapse-sap-sdk/tree/main/docs
- Program: `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`

#SynapseProtocol #SAP #Solana #AI #Agents #x402 #SDK
