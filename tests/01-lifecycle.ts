/**
 * SAP v2 — Test 01: Agent Lifecycle
 *
 * Complete agent lifecycle: register → update → deactivate → reactivate → close.
 * Also tests: report_calls, update_reputation.
 *
 * Best Practice: Ogni agente ha un solo PDA per wallet. Le operazioni
 * sono atomiche e non richiedono approvazione esterna.
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SynapseAgentSap } from "../target/types/synapse_agent_sap";
import { Keypair, SystemProgram, PublicKey } from "@solana/web3.js";
import { expect } from "chai";
import { BN } from "bn.js";
import {
  findGlobalPda,
  findAgentPda,
  findStatsPda,
  findVaultPda,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  defaultCapability,
  defaultPricing,
  defaultRegistrationArgs,
} from "./helpers";

describe("01 — Agent Lifecycle", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  // Wallets
  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;
  let statsPda: PublicKey;

  // ── Setup ──
  before(async () => {
    await airdrop(connection, authority.publicKey, 20);
    await airdrop(connection, agentOwner.publicKey, 20);
    globalPda = await ensureGlobalInitialized(program, authority);
  });

  // ── 1. Initialize Global Registry ──
  it("Global registry è inizializzato correttamente", async () => {
    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.totalAgents.toNumber()).to.equal(0);
    expect(global.activeAgents.toNumber()).to.equal(0);
    expect(global.authority.toBase58()).to.equal(
      authority.publicKey.toBase58()
    );
  });

  // ── 2. Register Agent ──
  it("Registra un agente con capabilities e pricing", async () => {
    const result = await registerAgent(program, agentOwner, globalPda, {
      name: "Jupiter Agent",
      description: "DeFi swap aggregator powered by Jupiter",
      capabilities: [
        defaultCapability("jupiter:swap"),
        defaultCapability("jupiter:quote"),
      ],
      pricing: [defaultPricing("standard")],
      protocols: ["x402", "a2a"],
      agentId: "did:sap:jupiter-agent-v1",
      agentUri: "https://jupiter.agent/card.json",
      x402Endpoint: "https://pay.jupiter.agent/x402",
    });
    agentPda = result.agentPda;
    statsPda = result.statsPda;

    // Verifica account
    const agent = await program.account.agentAccount.fetch(agentPda);
    expect(agent.name).to.equal("Jupiter Agent");
    expect(agent.isActive).to.equal(true);
    expect(agent.capabilities).to.have.length(2);
    expect(agent.capabilities[0].id).to.equal("jupiter:swap");
    expect(agent.pricing).to.have.length(1);
    expect(agent.protocols).to.include("x402");
    expect(agent.protocols).to.include("a2a");
    expect(agent.agentId).to.equal("did:sap:jupiter-agent-v1");
    expect(agent.agentUri).to.equal("https://jupiter.agent/card.json");
    expect(agent.x402Endpoint).to.equal("https://pay.jupiter.agent/x402");
    expect(agent.reputationScore).to.equal(0);
    expect(agent.totalFeedbacks).to.equal(0);
    expect(agent.wallet.toBase58()).to.equal(agentOwner.publicKey.toBase58());

    // Verifica stats
    const stats = await program.account.agentStats.fetch(statsPda);
    expect(stats.agent.toBase58()).to.equal(agentPda.toBase58());
    expect(stats.isActive).to.equal(true);
    expect(stats.totalCallsServed.toNumber()).to.equal(0);

    // Verifica global registry aggiornato
    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.totalAgents.toNumber()).to.equal(1);
    expect(global.activeAgents.toNumber()).to.equal(1);
  });

  // ── 3. Update Agent ──
  it("Aggiorna nome, description e pricing dell'agente", async () => {
    await program.methods
      .updateAgent(
        "Jupiter Agent Pro", // name
        "Professional DeFi aggregator", // description
        null, // capabilities (unchanged)
        [
          defaultPricing("standard"),
          {
            ...defaultPricing("premium"),
            tierId: "premium",
            pricePerCall: new BN(5_000_000),
            rateLimit: 500,
          },
        ], // pricing
        null, // protocols
        null, // agentId
        null, // agentUri
        null  // x402Endpoint
      )
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    const agent = await program.account.agentAccount.fetch(agentPda);
    expect(agent.name).to.equal("Jupiter Agent Pro");
    expect(agent.description).to.equal("Professional DeFi aggregator");
    expect(agent.pricing).to.have.length(2);
    expect(agent.pricing[1].tierId).to.equal("premium");
  });

  // ── 4. Report Calls ──
  it("Report calls_served incrementa il contatore su AgentStats", async () => {
    await program.methods
      .reportCalls(new BN(42))
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        agentStats: statsPda,
      })
      .signers([agentOwner])
      .rpc();

    const stats = await program.account.agentStats.fetch(statsPda);
    expect(stats.totalCallsServed.toNumber()).to.equal(42);
  });

  // ── 5. Update Reputation (self-reported metrics) ──
  it("Aggiorna latency e uptime dell'agente", async () => {
    await program.methods
      .updateReputation(150, 99) // 150ms avg latency, 99% uptime
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
      })
      .signers([agentOwner])
      .rpc();

    const agent = await program.account.agentAccount.fetch(agentPda);
    expect(agent.avgLatencyMs).to.equal(150);
    expect(agent.uptimePercent).to.equal(99);
  });

  // ── 6. Deactivate Agent ──
  it("Disattiva l'agente — is_active = false", async () => {
    await program.methods
      .deactivateAgent()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        agentStats: statsPda,
        globalRegistry: globalPda,
      })
      .signers([agentOwner])
      .rpc();

    const agent = await program.account.agentAccount.fetch(agentPda);
    expect(agent.isActive).to.equal(false);

    const stats = await program.account.agentStats.fetch(statsPda);
    expect(stats.isActive).to.equal(false);

    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.activeAgents.toNumber()).to.equal(0);
    expect(global.totalAgents.toNumber()).to.equal(1); // total non decresce
  });

  // ── 7. Reactivate Agent ──
  it("Riattiva l'agente — is_active = true", async () => {
    await program.methods
      .reactivateAgent()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        agentStats: statsPda,
        globalRegistry: globalPda,
      })
      .signers([agentOwner])
      .rpc();

    const agent = await program.account.agentAccount.fetch(agentPda);
    expect(agent.isActive).to.equal(true);

    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.activeAgents.toNumber()).to.equal(1);
  });

  // ── 8. Close Agent ──
  it("Chiude l'agente — PDA rimossa, rent rimborsato", async () => {
    // Per chiudere l'agente, vault_check deve essere vuoto (no vault)
    const [vaultCheck] = findVaultPda(agentPda);

    await program.methods
      .closeAgent()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        agentStats: statsPda,
        vaultCheck: vaultCheck,
        globalRegistry: globalPda,
      })
      .signers([agentOwner])
      .rpc();

    // PDA non esiste più
    const agentInfo = await connection.getAccountInfo(agentPda);
    expect(agentInfo).to.be.null;

    const statsInfo = await connection.getAccountInfo(statsPda);
    expect(statsInfo).to.be.null;

    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.activeAgents.toNumber()).to.equal(0);
  });

  // ── 9. Re-register (dopo close, stesso wallet = nuovo PDA) ──
  it("Può ri-registrarsi dopo close", async () => {
    const result = await registerAgent(program, agentOwner, globalPda, {
      name: "Jupiter Agent v2",
      description: "Version 2 after close & re-register",
    });
    agentPda = result.agentPda;
    statsPda = result.statsPda;

    const agent = await program.account.agentAccount.fetch(agentPda);
    expect(agent.name).to.equal("Jupiter Agent v2");
    expect(agent.isActive).to.equal(true);
    expect(agent.reputationScore).to.equal(0); // reputation resettata
  });
});
