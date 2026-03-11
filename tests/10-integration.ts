/**
 * SAP v2 — Test 10: Full Integration Scenario & Multi-Agent Indexing
 *
 * Simula un ecosistema completo:
 * 1. 3 agenti registrati con capability diverse
 * 2. Tool pubblicati e indicizzati per categoria
 * 3. Escrow creati e settlati
 * 4. Feedback incrociati tra agenti
 * 5. Attestazioni tra agenti
 * 6. Vault e inscriptions
 * 7. Ledger con ring buffer sigillato
 * 8. Indexing finale: query tutti gli agenti, reputazioni, ownership, tools
 *
 * Best Practice per Integratori:
 * - Usare gli indici (Capability/Protocol/ToolCategory) per la discovery
 * - Controllare sempre is_active prima di interagire con un agente
 * - Verificare reputation_score e total_feedbacks per la trust
 * - Usare escrow per pagamenti sicuri
 * - Vault per dati criptati sensibili
 * - Attestazioni per web of trust tra agenti
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SynapseAgentSap } from "../target/types/synapse_agent_sap";
import {
  Keypair,
  SystemProgram,
  PublicKey,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import { expect } from "chai";
import { BN } from "bn.js";
import {
  findGlobalPda,
  findAgentPda,
  findStatsPda,
  findFeedbackPda,
  findCapabilityIndexPda,
  findProtocolIndexPda,
  findToolCategoryIndexPda,
  findToolPda,
  findVaultPda,
  findSessionPda,
  findEpochPagePda,
  findEscrowPda,
  findAttestationPda,
  findLedgerPda,
  findLedgerPagePda,
  airdrop,
  ensureGlobalInitialized,
  defaultCapability,
  defaultPricing,
  defaultRegistrationArgs,
  sha256,
  sha256Bytes,
  randomHash,
  randomNonce,
  randomVaultNonce,
} from "./helpers";

// ─────────────────────────────────────────────────────────────────
//  Agent Profiles
// ─────────────────────────────────────────────────────────────────
interface AgentProfile {
  name: string;
  description: string;
  capabilities: string[];
  protocols: string[];
  tools: { name: string; category: number; method: number }[];
  wallet: Keypair;
  agentPda?: PublicKey;
  statsPda?: PublicKey;
}

describe("10 — Full Integration Scenario & Multi-Agent Indexing", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const externalReviewer = Keypair.generate();

  let globalPda: PublicKey;

  // 3 agenti con profili diversi
  const agents: AgentProfile[] = [
    {
      name: "SwapMaster",
      description: "DeFi swap agent powered by Jupiter aggregator",
      capabilities: ["defi:swap", "defi:quote", "analytics:price"],
      protocols: ["jupiter-v6", "raydium-v3"],
      tools: [
        { name: "swap-sol-usdc", category: 0, method: 1 }, // Swap, POST
        { name: "get-quote", category: 0, method: 0 },     // Swap, GET
      ],
      wallet: Keypair.generate(),
    },
    {
      name: "DataOracle",
      description: "On-chain data aggregation and analytics agent",
      capabilities: ["analytics:price", "data:feed", "data:aggregate"],
      protocols: ["switchboard-v2", "pyth-v2"],
      tools: [
        { name: "price-feed", category: 5, method: 0 },     // Data, GET
        { name: "bulk-aggregate", category: 8, method: 4 },  // Analytics, COMPOUND
      ],
      wallet: Keypair.generate(),
    },
    {
      name: "NFTCurator",
      description: "NFT marketplace curation and bidding agent",
      capabilities: ["nft:bid", "nft:list", "governance:vote"],
      protocols: ["tensor-v2", "magic-eden-v3"],
      tools: [
        { name: "place-bid", category: 3, method: 1 },      // NFT, POST
        { name: "list-nft", category: 3, method: 1 },       // NFT, POST
      ],
      wallet: Keypair.generate(),
    },
  ];

  // ═══════════════════════════════════════════════════════════════
  //  SETUP
  // ═══════════════════════════════════════════════════════════════

  before(async () => {
    const wallets = [
      authority.publicKey,
      externalReviewer.publicKey,
      ...agents.map((a) => a.wallet.publicKey),
    ];
    await Promise.all(
      wallets.map((pk) => airdrop(connection, pk, 30))
    );
    globalPda = await ensureGlobalInitialized(program, authority);
  });

  // ═══════════════════════════════════════════════════════════════
  //  PHASE 1: REGISTRAZIONE
  // ═══════════════════════════════════════════════════════════════

  it("1.1 — Registra 3 agenti con capability e pricing diversi", async () => {
    for (const agent of agents) {
      const [agentPda] = findAgentPda(agent.wallet.publicKey);
      const [statsPda] = findStatsPda(agentPda);

      const capabilities = agent.capabilities.map((id) => ({
        ...defaultCapability(),
        id,
      }));
      const pricing = [
        {
          ...defaultPricing(),
          baseFee: new BN(agent.name === "SwapMaster" ? 50_000 : 25_000),
        },
      ];

      await program.methods
        .registerAgent(
          agent.name,
          agent.description,
          capabilities,
          pricing,
          agent.protocols,
          null,
          null,
          agent.name === "SwapMaster" ? "https://swapmaster.ai/x402" : null
        )
        .accountsStrict({
          wallet: agent.wallet.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([agent.wallet])
        .rpc();

      agent.agentPda = agentPda;
      agent.statsPda = statsPda;
    }

    // Verify all registered
    const globalState = await program.account.globalRegistry.fetch(globalPda);
    expect(globalState.totalAgents.toNumber()).to.be.gte(3);

    for (const agent of agents) {
      const acct = await program.account.agentAccount.fetch(agent.agentPda!);
      expect(acct.name).to.equal(agent.name);
      expect(acct.isActive).to.be.true;
    }
  });

  // ═══════════════════════════════════════════════════════════════
  //  PHASE 2: PUBBLICAZIONE TOOLS + INDICIZZAZIONE
  // ═══════════════════════════════════════════════════════════════

  it("2.1 — Ogni agente pubblica i suoi tool", async () => {
    for (const agent of agents) {
      for (const tool of agent.tools) {
        const nameHash = sha256(tool.name);
        const [toolPda] = findToolPda(agent.agentPda!, nameHash);

        await program.methods
          .publishTool(
            tool.name,
            Array.from(nameHash),
            randomHash(),
            randomHash(),
            randomHash(),
            randomHash(),
            tool.method,    // http_method
            tool.category,  // category
            1,              // params_count
            1,              // required_params
            false           // is_compound
          )
          .accountsStrict({
            wallet: agent.wallet.publicKey,
            agent: agent.agentPda!,
            tool: toolPda,
            globalRegistry: globalPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([agent.wallet])
          .rpc();
      }
    }

    const globalState = await program.account.globalRegistry.fetch(globalPda);
    expect(globalState.totalTools).to.be.gte(6);
  });

  it("2.2 — Crea indici per capability 'analytics:price'", async () => {
    const capId = "analytics:price";
    const capHash = sha256(capId);
    const [capIdxPda] = findCapabilityIndexPda(capHash);

    // SwapMaster ha analytics:price
    await program.methods
      .initCapabilityIndex(capId, Array.from(capHash))
      .accountsStrict({
        wallet: agents[0].wallet.publicKey,
        agent: agents[0].agentPda!,
        capabilityIndex: capIdxPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agents[0].wallet])
      .rpc();

    // DataOracle ha analytics:price → aggiungi
    await program.methods
      .addToCapabilityIndex(Array.from(capHash))
      .accountsStrict({
        wallet: agents[1].wallet.publicKey,
        agent: agents[1].agentPda!,
        capabilityIndex: capIdxPda,
      })
      .signers([agents[1].wallet])
      .rpc();

    const idx = await program.account.capabilityIndex.fetch(capIdxPda);
    expect(idx.agents.length).to.equal(2);
    expect(idx.agents.map((a: PublicKey) => a.toBase58())).to.include(
      agents[0].agentPda!.toBase58()
    );
    expect(idx.agents.map((a: PublicKey) => a.toBase58())).to.include(
      agents[1].agentPda!.toBase58()
    );
  });

  it("2.3 — Crea indice protocollo per 'tensor-v2'", async () => {
    const protoId = "tensor-v2";
    const protoHash = sha256(protoId);
    const [protoIdxPda] = findProtocolIndexPda(protoHash);

    await program.methods
      .initProtocolIndex(protoId, Array.from(protoHash))
      .accountsStrict({
        wallet: agents[2].wallet.publicKey,
        agent: agents[2].agentPda!,
        protocolIndex: protoIdxPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agents[2].wallet])
      .rpc();

    const idx = await program.account.protocolIndex.fetch(protoIdxPda);
    expect(idx.agents.length).to.equal(1);
  });

  it("2.4 — Crea indice tool category 'Swap' e aggiunge tool", async () => {
    const [catIdxPda] = findToolCategoryIndexPda(0); // Swap=0

    const nameHash = sha256("swap-sol-usdc");
    const [toolPda] = findToolPda(agents[0].agentPda!, nameHash);

    // Init (no auto-add)
    await program.methods
      .initToolCategoryIndex(0)
      .accountsStrict({
        wallet: agents[0].wallet.publicKey,
        agent: agents[0].agentPda!,
        toolCategoryIndex: catIdxPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agents[0].wallet])
      .rpc();

    // Add tool to category index
    await program.methods
      .addToToolCategory(0)
      .accountsStrict({
        wallet: agents[0].wallet.publicKey,
        agent: agents[0].agentPda!,
        tool: toolPda,
        toolCategoryIndex: catIdxPda,
      })
      .signers([agents[0].wallet])
      .rpc();

    const idx = await program.account.toolCategoryIndex.fetch(catIdxPda);
    expect(idx.tools.length).to.equal(1);
  });

  // ═══════════════════════════════════════════════════════════════
  //  PHASE 3: FEEDBACK INCROCIATI
  // ═══════════════════════════════════════════════════════════════

  it("3.1 — DataOracle dà feedback a SwapMaster (900/1000)", async () => {
    const [feedbackPda] = findFeedbackPda(
      agents[0].agentPda!,
      agents[1].wallet.publicKey
    );

    await program.methods
      .giveFeedback(900, "reliable-swaps", null)
      .accountsStrict({
        reviewer: agents[1].wallet.publicKey,
        feedback: feedbackPda,
        agent: agents[0].agentPda!,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agents[1].wallet])
      .rpc();
  });

  it("3.2 — NFTCurator dà feedback a SwapMaster (800/1000)", async () => {
    const [feedbackPda] = findFeedbackPda(
      agents[0].agentPda!,
      agents[2].wallet.publicKey
    );

    await program.methods
      .giveFeedback(800, "good-agent", null)
      .accountsStrict({
        reviewer: agents[2].wallet.publicKey,
        feedback: feedbackPda,
        agent: agents[0].agentPda!,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agents[2].wallet])
      .rpc();
  });

  it("3.3 — Reviewer esterno dà feedback a DataOracle (950/1000)", async () => {
    const [feedbackPda] = findFeedbackPda(
      agents[1].agentPda!,
      externalReviewer.publicKey
    );

    await program.methods
      .giveFeedback(950, "excellent-data", null)
      .accountsStrict({
        reviewer: externalReviewer.publicKey,
        feedback: feedbackPda,
        agent: agents[1].agentPda!,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([externalReviewer])
      .rpc();
  });

  // ═══════════════════════════════════════════════════════════════
  //  PHASE 4: ESCROW tra DataOracle e SwapMaster
  // ═══════════════════════════════════════════════════════════════

  it("4.1 — DataOracle crea escrow verso SwapMaster e settle 3 calls", async () => {
    const [escrowPda] = findEscrowPda(
      agents[0].agentPda!,
      agents[1].wallet.publicKey
    );

    await program.methods
      .createEscrow(
        new BN(50_000), // lamports per call
        new BN(0),
        new BN(500_000), // deposit
        new BN(0),
        [],
        null,
        9
      )
      .accountsStrict({
        depositor: agents[1].wallet.publicKey,
        agent: agents[0].agentPda!,
        escrow: escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agents[1].wallet])
      .rpc();

    // SwapMaster settle 3 calls
    await program.methods
      .settleCalls(new BN(3), randomHash())
      .accountsStrict({
        wallet: agents[0].wallet.publicKey,
        agent: agents[0].agentPda!,
        agentStats: agents[0].statsPda!,
        escrow: escrowPda,
      })
      .signers([agents[0].wallet])
      .rpc();

    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    expect(escrow.totalCallsSettled.toNumber()).to.equal(3);
    expect(escrow.balance.toNumber()).to.equal(350_000); // 500k - 150k
  });

  // ═══════════════════════════════════════════════════════════════
  //  PHASE 5: ATTESTAZIONI (Web of Trust)
  // ═══════════════════════════════════════════════════════════════

  it("5.1 — DataOracle attesta SwapMaster come 'verified-dex'", async () => {
    const [attestPda] = findAttestationPda(
      agents[0].agentPda!,
      agents[1].wallet.publicKey
    );

    await program.methods
      .createAttestation("verified-dex", randomHash(), new BN(0))
      .accountsStrict({
        attester: agents[1].wallet.publicKey,
        agent: agents[0].agentPda!,
        attestation: attestPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agents[1].wallet])
      .rpc();
  });

  it("5.2 — SwapMaster attesta DataOracle come 'trusted-oracle'", async () => {
    const [attestPda] = findAttestationPda(
      agents[1].agentPda!,
      agents[0].wallet.publicKey
    );

    await program.methods
      .createAttestation("trusted-oracle", randomHash(), new BN(0))
      .accountsStrict({
        attester: agents[0].wallet.publicKey,
        agent: agents[1].agentPda!,
        attestation: attestPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agents[0].wallet])
      .rpc();
  });

  // ═══════════════════════════════════════════════════════════════
  //  PHASE 6: VAULT & INSCRIPTION (SwapMaster)
  // ═══════════════════════════════════════════════════════════════

  it("6.1 — SwapMaster apre vault, sessione e scrive inscription criptata", async () => {
    const agent = agents[0];
    const [vaultPda] = findVaultPda(agent.agentPda!);

    await program.methods
      .initVault(randomVaultNonce())
      .accountsStrict({
        wallet: agent.wallet.publicKey,
        agent: agent.agentPda!,
        vault: vaultPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agent.wallet])
      .rpc();

    const sessionId = sha256("swap-session-001");
    const [sessionPda] = findSessionPda(vaultPda, sessionId);

    await program.methods
      .openSession(Array.from(sessionId))
      .accountsStrict({
        wallet: agent.wallet.publicKey,
        agent: agent.agentPda!,
        vault: vaultPda,
        session: sessionPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agent.wallet])
      .rpc();

    // Compact inscription
    const data = Buffer.from(
      JSON.stringify({ pair: "SOL/USDC", amount: 1.5, slippage: 0.5 })
    );

    await program.methods
      .compactInscribe(0, data, randomNonce(), Array.from(sha256Bytes(data)))
      .accountsStrict({
        wallet: agent.wallet.publicKey,
        agent: agent.agentPda!,
        vault: vaultPda,
        session: sessionPda,
      })
      .signers([agent.wallet])
      .rpc();

    const session = await program.account.sessionLedger.fetch(sessionPda);
    expect(session.sequenceCounter).to.equal(1);
  });

  // ═══════════════════════════════════════════════════════════════
  //  PHASE 7: LEDGER (DataOracle)
  // ═══════════════════════════════════════════════════════════════

  it("7.1 — DataOracle crea vault+session+ledger, scrive entries e sigilla", async () => {
    const agent = agents[1];

    // Vault + Session setup (ledger requires a session)
    const [vaultPda] = findVaultPda(agent.agentPda!);
    await program.methods
      .initVault(randomVaultNonce())
      .accountsStrict({
        wallet: agent.wallet.publicKey,
        agent: agent.agentPda!,
        vault: vaultPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agent.wallet])
      .rpc();

    const sessionHash = sha256("oracle-ledger-session");
    const [sessionPda] = findSessionPda(vaultPda, sessionHash);
    await program.methods
      .openSession(Array.from(sessionHash))
      .accountsStrict({
        wallet: agent.wallet.publicKey,
        agent: agent.agentPda!,
        vault: vaultPda,
        session: sessionPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agent.wallet])
      .rpc();

    // Init Ledger (seeded by session PDA)
    const [ledgerPda] = findLedgerPda(sessionPda);

    await program.methods
      .initLedger()
      .accountsStrict({
        wallet: agent.wallet.publicKey,
        agent: agent.agentPda!,
        vault: vaultPda,
        session: sessionPda,
        ledger: ledgerPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agent.wallet])
      .rpc();

    // Write 3 entries
    for (let i = 0; i < 3; i++) {
      const entry = Buffer.from(
        JSON.stringify({ price: 150 + i * 0.1, ts: Date.now() })
      );
      await program.methods
        .writeLedger(entry, Array.from(sha256Bytes(entry)))
        .accountsStrict({
          wallet: agent.wallet.publicKey,
          session: sessionPda,
          vault: vaultPda,
          agent: agent.agentPda!,
          ledger: ledgerPda,
        })
        .signers([agent.wallet])
        .rpc();
    }

    // Seal → creates immutable LedgerPage
    const [pagePda] = findLedgerPagePda(ledgerPda, 0);
    await program.methods
      .sealLedger()
      .accountsStrict({
        wallet: agent.wallet.publicKey,
        session: sessionPda,
        vault: vaultPda,
        agent: agent.agentPda!,
        ledger: ledgerPda,
        page: pagePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agent.wallet])
      .rpc();

    const page = await program.account.ledgerPage.fetch(pagePda);
    expect(page.data.length).to.be.gt(0);
    expect(page.entriesInPage).to.be.gte(3);
  });

  // ═══════════════════════════════════════════════════════════════
  //  PHASE 8: MULTI-AGENT REPORTING
  // ═══════════════════════════════════════════════════════════════

  it("8.1 — Report calls per tutti gli agenti", async () => {
    for (const agent of agents) {
      await program.methods
        .reportCalls(new BN(100))
        .accountsStrict({
          wallet: agent.wallet.publicKey,
          agent: agent.agentPda!,
          agentStats: agent.statsPda!,
        })
        .signers([agent.wallet])
        .rpc();
    }

    for (const agent of agents) {
      const stats = await program.account.agentStats.fetch(agent.statsPda!);
      expect(stats.totalCallsServed.toNumber()).to.be.gte(100);
    }
  });

  it("8.2 — Update reputation (latency + uptime) per tutti", async () => {
    for (const agent of agents) {
      await program.methods
        .updateReputation(50, 98)
        .accountsStrict({
          wallet: agent.wallet.publicKey,
          agent: agent.agentPda!,
        })
        .signers([agent.wallet])
        .rpc();
    }
  });

  // ═══════════════════════════════════════════════════════════════
  //  PHASE 9: INDEXING FINALE — QUERY COMPLETA
  // ═══════════════════════════════════════════════════════════════

  it("9.1 — 🔍 INDEXING COMPLETO: agenti, reputazioni, ownership, tools, attestazioni", async () => {
    console.log("\n╔══════════════════════════════════════════════════════════╗");
    console.log("║         SAP v2 — FULL ECOSYSTEM INDEX REPORT            ║");
    console.log("╚══════════════════════════════════════════════════════════╝\n");

    // Global Registry
    const globalState = await program.account.globalRegistry.fetch(globalPda);
    console.log("── Global Registry ──────────────────────────────────");
    console.log(`  Total Agents : ${globalState.totalAgents.toNumber()}`);
    console.log(`  Total Tools  : ${globalState.totalTools}`);
    console.log(`  Active Agents: ${globalState.activeAgents.toNumber()}`);
    console.log(`  Total Vaults : ${globalState.totalVaults}`);
    console.log(
      `  Authority    : ${globalState.authority.toBase58().substring(0, 12)}…`
    );
    console.log();

    // Per-Agent Details
    for (let i = 0; i < agents.length; i++) {
      const agent = agents[i];
      const acct = await program.account.agentAccount.fetch(agent.agentPda!);
      const stats = await program.account.agentStats.fetch(agent.statsPda!);

      console.log(`── Agent ${i + 1}: ${acct.name} ──────────────────`);
      console.log(
        `  PDA          : ${agent.agentPda!.toBase58().substring(0, 16)}…`
      );
      console.log(`  Owner (wallet): ${acct.wallet.toBase58().substring(0, 16)}…`);
      console.log(`  Description  : ${acct.description}`);
      console.log(`  Active       : ${acct.isActive}`);
      console.log(
        `  Capabilities : ${acct.capabilities
          .map((c: any) => c.id)
          .join(", ")}`
      );
      console.log(
        `  Protocols    : ${acct.protocols.join(", ")}`
      );
      console.log(
        `  Pricing      : ${acct.pricing
          .map((p: any) => `${p.pricePerCall.toNumber()} lamports`)
          .join(", ")}`
      );
      console.log(
        `  x402 Endpoint: ${acct.x402Endpoint || "(none)"}`
      );
      console.log(`  Reputation   : ${acct.reputationScore}`);
      console.log(`  Feedbacks    : ${acct.totalFeedbacks}`);
      console.log(`  Avg Latency  : ${acct.avgLatencyMs}ms`);
      console.log(`  Uptime       : ${acct.uptimePercent}%`);
      console.log(`  Total Calls  : ${stats.totalCallsServed.toNumber()}`);
      console.log(
        `  Registered   : ${new Date(
          acct.createdAt.toNumber() * 1000
        ).toISOString()}`
      );
      console.log();
    }

    // Capability Index Discovery
    const capHash = sha256("analytics:price");
    const [capIdxPda] = findCapabilityIndexPda(capHash);
    const capIdx = await program.account.capabilityIndex.fetch(capIdxPda);
    console.log("── Capability Index: analytics:price ──────────────");
    console.log(
      `  Agents with this capability: ${capIdx.agents.length}`
    );
    for (const pk of capIdx.agents) {
      const agentAcct = await program.account.agentAccount.fetch(pk);
      console.log(`    → ${agentAcct.name} (rep: ${agentAcct.reputationScore})`);
    }
    console.log();

    // Protocol Index Discovery
    const protoHash = sha256("tensor-v2");
    const [protoIdxPda] = findProtocolIndexPda(protoHash);
    const protoIdx = await program.account.protocolIndex.fetch(protoIdxPda);
    console.log("── Protocol Index: tensor-v2 ───────────────────────");
    console.log(
      `  Agents supporting this protocol: ${protoIdx.agents.length}`
    );
    for (const pk of protoIdx.agents) {
      const agentAcct = await program.account.agentAccount.fetch(pk);
      console.log(`    → ${agentAcct.name}`);
    }
    console.log();

    // Tool Category Index
    const [catIdxPda] = findToolCategoryIndexPda(0);
    const catIdx = await program.account.toolCategoryIndex.fetch(catIdxPda);
    console.log("── Tool Category Index: Swap ──────────────────────");
    console.log(`  Tools in category: ${catIdx.tools.length}`);
    for (const toolPk of catIdx.tools) {
      const toolAcct = await program.account.toolDescriptor.fetch(toolPk);
      console.log(`    → ${toolAcct.toolName} (v${toolAcct.version})`);
    }
    console.log();

    // Attestation info
    console.log("── Attestations (Web of Trust) ────────────────────");
    const [att1Pda] = findAttestationPda(
      agents[0].agentPda!,
      agents[1].wallet.publicKey
    );
    const att1 = await program.account.agentAttestation.fetch(att1Pda);
    console.log(
      `  DataOracle → SwapMaster : type="${att1.attestationType}", active=${att1.isActive}`
    );

    const [att2Pda] = findAttestationPda(
      agents[1].agentPda!,
      agents[0].wallet.publicKey
    );
    const att2 = await program.account.agentAttestation.fetch(att2Pda);
    console.log(
      `  SwapMaster → DataOracle : type="${att2.attestationType}", active=${att2.isActive}`
    );
    console.log();

    // Escrow status
    const [escrowPda] = findEscrowPda(
      agents[0].agentPda!,
      agents[1].wallet.publicKey
    );
    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    console.log("── Escrow: DataOracle → SwapMaster ────────────────");
    console.log(`  Balance       : ${escrow.balance.toNumber()} lamports`);
    console.log(`  Settled Calls : ${escrow.totalCallsSettled.toNumber()}`);
    console.log(`  Rate/Call     : ${escrow.pricePerCall.toNumber()}`);
    console.log();

    console.log("╔══════════════════════════════════════════════════════════╗");
    console.log("║                 ✅ INDEX REPORT COMPLETE                ║");
    console.log("╚══════════════════════════════════════════════════════════╝\n");

    // Assertions: verify everything is consistent
    expect(globalState.totalAgents.toNumber()).to.be.gte(3);
    expect(capIdx.agents.length).to.equal(2);
    expect(protoIdx.agents.length).to.equal(1);
    expect(escrow.totalCallsSettled.toNumber()).to.equal(3);
    expect(att1.isActive).to.be.true;
    expect(att2.isActive).to.be.true;
  });

  // ═══════════════════════════════════════════════════════════════
  //  PHASE 10: BEST PRACTICES VERIFICATION
  // ═══════════════════════════════════════════════════════════════

  it("10.1 — Best Practice: verifica is_active prima di interagire", async () => {
    for (const agent of agents) {
      const acct = await program.account.agentAccount.fetch(agent.agentPda!);
      expect(acct.isActive).to.be.true;
    }
  });

  it("10.2 — Best Practice: verifica ownership tramite PDA → wallet match", async () => {
    for (const agent of agents) {
      const acct = await program.account.agentAccount.fetch(agent.agentPda!);
      expect(acct.wallet.toBase58()).to.equal(
        agent.wallet.publicKey.toBase58()
      );
    }
  });

  it("10.3 — Best Practice: tutti gli agenti hanno reputation coerente", async () => {
    for (const agent of agents) {
      const acct = await program.account.agentAccount.fetch(agent.agentPda!);
      if (acct.totalFeedbacks > 0) {
        // reputation_score = reputationSum * 10_000 / (total_feedbacks * 1_000)
        // So for non-zero feedbacks, reputation should be > 0
        expect(acct.reputationScore).to.be.gt(0);
      }
      expect(acct.uptimePercent).to.equal(98);
      expect(acct.avgLatencyMs).to.equal(50);
    }
  });
});
