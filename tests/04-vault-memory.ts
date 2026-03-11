/**
 * SAP v2 — Test 04: Memory Vault & Encrypted Inscriptions
 *
 * Vault lifecycle: init_vault → open_session → inscribe_memory →
 * compact_inscribe → checkpoint → close_session → close_vault.
 * Also tests: delegation, nonce rotation, epoch pages.
 *
 * Best Practice: Le inscriptions sono permanenti nei TX log (zero rent).
 * Il vault fornisce la chiave di encrypting per le sessioni.
 * I delegates possono scrivere per conto dell'agente.
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
  findVaultPda,
  findSessionPda,
  findEpochPagePda,
  findDelegatePda,
  findCheckpointPda,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  sha256,
  sha256Bytes,
  randomHash,
  randomNonce,
  randomVaultNonce,
} from "./helpers";

describe("04 — Memory Vault & Inscriptions", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();
  const delegateWallet = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;
  let vaultPda: PublicKey;
  let sessionPda: PublicKey;

  const sessionHashBuf = sha256("session-001");
  const vaultNonce = randomVaultNonce();

  before(async () => {
    await Promise.all([
      airdrop(connection, authority.publicKey, 20),
      airdrop(connection, agentOwner.publicKey, 20),
      airdrop(connection, delegateWallet.publicKey, 10),
    ]);
    globalPda = await ensureGlobalInitialized(program, authority);
    const result = await registerAgent(program, agentOwner, globalPda, {
      name: "VaultAgent",
    });
    agentPda = result.agentPda;
  });

  // ── 1. Init Vault ──
  it("Inizializza un memory vault per l'agente", async () => {
    [vaultPda] = findVaultPda(agentPda);

    await program.methods
      .initVault(vaultNonce)
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    const vault = await program.account.memoryVault.fetch(vaultPda);
    expect(vault.agent.toBase58()).to.equal(agentPda.toBase58());
    expect(vault.totalSessions).to.equal(0);
    expect(vault.totalInscriptions.toNumber()).to.equal(0);
    expect(vault.protocolVersion).to.equal(1);
    expect(vault.nonceVersion).to.equal(0);

    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.totalVaults).to.equal(1);
  });

  // ── 2. Open Session ──
  it("Apre una sessione di memoria", async () => {
    [sessionPda] = findSessionPda(vaultPda, sessionHashBuf);

    await program.methods
      .openSession(Array.from(sessionHashBuf))
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        session: sessionPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    const session = await program.account.sessionLedger.fetch(sessionPda);
    expect(session.isClosed).to.equal(false);
    expect(session.sequenceCounter).to.equal(0);
    expect(session.totalBytes.toNumber()).to.equal(0);
    expect(session.currentEpoch).to.equal(0);

    const vault = await program.account.memoryVault.fetch(vaultPda);
    expect(vault.totalSessions).to.equal(1);
  });

  // ── 3. Inscribe Memory (full args) ──
  it("Inscrive dati encrypted nella sessione", async () => {
    const encryptedData = Buffer.from("encrypted-message-001");
    const nonce = randomNonce();
    const contentHash = Array.from(sha256Bytes(encryptedData));
    const [epochPda] = findEpochPagePda(sessionPda, 0);

    await program.methods
      .inscribeMemory(
        0,                         // sequence
        encryptedData,             // encrypted_data
        nonce,                     // nonce (12 bytes)
        contentHash,               // content_hash
        1,                         // total_fragments
        0,                         // fragment_index
        0,                         // compression (none)
        0                          // epoch_index
      )
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        session: sessionPda,
        epochPage: epochPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    const session = await program.account.sessionLedger.fetch(sessionPda);
    expect(session.sequenceCounter).to.equal(1);
    expect(session.totalBytes.toNumber()).to.equal(encryptedData.length);
    expect(session.currentEpoch).to.equal(0);

    // Epoch page creata
    const epoch = await program.account.epochPage.fetch(epochPda);
    expect(epoch.epochIndex).to.equal(0);
    expect(epoch.inscriptionCount).to.equal(1);
  });

  // ── 4. Compact Inscribe (simplified API) ──
  it("Usa compact_inscribe per inscrivere in modo semplificato", async () => {
    const data = Buffer.from("compact-msg-002");
    const nonce = randomNonce();
    const contentHash = Array.from(sha256Bytes(data));

    await program.methods
      .compactInscribe(
        1,             // sequence
        data,
        nonce,
        contentHash
      )
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        session: sessionPda,
      })
      .signers([agentOwner])
      .rpc();

    const session = await program.account.sessionLedger.fetch(sessionPda);
    expect(session.sequenceCounter).to.equal(2);
  });

  // ── 5. Create Session Checkpoint ──
  it("Crea un checkpoint della sessione", async () => {
    const [checkpointPda] = findCheckpointPda(sessionPda, 0);

    await program.methods
      .createSessionCheckpoint(0)
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        session: sessionPda,
        checkpoint: checkpointPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    const cp = await program.account.sessionCheckpoint.fetch(checkpointPda);
    expect(cp.checkpointIndex).to.equal(0);
    expect(cp.sequenceAt).to.equal(2);

    const session = await program.account.sessionLedger.fetch(sessionPda);
    expect(session.totalCheckpoints).to.equal(1);
  });

  // ── 6. Add Vault Delegate ──
  it("Aggiunge un delegate con permesso di inscrivere", async () => {
    const [delegatePda] = findDelegatePda(vaultPda, delegateWallet.publicKey);

    await program.methods
      .addVaultDelegate(
        1, // permissions: inscribe only (bit 0)
        new BN(0) // never expires
      )
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        vaultDelegate: delegatePda,
        delegate: delegateWallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    const del = await program.account.vaultDelegate.fetch(delegatePda);
    expect(del.permissions).to.equal(1);
    expect(del.delegate.toBase58()).to.equal(
      delegateWallet.publicKey.toBase58()
    );
  });

  // ── 7. Delegated Inscribe ──
  it("Il delegate inscrive dati per conto dell'agente", async () => {
    const data = Buffer.from("delegated-msg-003");
    const nonce = randomNonce();
    const contentHash = Array.from(sha256Bytes(data));
    const [epochPda] = findEpochPagePda(sessionPda, 0);
    const [delegatePda] = findDelegatePda(vaultPda, delegateWallet.publicKey);

    await program.methods
      .inscribeMemoryDelegated(
        2,             // sequence
        data,
        nonce,
        contentHash,
        1,             // total_fragments
        0,             // fragment_index
        0,             // compression
        0              // epoch_index
      )
      .accountsStrict({
        delegateSigner: delegateWallet.publicKey,
        agent: agentPda,
        vault: vaultPda,
        vaultDelegate: delegatePda,
        session: sessionPda,
        epochPage: epochPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([delegateWallet])
      .rpc();

    const session = await program.account.sessionLedger.fetch(sessionPda);
    expect(session.sequenceCounter).to.equal(3);
  });

  // ── 8. Rotate Vault Nonce ──
  it("Ruota il nonce del vault", async () => {
    const newNonce = randomVaultNonce();

    await program.methods
      .rotateVaultNonce(newNonce)
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
      })
      .signers([agentOwner])
      .rpc();

    const vault = await program.account.memoryVault.fetch(vaultPda);
    expect(vault.nonceVersion).to.equal(1);
  });

  // ── 9. Close Session ──
  it("Chiude la sessione — is_closed = true", async () => {
    await program.methods
      .closeSession()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        session: sessionPda,
      })
      .signers([agentOwner])
      .rpc();

    const session = await program.account.sessionLedger.fetch(sessionPda);
    expect(session.isClosed).to.equal(true);
  });

  // ── 10. Close Epoch Page (solo dopo close session) ──
  it("Chiude l'epoch page — rent rimborsato", async () => {
    const [epochPda] = findEpochPagePda(sessionPda, 0);

    await program.methods
      .closeEpochPage(0)
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        session: sessionPda,
        epochPage: epochPda,
      })
      .signers([agentOwner])
      .rpc();

    const info = await connection.getAccountInfo(epochPda);
    expect(info).to.be.null;
  });

  // ── 11. Close Checkpoint ──
  it("Chiude il checkpoint — rent rimborsato", async () => {
    const [checkpointPda] = findCheckpointPda(sessionPda, 0);

    await program.methods
      .closeCheckpoint(0)
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        session: sessionPda,
        checkpoint: checkpointPda,
      })
      .signers([agentOwner])
      .rpc();

    const info = await connection.getAccountInfo(checkpointPda);
    expect(info).to.be.null;
  });

  // ── 12. Revoke Delegate ──
  it("Revoca il delegate", async () => {
    const [delegatePda] = findDelegatePda(vaultPda, delegateWallet.publicKey);

    await program.methods
      .revokeVaultDelegate()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        vaultDelegate: delegatePda,
      })
      .signers([agentOwner])
      .rpc();

    const info = await connection.getAccountInfo(delegatePda);
    expect(info).to.be.null;
  });

  // ── 13. Close Session PDA ──
  it("Chiude la Session PDA — rent rimborsato", async () => {
    await program.methods
      .closeSessionPda()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        session: sessionPda,
      })
      .signers([agentOwner])
      .rpc();

    const info = await connection.getAccountInfo(sessionPda);
    expect(info).to.be.null;
  });

  // ── 14. Close Vault ──
  it("Chiude il vault — rent rimborsato", async () => {
    await program.methods
      .closeVault()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        globalRegistry: globalPda,
      })
      .signers([agentOwner])
      .rpc();

    const info = await connection.getAccountInfo(vaultPda);
    expect(info).to.be.null;

    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.totalVaults).to.equal(0);
  });
});
