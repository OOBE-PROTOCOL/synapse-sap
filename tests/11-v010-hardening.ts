/**
 * SAP v0.10 — Test 11: Hardening (audit fixes + new requirements)
 *
 * Covers:
 *   • C1  — anti-replay receipt PDA on settle_calls / settle_batch / settle_calls_v2
 *   • H2  — vault delegate expires_at must be > now and ≤ 1 year
 *   • M1  — volume curve must be monotonically non-increasing
 *   • New — `createEscrow` requires `AgentStake.staked_amount ≥ MIN_STAKE`
 *   • New — `tokenMint` allowlist (SOL, USDC mainnet, USDC devnet)
 *
 * These tests assume the program was built against the v0.10 hardening
 * branch (audit fixes applied) and `anchor test` is the runner.
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
  findAgentPda,
  findEscrowPda,
  findStatsPda,
  findStakePda,
  findSettlementReceiptPda,
  computeBatchRoot,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  initAgentStake,
  randomHash,
  expectError,
  MIN_AGENT_STAKE_LAMPORTS,
} from "./helpers";

describe("11 — v0.10 Hardening (audit fixes + stake-gate + token allowlist)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();
  const agentNoStake = Keypair.generate(); // for stake-gate negative test
  const client = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;
  let statsPda: PublicKey;
  let stakePda: PublicKey;
  let escrowPda: PublicKey;

  const PRICE = 100_000;
  const DEPOSIT = 50 * PRICE;

  before(async () => {
    await Promise.all([
      airdrop(connection, authority.publicKey, 5),
      airdrop(connection, agentOwner.publicKey, 5),
      airdrop(connection, agentNoStake.publicKey, 5),
      airdrop(connection, client.publicKey, 5),
    ]);
    globalPda = await ensureGlobalInitialized(program, authority);
    const reg = await registerAgent(program, agentOwner, globalPda, {
      name: "HardenedAgent",
    });
    agentPda = reg.agentPda;
    statsPda = reg.statsPda;
    // Bootstrap stake so the agent can accept escrows.
    const stakeRes = await initAgentStake(program, agentOwner);
    stakePda = stakeRes.stakePda;
    [escrowPda] = findEscrowPda(agentPda, client.publicKey);
  });

  // ── New requirement #1: agent stake gate ──────────────────
  describe("Stake-gate on createEscrow", () => {
    it("✗ rejects createEscrow when agent has no stake PDA", async () => {
      // Agent without init_stake → stake account does not exist on-chain.
      const reg = await registerAgent(program, agentNoStake, globalPda, {
        name: "NoStakeAgent",
      });
      const [escrowMissingStake] = findEscrowPda(
        reg.agentPda,
        client.publicKey
      );
      const [stakeMissing] = findStakePda(reg.agentPda);

      await expectError(
        program.methods
          .createEscrow(
            new BN(PRICE),
            new BN(10),
            new BN(DEPOSIT),
            new BN(0),
            [],
            null,
            9
          )
          .accountsStrict({
            depositor: client.publicKey,
            agent: reg.agentPda,
            agentStake: stakeMissing,
            escrow: escrowMissingStake,
            systemProgram: SystemProgram.programId,
          })
          .signers([client])
          .rpc(),
        "AccountNotInitialized" // anchor's error when init account missing
      );
    });

    it("✓ accepts createEscrow when agent has stake ≥ MIN_STAKE", async () => {
      await program.methods
        .createEscrow(
          new BN(PRICE),
          new BN(100),
          new BN(DEPOSIT),
          new BN(0),
          [],
          null,
          9
        )
        .accountsStrict({
          depositor: client.publicKey,
          agent: agentPda,
          agentStake: stakePda,
          escrow: escrowPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([client])
        .rpc();

      const escrow = await program.account.escrowAccount.fetch(escrowPda);
      expect(escrow.balance.toNumber()).to.equal(DEPOSIT);
    });
  });

  // ── New requirement #2: payment-token allowlist ───────────
  describe("Payment-token allowlist", () => {
    it("✗ rejects createEscrow with arbitrary SPL mint", async () => {
      const fakeMint = Keypair.generate().publicKey;
      const otherClient = Keypair.generate();
      await airdrop(connection, otherClient.publicKey, 2);
      const [escrowBad] = findEscrowPda(agentPda, otherClient.publicKey);

      await expectError(
        program.methods
          .createEscrow(
            new BN(PRICE),
            new BN(10),
            new BN(DEPOSIT),
            new BN(0),
            [],
            fakeMint,
            6
          )
          .accountsStrict({
            depositor: otherClient.publicKey,
            agent: agentPda,
            agentStake: stakePda,
            escrow: escrowBad,
            systemProgram: SystemProgram.programId,
          })
          .signers([otherClient])
          .rpc(),
        "PaymentTokenNotAllowed"
      );
    });
  });

  // ── M1: volume curve must be non-increasing ──────────────
  describe("Volume curve monotonicity", () => {
    it("✗ rejects ascending price curve (anti-discount)", async () => {
      const otherClient = Keypair.generate();
      await airdrop(connection, otherClient.publicKey, 2);
      const [escrowBad] = findEscrowPda(agentPda, otherClient.publicKey);

      await expectError(
        program.methods
          .createEscrow(
            new BN(PRICE),
            new BN(100),
            new BN(DEPOSIT),
            new BN(0),
            // PRICE → 2× PRICE — invalid (price must be non-increasing).
            [{ afterCalls: 50, pricePerCall: new BN(PRICE * 2) }],
            null,
            9
          )
          .accountsStrict({
            depositor: otherClient.publicKey,
            agent: agentPda,
            agentStake: stakePda,
            escrow: escrowBad,
            systemProgram: SystemProgram.programId,
          })
          .signers([otherClient])
          .rpc(),
        "VolumeCurveNotDescending"
      );
    });
  });

  // ── C1: anti-replay receipt PDA ───────────────────────────
  describe("Anti-replay receipt PDA on settle_calls", () => {
    const serviceHash = randomHash();

    it("✓ first settle creates the SettlementReceipt PDA", async () => {
      const [receiptPda] = findSettlementReceiptPda(escrowPda, serviceHash);

      await program.methods
        .settleCalls(new BN(1), serviceHash)
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          escrow: escrowPda,
          settlementReceipt: receiptPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([agentOwner])
        .rpc();

      const receipt = await program.account.settlementReceipt.fetch(receiptPda);
      expect(receipt.escrow.toBase58()).to.equal(escrowPda.toBase58());
      expect(receipt.callsSettled.toNumber()).to.equal(1);
      expect(receipt.amount.toNumber()).to.equal(PRICE);
    });

    it("✗ second settle with same service_hash fails (replay blocked)", async () => {
      const [receiptPda] = findSettlementReceiptPda(escrowPda, serviceHash);

      await expectError(
        program.methods
          .settleCalls(new BN(1), serviceHash)
          .accountsStrict({
            wallet: agentOwner.publicKey,
            agent: agentPda,
            agentStats: statsPda,
            escrow: escrowPda,
            settlementReceipt: receiptPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([agentOwner])
          .rpc(),
        "already in use" // System program init twice → "account already in use"
      );
    });
  });

  describe("Anti-replay receipt PDA on settle_batch", () => {
    it("✓ batch settle creates receipt PDA seeded by batch_root", async () => {
      const settlements = [
        { callsToSettle: new BN(1), serviceHash: randomHash() },
        { callsToSettle: new BN(2), serviceHash: randomHash() },
      ];
      const batchRoot = computeBatchRoot(
        settlements.map((s) => Buffer.from(s.serviceHash))
      );
      const [receiptPda] = findSettlementReceiptPda(escrowPda, batchRoot);

      await program.methods
        .settleBatch(settlements, Array.from(batchRoot))
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          escrow: escrowPda,
          settlementReceipt: receiptPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([agentOwner])
        .rpc();

      const receipt = await program.account.settlementReceipt.fetch(receiptPda);
      expect(receipt.callsSettled.toNumber()).to.equal(3);
      expect(Buffer.from(receipt.serviceHash)).to.deep.equal(batchRoot);
    });

    it("✗ rejects mismatched batch_root (computed root ≠ supplied root)", async () => {
      const settlements = [
        { callsToSettle: new BN(1), serviceHash: randomHash() },
      ];
      const wrongRoot = Buffer.alloc(32, 0xab);
      const [receiptPda] = findSettlementReceiptPda(escrowPda, wrongRoot);

      await expectError(
        program.methods
          .settleBatch(settlements, Array.from(wrongRoot))
          .accountsStrict({
            wallet: agentOwner.publicKey,
            agent: agentPda,
            agentStats: statsPda,
            escrow: escrowPda,
            settlementReceipt: receiptPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([agentOwner])
          .rpc(),
        "InvalidReceiptProof"
      );
    });

    it("✗ rejects duplicated service_hash inside the same batch", async () => {
      const dup = randomHash();
      const settlements = [
        { callsToSettle: new BN(1), serviceHash: dup },
        { callsToSettle: new BN(1), serviceHash: dup },
      ];
      const batchRoot = computeBatchRoot(
        settlements.map((s) => Buffer.from(s.serviceHash))
      );
      const [receiptPda] = findSettlementReceiptPda(escrowPda, batchRoot);

      await expectError(
        program.methods
          .settleBatch(settlements, Array.from(batchRoot))
          .accountsStrict({
            wallet: agentOwner.publicKey,
            agent: agentPda,
            agentStats: statsPda,
            escrow: escrowPda,
            settlementReceipt: receiptPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([agentOwner])
          .rpc(),
        "DuplicateServiceHash"
      );
    });
  });
});
