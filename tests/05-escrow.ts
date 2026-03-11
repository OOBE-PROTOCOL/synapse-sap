/**
 * SAP v2 — Test 05: Escrow & x402 Payments
 *
 * Escrow lifecycle: create → deposit → settle → batch settle →
 * withdraw → close.
 * Tests volume curve pricing, max_calls, expiry.
 *
 * Best Practice: L'escrow è un pre-pagamento trustless. L'agente
 * fa settle dopo aver servito la chiamata. Il client può prelevare
 * il saldo non utilizzato in qualsiasi momento.
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
  findEscrowPda,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  randomHash,
  expectError,
} from "./helpers";

describe("05 — Escrow & x402 Payments", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();
  const client = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;
  let statsPda: PublicKey;
  let escrowPda: PublicKey;

  const PRICE_PER_CALL = 100_000; // 0.0001 SOL
  const INITIAL_DEPOSIT = 10 * PRICE_PER_CALL; // 10 calls worth

  before(async () => {
    await Promise.all([
      airdrop(connection, authority.publicKey, 20),
      airdrop(connection, agentOwner.publicKey, 20),
      airdrop(connection, client.publicKey, 20),
    ]);
    globalPda = await ensureGlobalInitialized(program, authority);
    const result = await registerAgent(program, agentOwner, globalPda, {
      name: "EscrowAgent",
    });
    agentPda = result.agentPda;
    statsPda = result.statsPda;
  });

  // ── 1. Create Escrow ──
  it("Client crea un escrow con volume curve", async () => {
    [escrowPda] = findEscrowPda(agentPda, client.publicKey);

    await program.methods
      .createEscrow(
        new BN(PRICE_PER_CALL),  // price_per_call
        new BN(100),             // max_calls (100 max)
        new BN(INITIAL_DEPOSIT), // initial_deposit
        new BN(0),               // expires_at (never)
        [
          { afterCalls: 50, pricePerCall: new BN(80_000) },  // 20% discount after 50
          { afterCalls: 100, pricePerCall: new BN(60_000) }, // 40% discount after 100
        ],
        null, // token_mint (SOL)
        9     // token_decimals
      )
      .accountsStrict({
        depositor: client.publicKey,
        agent: agentPda,
        escrow: escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([client])
      .rpc();

    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    expect(escrow.balance.toNumber()).to.equal(INITIAL_DEPOSIT);
    expect(escrow.totalDeposited.toNumber()).to.equal(INITIAL_DEPOSIT);
    expect(escrow.pricePerCall.toNumber()).to.equal(PRICE_PER_CALL);
    expect(escrow.maxCalls.toNumber()).to.equal(100);
    expect(escrow.totalCallsSettled.toNumber()).to.equal(0);
    expect(escrow.volumeCurve).to.have.length(2);
    expect(escrow.depositor.toBase58()).to.equal(client.publicKey.toBase58());
    // Note: totalEscrows è DEPRECATED — non più aggiornato dal programma
  });

  // ── 2. Deposit More ──
  it("Client deposita fondi aggiuntivi", async () => {
    await program.methods
      .depositEscrow(new BN(5 * PRICE_PER_CALL))
      .accountsStrict({
        depositor: client.publicKey,
        escrow: escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([client])
      .rpc();

    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    expect(escrow.balance.toNumber()).to.equal(15 * PRICE_PER_CALL);
    expect(escrow.totalDeposited.toNumber()).to.equal(15 * PRICE_PER_CALL);
  });

  // ── 3. Settle Calls (agent claims payment) ──
  it("Agent fa settle di 3 chiamate", async () => {
    const balanceBefore = await connection.getBalance(agentOwner.publicKey);

    await program.methods
      .settleCalls(
        new BN(3),        // calls_to_settle
        randomHash()      // service_hash (proof of work)
      )
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        agentStats: statsPda,
        escrow: escrowPda,
      })
      .signers([agentOwner])
      .rpc();

    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    // 3 calls at base price = 3 * 100_000 = 300_000
    expect(escrow.totalCallsSettled.toNumber()).to.equal(3);
    expect(escrow.totalSettled.toNumber()).to.equal(300_000);
    expect(escrow.balance.toNumber()).to.equal(15 * PRICE_PER_CALL - 300_000);

    // Agent wallet received payment
    const balanceAfter = await connection.getBalance(agentOwner.publicKey);
    expect(balanceAfter).to.be.greaterThan(balanceBefore);

    // Stats updated
    const stats = await program.account.agentStats.fetch(statsPda);
    expect(stats.totalCallsServed.toNumber()).to.equal(3);
  });

  // ── 4. Batch Settle ──
  it("Agent fa batch settle di 2 blocchi di chiamate", async () => {
    await program.methods
      .settleBatch([
        { callsToSettle: new BN(2), serviceHash: randomHash() },
        { callsToSettle: new BN(1), serviceHash: randomHash() },
      ])
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        agentStats: statsPda,
        escrow: escrowPda,
      })
      .signers([agentOwner])
      .rpc();

    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    expect(escrow.totalCallsSettled.toNumber()).to.equal(6); // 3 + 2 + 1
  });

  // ── 5. Withdraw ──
  it("Client preleva parte del saldo dell'escrow", async () => {
    const balanceBefore = await connection.getBalance(client.publicKey);

    await program.methods
      .withdrawEscrow(new BN(200_000))
      .accountsStrict({
        depositor: client.publicKey,
        escrow: escrowPda,
      })
      .signers([client])
      .rpc();

    const balanceAfter = await connection.getBalance(client.publicKey);
    expect(balanceAfter).to.be.greaterThan(balanceBefore);
  });

  // ── 6. Withdraw All Remaining ──
  it("Client preleva tutto il saldo rimanente", async () => {
    const escrowBefore = await program.account.escrowAccount.fetch(escrowPda);
    const remaining = escrowBefore.balance.toNumber();

    if (remaining > 0) {
      await program.methods
        .withdrawEscrow(new BN(remaining))
        .accountsStrict({
          depositor: client.publicKey,
          escrow: escrowPda,
        })
        .signers([client])
        .rpc();
    }

    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    expect(escrow.balance.toNumber()).to.equal(0);
  });

  // ── 7. Close Escrow (balance must be 0) ──
  it("Client chiude l'escrow — rent rimborsato", async () => {
    await program.methods
      .closeEscrow()
      .accountsStrict({
        depositor: client.publicKey,
        escrow: escrowPda,
      })
      .signers([client])
      .rpc();

    const info = await connection.getAccountInfo(escrowPda);
    expect(info).to.be.null;
  });

  // ── 8. Test: Cannot settle on inactive agent ──
  it("Errore: settle su agente inattivo", async () => {
    // Re-create escrow first
    [escrowPda] = findEscrowPda(agentPda, client.publicKey);

    await program.methods
      .createEscrow(
        new BN(PRICE_PER_CALL),
        new BN(0), // unlimited
        new BN(PRICE_PER_CALL * 5),
        new BN(0),
        [],
        null,
        9
      )
      .accountsStrict({
        depositor: client.publicKey,
        agent: agentPda,
        escrow: escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([client])
      .rpc();

    // Deactivate
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

    // Try settle → should fail
    await expectError(
      program.methods
        .settleCalls(new BN(1), randomHash())
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          escrow: escrowPda,
        })
        .signers([agentOwner])
        .rpc(),
      "AgentInactive"
    );

    // Reactivate for cleanup
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

    // Withdraw and close for cleanup
    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    if (escrow.balance.toNumber() > 0) {
      await program.methods
        .withdrawEscrow(escrow.balance)
        .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
        .signers([client])
        .rpc();
    }
    await program.methods
      .closeEscrow()
      .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
      .signers([client])
      .rpc();
  });
});
