/**
 * SAP v2 — Test 07: Indexing & Discovery
 *
 * Capability/Protocol/ToolCategory indexes: init → add → remove → close.
 * Tests multi-agent discovery queries.
 *
 * Best Practice: Gli indici sono PDA condivise. Il primo agente a
 * registrare una capability la crea (init + auto-add). Gli altri
 * agenti si aggiungono con add_to_*.
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
  findCapabilityIndexPda,
  findProtocolIndexPda,
  findToolCategoryIndexPda,
  findToolPda,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  sha256,
  expectError,
} from "./helpers";

describe("07 — Indexing & Discovery", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const wallet1 = Keypair.generate();
  const wallet2 = Keypair.generate();
  const wallet3 = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda1: PublicKey;
  let agentPda2: PublicKey;
  let agentPda3: PublicKey;

  const capId = "jupiter:swap";
  const capHash = sha256(capId);
  const protoId = "x402";
  const protoHash = sha256(protoId);

  before(async () => {
    await Promise.all([
      airdrop(connection, authority.publicKey, 20),
      airdrop(connection, wallet1.publicKey, 20),
      airdrop(connection, wallet2.publicKey, 20),
      airdrop(connection, wallet3.publicKey, 20),
    ]);
    globalPda = await ensureGlobalInitialized(program, authority);

    // Register 3 agents
    const r1 = await registerAgent(program, wallet1, globalPda, {
      name: "Agent1",
    });
    agentPda1 = r1.agentPda;

    const r2 = await registerAgent(program, wallet2, globalPda, {
      name: "Agent2",
    });
    agentPda2 = r2.agentPda;

    const r3 = await registerAgent(program, wallet3, globalPda, {
      name: "Agent3",
    });
    agentPda3 = r3.agentPda;
  });

  // ═══════════════════════════════════════════════════════════════
  //  Capability Index
  // ═══════════════════════════════════════════════════════════════

  it("Init capability index 'jupiter:swap' — Agent1 auto-aggiunto", async () => {
    const [capIdxPda] = findCapabilityIndexPda(capHash);

    await program.methods
      .initCapabilityIndex(capId, Array.from(capHash))
      .accountsStrict({
        wallet: wallet1.publicKey,
        agent: agentPda1,
        capabilityIndex: capIdxPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([wallet1])
      .rpc();

    const idx = await program.account.capabilityIndex.fetch(capIdxPda);
    expect(idx.agents).to.have.length(1);
    expect(idx.agents[0].toBase58()).to.equal(agentPda1.toBase58());
  });

  it("Agent2 si aggiunge all'indice capability", async () => {
    const [capIdxPda] = findCapabilityIndexPda(capHash);

    await program.methods
      .addToCapabilityIndex(Array.from(capHash))
      .accountsStrict({
        wallet: wallet2.publicKey,
        agent: agentPda2,
        capabilityIndex: capIdxPda,
      })
      .signers([wallet2])
      .rpc();

    const idx = await program.account.capabilityIndex.fetch(capIdxPda);
    expect(idx.agents).to.have.length(2);
  });

  it("Agent3 si aggiunge — 3 agenti nell'indice", async () => {
    const [capIdxPda] = findCapabilityIndexPda(capHash);

    await program.methods
      .addToCapabilityIndex(Array.from(capHash))
      .accountsStrict({
        wallet: wallet3.publicKey,
        agent: agentPda3,
        capabilityIndex: capIdxPda,
      })
      .signers([wallet3])
      .rpc();

    const idx = await program.account.capabilityIndex.fetch(capIdxPda);
    expect(idx.agents).to.have.length(3);
  });

  it("Query: tutti gli agenti con capability 'jupiter:swap'", async () => {
    const [capIdxPda] = findCapabilityIndexPda(capHash);
    const idx = await program.account.capabilityIndex.fetch(capIdxPda);

    // Fetch each agent and verify
    const agents = await Promise.all(
      idx.agents.map((a) => program.account.agentAccount.fetch(a))
    );
    const names = agents.map((a) => a.name);
    expect(names).to.include("Agent1");
    expect(names).to.include("Agent2");
    expect(names).to.include("Agent3");
  });

  it("Agent2 si rimuove dall'indice capability", async () => {
    const [capIdxPda] = findCapabilityIndexPda(capHash);

    await program.methods
      .removeFromCapabilityIndex(Array.from(capHash))
      .accountsStrict({
        wallet: wallet2.publicKey,
        agent: agentPda2,
        capabilityIndex: capIdxPda,
      })
      .signers([wallet2])
      .rpc();

    const idx = await program.account.capabilityIndex.fetch(capIdxPda);
    expect(idx.agents).to.have.length(2);
    // Agent2 non è più nell'indice
    const hasAgent2 = idx.agents.some(
      (a) => a.toBase58() === agentPda2.toBase58()
    );
    expect(hasAgent2).to.equal(false);
  });

  // ═══════════════════════════════════════════════════════════════
  //  Protocol Index
  // ═══════════════════════════════════════════════════════════════

  it("Init protocol index 'x402' — Agent1 auto-aggiunto", async () => {
    const [protoIdxPda] = findProtocolIndexPda(protoHash);

    await program.methods
      .initProtocolIndex(protoId, Array.from(protoHash))
      .accountsStrict({
        wallet: wallet1.publicKey,
        agent: agentPda1,
        protocolIndex: protoIdxPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([wallet1])
      .rpc();

    const idx = await program.account.protocolIndex.fetch(protoIdxPda);
    expect(idx.agents).to.have.length(1);
    expect(idx.protocolId).to.equal("x402");
  });

  it("Agent3 si aggiunge all'indice protocol x402", async () => {
    const [protoIdxPda] = findProtocolIndexPda(protoHash);

    await program.methods
      .addToProtocolIndex(Array.from(protoHash))
      .accountsStrict({
        wallet: wallet3.publicKey,
        agent: agentPda3,
        protocolIndex: protoIdxPda,
      })
      .signers([wallet3])
      .rpc();

    const idx = await program.account.protocolIndex.fetch(protoIdxPda);
    expect(idx.agents).to.have.length(2);
  });

  // ═══════════════════════════════════════════════════════════════
  //  Tool Category Index
  // ═══════════════════════════════════════════════════════════════

  it("Init tool category index per Swap (category=0)", async () => {
    const [toolCatPda] = findToolCategoryIndexPda(0);

    await program.methods
      .initToolCategoryIndex(0)
      .accountsStrict({
        wallet: wallet1.publicKey,
        agent: agentPda1,
        toolCategoryIndex: toolCatPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([wallet1])
      .rpc();

    const idx = await program.account.toolCategoryIndex.fetch(toolCatPda);
    expect(idx.category).to.equal(0);
    expect(idx.tools).to.have.length(0);
  });

  it("Pubblica tool Swap e lo aggiunge alla category index", async () => {
    const toolName = "swapTokens";
    const toolNameHash = sha256(toolName);
    const [toolPda] = findToolPda(agentPda1, toolNameHash);
    const [toolCatPda] = findToolCategoryIndexPda(0);

    // Publish
    await program.methods
      .publishTool(
        toolName,
        Array.from(toolNameHash),
        Array.from(sha256("jupiter")),
        Array.from(sha256("swap tokens")),
        Array.from(sha256("{}")),
        Array.from(sha256("{}")),
        1, // POST
        0, // Swap
        2,
        2,
        false
      )
      .accountsStrict({
        wallet: wallet1.publicKey,
        agent: agentPda1,
        tool: toolPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([wallet1])
      .rpc();

    // Add to category index
    await program.methods
      .addToToolCategory(0)
      .accountsStrict({
        wallet: wallet1.publicKey,
        agent: agentPda1,
        tool: toolPda,
        toolCategoryIndex: toolCatPda,
      })
      .signers([wallet1])
      .rpc();

    const idx = await program.account.toolCategoryIndex.fetch(toolCatPda);
    expect(idx.tools).to.have.length(1);
    expect(idx.tools[0].toBase58()).to.equal(toolPda.toBase58());
  });

  it("Query: tutti i tool nella category Swap", async () => {
    const [toolCatPda] = findToolCategoryIndexPda(0);
    const idx = await program.account.toolCategoryIndex.fetch(toolCatPda);

    const tools = await Promise.all(
      idx.tools.map((t) => program.account.toolDescriptor.fetch(t))
    );
    expect(tools[0].toolName).to.equal("swapTokens");
    expect(tools[0].isActive).to.equal(true);
  });

  // ═══════════════════════════════════════════════════════════════
  //  Cleanup: close indexes
  // ═══════════════════════════════════════════════════════════════

  it("Rimuove tool dalla category e chiude l'indice", async () => {
    const toolName = "swapTokens";
    const toolNameHash = sha256(toolName);
    const [toolPda] = findToolPda(agentPda1, toolNameHash);
    const [toolCatPda] = findToolCategoryIndexPda(0);

    // Remove from category
    await program.methods
      .removeFromToolCategory(0)
      .accountsStrict({
        wallet: wallet1.publicKey,
        agent: agentPda1,
        tool: toolPda,
        toolCategoryIndex: toolCatPda,
      })
      .signers([wallet1])
      .rpc();

    // Close category index (now empty)
    await program.methods
      .closeToolCategoryIndex(0)
      .accountsStrict({
        wallet: wallet1.publicKey,
        agent: agentPda1,
        toolCategoryIndex: toolCatPda,
      })
      .signers([wallet1])
      .rpc();

    const info = await connection.getAccountInfo(toolCatPda);
    expect(info).to.be.null;
  });

  it("Errore: non si può chiudere un capability index non vuoto", async () => {
    const [capIdxPda] = findCapabilityIndexPda(capHash);

    await expectError(
      program.methods
        .closeCapabilityIndex(Array.from(capHash))
        .accountsStrict({
          wallet: wallet1.publicKey,
          agent: agentPda1,
          capabilityIndex: capIdxPda,
          globalRegistry: globalPda,
        })
        .signers([wallet1])
        .rpc(),
      "IndexNotEmpty"
    );
  });
});
