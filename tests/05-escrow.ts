/**
 * SAP v2 - Test 05: Escrow V2 & x402 Payments
 *
 * Escrow lifecycle: create -> deposit -> settle -> withdraw -> close.
 * V1 escrow instructions are intentionally not used here.
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SynapseAgentSap } from "../target/types/synapse_agent_sap";
import { Keypair, SystemProgram, PublicKey } from "@solana/web3.js";
import { expect } from "chai";
import { BN } from "bn.js";
import {
  findEscrowV2Pda,
  PROTOCOL_TREASURY,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  initAgentStake,
  randomHash,
  expectError,
} from "./helpers";

describe("05 - Escrow V2 & x402 Payments", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();
  const client = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;
  let statsPda: PublicKey;
  let pricingPda: PublicKey;
  let stakePda: PublicKey;
  let escrowPda: PublicKey;
  let escrowNonce = 1;

  const PRICE_PER_CALL = 1_000_000;
  const INITIAL_DEPOSIT = 10 * PRICE_PER_CALL;
  const PROTOCOL_FEE_BPS = 50;

  const protocolFee = (amount: number) =>
    Math.floor((amount * PROTOCOL_FEE_BPS) / 10_000);

  const coSignedRemainingAccounts = () => [
    { pubkey: PROTOCOL_TREASURY, isWritable: true, isSigner: false },
    { pubkey: client.publicKey, isWritable: false, isSigner: true },
  ];

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
    pricingPda = result.pricingPda;
    const stakeRes = await initAgentStake(program, agentOwner);
    stakePda = stakeRes.stakePda;
  });

  it("Client crea un escrow V2 CoSigned con volume curve", async () => {
    [escrowPda] = findEscrowV2Pda(agentPda, client.publicKey, escrowNonce);

    await program.methods
      .createEscrowV2(
        new BN(escrowNonce),
        new BN(PRICE_PER_CALL),
        new BN(100),
        new BN(INITIAL_DEPOSIT),
        new BN(0),
        [
          { afterCalls: 50, pricePerCall: new BN(800_000) },
          { afterCalls: 100, pricePerCall: new BN(600_000) },
        ],
        null,
        9,
        1,
        new BN(0),
        client.publicKey,
        null
      )
      .accountsStrict({
        depositor: client.publicKey,
        agent: agentPda,
        agentStake: stakePda,
        agentStats: statsPda,
        pricingMenu: pricingPda,
        escrow: escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([client])
      .rpc();

    const escrow = await program.account.escrowAccountV2.fetch(escrowPda);
    expect(escrow.balance.toNumber()).to.equal(INITIAL_DEPOSIT);
    expect(escrow.totalDeposited.toNumber()).to.equal(INITIAL_DEPOSIT);
    expect(escrow.pricePerCall.toNumber()).to.equal(PRICE_PER_CALL);
    expect(escrow.maxCalls.toNumber()).to.equal(100);
    expect(escrow.totalCallsSettled.toNumber()).to.equal(0);
    expect(escrow.volumeCurve).to.have.length(2);
    expect(escrow.depositor.toBase58()).to.equal(client.publicKey.toBase58());
  });

  it("Client deposita fondi aggiuntivi", async () => {
    await program.methods
      .depositEscrowV2(new BN(escrowNonce), new BN(5 * PRICE_PER_CALL))
      .accountsStrict({
        depositor: client.publicKey,
        escrow: escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([client])
      .rpc();

    const escrow = await program.account.escrowAccountV2.fetch(escrowPda);
    expect(escrow.balance.toNumber()).to.equal(15 * PRICE_PER_CALL);
    expect(escrow.totalDeposited.toNumber()).to.equal(15 * PRICE_PER_CALL);
  });

  it("Agent fa settle di 3 chiamate", async () => {
    const balanceBefore = await connection.getBalance(agentOwner.publicKey);
    const svcHash = randomHash();
    const amount = 3 * PRICE_PER_CALL;

    await program.methods
      .settleCallsV2(new BN(escrowNonce), new BN(3), svcHash)
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        agentStats: statsPda,
        escrow: escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(coSignedRemainingAccounts())
      .signers([agentOwner, client])
      .rpc();

    const escrow = await program.account.escrowAccountV2.fetch(escrowPda);
    expect(escrow.totalCallsSettled.toNumber()).to.equal(3);
    expect(escrow.totalSettled.toNumber()).to.equal(amount);
    expect(escrow.balance.toNumber()).to.equal(
      15 * PRICE_PER_CALL - amount - protocolFee(amount)
    );

    const balanceAfter = await connection.getBalance(agentOwner.publicKey);
    expect(balanceAfter).to.be.greaterThan(balanceBefore);

    const stats = await program.account.agentStats.fetch(statsPda);
    expect(stats.totalCallsServed.toNumber()).to.equal(3);
  });

  it("Agent fa un secondo settle V2", async () => {
    const amount = 3 * PRICE_PER_CALL;

    await program.methods
      .settleCallsV2(new BN(escrowNonce), new BN(3), randomHash())
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        agentStats: statsPda,
        escrow: escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(coSignedRemainingAccounts())
      .signers([agentOwner, client])
      .rpc();

    const escrow = await program.account.escrowAccountV2.fetch(escrowPda);
    expect(escrow.totalCallsSettled.toNumber()).to.equal(6);
    expect(escrow.totalSettled.toNumber()).to.equal(6 * PRICE_PER_CALL);
    expect(escrow.balance.toNumber()).to.equal(
      15 * PRICE_PER_CALL - amount * 2 - protocolFee(amount) * 2
    );
  });

  it("Client preleva parte del saldo dell'escrow", async () => {
    const balanceBefore = await connection.getBalance(client.publicKey);

    await program.methods
      .withdrawEscrowV2(new BN(200_000))
      .accountsStrict({
        depositor: client.publicKey,
        escrow: escrowPda,
      })
      .signers([client])
      .rpc();

    const balanceAfter = await connection.getBalance(client.publicKey);
    expect(balanceAfter).to.be.greaterThan(balanceBefore);
  });

  it("Client preleva tutto il saldo rimanente", async () => {
    const escrowBefore = await program.account.escrowAccountV2.fetch(escrowPda);
    const remaining = escrowBefore.balance.toNumber();

    if (remaining > 0) {
      await program.methods
        .withdrawEscrowV2(new BN(remaining))
        .accountsStrict({
          depositor: client.publicKey,
          escrow: escrowPda,
        })
        .signers([client])
        .rpc();
    }

    const escrow = await program.account.escrowAccountV2.fetch(escrowPda);
    expect(escrow.balance.toNumber()).to.equal(0);
  });

  it("Client chiude l'escrow V2 - rent rimborsato", async () => {
    await program.methods
      .closeEscrowV2()
      .accountsStrict({
        depositor: client.publicKey,
        escrow: escrowPda,
        agentStats: statsPda,
      })
      .signers([client])
      .rpc();

    const info = await connection.getAccountInfo(escrowPda);
    expect(info).to.be.null;
  });

  it("Errore: settle V2 su agente inattivo", async () => {
    escrowNonce += 1;
    [escrowPda] = findEscrowV2Pda(agentPda, client.publicKey, escrowNonce);

    await program.methods
      .createEscrowV2(
        new BN(escrowNonce),
        new BN(PRICE_PER_CALL),
        new BN(0),
        new BN(PRICE_PER_CALL * 5),
        new BN(0),
        [],
        null,
        9,
        1,
        new BN(0),
        client.publicKey,
        null
      )
      .accountsStrict({
        depositor: client.publicKey,
        agent: agentPda,
        agentStake: stakePda,
        agentStats: statsPda,
        pricingMenu: pricingPda,
        escrow: escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([client])
      .rpc();

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

    await expectError(
      program.methods
        .settleCallsV2(new BN(escrowNonce), new BN(1), randomHash())
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          escrow: escrowPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts(coSignedRemainingAccounts())
        .signers([agentOwner, client])
        .rpc(),
      "AgentInactive"
    );

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

    const escrow = await program.account.escrowAccountV2.fetch(escrowPda);
    if (escrow.balance.toNumber() > 0) {
      await program.methods
        .withdrawEscrowV2(escrow.balance)
        .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
        .signers([client])
        .rpc();
    }
    await program.methods
      .closeEscrowV2()
      .accountsStrict({
        depositor: client.publicKey,
        escrow: escrowPda,
        agentStats: statsPda,
      })
      .signers([client])
      .rpc();
  });
});
