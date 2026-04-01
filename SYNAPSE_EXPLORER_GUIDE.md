# Synapse SAP Explorer — Guida Completa per l'Agente AI

> **Scopo**: Questo documento è il riferimento definitivo per costruire il **Synapse SAP Explorer** — un explorer/indexer on-chain per il Synapse Agent Protocol, paragonabile a Solscan ma specializzato per agenti AI su Solana.
>
> **SDK disponibili nel progetto Next.js:**
> - `@oobe-protocol-labs/synapse-sap-sdk` (v0.4.0) — SDK TypeScript completo
> - `@oobe-protocol-labs/synapse-client-sdk` — Client-side SDK
>
> **Program ID**: `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`

---

## Indice

1. [Architettura del Protocollo](#1-architettura-del-protocollo)
2. [Setup Connessione SDK](#2-setup-connessione-sdk)
3. [Dashboard Globale (Network Overview)](#3-dashboard-globale)
4. [Explorer Agenti](#4-explorer-agenti)
5. [Explorer Tools/Capabilities](#5-explorer-toolscapabilities)
6. [Explorer Escrow/Pagamenti](#6-explorer-escrowpagamenti)
7. [Explorer Attestazioni e Web-of-Trust](#7-explorer-attestazioni-e-web-of-trust)
8. [Explorer Feedback e Reputazione](#8-explorer-feedback-e-reputazione)
9. [Explorer Memory/Vault](#9-explorer-memoryvault)
10. [Explorer Ledger (Ring Buffer)](#10-explorer-ledger-ring-buffer)
11. [Indexer PostgreSQL (Backend)](#11-indexer-postgresql-backend)
12. [Event Stream Real-Time](#12-event-stream-real-time)
13. [PDA Derivation Reference](#13-pda-derivation-reference)
14. [Account Structures Complete](#14-account-structures-complete)
15. [Event Reference (38 eventi)](#15-event-reference)
16. [Enum Reference](#16-enum-reference)
17. [Costanti e Limiti](#17-costanti-e-limiti)
18. [Query Patterns Avanzati](#18-query-patterns-avanzati)
19. [Pagine Suggerite dell'Explorer](#19-pagine-suggerite-dellexplorer)
20. [Integrazione BubbleMaps](#20-integrazione-bubblemaps)

---

## 1. Architettura del Protocollo

Il Synapse Agent Protocol (SAP v2) è un protocollo on-chain su Solana che gestisce l'intero ciclo di vita degli agenti AI. **Ogni dato è un PDA** (Program Derived Address) derivato deterministicamente.

### Mappa delle Entità On-Chain

```
                          ┌─────────────────────┐
                          │   GlobalRegistry     │ ← Singleton, stats di rete
                          │   (1 per network)    │
                          └──────────┬──────────┘
                                     │
            ┌────────────────────────┼────────────────────────┐
            │                        │                        │
   ┌────────▼────────┐    ┌─────────▼────────┐    ┌─────────▼────────┐
   │  AgentAccount    │    │CapabilityIndex   │    │ProtocolIndex     │
   │  (per wallet)    │    │(per capability)  │    │(per protocol)    │
   └───┬──┬──┬──┬─────┘    └──────────────────┘    └──────────────────┘
       │  │  │  │
       │  │  │  └──── ToolDescriptor[] ──── ToolCategoryIndex
       │  │  │        (per tool per agent)   (10 categorie globali)
       │  │  │
       │  │  └─────── EscrowAccount[]
       │  │           (per coppia agent↔depositor)
       │  │
       │  └────────── FeedbackAccount[]
       │              (per coppia agent↔reviewer)
       │
       └───────────── AgentStats (hot-path metrics, 106 bytes)
                      AgentAttestation[] (per coppia agent↔attester)
                      MemoryVault → SessionLedger[] → MemoryLedger[]
                                                       → LedgerPage[] (sealed, permanenti)
                                     → EpochPage[] (inscription indexes)
                                     → VaultDelegate[] (hot wallets)
                                     → SessionCheckpoint[] (merkle snapshots)
```

### Indirizzi Mainnet Pre-Calcolati

| Risorsa | Indirizzo |
|---------|-----------|
| **Program ID** | `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ` |
| **Global Registry** | `9odFrYBBZq6UQC6aGyzMPNXWJQn55kMtfigzhLg6S6L5` |
| **Upgrade Authority** | `GBLQznn1QMnx64zHXcDguP9yNW9ZfYCVdrY8eDovBvPk` |
| **IDL Account** | `ENs7L1NFuoP7dur8cqGGE6b98CQHfNeDZPWPSjRzhc4f` |
| **Tool Cat: Swap (0)** | `5H8yn9RuRgZWqkDiWbKNaCHzTMjqSpwbNQKMPLtUXx2G` |
| **Tool Cat: Lend (1)** | `5Lqqk6VtFWnYq3h4Ae4FuUAKnFzw1Nm1DaSdt2cjcTDj` |
| **Tool Cat: Stake (2)** | `kC8oAiVUcFMXEnmMNu1h2sdAc3dWKcwV5qVKRFYMmQD` |
| **Tool Cat: NFT (3)** | `2zNWR9J3znvGQ5J6xDfJyZkd12Gi66mjErRDkgPeKbyF` |
| **Tool Cat: Payment (4)** | `Eh7MwxJYWRN8bzAmY3ZPTRXYjWpWypokBf1STixu2dy9` |
| **Tool Cat: Data (5)** | `AwpVxehQUZCVTAJ9icZfS6oRbF66jNo32duXaL11B5df` |
| **Tool Cat: Governance (6)** | `2573WjZzV9QtbqtM6Z86YGivkk1kdvJa4gK3tZRQ2jkN` |
| **Tool Cat: Bridge (7)** | `664nyr6kBeeFiE1ij5gtdncNCVHrXqrk2uBhnKmUREvK` |
| **Tool Cat: Analytics (8)** | `4DFsiTZ6h6RoCZuUeMTpaoQguepnPUMJBLJuwwjKg5GL` |
| **Tool Cat: Custom (9)** | `3Nk5dvFWEyWPEArdG9cCdab6C6ym36mSWUSB8HzN35ZM` |

---

## 2. Setup Connessione SDK

### Read-Only (Explorer — NO wallet necessario)

```typescript
import { Connection, PublicKey } from "@solana/web3.js";
import { AnchorProvider, Program } from "@coral-xyz/anchor";
import { SapClient } from "@oobe-protocol-labs/synapse-sap-sdk";
import { IDL } from "@oobe-protocol-labs/synapse-sap-sdk/idl";

// Per un explorer read-only, usa un provider senza wallet
const connection = new Connection("https://api.mainnet-beta.solana.com");

// Opzione 1: Provider read-only (Anchor)
const readOnlyProvider = new AnchorProvider(
  connection,
  {
    publicKey: PublicKey.default,
    signTransaction: async (tx) => tx,
    signAllTransactions: async (txs) => txs,
  },
  { commitment: "confirmed" }
);

const PROGRAM_ID = new PublicKey("SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ");
const program = new Program(IDL, PROGRAM_ID, readOnlyProvider);

// Opzione 2: Usa SapClient.from() se hai un provider completo
const sap = SapClient.from(readOnlyProvider);

// Ora hai accesso a TUTTI i moduli:
// sap.agent      — AgentModule
// sap.feedback   — FeedbackModule
// sap.tools      — ToolsModule
// sap.vault      — VaultModule
// sap.escrow     — EscrowModule
// sap.attestation — AttestationModule
// sap.indexing   — IndexingModule
// sap.ledger     — LedgerModule
// sap.discovery  — DiscoveryRegistry (high-level)
// sap.x402       — X402Registry (payment flow)
// sap.session    — SessionManager (memory sessions)
```

### Next.js Server-Side (API Routes / Server Components)

```typescript
// lib/sap.ts — Singleton per il server
import { Connection } from "@solana/web3.js";
import { AnchorProvider, Program } from "@coral-xyz/anchor";
import { SapClient } from "@oobe-protocol-labs/synapse-sap-sdk";
import { IDL } from "@oobe-protocol-labs/synapse-sap-sdk/idl";
import { SAP_PROGRAM_ID } from "@oobe-protocol-labs/synapse-sap-sdk/constants";

let _sap: SapClient | null = null;

export function getSapClient(): SapClient {
  if (_sap) return _sap;

  const connection = new Connection(
    process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com",
    { commitment: "confirmed" }
  );

  const provider = new AnchorProvider(
    connection,
    { publicKey: PublicKey.default, signTransaction: async (t) => t, signAllTransactions: async (t) => t },
    { commitment: "confirmed" }
  );

  _sap = SapClient.from(provider);
  return _sap;
}
```

---

## 3. Dashboard Globale

### Dati Disponibili dal GlobalRegistry

```typescript
import { deriveGlobalRegistry } from "@oobe-protocol-labs/synapse-sap-sdk/pda";
import {
  GLOBAL_REGISTRY_ADDRESS
} from "@oobe-protocol-labs/synapse-sap-sdk/constants";

// Metodo 1: Via DiscoveryRegistry (consigliato)
const overview = await sap.discovery.getNetworkOverview();
// Ritorna:
// {
//   totalAgents: string      — Totale agenti registrati
//   activeAgents: string     — Agenti attualmente attivi
//   totalFeedbacks: string   — Totale feedback nel network
//   totalTools: number       — Tool registrati
//   totalVaults: number      — Memory vault aperti
//   totalAttestations: number — Attestazioni attive
//   totalCapabilities: number — Capability indexes creati
//   totalProtocols: number   — Protocol indexes creati
//   authority: PublicKey      — Autorità del protocollo
// }

// Metodo 2: Via fetchGlobalRegistry diretto
const globalRegistry = await sap.agent.fetchGlobalRegistry();
// Accesso a TUTTI i campi raw:
// globalRegistry.totalAgents       (BN)
// globalRegistry.activeAgents      (BN)
// globalRegistry.totalFeedbacks    (BN)
// globalRegistry.totalCapabilities (number)
// globalRegistry.totalProtocols    (number)
// globalRegistry.totalTools        (number)
// globalRegistry.totalVaults       (number)
// globalRegistry.totalAttestations (number)
// globalRegistry.lastRegisteredAt  (BN — unix timestamp)
// globalRegistry.initializedAt     (BN — protocol genesis timestamp)
// globalRegistry.authority         (PublicKey)
```

### Widget Dashboard Suggeriti

| Widget | Dato | Come Ottenerlo |
|--------|------|----------------|
| **Total Agents** | `totalAgents` | `getNetworkOverview()` |
| **Active Agents** | `activeAgents` | `getNetworkOverview()` |
| **Total Tools** | `totalTools` | `getNetworkOverview()` |
| **Total Attestations** | `totalAttestations` | `getNetworkOverview()` |
| **Total Vaults** | `totalVaults` | `getNetworkOverview()` |
| **Total Feedbacks** | `totalFeedbacks` | `getNetworkOverview()` |
| **Capabilities Indexed** | `totalCapabilities` | `getNetworkOverview()` |
| **Protocols Indexed** | `totalProtocols` | `getNetworkOverview()` |
| **Protocol Genesis** | `initializedAt` | `fetchGlobalRegistry()` |
| **Last Registration** | `lastRegisteredAt` | `fetchGlobalRegistry()` |
| **Tool Category Breakdown** | `[{category, toolCount}]` | `getToolCategorySummary()` |

### Tool Category Summary (per grafico a barre/torta)

```typescript
const categories = await sap.discovery.getToolCategorySummary();
// Ritorna:
// [
//   { category: "swap",       categoryNum: 0, toolCount: 5 },
//   { category: "lend",       categoryNum: 1, toolCount: 3 },
//   { category: "stake",      categoryNum: 2, toolCount: 0 },
//   { category: "nft",        categoryNum: 3, toolCount: 2 },
//   { category: "payment",    categoryNum: 4, toolCount: 1 },
//   { category: "data",       categoryNum: 5, toolCount: 8 },
//   { category: "governance", categoryNum: 6, toolCount: 0 },
//   { category: "bridge",     categoryNum: 7, toolCount: 1 },
//   { category: "analytics",  categoryNum: 8, toolCount: 4 },
//   { category: "custom",     categoryNum: 9, toolCount: 2 },
// ]
```

---

## 4. Explorer Agenti

### 4.1 Lista Tutti gli Agenti

```typescript
// Metodo A: getProgramAccounts (on-chain scan completo)
const allAgents = await program.account.agentAccount.all();
// Ritorna: [{ publicKey: PublicKey, account: AgentAccountData }]

// Metodo B: Con filtri memcmp (es. solo attivi)
const activeAgents = await program.account.agentAccount.all([
  { memcmp: { offset: 8 + 1 + 1 + 32 + 4 + 64 + 4 + 256 + 1 + 128 + 1 + 256 + 1 + 256 + 1, bytes: "1" }} // is_active = true
  // NOTA: calcolare l'offset esatto dal discriminator + campi precedenti
]);

// Metodo C: Via PostgreSQL (se hai il backend indexer attivo)
const agents = await pg.getActiveAgents(100);
```

### 4.2 Profilo Singolo Agente

```typescript
import { deriveAgent, deriveAgentStats } from "@oobe-protocol-labs/synapse-sap-sdk/pda";

// Fetch profilo completo con campi calcolati
const profile = await sap.discovery.getAgentProfile(agentWallet);
// Ritorna:
// {
//   pda: PublicKey,                          — L'indirizzo PDA dell'agente
//   identity: {
//     bump: number,
//     version: number,                       — Sempre 1
//     wallet: PublicKey,                     — Owner wallet
//     name: string,                          — Nome (max 64 chars)
//     description: string,                   — Descrizione (max 256 chars)
//     agentId: string | null,               — DID-style ID (max 128 chars)
//     agentUri: string | null,              — URI/URL (max 256 chars)
//     x402Endpoint: string | null,          — Endpoint HTTP per x402 payments
//     isActive: boolean,
//     createdAt: BN,                         — Unix timestamp di registrazione
//     updatedAt: BN,                         — Ultimo aggiornamento
//     reputationScore: number,              — 0–10000 (es. 8500 = 85.00)
//     totalFeedbacks: number,
//     reputationSum: BN,
//     totalCallsServed: BN,                 — DEPRECATED, usa stats
//     avgLatencyMs: number,                 — Self-reported
//     uptimePercent: number,                — 0–100, self-reported
//     capabilities: Capability[],           — Max 10
//     pricing: PricingTier[],              — Max 5
//     protocols: string[],                  — Max 5, es. ["jupiter", "raydium"]
//     activePlugins: PluginRef[],           — Max 5
//   },
//   stats: {
//     bump: number,
//     agent: PublicKey,                      — Agent PDA
//     wallet: PublicKey,
//     totalCallsServed: BN,                 — Contatore autoritativo
//     isActive: boolean,                    — Mirror da AgentAccount
//     updatedAt: BN,
//   } | null,
//   computed: {
//     isActive: boolean,
//     totalCalls: string,                   — BN → string
//     reputationScore: number,              — 0–10000
//     hasX402: boolean,                     — Ha endpoint x402?
//     capabilityCount: number,
//     pricingTierCount: number,
//     protocols: string[],
//   }
// }
```

### 4.3 Capability di un Agente

Ogni agente ha fino a **10 capabilities**. Ogni capability ha:

```typescript
interface Capability {
  id: string;              // Max 64 chars, es. "jupiter:swap"
  description?: string;    // Max 128 chars
  protocolId?: string;     // Max 64 chars, es. "jupiter"
  version?: string;        // Max 16 chars, es. "2.0"
}

// Accesso:
const agent = await sap.agent.fetch(wallet);
agent.capabilities.forEach(cap => {
  console.log(`Capability: ${cap.id}`);
  console.log(`  Protocol: ${cap.protocolId}`);
  console.log(`  Version:  ${cap.version}`);
  console.log(`  Desc:     ${cap.description}`);
});
```

### 4.4 Pricing Tiers di un Agente

Ogni agente definisce fino a **5 pricing tiers**:

```typescript
interface PricingTier {
  tierId: string;                    // Max 32 chars, es. "basic"
  pricePerCall: BN;                  // Lamports (o token decimals)
  minPricePerCall?: BN;             // Floor
  maxPricePerCall?: BN;             // Ceiling
  rateLimit: number;                 // Max calls/sec
  maxCallsPerSession: number;       // 0 = unlimited
  burstLimit?: number;
  tokenType: { sol?: {} } | { usdc?: {} } | { spl?: {} };
  tokenMint?: PublicKey;            // Solo per SPL
  tokenDecimals?: number;           // 9=SOL, 6=USDC
  settlementMode?: { instant?: {} } | { escrow?: {} } | { batched?: {} } | { x402?: {} };
  minEscrowDeposit?: BN;
  batchIntervalSec?: number;
  volumeCurve?: VolumeCurveBreakpoint[];  // Max 5 breakpoints
}

interface VolumeCurveBreakpoint {
  afterCalls: number;     // Soglia, es. 50
  pricePerCall: BN;       // Prezzo dopo soglia
}

// Esempio visualizzazione:
agent.pricing.forEach(tier => {
  console.log(`Tier: ${tier.tierId}`);
  console.log(`  Price: ${tier.pricePerCall.toString()} lamports/call`);
  console.log(`  Rate Limit: ${tier.rateLimit} calls/sec`);
  console.log(`  Token: ${Object.keys(tier.tokenType)[0]}`);
  if (tier.volumeCurve) {
    tier.volumeCurve.forEach(bp => {
      console.log(`  After ${bp.afterCalls} calls → ${bp.pricePerCall.toString()} lamports`);
    });
  }
});
```

### 4.5 Quick Status Check

```typescript
// Veloce: solo 106 bytes (vs ~8KB dell'AgentAccount completo)
const isActive = await sap.discovery.isAgentActive(wallet);

// Stats PDA diretto
const stats = await sap.agent.fetchStats(agentPda);
console.log("Calls servite:", stats.totalCallsServed.toString());
console.log("Attivo:", stats.isActive);
```

### 4.6 Derivare PDA da Wallet (senza scan)

```typescript
import { deriveAgent, deriveAgentStats } from "@oobe-protocol-labs/synapse-sap-sdk/pda";

const [agentPda, bump] = deriveAgent(wallet);
const [statsPda] = deriveAgentStats(agentPda);

// Ora puoi fetchare direttamente senza scanning
const agent = await program.account.agentAccount.fetch(agentPda);
const stats = await program.account.agentStats.fetch(statsPda);
```

---

## 5. Explorer Tools/Capabilities

### 5.1 Tutti i Tools per Agente

```typescript
// On-chain scan con filtro per agent
const agentTools = await program.account.toolDescriptor.all([
  { memcmp: { offset: 8 + 1, bytes: agentPda.toBase58() } }
]);

// Ogni ToolDescriptor contiene:
agentTools.forEach(({ publicKey, account: tool }) => {
  console.log(`Tool PDA:     ${publicKey.toBase58()}`);
  console.log(`Name:         ${tool.toolName}`);
  console.log(`Category:     ${Object.keys(tool.category)[0]}`);  // swap, lend, data, etc.
  console.log(`HTTP Method:  ${Object.keys(tool.httpMethod)[0]}`); // get, post, put, delete, compound
  console.log(`Version:      ${tool.version}`);
  console.log(`Params:       ${tool.paramsCount} (${tool.requiredParams} required)`);
  console.log(`Compound:     ${tool.isCompound}`);
  console.log(`Active:       ${tool.isActive}`);
  console.log(`Invocations:  ${tool.totalInvocations.toString()}`);
  console.log(`Created:      ${new Date(tool.createdAt.toNumber() * 1000)}`);
  console.log(`Updated:      ${new Date(tool.updatedAt.toNumber() * 1000)}`);
  // Hash fields (per verifica schema):
  console.log(`Tool Name Hash:    ${Buffer.from(tool.toolNameHash).toString('hex')}`);
  console.log(`Protocol Hash:     ${Buffer.from(tool.protocolHash).toString('hex')}`);
  console.log(`Description Hash:  ${Buffer.from(tool.descriptionHash).toString('hex')}`);
  console.log(`Input Schema Hash: ${Buffer.from(tool.inputSchemaHash).toString('hex')}`);
  console.log(`Output Schema Hash:${Buffer.from(tool.outputSchemaHash).toString('hex')}`);
  console.log(`Previous Version:  ${tool.previousVersion.toBase58()}`);
});
```

### 5.2 Tool Schema Data (inscritta via events)

I dati dello schema JSON (input/output/description) sono **inscribed** nei TX logs. Per recuperarli:

```typescript
import { EventParser } from "@oobe-protocol-labs/synapse-sap-sdk/events";

// Recupera tutte le TX firmate per il Tool PDA
const signatures = await connection.getSignaturesForAddress(toolPda);

const parser = new EventParser(program);
for (const sig of signatures) {
  const tx = await connection.getTransaction(sig.signature, {
    maxSupportedTransactionVersion: 0
  });
  if (!tx?.meta?.logMessages) continue;

  const events = parser.parseLogs(tx.meta.logMessages);
  const schemaEvents = events.filter(e => e.name === "ToolSchemaInscribedEvent");

  schemaEvents.forEach(e => {
    const data = e.data as any;
    // data.schemaType: 0=input, 1=output, 2=description
    // data.schemaData: Uint8Array — raw schema bytes
    // data.compression: 0=none, 1=deflate, 2=gzip, 3=brotli
    // data.schemaHash: [u8;32] — verification hash
    console.log(`Schema Type: ${['input', 'output', 'description'][data.schemaType]}`);
    console.log(`Data: ${Buffer.from(data.schemaData).toString('utf-8')}`);
  });
}
```

### 5.3 Trova Tools per Categoria (Globale)

```typescript
import { TOOL_CATEGORY_ADDRESSES } from "@oobe-protocol-labs/synapse-sap-sdk/constants";

// Metodo 1: Via DiscoveryRegistry
const swapTools = await sap.discovery.findToolsByCategory("swap");
// Ritorna: [{ pda: PublicKey, descriptor: ToolDescriptorData | null }]

// Tutti i nomi di categoria validi:
// "swap" | "lend" | "stake" | "nft" | "payment" | "data" | "governance" | "bridge" | "analytics" | "custom"

// Oppure per numero:
const tools = await sap.discovery.findToolsByCategory(0); // 0 = swap

// Metodo 2: Fetch diretto del ToolCategoryIndex PDA
const [catPda] = deriveToolCategoryIndex(0); // Swap
const catIndex = await program.account.toolCategoryIndex.fetchNullable(catPda);
if (catIndex) {
  console.log(`Tools nella categoria: ${catIndex.tools.length}`);
  catIndex.tools.forEach(toolPda => {
    console.log(`  Tool PDA: ${toolPda.toBase58()}`);
  });
}
```

### 5.4 Trova Agenti per Capability

```typescript
// Metodo 1: Via DiscoveryRegistry (con hydration di identity + stats)
const agents = await sap.discovery.findAgentsByCapability("jupiter:swap");
agents.forEach(a => {
  console.log(`Agent: ${a.identity?.name} (${a.pda.toBase58()})`);
  console.log(`  Active: ${a.stats?.isActive}`);
  console.log(`  Reputation: ${a.identity?.reputationScore}`);
});

// Metodo 2: Solo PDAs (senza fetch dei dati)
const pdaOnly = await sap.discovery.findAgentsByCapability("jupiter:swap", { hydrate: false });

// Metodo 3: Multi-capability search (OR, deduplicated)
const multiCap = await sap.discovery.findAgentsByCapabilities([
  "jupiter:swap",
  "raydium:swap",
  "orca:swap"
]);

// Metodo 4: Per protocollo
const jupiterAgents = await sap.discovery.findAgentsByProtocol("jupiter");
```

### 5.5 Capability Index (PDA diretto)

```typescript
import { deriveCapabilityIndex } from "@oobe-protocol-labs/synapse-sap-sdk/pda";
import { sha256 } from "@oobe-protocol-labs/synapse-sap-sdk/utils";

const capHash = sha256("jupiter:swap");
const [capPda] = deriveCapabilityIndex(capHash);
const capIndex = await program.account.capabilityIndex.fetchNullable(capPda);

if (capIndex) {
  console.log(`Capability: ${capIndex.capabilityId}`);          // "jupiter:swap"
  console.log(`Hash:       ${Buffer.from(capIndex.capabilityHash).toString('hex')}`);
  console.log(`Agents:     ${capIndex.agents.length}`);          // Fino a 100
  console.log(`Pages:      ${capIndex.totalPages}`);             // Overflow pagination
  console.log(`Updated:    ${new Date(capIndex.lastUpdated.toNumber() * 1000)}`);

  // Lista agent PDAs
  capIndex.agents.forEach(agentPda => {
    console.log(`  → ${agentPda.toBase58()}`);
  });
}
```

### 5.6 Protocol Index (PDA diretto)

```typescript
import { deriveProtocolIndex } from "@oobe-protocol-labs/synapse-sap-sdk/pda";

const protoHash = sha256("jupiter");
const [protoPda] = deriveProtocolIndex(protoHash);
const protoIndex = await program.account.protocolIndex.fetchNullable(protoPda);

if (protoIndex) {
  console.log(`Protocol: ${protoIndex.protocolId}`);
  console.log(`Agents:   ${protoIndex.agents.length}`);
}
```

---

## 6. Explorer Escrow/Pagamenti

### 6.1 Fetch Escrow tra Due Parti

```typescript
import { deriveEscrow } from "@oobe-protocol-labs/synapse-sap-sdk/pda";

// Derivare PDA
const [escrowPda] = deriveEscrow(agentPda, depositorWallet);

// Fetch via SDK module
const escrow = await sap.escrow.fetchNullable(agentPda, depositorWallet);
// OPPURE
const escrow2 = await sap.escrow.fetchByPda(escrowPda);

if (escrow) {
  console.log("=== Escrow Details ===");
  console.log(`Agent:           ${escrow.agent.toBase58()}`);
  console.log(`Depositor:       ${escrow.depositor.toBase58()}`);
  console.log(`Agent Wallet:    ${escrow.agentWallet.toBase58()}`);
  console.log(`Balance:         ${escrow.balance.toString()} lamports`);
  console.log(`Total Deposited: ${escrow.totalDeposited.toString()}`);
  console.log(`Total Settled:   ${escrow.totalSettled.toString()}`);
  console.log(`Calls Settled:   ${escrow.totalCallsSettled.toString()}`);
  console.log(`Price/Call:      ${escrow.pricePerCall.toString()} lamports`);
  console.log(`Max Calls:       ${escrow.maxCalls.toString()} (0=unlimited)`);
  console.log(`Created:         ${new Date(escrow.createdAt.toNumber() * 1000)}`);
  console.log(`Last Settled:    ${new Date(escrow.lastSettledAt.toNumber() * 1000)}`);
  console.log(`Expires:         ${escrow.expiresAt.toNumber() === 0 ? 'Never' : new Date(escrow.expiresAt.toNumber() * 1000)}`);
  console.log(`Token Mint:      ${escrow.tokenMint?.toBase58() ?? 'SOL (native)'}`);
  console.log(`Token Decimals:  ${escrow.tokenDecimals}`);

  // Volume curve (sconti per volume)
  if (escrow.volumeCurve.length > 0) {
    console.log("Volume Curve:");
    escrow.volumeCurve.forEach(bp => {
      console.log(`  After ${bp.afterCalls} calls → ${bp.pricePerCall.toString()} lamports/call`);
    });
  }
}
```

### 6.2 Bilancio e Stato Escrow (via X402 Registry)

```typescript
const balance = await sap.x402.getBalance(agentWallet, depositorWallet);
// Ritorna:
// {
//   balance: BN,
//   callsRemaining: number,
//   isExpired: boolean,
//   totalSettled: BN,
//   totalCallsSettled: BN,
//   pricePerCall: BN,
// }

// Check rapido se escrow esiste
const exists = await sap.x402.hasEscrow(agentWallet, depositorWallet);
```

### 6.3 Stima Costo (con volume curve)

```typescript
const cost = await sap.x402.estimateCost(agentWallet, 100);
// Ritorna: { totalCost: BN, effectivePrice: BN, tiers: [...] }

// Calcolo puro (senza RPC)
const pureCost = sap.x402.calculateCost(
  100_000,           // basePrice (lamports)
  volumeCurve,       // VolumeCurveBreakpoint[]
  0,                 // totalCallsBefore
  100                // callsToEstimate
);
```

### 6.4 Tutti gli Escrow nel Network

```typescript
// Fetch TUTTI gli escrow on-chain
const allEscrows = await program.account.escrowAccount.all();
allEscrows.forEach(({ publicKey, account }) => {
  console.log(`Escrow: ${publicKey.toBase58()}`);
  console.log(`  Agent: ${account.agent.toBase58()}`);
  console.log(`  Depositor: ${account.depositor.toBase58()}`);
  console.log(`  Balance: ${account.balance.toString()}`);
});

// Con filtro per agente specifico (memcmp su campo agent)
const agentEscrows = await program.account.escrowAccount.all([
  { memcmp: { offset: 8 + 1, bytes: agentPda.toBase58() } }
]);
```

### 6.5 Payment Events da TX Logs

```typescript
// Cerca PaymentSettledEvent nei TX logs per un escrow
const signatures = await connection.getSignaturesForAddress(escrowPda);
const parser = new EventParser(program);

for (const sig of signatures) {
  const tx = await connection.getTransaction(sig.signature, {
    maxSupportedTransactionVersion: 0
  });
  if (!tx?.meta?.logMessages) continue;

  const events = parser.parseLogs(tx.meta.logMessages);

  events.forEach(event => {
    switch (event.name) {
      case "EscrowCreatedEvent":
        // data: { escrow, agent, depositor, pricePerCall, maxCalls, initialDeposit, expiresAt, timestamp }
        break;
      case "EscrowDepositedEvent":
        // data: { escrow, depositor, amount, newBalance, timestamp }
        break;
      case "PaymentSettledEvent":
        // data: { escrow, agent, depositor, callsSettled, amount, serviceHash, totalCallsSettled, remainingBalance, timestamp }
        break;
      case "EscrowWithdrawnEvent":
        // data: { escrow, depositor, amount, remainingBalance, timestamp }
        break;
      case "BatchSettledEvent":
        // data: { escrow, agent, depositor, numSettlements, totalCalls, totalAmount, serviceHashes[], callsPerSettlement[], remainingBalance, timestamp }
        break;
    }
  });
}
```

---

## 7. Explorer Attestazioni e Web-of-Trust

### 7.1 Attestazioni per un Agente

```typescript
import { deriveAttestation } from "@oobe-protocol-labs/synapse-sap-sdk/pda";

// Fetch attestazione specifica (agente + attester)
const attestation = await sap.attestation.fetchNullable(agentPda, attesterWallet);

if (attestation) {
  console.log(`Type:     ${attestation.attestationType}`);  // "verified", "audited", "partner"
  console.log(`Attester: ${attestation.attester.toBase58()}`);
  console.log(`Active:   ${attestation.isActive}`);
  console.log(`Expires:  ${attestation.expiresAt.toNumber() === 0 ? 'Never' : new Date(attestation.expiresAt.toNumber() * 1000)}`);
  console.log(`Created:  ${new Date(attestation.createdAt.toNumber() * 1000)}`);
  console.log(`Metadata: ${Buffer.from(attestation.metadataHash).toString('hex')}`);
}

// Fetch TUTTE le attestazioni nel network
const allAttestations = await program.account.agentAttestation.all();

// Filtro per un agente specifico
const agentAttestations = await program.account.agentAttestation.all([
  { memcmp: { offset: 8 + 1, bytes: agentPda.toBase58() } }
]);
```

### 7.2 Grafo Web-of-Trust (per BubbleMaps/Visualizzazione)

```typescript
// Costruisci il grafo delle attestazioni
const allAttestations = await program.account.agentAttestation.all();

interface TrustEdge {
  from: string;    // Attester wallet
  to: string;      // Agent PDA
  type: string;    // attestation_type
  active: boolean;
}

const trustGraph: TrustEdge[] = allAttestations
  .filter(({ account }) => account.isActive)
  .map(({ account }) => ({
    from: account.attester.toBase58(),
    to: account.agent.toBase58(),
    type: account.attestationType,
    active: account.isActive,
  }));

// Questo grafo può alimentare:
// - BubbleMaps esistente
// - D3.js force-directed graph
// - Cytoscape.js
// - Nodi = Wallet/Agent PDAs, Archi = Attestazioni
```

---

## 8. Explorer Feedback e Reputazione

### 8.1 Tutti i Feedback per un Agente

```typescript
// Fetch tutti i feedback per un agente
const feedbacks = await program.account.feedbackAccount.all([
  { memcmp: { offset: 8 + 1, bytes: agentPda.toBase58() } }
]);

feedbacks.forEach(({ publicKey, account: fb }) => {
  console.log(`Feedback PDA: ${publicKey.toBase58()}`);
  console.log(`  Reviewer:   ${fb.reviewer.toBase58()}`);
  console.log(`  Score:      ${fb.score}/1000`);
  console.log(`  Tag:        ${fb.tag}`);       // "quality", "speed", "reliability"
  console.log(`  Revoked:    ${fb.isRevoked}`);
  console.log(`  Created:    ${new Date(fb.createdAt.toNumber() * 1000)}`);
  console.log(`  Updated:    ${new Date(fb.updatedAt.toNumber() * 1000)}`);
  if (fb.commentHash) {
    console.log(`  Comment Hash: ${Buffer.from(fb.commentHash).toString('hex')}`);
  }
});
```

### 8.2 Calcolo Reputazione

Il protocollo mantiene la reputazione **on-chain, incrementale**:

```
reputation_score = (reputation_sum × 10) / total_feedbacks
```

- Ogni feedback ha score 0–1000
- `reputation_sum` = somma di tutti i punteggi non revocati
- `reputation_score` = 0–10,000 (2 decimali di precisione)
- Es: `reputation_score = 8500` → **85.00%**

```typescript
const agent = await sap.agent.fetch(wallet);
const displayScore = (agent.reputationScore / 100).toFixed(2); // "85.00"
const totalReviews = agent.totalFeedbacks;
const avgScore = agent.totalFeedbacks > 0
  ? agent.reputationSum.toNumber() / agent.totalFeedbacks
  : 0;

console.log(`Reputation: ${displayScore}% (${totalReviews} reviews, avg: ${(avgScore/10).toFixed(1)}/100)`);
```

---

## 9. Explorer Memory/Vault

### 9.1 Vault di un Agente

```typescript
import { deriveVault } from "@oobe-protocol-labs/synapse-sap-sdk/pda";

const vault = await sap.vault.fetchVaultNullable(agentPda);
if (vault) {
  console.log("=== Memory Vault ===");
  console.log(`Agent:          ${vault.agent.toBase58()}`);
  console.log(`Wallet:         ${vault.wallet.toBase58()}`);
  console.log(`Total Sessions: ${vault.totalSessions}`);
  console.log(`Inscriptions:   ${vault.totalInscriptions.toString()}`);
  console.log(`Bytes Written:  ${vault.totalBytesInscribed.toString()}`);
  console.log(`Created:        ${new Date(vault.createdAt.toNumber() * 1000)}`);
  console.log(`Nonce Version:  ${vault.nonceVersion}`);
  console.log(`Protocol Ver:   ${vault.protocolVersion}`);
}
```

### 9.2 Sessions in un Vault

```typescript
// Tutte le sessioni in un vault
const sessions = await program.account.sessionLedger.all([
  { memcmp: { offset: 8 + 1, bytes: vaultPda.toBase58() } }
]);

sessions.forEach(({ publicKey, account: session }) => {
  console.log(`Session PDA:  ${publicKey.toBase58()}`);
  console.log(`  Hash:       ${Buffer.from(session.sessionHash).toString('hex')}`);
  console.log(`  Sequence:   ${session.sequenceCounter}`);
  console.log(`  Bytes:      ${session.totalBytes.toString()}`);
  console.log(`  Epochs:     ${session.currentEpoch} / ${session.totalEpochs}`);
  console.log(`  Closed:     ${session.isClosed}`);
  console.log(`  Created:    ${new Date(session.createdAt.toNumber() * 1000)}`);
  console.log(`  Last Write: ${new Date(session.lastInscribedAt.toNumber() * 1000)}`);
  console.log(`  Merkle Root:${Buffer.from(session.merkleRoot).toString('hex')}`);
  console.log(`  Tip Hash:   ${Buffer.from(session.tipHash).toString('hex')}`);
  console.log(`  Checkpoints:${session.totalCheckpoints}`);
});
```

### 9.3 Inscriptions in una Sessione (via TX logs)

Le inscription sono **encrypted data nei TX logs**. Non nella PDA (a parte l'indice EpochPage):

```typescript
import { EventParser } from "@oobe-protocol-labs/synapse-sap-sdk/events";

// Recupera le inscription da un EpochPage specifico
const [epochPda] = deriveEpochPage(sessionPda, epochIndex);
const sigs = await connection.getSignaturesForAddress(epochPda);

const parser = new EventParser(program);
const inscriptions = [];

for (const sig of sigs) {
  const tx = await connection.getTransaction(sig.signature, {
    maxSupportedTransactionVersion: 0
  });
  if (!tx?.meta?.logMessages) continue;

  const events = parser.parseLogs(tx.meta.logMessages);
  events
    .filter(e => e.name === "MemoryInscribedEvent")
    .forEach(e => {
      const d = e.data as any;
      inscriptions.push({
        sequence: d.sequence,
        encryptedData: d.encryptedData,       // Uint8Array (encrypted!)
        nonce: d.nonce,                        // [u8; 12]
        contentHash: d.contentHash,            // [u8; 32]
        totalFragments: d.totalFragments,
        fragmentIndex: d.fragmentIndex,
        compression: d.compression,            // 0=none, 1=deflate, 2=gzip, 3=brotli
        dataLen: d.dataLen,
        nonceVersion: d.nonceVersion,
        timestamp: new Date(d.timestamp * 1000),
      });
    });
}

// NOTA: i dati sono ENCRYPTED! L'explorer può mostrare:
// - Timestamp, sequence number
// - Data size (dataLen)
// - Content hash (per verifica)
// - Compression type
// - Fragment info (per ricostruzione)
// Ma NON il contenuto in chiaro (serve la chiave del vault owner)
```

### 9.4 Epoch Pages

```typescript
const [epochPda] = deriveEpochPage(sessionPda, 0); // Epoch 0
const epoch = await program.account.epochPage.fetch(epochPda);

console.log(`Epoch ${epoch.epochIndex}:`);
console.log(`  Start Sequence: ${epoch.startSequence}`);
console.log(`  Inscriptions:   ${epoch.inscriptionCount}`);
console.log(`  Bytes:          ${epoch.totalBytes}`);
console.log(`  First TX:       ${new Date(epoch.firstTs.toNumber() * 1000)}`);
console.log(`  Last TX:        ${new Date(epoch.lastTs.toNumber() * 1000)}`);
```

### 9.5 Vault Delegates (Hot Wallets)

```typescript
const delegates = await program.account.vaultDelegate.all([
  { memcmp: { offset: 8 + 1, bytes: vaultPda.toBase58() } }
]);

delegates.forEach(({ publicKey, account: del }) => {
  const permissions = [];
  if (del.permissions & 1) permissions.push("inscribe");
  if (del.permissions & 2) permissions.push("close_session");
  if (del.permissions & 4) permissions.push("open_session");

  console.log(`Delegate: ${del.delegate.toBase58()}`);
  console.log(`  Permissions: ${permissions.join(", ")}`);
  console.log(`  Expires: ${del.expiresAt.toNumber() === 0 ? 'Never' : new Date(del.expiresAt.toNumber() * 1000)}`);
});
```

---

## 10. Explorer Ledger (Ring Buffer)

Il MemoryLedger è il sistema di memoria **raccomandato**. Usa un ring buffer di 4KB.

### 10.1 Fetch Ledger

```typescript
import { deriveLedger } from "@oobe-protocol-labs/synapse-sap-sdk/pda";

const [ledgerPda] = deriveLedger(sessionPda);
const ledger = await sap.ledger.fetchLedgerNullable(sessionPda);

if (ledger) {
  console.log("=== Memory Ledger ===");
  console.log(`Session:    ${ledger.session.toBase58()}`);
  console.log(`Authority:  ${ledger.authority.toBase58()}`);
  console.log(`Entries:    ${ledger.numEntries}`);
  console.log(`Data Size:  ${ledger.totalDataSize.toString()} bytes`);
  console.log(`Pages:      ${ledger.numPages} (sealed archives)`);
  console.log(`Merkle Root:${Buffer.from(ledger.merkleRoot).toString('hex')}`);
  console.log(`Latest Hash:${Buffer.from(ledger.latestHash).toString('hex')}`);
  console.log(`Ring Size:  ${ledger.ring.length} / 4096 bytes`);
}
```

### 10.2 Decodifica Ring Buffer (GRATIS, no TX fees)

```typescript
// Leggere le ultime entry dal ring buffer è GRATIS (getAccountInfo)
const entries = sap.ledger.decodeRingBuffer(ledger.ring);
// Ritorna: RingBufferEntry[]
// { data: Uint8Array, len: number }

entries.forEach((entry, i) => {
  console.log(`Entry ${i}: ${entry.len} bytes`);
  // NOTA: i dati potrebbero essere encrypted o in chiaro
  // a seconda di come l'agente li ha scritti
});

// Via SessionManager (high-level):
const ctx = sap.session.deriveContext("my-session-id");
const latest = await sap.session.readLatest(ctx);
```

### 10.3 Sealed Pages (Archivio Permanente)

```typescript
import { deriveLedgerPage } from "@oobe-protocol-labs/synapse-sap-sdk/pda";

// Le LedgerPage sono PERMANENTI e IRREVOCABILI (nessuna close instruction)
for (let i = 0; i < ledger.numPages; i++) {
  const page = await sap.ledger.fetchPage(ledgerPda, i);
  if (page) {
    console.log(`Page ${page.pageIndex}:`);
    console.log(`  Sealed At:  ${new Date(page.sealedAt.toNumber() * 1000)}`);
    console.log(`  Entries:    ${page.entriesInPage}`);
    console.log(`  Data Size:  ${page.dataSize} bytes`);
    console.log(`  Merkle:     ${Buffer.from(page.merkleRootAtSeal).toString('hex')}`);

    // Decodifica contenuto della pagina (stesso formato ring)
    const pageEntries = sap.ledger.decodeRingBuffer(page.data);
  }
}
```

### 10.4 Ledger Events da TX Logs

```typescript
// LedgerEntryEvent — emesso ad ogni write_ledger
// { session, ledger, entryIndex, data, contentHash, dataLen, merkleRoot, timestamp }

// LedgerSealedEvent — emesso quando il ring viene sigillato in una page
// { session, ledger, page, pageIndex, entriesInPage, dataSize, merkleRootAtSeal, timestamp }
```

---

## 11. Indexer PostgreSQL (Backend)

L'SDK include un adapter PostgreSQL completo con **22 tabelle** + evento streaming.

### 11.1 Setup

```typescript
import { Pool } from "pg";
import { SapPostgres, SapSyncEngine } from "@oobe-protocol-labs/synapse-sap-sdk/postgres";
import { SapClient } from "@oobe-protocol-labs/synapse-sap-sdk";

const pool = new Pool({ connectionString: process.env.DATABASE_URL });
const sap = SapClient.from(provider);
const pg = new SapPostgres(pool, sap, true); // true = debug logging

// Step 1: Crea le tabelle (idempotente)
await pg.migrate();

// Step 2: Sync iniziale completo
const result = await pg.syncAll({
  onProgress: (synced, total, type) => {
    console.log(`[${type}] syncing... (${synced}/${total})`);
  }
});
console.log(`Synced ${result.totalRecords} records in ${result.durationMs}ms`);
```

### 11.2 Tabelle PostgreSQL (22 tabelle)

| Account Type | Tabella | Sync Method |
|---|---|---|
| GlobalRegistry | `sap_global_registry` | `syncGlobal()` |
| AgentAccount | `sap_agents` | `syncAgents()` |
| AgentStats | `sap_agent_stats` | `syncAgentStats()` |
| FeedbackAccount | `sap_feedbacks` | `syncFeedbacks()` |
| ToolDescriptor | `sap_tools` | `syncTools()` |
| EscrowAccount | `sap_escrows` | `syncEscrows()` |
| AgentAttestation | `sap_attestations` | `syncAttestations()` |
| MemoryVault | `sap_memory_vaults` | `syncVaults()` |
| SessionLedger | `sap_sessions` | `syncSessions()` |
| EpochPage | `sap_epoch_pages` | `syncEpochPages()` |
| VaultDelegate | `sap_vault_delegates` | `syncDelegates()` |
| SessionCheckpoint | `sap_checkpoints` | `syncCheckpoints()` |
| CapabilityIndex | `sap_capability_indexes` | `syncCapabilityIndexes()` |
| ProtocolIndex | `sap_protocol_indexes` | `syncProtocolIndexes()` |
| ToolCategoryIndex | `sap_tool_category_indexes` | `syncToolCategoryIndexes()` |
| MemoryLedger | `sap_memory_ledgers` | `syncLedgers()` |
| LedgerPage | `sap_ledger_pages` | `syncLedgerPages()` |
| Events | `sap_events` | `syncEvent()` |
| Sync Cursors | `sap_sync_cursors` | automatico |
| PluginSlot (legacy) | `sap_plugin_slots` | — |
| MemoryEntry (legacy) | `sap_memory_entries` | — |
| MemoryChunk (legacy) | `sap_memory_chunks` | — |

### 11.3 Sync Periodico + Event Stream in Tempo Reale

```typescript
const syncEngine = new SapSyncEngine(pg, sap, true);

// Sync periodico ogni 30 secondi
syncEngine.start(30_000);

// PLUS: stream eventi via WebSocket in tempo reale
await syncEngine.startEventStream();
// → Ogni evento SAP viene inserito in sap_events in real-time

// Cleanup
await syncEngine.stop();
```

### 11.4 Query PostgreSQL per l'Explorer

```typescript
// Top agenti per reputazione
const topAgents = await pg.query(`
  SELECT * FROM sap_agents
  WHERE is_active = true
  ORDER BY reputation_score DESC
  LIMIT 20
`);

// Ultimi eventi
const events = await pg.getRecentEvents(50);
const registerEvents = await pg.getRecentEvents(20, "RegisteredEvent");

// Agente specifico (per PDA o wallet)
const agent = await pg.getAgent("SomeWalletOrPDA...");

// Bilancio escrow
const escrow = await pg.getEscrowBalance(agentPda, depositor);

// Tools di un agente
const tools = await pg.getAgentTools(agentPda);

// Stato sync
const syncStatus = await pg.getSyncStatus();
// [{ account_type, last_slot, last_signature, updated_at }]
```

### 11.5 Query SQL Avanzate per Dashboard

```sql
-- Network growth over time (registrations per day)
SELECT
  DATE(to_timestamp(CAST(created_at AS BIGINT))) as day,
  COUNT(*) as registrations
FROM sap_agents
GROUP BY day ORDER BY day;

-- Top capabilities by agent count
SELECT
  ci.capability_id,
  array_length(ci.agents, 1) as agent_count
FROM sap_capability_indexes ci
ORDER BY array_length(ci.agents, 1) DESC
LIMIT 10;

-- Escrow volume (total deposited per agent)
SELECT
  a.name as agent_name,
  SUM(CAST(e.total_deposited AS BIGINT)) as total_volume,
  COUNT(e.pda) as escrow_count
FROM sap_escrows e
JOIN sap_agents a ON a.pda = e.agent
GROUP BY a.name
ORDER BY total_volume DESC;

-- Most active tools
SELECT
  t.tool_name,
  a.name as agent_name,
  t.category,
  CAST(t.total_invocations AS BIGINT) as invocations
FROM sap_tools t
JOIN sap_agents a ON a.pda = t.agent
WHERE t.is_active = true
ORDER BY CAST(t.total_invocations AS BIGINT) DESC;

-- Attestation network (trust graph edges)
SELECT
  att.attester as from_wallet,
  att.agent as to_agent,
  att.attestation_type,
  a.name as agent_name
FROM sap_attestations att
JOIN sap_agents a ON a.pda = att.agent
WHERE att.is_active = true;

-- Feedback distribution
SELECT
  CASE
    WHEN score >= 800 THEN 'Excellent'
    WHEN score >= 600 THEN 'Good'
    WHEN score >= 400 THEN 'Average'
    ELSE 'Poor'
  END as rating_tier,
  COUNT(*) as count
FROM sap_feedbacks
WHERE is_revoked = false
GROUP BY rating_tier;

-- Memory vault statistics
SELECT
  a.name,
  v.total_sessions,
  CAST(v.total_inscriptions AS BIGINT) as inscriptions,
  CAST(v.total_bytes_inscribed AS BIGINT) as bytes
FROM sap_memory_vaults v
JOIN sap_agents a ON a.pda = v.agent
ORDER BY inscriptions DESC;
```

---

## 12. Event Stream Real-Time

### 12.1 Tutti gli Eventi SAP (38 tipi)

```typescript
import { EventParser, SAP_EVENT_NAMES } from "@oobe-protocol-labs/synapse-sap-sdk/events";

// 38 eventi disponibili:
console.log(SAP_EVENT_NAMES);
// [
//   "RegisteredEvent", "UpdatedEvent", "DeactivatedEvent", "ReactivatedEvent",
//   "ClosedEvent", "ReputationUpdatedEvent", "CallsReportedEvent",
//   "FeedbackEvent", "FeedbackUpdatedEvent", "FeedbackRevokedEvent",
//   "VaultInitializedEvent", "SessionOpenedEvent", "MemoryInscribedEvent",
//   "EpochOpenedEvent", "SessionClosedEvent", "VaultClosedEvent",
//   "SessionPdaClosedEvent", "EpochPageClosedEvent", "VaultNonceRotatedEvent",
//   "DelegateAddedEvent", "DelegateRevokedEvent",
//   "ToolPublishedEvent", "ToolSchemaInscribedEvent", "ToolUpdatedEvent",
//   "ToolDeactivatedEvent", "ToolReactivatedEvent", "ToolClosedEvent",
//   "ToolInvocationReportedEvent", "CheckpointCreatedEvent",
//   "EscrowCreatedEvent", "EscrowDepositedEvent", "PaymentSettledEvent",
//   "EscrowWithdrawnEvent", "BatchSettledEvent",
//   "AttestationCreatedEvent", "AttestationRevokedEvent",
//   "LedgerEntryEvent", "LedgerSealedEvent",
// ]
```

### 12.2 WebSocket Live Feed (per Activity Feed UI)

```typescript
const parser = new EventParser(program);

// Subscribe a tutti gli eventi del programma SAP
connection.onLogs(
  PROGRAM_ID,
  (logInfo) => {
    const events = parser.parseLogs(logInfo.logs);
    events.forEach(event => {
      // Invia all'UI via Server-Sent Events o WebSocket
      broadcastToUI({
        name: event.name,
        data: event.data,
        signature: logInfo.signature,
        timestamp: Date.now(),
      });
    });
  },
  "confirmed"
);
```

### 12.3 Event Data Shapes

```typescript
// ── Agent Events ──
// RegisteredEvent
{ agent: PublicKey, wallet: PublicKey, name: string, capabilities: string[], timestamp: BN }

// UpdatedEvent
{ agent: PublicKey, wallet: PublicKey, updatedFields: string[], timestamp: BN }

// ── Feedback Events ──
// FeedbackEvent
{ agent: PublicKey, reviewer: PublicKey, score: number, tag: string, timestamp: BN }

// ── Tool Events ──
// ToolPublishedEvent
{ agent: PublicKey, tool: PublicKey, toolName: string, protocolHash: number[],
  version: number, httpMethod: object, category: object,
  paramsCount: number, requiredParams: number, isCompound: boolean, timestamp: BN }

// ── Escrow Events ──
// PaymentSettledEvent
{ escrow: PublicKey, agent: PublicKey, depositor: PublicKey,
  callsSettled: BN, amount: BN, serviceHash: number[],
  totalCallsSettled: BN, remainingBalance: BN, timestamp: BN }

// ── Attestation Events ──
// AttestationCreatedEvent
{ agent: PublicKey, attester: PublicKey, attestationType: string, expiresAt: BN, timestamp: BN }

// ── Ledger Events ──
// LedgerEntryEvent
{ session: PublicKey, ledger: PublicKey, entryIndex: number,
  data: number[], contentHash: number[], dataLen: number,
  merkleRoot: number[], timestamp: BN }
```

---

## 13. PDA Derivation Reference

Tutte le funzioni sono in `@oobe-protocol-labs/synapse-sap-sdk/pda`:

```typescript
import {
  deriveGlobalRegistry,        // () → [PDA, bump]
  deriveAgent,                 // (wallet) → [PDA, bump]
  deriveAgentStats,            // (agentPda) → [PDA, bump]
  deriveFeedback,              // (agentPda, reviewer) → [PDA, bump]
  deriveCapabilityIndex,       // (capHash) → [PDA, bump]
  deriveProtocolIndex,         // (protoHash) → [PDA, bump]
  deriveToolCategoryIndex,     // (categoryNum) → [PDA, bump]
  deriveVault,                 // (agentPda) → [PDA, bump]
  deriveSession,               // (vaultPda, sessionHash) → [PDA, bump]
  deriveEpochPage,             // (sessionPda, epochIndex) → [PDA, bump]
  deriveVaultDelegate,         // (vaultPda, delegate) → [PDA, bump]
  deriveCheckpoint,            // (sessionPda, checkpointIndex) → [PDA, bump]
  deriveTool,                  // (agentPda, toolName) → [PDA, bump]
  deriveEscrow,                // (agentPda, depositor) → [PDA, bump]
  deriveAttestation,           // (agentPda, attester) → [PDA, bump]
  deriveLedger,                // (sessionPda) → [PDA, bump]
  deriveLedgerPage,            // (ledgerPda, pageIndex) → [PDA, bump]
  derivePlugin,                // (agentPda, pluginType) → [PDA, bump]  (legacy)
  deriveMemoryEntry,           // (agentPda, entryHash) → [PDA, bump]  (legacy)
  deriveMemoryChunk,           // (memoryPda, chunkIndex) → [PDA, bump] (legacy)
  deriveBuffer,                // (sessionPda, pageIndex) → [PDA, bump] (legacy)
  deriveDigest,                // (sessionPda) → [PDA, bump]            (legacy)
} from "@oobe-protocol-labs/synapse-sap-sdk/pda";

// IMPORTANTE: Le funzioni che prendono hash (capHash, protoHash)
// richiedono già il buffer SHA-256. Usa sha256() per calcolarli:
import { sha256 } from "@oobe-protocol-labs/synapse-sap-sdk/utils";
const capHash = sha256("jupiter:swap"); // Uint8Array(32)
```

---

## 14. Account Structures Complete

### Account Discriminators (per getProgramAccounts filters)

Ogni account Anchor ha un **discriminator** di 8 bytes all'offset 0. Per filtrare:

```typescript
// L'account discriminator è sha256("account:AccountName")[0..8]
import { BorshAccountsCoder } from "@coral-xyz/anchor";

const coder = new BorshAccountsCoder(IDL);

// Esempio: trova tutti i ToolDescriptor
const discriminator = coder.accountDiscriminator("ToolDescriptor");

const tools = await connection.getProgramAccounts(PROGRAM_ID, {
  filters: [
    { memcmp: { offset: 0, bytes: bs58.encode(discriminator) } },
    // Filtri aggiuntivi per campi specifici...
  ]
});
```

### Offsets dei Campi (per memcmp filters)

```
AgentAccount layout:
  [0..8]    discriminator
  [8]       bump (u8)
  [9]       version (u8)
  [10..42]  wallet (Pubkey, 32 bytes)
  [42..46]  name_len (u32 LE)
  [46..110] name (max 64 bytes)
  ... (campi a lunghezza variabile, offsets dipendono dai dati)

ToolDescriptor layout (campi fissi):
  [0..8]    discriminator
  [8]       bump (u8)
  [9..41]   agent (Pubkey, 32 bytes)     ← OFFSET 9 per filtro agent
  [41..73]  tool_name_hash ([u8;32])
  ... campi stringa a lunghezza variabile

EscrowAccount layout:
  [0..8]    discriminator
  [8]       bump (u8)
  [9..41]   agent (Pubkey, 32 bytes)     ← OFFSET 9 per filtro agent
  [41..73]  depositor (Pubkey, 32 bytes) ← OFFSET 41 per filtro depositor
  ...

FeedbackAccount layout:
  [0..8]    discriminator
  [8]       bump (u8)
  [9..41]   agent (Pubkey, 32 bytes)     ← OFFSET 9 per filtro agent
  [41..73]  reviewer (Pubkey, 32 bytes)  ← OFFSET 41 per filtro reviewer
  ...

AgentAttestation layout:
  [0..8]    discriminator
  [8]       bump (u8)
  [9..41]   agent (Pubkey, 32 bytes)     ← OFFSET 9 per filtro agent
  [41..73]  attester (Pubkey, 32 bytes)  ← OFFSET 41 per filtro attester
```

---

## 15. Event Reference

### Tutti i 38 eventi con i loro campi

| # | Evento | Contesto | Campi Principali |
|---|--------|----------|-----------------|
| 1 | `RegisteredEvent` | Agent creato | agent, wallet, name, capabilities[], timestamp |
| 2 | `UpdatedEvent` | Agent aggiornato | agent, wallet, updatedFields[], timestamp |
| 3 | `DeactivatedEvent` | Agent disattivato | agent, wallet, timestamp |
| 4 | `ReactivatedEvent` | Agent riattivato | agent, wallet, timestamp |
| 5 | `ClosedEvent` | Agent chiuso | agent, wallet, timestamp |
| 6 | `ReputationUpdatedEvent` | Metriche aggiornate | agent, wallet, avgLatencyMs, uptimePercent, timestamp |
| 7 | `CallsReportedEvent` | Calls auto-riportate | agent, wallet, callsReported, totalCallsServed, timestamp |
| 8 | `FeedbackEvent` | Feedback dato | agent, reviewer, score, tag, timestamp |
| 9 | `FeedbackUpdatedEvent` | Feedback aggiornato | agent, reviewer, oldScore, newScore, timestamp |
| 10 | `FeedbackRevokedEvent` | Feedback revocato | agent, reviewer, timestamp |
| 11 | `VaultInitializedEvent` | Vault creato | agent, vault, wallet, timestamp |
| 12 | `SessionOpenedEvent` | Sessione aperta | vault, session, sessionHash, timestamp |
| 13 | `MemoryInscribedEvent` | Dato iscritto | vault, session, sequence, epochIndex, encryptedData, nonce, contentHash, totalFragments, fragmentIndex, compression, dataLen, nonceVersion, timestamp |
| 14 | `EpochOpenedEvent` | Nuova epoch | session, epochPage, epochIndex, startSequence, timestamp |
| 15 | `SessionClosedEvent` | Sessione chiusa | vault, session, totalInscriptions, totalBytes, totalEpochs, timestamp |
| 16 | `VaultClosedEvent` | Vault chiuso | vault, agent, wallet, totalSessions, totalInscriptions, timestamp |
| 17 | `SessionPdaClosedEvent` | PDA sessione chiuso | vault, session, totalInscriptions, totalBytes, timestamp |
| 18 | `EpochPageClosedEvent` | Epoch page chiuso | session, epochPage, epochIndex, timestamp |
| 19 | `VaultNonceRotatedEvent` | Nonce ruotato | vault, wallet, oldNonce, newNonce, nonceVersion, timestamp |
| 20 | `DelegateAddedEvent` | Delegate aggiunto | vault, delegate, permissions, expiresAt, timestamp |
| 21 | `DelegateRevokedEvent` | Delegate revocato | vault, delegate, timestamp |
| 22 | `ToolPublishedEvent` | Tool pubblicato | agent, tool, toolName, protocolHash, version, httpMethod, category, paramsCount, requiredParams, isCompound, timestamp |
| 23 | `ToolSchemaInscribedEvent` | Schema iscritto | agent, tool, toolName, schemaType, schemaData, schemaHash, compression, version, timestamp |
| 24 | `ToolUpdatedEvent` | Tool aggiornato | agent, tool, toolName, oldVersion, newVersion, timestamp |
| 25 | `ToolDeactivatedEvent` | Tool disattivato | agent, tool, toolName, timestamp |
| 26 | `ToolReactivatedEvent` | Tool riattivato | agent, tool, toolName, timestamp |
| 27 | `ToolClosedEvent` | Tool chiuso | agent, tool, toolName, totalInvocations, timestamp |
| 28 | `ToolInvocationReportedEvent` | Invocazioni riportate | agent, tool, invocationsReported, totalInvocations, timestamp |
| 29 | `CheckpointCreatedEvent` | Checkpoint creato | session, checkpoint, checkpointIndex, merkleRoot, sequenceAt, epochAt, timestamp |
| 30 | `EscrowCreatedEvent` | Escrow creato | escrow, agent, depositor, pricePerCall, maxCalls, initialDeposit, expiresAt, timestamp |
| 31 | `EscrowDepositedEvent` | Fondi depositati | escrow, depositor, amount, newBalance, timestamp |
| 32 | `PaymentSettledEvent` | Pagamento settato | escrow, agent, depositor, callsSettled, amount, serviceHash, totalCallsSettled, remainingBalance, timestamp |
| 33 | `EscrowWithdrawnEvent` | Fondi ritirati | escrow, depositor, amount, remainingBalance, timestamp |
| 34 | `BatchSettledEvent` | Batch settlement | escrow, agent, depositor, numSettlements, totalCalls, totalAmount, serviceHashes[], callsPerSettlement[], remainingBalance, timestamp |
| 35 | `AttestationCreatedEvent` | Attestazione creata | agent, attester, attestationType, expiresAt, timestamp |
| 36 | `AttestationRevokedEvent` | Attestazione revocata | agent, attester, attestationType, timestamp |
| 37 | `LedgerEntryEvent` | Ledger write | session, ledger, entryIndex, data, contentHash, dataLen, merkleRoot, timestamp |
| 38 | `LedgerSealedEvent` | Ledger sealed | session, ledger, page, pageIndex, entriesInPage, dataSize, merkleRootAtSeal, timestamp |

---

## 16. Enum Reference

### ToolCategory (10 categorie)

| Nome | Valore | Descrizione | PDA Pre-calcolato |
|------|--------|-------------|-------------------|
| Swap | 0 | Token swaps (Jupiter, Raydium, Orca) | `5H8yn9RuRgZWqkDiWbKNaCHzTMjqSpwbNQKMPLtUXx2G` |
| Lend | 1 | Lending/borrowing (Marginfi, Kamino) | `5Lqqk6VtFWnYq3h4Ae4FuUAKnFzw1Nm1DaSdt2cjcTDj` |
| Stake | 2 | Staking (Marinade, Jito) | `kC8oAiVUcFMXEnmMNu1h2sdAc3dWKcwV5qVKRFYMmQD` |
| Nft | 3 | NFT mint/trade (Tensor, MagicEden) | `2zNWR9J3znvGQ5J6xDfJyZkd12Gi66mjErRDkgPeKbyF` |
| Payment | 4 | Payments/transfers | `Eh7MwxJYWRN8bzAmY3ZPTRXYjWpWypokBf1STixu2dy9` |
| Data | 5 | Data queries/feeds (Pyth, Switchboard) | `AwpVxehQUZCVTAJ9icZfS6oRbF66jNo32duXaL11B5df` |
| Governance | 6 | DAO/voting (Realms, Squads) | `2573WjZzV9QtbqtM6Z86YGivkk1kdvJa4gK3tZRQ2jkN` |
| Bridge | 7 | Cross-chain (Wormhole, deBridge) | `664nyr6kBeeFiE1ij5gtdncNCVHrXqrk2uBhnKmUREvK` |
| Analytics | 8 | On-chain analytics | `4DFsiTZ6h6RoCZuUeMTpaoQguepnPUMJBLJuwwjKg5GL` |
| Custom | 9 | Uncategorized | `3Nk5dvFWEyWPEArdG9cCdab6C6ym36mSWUSB8HzN35ZM` |

### ToolHttpMethod

| Nome | Valore |
|------|--------|
| Get | 0 |
| Post | 1 |
| Put | 2 |
| Delete | 3 |
| Compound | 4 |

### TokenType

| Nome | Valore |
|------|--------|
| Sol | 0 |
| Usdc | 1 |
| Spl | 2 |

### SettlementMode

| Nome | Valore | Descrizione |
|------|--------|-------------|
| Instant | 0 | Pagamento per-call on-chain |
| Escrow | 1 | Escrow pre-funded |
| Batched | 2 | Accumulo off-chain |
| X402 | 3 | HTTP x402 protocol (default) |

### DelegatePermission (Bitmask)

| Flag | Valore | Descrizione |
|------|--------|-------------|
| Inscribe | 1 | Può scrivere nella vault |
| CloseSession | 2 | Può chiudere sessioni |
| OpenSession | 4 | Può aprire sessioni |
| All | 7 | Tutti i permessi |

### CompressionType

| Nome | Valore |
|------|--------|
| None | 0 |
| Deflate | 1 |
| Gzip | 2 |
| Brotli | 3 |

---

## 17. Costanti e Limiti

```typescript
import {
  MAX_NAME_LEN,           // 64  — Agent name
  MAX_DESC_LEN,           // 256 — Description
  MAX_URI_LEN,            // 256 — URI/endpoint
  MAX_AGENT_ID_LEN,       // 128 — DID-style ID
  MAX_TAG_LEN,            // 32  — Feedback tag
  MAX_TOOL_NAME_LEN,      // 32  — Tool name
  MAX_ATTESTATION_TYPE_LEN, // 32 — Attestation type
  MAX_CAPABILITIES,       // 10  — Per agent
  MAX_PRICING_TIERS,      // 5   — Per agent
  MAX_PROTOCOLS,          // 5   — Per agent
  MAX_PLUGINS,            // 5   — Per agent
  MAX_VOLUME_CURVE_POINTS,// 5   — Per escrow
  MAX_AGENTS_PER_INDEX,   // 100 — Per capability/protocol index
  MAX_TOOLS_PER_CATEGORY, // 100 — Per category index
  MAX_BATCH_SETTLEMENTS,  // 10  — Per settle_batch TX
  MAX_FEEDBACK_SCORE,     // 1000 — Score range 0-1000
  MAX_INSCRIPTION_SIZE,   // 750 — Per inscription/write call
  RING_CAPACITY,          // 4096 — Ledger ring buffer bytes
  INSCRIPTIONS_PER_EPOCH, // 1000 — Trigger new EpochPage
} from "@oobe-protocol-labs/synapse-sap-sdk/constants";
```

---

## 18. Query Patterns Avanzati

### 18.1 Wallet → Tutto (Agent Profile Page)

```typescript
async function getFullAgentProfile(wallet: PublicKey) {
  const [agentPda] = deriveAgent(wallet);
  const [statsPda] = deriveAgentStats(agentPda);
  const [vaultPda] = deriveVault(agentPda);

  // Fetch tutto in parallelo
  const [agent, stats, vault, tools, escrows, feedbacks, attestations] = await Promise.all([
    program.account.agentAccount.fetchNullable(agentPda),
    program.account.agentStats.fetchNullable(statsPda),
    program.account.memoryVault.fetchNullable(vaultPda),
    program.account.toolDescriptor.all([{ memcmp: { offset: 9, bytes: agentPda.toBase58() } }]),
    program.account.escrowAccount.all([{ memcmp: { offset: 9, bytes: agentPda.toBase58() } }]),
    program.account.feedbackAccount.all([{ memcmp: { offset: 9, bytes: agentPda.toBase58() } }]),
    program.account.agentAttestation.all([{ memcmp: { offset: 9, bytes: agentPda.toBase58() } }]),
  ]);

  return {
    pda: agentPda,
    agent,
    stats,
    vault,
    tools: tools.map(t => ({ pda: t.publicKey, ...t.account })),
    escrows: escrows.map(e => ({ pda: e.publicKey, ...e.account })),
    feedbacks: feedbacks.map(f => ({ pda: f.publicKey, ...f.account })),
    attestations: attestations.map(a => ({ pda: a.publicKey, ...a.account })),
    computed: {
      reputationDisplay: agent ? (agent.reputationScore / 100).toFixed(2) + "%" : "N/A",
      totalCalls: stats?.totalCallsServed.toString() ?? "0",
      toolCount: tools.length,
      escrowCount: escrows.length,
      feedbackCount: feedbacks.length,
      attestationCount: attestations.length,
      totalEscrowBalance: escrows.reduce((sum, e) => sum + Number(e.account.balance), 0),
      hasVault: !!vault,
      vaultInscriptions: vault?.totalInscriptions.toString() ?? "0",
    }
  };
}
```

### 18.2 Tutti i PDA di un Agente (per pagina dettaglio)

```typescript
function deriveAllAgentPDAs(wallet: PublicKey) {
  const [agentPda] = deriveAgent(wallet);
  const [statsPda] = deriveAgentStats(agentPda);
  const [vaultPda] = deriveVault(agentPda);

  return {
    agent: agentPda,
    stats: statsPda,
    vault: vaultPda,
    // I seguenti richiedono parametri aggiuntivi:
    // tool: deriveTool(agentPda, toolName)
    // escrow: deriveEscrow(agentPda, depositor)
    // feedback: deriveFeedback(agentPda, reviewer)
    // attestation: deriveAttestation(agentPda, attester)
  };
}
```

### 18.3 Search Bar (Universal Search)

```typescript
async function universalSearch(query: string) {
  const results: any[] = [];

  // 1. Check se è un indirizzo base58 (wallet o PDA)
  try {
    const pubkey = new PublicKey(query);

    // Prova come wallet agent
    const [agentPda] = deriveAgent(pubkey);
    const agent = await program.account.agentAccount.fetchNullable(agentPda);
    if (agent) {
      results.push({ type: "agent", pda: agentPda, data: agent, wallet: pubkey });
    }

    // Prova come PDA diretto (fetch tutti i tipi di account)
    const accountInfo = await connection.getAccountInfo(pubkey);
    if (accountInfo?.owner.equals(PROGRAM_ID)) {
      // È un PDA SAP — prova a decodificare
      try { results.push({ type: "tool", pda: pubkey, data: await program.account.toolDescriptor.fetch(pubkey) }); } catch {}
      try { results.push({ type: "escrow", pda: pubkey, data: await program.account.escrowAccount.fetch(pubkey) }); } catch {}
      try { results.push({ type: "feedback", pda: pubkey, data: await program.account.feedbackAccount.fetch(pubkey) }); } catch {}
      try { results.push({ type: "attestation", pda: pubkey, data: await program.account.agentAttestation.fetch(pubkey) }); } catch {}
      // ... altri tipi
    }
  } catch {
    // Non è un indirizzo — cerca per nome/capability
    const allAgents = await program.account.agentAccount.all();
    const nameMatches = allAgents.filter(a =>
      a.account.name.toLowerCase().includes(query.toLowerCase()) ||
      a.account.capabilities.some(c => c.id.toLowerCase().includes(query.toLowerCase()))
    );
    results.push(...nameMatches.map(a => ({ type: "agent", pda: a.publicKey, data: a.account })));
  }

  return results;
}
```

### 18.4 Activity Feed (Timeline)

```typescript
// Combina eventi diversi in un feed cronologico
async function getActivityFeed(agentPda: PublicKey, limit = 50) {
  // Via PostgreSQL (se hai l'indexer):
  const events = await pg.query(`
    SELECT * FROM sap_events
    WHERE agent_pda = $1
    ORDER BY id DESC
    LIMIT $2
  `, [agentPda.toBase58(), limit]);

  // Via on-chain (senza indexer):
  const signatures = await connection.getSignaturesForAddress(agentPda, { limit });
  const parser = new EventParser(program);

  const feed = [];
  for (const sig of signatures) {
    const tx = await connection.getTransaction(sig.signature, {
      maxSupportedTransactionVersion: 0
    });
    if (!tx?.meta?.logMessages) continue;
    const events = parser.parseLogs(tx.meta.logMessages);
    feed.push(...events.map(e => ({
      ...e,
      signature: sig.signature,
      blockTime: tx.blockTime,
    })));
  }

  return feed;
}
```

---

## 19. Pagine Suggerite dell'Explorer

### Mappa Pagine

```
/                              → Dashboard (GlobalRegistry stats, charts, activity)
/agents                        → Lista agenti (filtri: attivi, per capability, per protocol)
/agents/[wallet]               → Profilo agente (identity, stats, tools, escrows, feedbacks)
/agents/[wallet]/tools         → Tools dell'agente (con schemas)
/agents/[wallet]/escrows       → Escrow dell'agente
/agents/[wallet]/feedbacks     → Feedback ricevuti
/agents/[wallet]/attestations  → Attestazioni ricevute/date
/agents/[wallet]/vault         → Memory vault con sessioni
/agents/[wallet]/activity      → Activity feed (evento timeline)

/tools                         → Tutti i tools per categoria
/tools/[pda]                   → Dettaglio tool (schema, invocazioni)
/tools/categories              → Overview 10 categorie

/capabilities                  → Tutti i capability index
/capabilities/[id]             → Agenti con questa capability
/protocols                     → Tutti i protocol index
/protocols/[id]                → Agenti con questo protocollo

/escrows                       → Tutti gli escrow attivi
/escrows/[pda]                 → Dettaglio escrow (balance, history)

/attestations                  → Grafo attestazioni (Web-of-Trust)
/attestations/[pda]            → Dettaglio attestazione

/reputation                    → Leaderboard reputazione
/reputation/[wallet]           → Breakdown feedback

/network                       → Network stats, protocol health
/network/trust-graph           → Visualizzazione BubbleMaps
/network/activity              → Live event feed

/search                        → Universal search (wallet, PDA, name, capability)
/tx/[signature]                → Dettaglio transazione con eventi SAP parsati
```

### Componenti UI Suggeriti per Ogni Pagina

#### Dashboard (`/`)
- **Stat Cards**: Total Agents, Active Agents, Total Tools, Total Attestations, Total Vaults
- **Tool Category Chart**: Donut/bar chart delle 10 categorie
- **Recent Activity**: Live feed degli ultimi 20 eventi
- **Top Agents**: Top 5 per reputazione
- **Network Growth**: Grafico registrazioni nel tempo

#### Agent Profile (`/agents/[wallet]`)
- **Header**: Nome, descrizione, wallet (copiabile), PDA (copiabile), badges (Active/Inactive, Verified)
- **Stats Grid**: Reputation (con gauge), Total Calls, Uptime%, Latency
- **Capabilities**: Tag/chip list
- **Pricing Tiers**: Tabella espandibile con volume curves
- **Protocols**: Badge list
- **Tools Tab**: Card grid con nome, categoria, HTTP method, invocazioni
- **Escrows Tab**: Tabella con depositor, balance, calls, expiry
- **Feedbacks Tab**: Lista con score, tag, reviewer, data
- **Attestations Tab**: Badge con type, attester, expiry
- **Vault Tab**: Sessioni con inscription count, bytes, stato
- **Activity Tab**: Timeline eventi

#### Trust Graph (`/network/trust-graph`)
- **BubbleMaps Integration**: Nodi = Agenti, Archi = Attestazioni
- **Filtri**: Per attestation type, solo attive, time range
- **Tooltip sui nodi**: Nome, reputazione, capabilities

---

## 20. Integrazione BubbleMaps

### Dati per BubbleMaps

```typescript
// Costruisci il dataset per BubbleMaps / force-directed graph
async function buildTrustGraphData() {
  const [allAgents, allAttestations, allFeedbacks] = await Promise.all([
    program.account.agentAccount.all(),
    program.account.agentAttestation.all(),
    program.account.feedbackAccount.all(),
  ]);

  // NODI: ogni agente
  const nodes = allAgents.map(({ publicKey, account }) => ({
    id: publicKey.toBase58(),
    label: account.name,
    wallet: account.wallet.toBase58(),
    reputation: account.reputationScore / 100,  // 0-100
    isActive: account.isActive,
    capabilities: account.capabilities.map(c => c.id),
    size: Math.max(10, Math.log(Number(account.totalCallsServed) + 1) * 5),
    color: account.isActive ? "#00ff88" : "#666666",
  }));

  // ARCHI DI ATTESTAZIONE (trust)
  const trustEdges = allAttestations
    .filter(({ account }) => account.isActive)
    .map(({ account }) => ({
      source: account.attester.toBase58(), // wallet dell'attester → va mappato al nodo agente
      target: account.agent.toBase58(),    // PDA dell'agente attestato
      type: "attestation",
      label: account.attestationType,
      color: "#4a90d9",
    }));

  // ARCHI DI FEEDBACK (reputation)
  const feedbackEdges = allFeedbacks
    .filter(({ account }) => !account.isRevoked)
    .map(({ account }) => ({
      source: account.reviewer.toBase58(),
      target: account.agent.toBase58(),
      type: "feedback",
      label: `${account.score}/1000`,
      color: account.score >= 700 ? "#00cc66" : account.score >= 400 ? "#ffaa00" : "#ff4444",
    }));

  return { nodes, edges: [...trustEdges, ...feedbackEdges] };
}
```

---

## Helper Utilities

```typescript
import { sha256, hashToArray, serializeAccount } from "@oobe-protocol-labs/synapse-sap-sdk/utils";

// sha256: string/Buffer/Uint8Array → Uint8Array(32)
const hash = sha256("jupiter:swap");

// hashToArray: Uint8Array → number[] (per Anchor args)
const arr = hashToArray(hash);

// serializeAccount: deep-serialize per JSON API
// - PublicKey → base58 string
// - BN → string
// - Uint8Array → hex string
const jsonSafe = serializeAccount(rawAccountData);
// Perfetto per le API routes Next.js:
// return NextResponse.json(serializeAccount(agent));
```

---

## Checklist Implementazione

- [ ] **Setup SDK** — Singleton `SapClient` nel progetto Next.js
- [ ] **Dashboard** — `getNetworkOverview()` + `getToolCategorySummary()`
- [ ] **Agent List** — `program.account.agentAccount.all()` con paginazione
- [ ] **Agent Profile** — `getAgentProfile()` + tools/escrows/feedbacks/attestations in parallelo
- [ ] **Tool Explorer** — `findToolsByCategory()` per ogni categoria + schema da TX logs
- [ ] **Capability Search** — `findAgentsByCapability()` + `findAgentsByProtocol()`
- [ ] **Escrow Monitor** — `escrowAccount.all()` + `getBalance()` per dettagli
- [ ] **Trust Graph** — `agentAttestation.all()` → BubbleMaps integration
- [ ] **Reputation Board** — Sort agents per `reputationScore` DESC
- [ ] **Feedback System** — `feedbackAccount.all()` con filtri per agent/reviewer
- [ ] **Memory Explorer** — vault/session/ledger con ring buffer decode
- [ ] **Live Activity** — `EventParser` + WebSocket `onLogs()`
- [ ] **Search** — Universal search per wallet/PDA/nome/capability
- [ ] **PostgreSQL Indexer** — `SapPostgres` + `SapSyncEngine` per backend
- [ ] **API Routes** — `serializeAccount()` per ogni endpoint

---

> **Nota**: Tutte le letture on-chain sono **gratuite** (nessun TX fee per `getAccountInfo` / `getProgramAccounts`). Le uniche operazioni a pagamento sono le transazioni di scrittura. L'explorer è 100% read-only.
>
> **RPC Consigliato**: Per production, usa un RPC dedicato (Helius, Triton, QuickNode) per evitare rate limits sui `getProgramAccounts`.
