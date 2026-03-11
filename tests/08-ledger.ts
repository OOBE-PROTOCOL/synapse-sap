/**
 * SAP v2 — Test 08: MemoryLedger (Unified Onchain Memory)
 *
 * Ledger lifecycle: init → write → seal (immutable page) → close.
 * Tests ring buffer behavior, merkle root, LedgerPage immutability.
 *
 * Best Practice: Il MemoryLedger è il sistema di memoria consigliato.
 * Combina ring buffer (lettura istantanea) con TX log (permanenza).
 * Le LedgerPage sono IMMUTABILI — nessuna close instruction esiste.
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
  findLedgerPda,
  findLedgerPagePda,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  sha256,
  sha256Bytes,
  randomVaultNonce,
  randomNonce,
  expectError,
} from "./helpers";

describe("08 — MemoryLedger (Ring Buffer + TX Logs)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;
  let vaultPda: PublicKey;
  let sessionPda: PublicKey;
  let ledgerPda: PublicKey;

  const sessionHash = sha256("ledger-session-001");

  before(async () => {
    await Promise.all([
      airdrop(connection, authority.publicKey, 20),
      airdrop(connection, agentOwner.publicKey, 50),
    ]);
    globalPda = await ensureGlobalInitialized(program, authority);

    // Register agent + vault + session
    const result = await registerAgent(program, agentOwner, globalPda, {
      name: "LedgerAgent",
    });
    agentPda = result.agentPda;

    [vaultPda] = findVaultPda(agentPda);
    await program.methods
      .initVault(randomVaultNonce())
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    [sessionPda] = findSessionPda(vaultPda, sessionHash);
    await program.methods
      .openSession(Array.from(sessionHash))
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        session: sessionPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();
  });

  // ── 1. Init Ledger ──
  it("Inizializza un MemoryLedger per la sessione", async () => {
    [ledgerPda] = findLedgerPda(sessionPda);

    await program.methods
      .initLedger()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        vault: vaultPda,
        session: sessionPda,
        ledger: ledgerPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    const ledger = await program.account.memoryLedger.fetch(ledgerPda);
    expect(ledger.numEntries).to.equal(0);
    expect(ledger.totalDataSize.toNumber()).to.equal(0);
    expect(ledger.numPages).to.equal(0);
    expect(ledger.authority.toBase58()).to.equal(
      agentOwner.publicKey.toBase58()
    );
  });

  // ── 2. Write to Ledger ──
  it("Scrive dati nel ring buffer del ledger", async () => {
    const data = Buffer.from("Hello SAP v2 — first ledger entry");
    const contentHash = Array.from(sha256Bytes(data));

    await program.methods
      .writeLedger(data, contentHash)
      .accountsStrict({
        wallet: agentOwner.publicKey,
        session: sessionPda,
        vault: vaultPda,
        agent: agentPda,
        ledger: ledgerPda,
      })
      .signers([agentOwner])
      .rpc();

    const ledger = await program.account.memoryLedger.fetch(ledgerPda);
    expect(ledger.numEntries).to.equal(1);
    expect(ledger.totalDataSize.toNumber()).to.equal(data.length);
    // Ring buffer contains [len_u16_le][data]
    expect(ledger.ring.length).to.be.greaterThan(0);
    // Merkle root updated (non-zero)
    const merkleIsZero = ledger.merkleRoot.every((b) => b === 0);
    expect(merkleIsZero).to.equal(false);
  });

  // ── 3. Write Multiple Entries ──
  it("Scrive 5 entries — ring buffer si riempie progressivamente", async () => {
    for (let i = 2; i <= 6; i++) {
      const data = Buffer.from(`Entry #${i} — SAP v2 ledger data`);
      const contentHash = Array.from(sha256Bytes(data));

      await program.methods
        .writeLedger(data, contentHash)
        .accountsStrict({
          wallet: agentOwner.publicKey,
          session: sessionPda,
          vault: vaultPda,
          agent: agentPda,
          ledger: ledgerPda,
        })
        .signers([agentOwner])
        .rpc();
    }

    const ledger = await program.account.memoryLedger.fetch(ledgerPda);
    expect(ledger.numEntries).to.equal(6);
    expect(ledger.ring.length).to.be.greaterThan(0);
  });

  // ── 4. Seal Ledger → Immutable LedgerPage ──
  it("Seal crea una LedgerPage IMMUTABILE e svuota il ring", async () => {
    const [pagePda] = findLedgerPagePda(ledgerPda, 0);

    await program.methods
      .sealLedger()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        session: sessionPda,
        vault: vaultPda,
        agent: agentPda,
        ledger: ledgerPda,
        page: pagePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    // Page sealed
    const page = await program.account.ledgerPage.fetch(pagePda);
    expect(page.pageIndex).to.equal(0);
    expect(page.entriesInPage).to.equal(6);
    expect(page.dataSize).to.be.greaterThan(0);
    expect(page.data.length).to.be.greaterThan(0);

    // Ledger ring cleared, numPages incremented
    const ledger = await program.account.memoryLedger.fetch(ledgerPda);
    expect(ledger.numPages).to.equal(1);
    // Ring is now empty (cleared after seal)
    expect(ledger.ring.length).to.equal(0);
  });

  // ── 5. Write After Seal → New Ring Cycle ──
  it("Scrive ancora dopo il seal — nuovo ciclo nel ring", async () => {
    const data = Buffer.from("Post-seal entry #7");
    const contentHash = Array.from(sha256Bytes(data));

    await program.methods
      .writeLedger(data, contentHash)
      .accountsStrict({
        wallet: agentOwner.publicKey,
        session: sessionPda,
        vault: vaultPda,
        agent: agentPda,
        ledger: ledgerPda,
      })
      .signers([agentOwner])
      .rpc();

    const ledger = await program.account.memoryLedger.fetch(ledgerPda);
    expect(ledger.numEntries).to.equal(7);
    expect(ledger.ring.length).to.be.greaterThan(0);
  });

  // ── 6. Seal Again → Second Page ──
  it("Secondo seal — pagina #1 creata", async () => {
    const [pagePda] = findLedgerPagePda(ledgerPda, 1);

    await program.methods
      .sealLedger()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        session: sessionPda,
        vault: vaultPda,
        agent: agentPda,
        ledger: ledgerPda,
        page: pagePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    const page = await program.account.ledgerPage.fetch(pagePda);
    expect(page.pageIndex).to.equal(1);
    expect(page.entriesInPage).to.equal(1);

    const ledger = await program.account.memoryLedger.fetch(ledgerPda);
    expect(ledger.numPages).to.equal(2);
  });

  // ── 7. Error: Seal Empty Ring ──
  it("Errore: non si può fare seal con ring vuoto", async () => {
    const [pagePda] = findLedgerPagePda(ledgerPda, 2);

    await expectError(
      program.methods
        .sealLedger()
        .accountsStrict({
          wallet: agentOwner.publicKey,
          session: sessionPda,
          vault: vaultPda,
          agent: agentPda,
          ledger: ledgerPda,
          page: pagePda,
          systemProgram: SystemProgram.programId,
        })
        .signers([agentOwner])
        .rpc(),
      "LedgerRingEmpty"
    );
  });

  // ── 8. Error: Write > 750 bytes ──
  it("Errore: dati > 750 bytes", async () => {
    const bigData = Buffer.alloc(751, 0x42);
    const contentHash = Array.from(sha256Bytes(bigData));

    await expectError(
      program.methods
        .writeLedger(bigData, contentHash)
        .accountsStrict({
          wallet: agentOwner.publicKey,
          session: sessionPda,
          vault: vaultPda,
          agent: agentPda,
          ledger: ledgerPda,
        })
        .signers([agentOwner])
        .rpc(),
      "LedgerDataTooLarge"
    );
  });

  // ── 9. Close Ledger (ring is empty after seal) ──
  it("Chiude il ledger — rent rimborsato", async () => {
    await program.methods
      .closeLedger()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        session: sessionPda,
        vault: vaultPda,
        agent: agentPda,
        ledger: ledgerPda,
      })
      .signers([agentOwner])
      .rpc();

    const info = await connection.getAccountInfo(ledgerPda);
    expect(info).to.be.null;
  });

  // ── 10. LedgerPages Are PERMANENT ──
  it("Le LedgerPage sono permanenti — non eliminabili", async () => {
    // Pages 0 and 1 still exist
    const [page0Pda] = findLedgerPagePda(ledgerPda, 0);
    const [page1Pda] = findLedgerPagePda(ledgerPda, 1);

    const page0 = await program.account.ledgerPage.fetch(page0Pda);
    expect(page0.entriesInPage).to.equal(6);

    const page1 = await program.account.ledgerPage.fetch(page1Pda);
    expect(page1.entriesInPage).to.equal(1);

    // There is NO closeLedgerPage instruction — pages are immutable
  });
});
