/**
 * SAP v2 — Test 06: Attestation (Web of Trust)
 *
 * Attestation lifecycle: create → revoke → close.
 * Tests: self-attestation blocked, type + expiry validation.
 *
 * Best Practice: Le attestazioni sono trust signals istituzionali.
 * La fiducia viene dalla reputazione dell'attester, non dal contenuto.
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
  findAttestationPda,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  sha256,
  expectError,
} from "./helpers";

describe("06 — Attestation (Web of Trust)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();
  const auditor = Keypair.generate();
  const partner = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;

  before(async () => {
    await Promise.all([
      airdrop(connection, authority.publicKey, 20),
      airdrop(connection, agentOwner.publicKey, 20),
      airdrop(connection, auditor.publicKey, 10),
      airdrop(connection, partner.publicKey, 10),
    ]);
    globalPda = await ensureGlobalInitialized(program, authority);
    const result = await registerAgent(program, agentOwner, globalPda, {
      name: "AttestAgent",
    });
    agentPda = result.agentPda;
  });

  // ── 1. Create Attestation ──
  it("Auditor crea attestazione 'audited' per l'agente", async () => {
    const [attestPda] = findAttestationPda(agentPda, auditor.publicKey);
    const metadataHash = Array.from(sha256("audit-report-v1.pdf"));

    await program.methods
      .createAttestation(
        "audited",
        metadataHash,
        new BN(0) // never expires
      )
      .accountsStrict({
        attester: auditor.publicKey,
        agent: agentPda,
        attestation: attestPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([auditor])
      .rpc();

    const attest = await program.account.agentAttestation.fetch(attestPda);
    expect(attest.attestationType).to.equal("audited");
    expect(attest.isActive).to.equal(true);
    expect(attest.attester.toBase58()).to.equal(auditor.publicKey.toBase58());
    expect(attest.agent.toBase58()).to.equal(agentPda.toBase58());

    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.totalAttestations).to.equal(1);
  });

  // ── 2. Second Attestation (different attester) ──
  it("Partner crea attestazione 'partner' per lo stesso agente", async () => {
    const [attestPda] = findAttestationPda(agentPda, partner.publicKey);

    await program.methods
      .createAttestation(
        "partner",
        Array.from(sha256("partner-agreement")),
        new BN(0)
      )
      .accountsStrict({
        attester: partner.publicKey,
        agent: agentPda,
        attestation: attestPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([partner])
      .rpc();

    const attest = await program.account.agentAttestation.fetch(attestPda);
    expect(attest.attestationType).to.equal("partner");
    expect(attest.isActive).to.equal(true);

    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.totalAttestations).to.equal(2);
  });

  // ── 3. Self-Attestation Blocked ──
  it("Errore: agent non può auto-attestarsi", async () => {
    const [attestPda] = findAttestationPda(agentPda, agentOwner.publicKey);

    await expectError(
      program.methods
        .createAttestation(
          "self-verified",
          Array.from(sha256("trust-me-bro")),
          new BN(0)
        )
        .accountsStrict({
          attester: agentOwner.publicKey,
          agent: agentPda,
          attestation: attestPda,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([agentOwner])
        .rpc(),
      "SelfAttestationNotAllowed"
    );
  });

  // ── 4. Revoke Attestation ──
  it("Auditor revoca la sua attestazione", async () => {
    const [attestPda] = findAttestationPda(agentPda, auditor.publicKey);

    await program.methods
      .revokeAttestation()
      .accountsStrict({
        attester: auditor.publicKey,
        agent: agentPda,
        attestation: attestPda,
      })
      .signers([auditor])
      .rpc();

    const attest = await program.account.agentAttestation.fetch(attestPda);
    expect(attest.isActive).to.equal(false);
  });

  // ── 5. Double Revoke Blocked ──
  it("Errore: non si può revocare due volte", async () => {
    const [attestPda] = findAttestationPda(agentPda, auditor.publicKey);

    await expectError(
      program.methods
        .revokeAttestation()
        .accountsStrict({
          attester: auditor.publicKey,
          agent: agentPda,
          attestation: attestPda,
        })
        .signers([auditor])
        .rpc(),
      "AttestationAlreadyRevoked"
    );
  });

  // ── 6. Close Attestation (must be revoked first) ──
  it("Auditor chiude l'attestazione revocata — rent rimborsato", async () => {
    const [attestPda] = findAttestationPda(agentPda, auditor.publicKey);

    await program.methods
      .closeAttestation()
      .accountsStrict({
        attester: auditor.publicKey,
        agent: agentPda,
        attestation: attestPda,
        globalRegistry: globalPda,
      })
      .signers([auditor])
      .rpc();

    const info = await connection.getAccountInfo(attestPda);
    expect(info).to.be.null;
  });

  // ── 7. Cannot Close Non-Revoked Attestation ──
  it("Errore: non si può chiudere un'attestazione attiva", async () => {
    const [attestPda] = findAttestationPda(agentPda, partner.publicKey);

    await expectError(
      program.methods
        .closeAttestation()
        .accountsStrict({
          attester: partner.publicKey,
          agent: agentPda,
          attestation: attestPda,
          globalRegistry: globalPda,
        })
        .signers([partner])
        .rpc(),
      "AttestationNotRevoked"
    );
  });
});
