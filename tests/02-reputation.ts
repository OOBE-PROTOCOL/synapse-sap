/**
 * SAP v2 — Test 02: Reputation & Feedback
 *
 * Feedback lifecycle: give → update → revoke → close.
 * Tests weighted average reputation calculation onchain.
 * Tests multiple reviewers per agent.
 *
 * Best Practice: Reputation è calcolata onchain in modo trustless.
 * Ogni reviewer può dare un solo feedback per agente (PDA unica).
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
  findFeedbackPda,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  sha256,
} from "./helpers";

describe("02 — Reputation & Feedback", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();
  const reviewer1 = Keypair.generate();
  const reviewer2 = Keypair.generate();
  const reviewer3 = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;

  before(async () => {
    await Promise.all([
      airdrop(connection, authority.publicKey, 20),
      airdrop(connection, agentOwner.publicKey, 20),
      airdrop(connection, reviewer1.publicKey, 10),
      airdrop(connection, reviewer2.publicKey, 10),
      airdrop(connection, reviewer3.publicKey, 10),
    ]);
    globalPda = await ensureGlobalInitialized(program, authority);
    const result = await registerAgent(program, agentOwner, globalPda, {
      name: "FeedbackAgent",
    });
    agentPda = result.agentPda;
  });

  // ── 1. Give Feedback (reviewer1 → score 800/1000) ──
  it("Reviewer 1 dà feedback con score 800", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, reviewer1.publicKey);
    const commentHash = Array.from(sha256("Great agent!"));

    await program.methods
      .giveFeedback(800, "excellent", commentHash)
      .accountsStrict({
        reviewer: reviewer1.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([reviewer1])
      .rpc();

    const fb = await program.account.feedbackAccount.fetch(feedbackPda);
    expect(fb.score).to.equal(800);
    expect(fb.tag).to.equal("excellent");
    expect(fb.isRevoked).to.equal(false);
    expect(fb.reviewer.toBase58()).to.equal(reviewer1.publicKey.toBase58());

    // Reputation: 800/1000 * 10000 = 8000
    const agent = await program.account.agentAccount.fetch(agentPda);
    expect(agent.totalFeedbacks).to.equal(1);
    expect(agent.reputationSum.toNumber()).to.equal(800);
    // reputation_score = (800 * 10000) / (1 * 1000) = 8000
    expect(agent.reputationScore).to.equal(8000);
  });

  // ── 2. Give Feedback (reviewer2 → score 600/1000) ──
  it("Reviewer 2 dà feedback con score 600 — media ponderata", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, reviewer2.publicKey);

    await program.methods
      .giveFeedback(600, "good", null)
      .accountsStrict({
        reviewer: reviewer2.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([reviewer2])
      .rpc();

    const agent = await program.account.agentAccount.fetch(agentPda);
    expect(agent.totalFeedbacks).to.equal(2);
    expect(agent.reputationSum.toNumber()).to.equal(1400); // 800 + 600
    // reputation_score = (1400 * 10000) / (2 * 1000) = 7000
    expect(agent.reputationScore).to.equal(7000);
  });

  // ── 3. Give Feedback (reviewer3 → score 1000/1000) ──
  it("Reviewer 3 dà punteggio massimo — 1000/1000", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, reviewer3.publicKey);

    await program.methods
      .giveFeedback(1000, "perfect", null)
      .accountsStrict({
        reviewer: reviewer3.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([reviewer3])
      .rpc();

    const agent = await program.account.agentAccount.fetch(agentPda);
    expect(agent.totalFeedbacks).to.equal(3);
    expect(agent.reputationSum.toNumber()).to.equal(2400); // 800 + 600 + 1000
    // reputation_score = (2400 * 10000) / (3 * 1000) = 8000
    expect(agent.reputationScore).to.equal(8000);
  });

  // ── 4. Update Feedback ──
  it("Reviewer 1 aggiorna il suo feedback da 800 a 900", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, reviewer1.publicKey);

    await program.methods
      .updateFeedback(900, "outstanding", null)
      .accountsStrict({
        reviewer: reviewer1.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
      })
      .signers([reviewer1])
      .rpc();

    const fb = await program.account.feedbackAccount.fetch(feedbackPda);
    expect(fb.score).to.equal(900);
    expect(fb.tag).to.equal("outstanding");

    const agent = await program.account.agentAccount.fetch(agentPda);
    // sum = 2400 - 800 + 900 = 2500
    expect(agent.reputationSum.toNumber()).to.equal(2500);
    // reputation_score = (2500 * 10000) / (3 * 1000) = 8333
    expect(agent.reputationScore).to.equal(8333);
  });

  // ── 5. Revoke Feedback ──
  it("Reviewer 2 revoca il suo feedback", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, reviewer2.publicKey);

    await program.methods
      .revokeFeedback()
      .accountsStrict({
        reviewer: reviewer2.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
      })
      .signers([reviewer2])
      .rpc();

    const fb = await program.account.feedbackAccount.fetch(feedbackPda);
    expect(fb.isRevoked).to.equal(true);

    const agent = await program.account.agentAccount.fetch(agentPda);
    // sum = 2500 - 600 = 1900, total = 2
    expect(agent.totalFeedbacks).to.equal(2);
    expect(agent.reputationSum.toNumber()).to.equal(1900);
    // reputation_score = (1900 * 10000) / (2 * 1000) = 9500
    expect(agent.reputationScore).to.equal(9500);
  });

  // ── 6. Close Revoked Feedback ──
  it("Reviewer 2 chiude il feedback revocato — rent rimborsato", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, reviewer2.publicKey);

    await program.methods
      .closeFeedback()
      .accountsStrict({
        reviewer: reviewer2.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
        globalRegistry: globalPda,
      })
      .signers([reviewer2])
      .rpc();

    const info = await connection.getAccountInfo(feedbackPda);
    expect(info).to.be.null;
  });

  // ── 7. Verifica finale: reputation corretta con 2 feedback attivi ──
  it("Reputation finale corretta: 2 feedback attivi, media = 9500", async () => {
    const agent = await program.account.agentAccount.fetch(agentPda);
    expect(agent.totalFeedbacks).to.equal(2);
    // Reviewer1: 900, Reviewer3: 1000 → sum=1900, avg=950/1000 → 9500/10000
    expect(agent.reputationScore).to.equal(9500);
    // Global feedback counter
    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.totalFeedbacks.toNumber()).to.equal(2);
  });
});
