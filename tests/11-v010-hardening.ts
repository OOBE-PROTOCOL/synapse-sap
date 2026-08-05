/**
 * SAP v2 - Test 11: Hardening
 *
 * Covers current production escrow surfaces:
 * - Escrow V2 stake gate
 * - SOL/USDC payment-token allowlist
 * - Volume curve monotonicity
 * - Removal of deprecated V1 receipt/batch paths from the IDL
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SynapseAgentSap } from "../target/types/synapse_agent_sap";
import { Keypair, SystemProgram, PublicKey } from "@solana/web3.js";
import { expect } from "chai";
import { BN } from "bn.js";
import {
  findEscrowV2Pda,
  findStakePda,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  initAgentStake,
  expectError,
  MIN_AGENT_STAKE_LAMPORTS,
} from "./helpers";

describe("11 - SAP v2 hardening", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();
  const agentNoStake = Keypair.generate();
  const client = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;
  let statsPda: PublicKey;
  let pricingPda: PublicKey;
  let stakePda: PublicKey;
  let escrowNonce = 1;

  const PRICE = 1_000_000;
  const DEPOSIT = 50 * PRICE;

  before(async () => {
    await Promise.all([
      airdrop(connection, authority.publicKey, 10),
      airdrop(connection, agentOwner.publicKey, 10),
      airdrop(connection, agentNoStake.publicKey, 10),
      airdrop(connection, client.publicKey, 10),
    ]);
    globalPda = await ensureGlobalInitialized(program, authority);
    const reg = await registerAgent(program, agentOwner, globalPda, {
      name: "HardenedAgent",
    });
    agentPda = reg.agentPda;
    statsPda = reg.statsPda;
    pricingPda = reg.pricingPda;

    const stakeRes = await initAgentStake(program, agentOwner);
    stakePda = stakeRes.stakePda;
    const stake = await program.account.agentStake.fetch(stakePda);
    expect(stake.stakedAmount.toNumber()).to.be.gte(MIN_AGENT_STAKE_LAMPORTS);
  });

  function nextEscrow(agent: PublicKey, depositor: PublicKey) {
    const nonce = escrowNonce++;
    const [escrowPda] = findEscrowV2Pda(agent, depositor, nonce);
    return { nonce, escrowPda };
  }

  function createEscrowV2Ix(params: {
    nonce: number;
    escrowPda: PublicKey;
    depositor: PublicKey;
    signer: Keypair;
    agent: PublicKey;
    agentStake: PublicKey;
    agentStats: PublicKey;
    pricingMenu: PublicKey;
    tokenMint: PublicKey | null;
    tokenDecimals: number;
    volumeCurve?: Array<{
      afterCalls: number;
      pricePerCall: InstanceType<typeof BN>;
    }>;
  }) {
    return program.methods
      .createEscrowV2(
        new BN(params.nonce),
        new BN(PRICE),
        new BN(10),
        new BN(DEPOSIT),
        new BN(0),
        params.volumeCurve ?? [],
        params.tokenMint,
        params.tokenDecimals,
        1,
        new BN(0),
        params.depositor,
        null
      )
      .accountsStrict({
        depositor: params.depositor,
        agent: params.agent,
        agentStake: params.agentStake,
        agentStats: params.agentStats,
        pricingMenu: params.pricingMenu,
        escrow: params.escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([params.signer]);
  }

  describe("Stake-gate on createEscrowV2", () => {
    it("rejects createEscrowV2 when agent has no stake PDA", async () => {
      const reg = await registerAgent(program, agentNoStake, globalPda, {
        name: "NoStakeAgent",
      });
      const { nonce, escrowPda } = nextEscrow(reg.agentPda, client.publicKey);
      const [stakeMissing] = findStakePda(reg.agentPda);

      await expectError(
        createEscrowV2Ix({
          nonce,
          escrowPda,
          depositor: client.publicKey,
          signer: client,
          agent: reg.agentPda,
          agentStake: stakeMissing,
          agentStats: reg.statsPda,
          pricingMenu: reg.pricingPda,
          tokenMint: null,
          tokenDecimals: 9,
        }).rpc(),
        "AccountNotInitialized"
      );
    });

    it("accepts createEscrowV2 when agent has stake >= MIN_STAKE", async () => {
      const { nonce, escrowPda } = nextEscrow(agentPda, client.publicKey);

      await createEscrowV2Ix({
        nonce,
        escrowPda,
        depositor: client.publicKey,
        signer: client,
        agent: agentPda,
        agentStake: stakePda,
        agentStats: statsPda,
        pricingMenu: pricingPda,
        tokenMint: null,
        tokenDecimals: 9,
      }).rpc();

      const escrow = await program.account.escrowAccountV2.fetch(escrowPda);
      expect(escrow.balance.toNumber()).to.equal(DEPOSIT);
    });
  });

  describe("Payment-token allowlist", () => {
    it("rejects createEscrowV2 with arbitrary SPL mint", async () => {
      const fakeMint = Keypair.generate().publicKey;
      const otherClient = Keypair.generate();
      await airdrop(connection, otherClient.publicKey, 2);
      const { nonce, escrowPda } = nextEscrow(agentPda, otherClient.publicKey);

      await expectError(
        createEscrowV2Ix({
          nonce,
          escrowPda,
          depositor: otherClient.publicKey,
          signer: otherClient,
          agent: agentPda,
          agentStake: stakePda,
          agentStats: statsPda,
          pricingMenu: pricingPda,
          tokenMint: fakeMint,
          tokenDecimals: 6,
        }).rpc(),
        "InvalidPaymentToken"
      );
    });
  });

  describe("Volume curve monotonicity", () => {
    it("rejects ascending price curve", async () => {
      const otherClient = Keypair.generate();
      await airdrop(connection, otherClient.publicKey, 2);
      const { nonce, escrowPda } = nextEscrow(agentPda, otherClient.publicKey);

      await expectError(
        createEscrowV2Ix({
          nonce,
          escrowPda,
          depositor: otherClient.publicKey,
          signer: otherClient,
          agent: agentPda,
          agentStake: stakePda,
          agentStats: statsPda,
          pricingMenu: pricingPda,
          tokenMint: null,
          tokenDecimals: 9,
          volumeCurve: [{ afterCalls: 50, pricePerCall: new BN(PRICE * 2) }],
        }).rpc(),
        "VolumeCurveNotDescending"
      );
    });
  });

  describe("Deprecated V1 settlement surfaces", () => {
    it("does not expose V1 receipt or batch settlement accounts/instructions", async () => {
      const instructionNames = program.idl.instructions.map((ix) => ix.name);
      const accountNames =
        program.idl.accounts?.map((account) => account.name) ?? [];

      expect(instructionNames).to.not.include("settle_calls");
      expect(instructionNames).to.not.include("settle_batch");
      expect(accountNames).to.not.include("settlement_receipt");
      expect(accountNames).to.not.include("escrow_account");
    });
  });
});
