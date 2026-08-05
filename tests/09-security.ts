/**
 * SAP v2 — Test 09: Security & Exploit Prevention
 *
 * Testa TUTTI i path di errore critici:
 * - Self-review/self-attestation bloccati
 * - Unauthorized access (wallet diverso)
 * - Input validation (name, description, score limits)
 * - Escrow guards (insufficient balance, expired, max calls)
 * - Vault guards (session closed, wrong sequence)
 * - Overflow protection
 *
 * Best Practice: Ogni exploit path deve essere testato.
 * Se un test "passa" quando dovrebbe fallire, c'è un exploit.
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
  findPricingPda,
  findFeedbackPda,
  findEscrowV2Pda,
  findVaultPda,
  findSessionPda,
  findAttestationPda,
  findToolPda,
  findLedgerPda,
  findStakePda,
  PROTOCOL_TREASURY,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  initAgentStake,
  defaultCapability,
  defaultPricing,
  sha256,
  sha256Bytes,
  randomHash,
  randomNonce,
  randomVaultNonce,
  expectError,
} from "./helpers";

describe("09 — Security & Exploit Prevention", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();
  const attacker = Keypair.generate();
  const reviewer = Keypair.generate();
  const client = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;
  let statsPda: PublicKey;
  let stakePda: PublicKey;
  let pricingPda: PublicKey;
  let escrowNonce = 1;

  before(async () => {
    await Promise.all([
      airdrop(connection, authority.publicKey, 20),
      airdrop(connection, agentOwner.publicKey, 30),
      airdrop(connection, attacker.publicKey, 20),
      airdrop(connection, reviewer.publicKey, 10),
      airdrop(connection, client.publicKey, 20),
    ]);
    globalPda = await ensureGlobalInitialized(program, authority);
    const result = await registerAgent(program, agentOwner, globalPda, {
      name: "SecurityAgent",
      description: "Agent for security testing",
    });
    agentPda = result.agentPda;
    statsPda = result.statsPda;
    pricingPda = result.pricingPda;
    // v0.10 — stake-gate: bootstrap stake before any escrow.
    const stakeRes = await initAgentStake(program, agentOwner);
    stakePda = stakeRes.stakePda;
  });

  async function createSolCoSignedEscrowV2(params: {
    pricePerCall: number;
    maxCalls: number;
    initialDeposit: number;
  }): Promise<{ escrowPda: PublicKey; nonce: number }> {
    const nonce = escrowNonce++;
    const [escrowPda] = findEscrowV2Pda(agentPda, client.publicKey, nonce);

    await program.methods
      .createEscrowV2(
        new BN(nonce),
        new BN(params.pricePerCall),
        new BN(params.maxCalls),
        new BN(params.initialDeposit),
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

    return { escrowPda, nonce };
  }

  function coSignedSettlementRemainingAccounts() {
    return [
      { pubkey: PROTOCOL_TREASURY, isWritable: true, isSigner: false },
      { pubkey: client.publicKey, isWritable: false, isSigner: true },
    ];
  }

  // ═══════════════════════════════════════════════════════════════
  //  INPUT VALIDATION
  // ═══════════════════════════════════════════════════════════════

  it("Errore: register valida senza treasury fee account", async () => {
    const w = Keypair.generate();
    await airdrop(connection, w.publicKey, 5);
    const [ap] = findAgentPda(w.publicKey);
    const [sp] = findStatsPda(ap);
    const [pp] = findPricingPda(ap);

    await expectError(
      program.methods
        .registerAgent(
          "NoTreasuryAgent",
          "valid registration without treasury account",
          [defaultCapability()],
          [defaultPricing()],
          ["x402"],
          null,
          null,
          null
        )
        .accountsStrict({
          wallet: w.publicKey,
          agent: ap,
          agentStats: sp,
          pricingMenu: pp,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([w])
        .rpc(),
      "InvalidTreasury"
    );
  });

  it("Errore: nome vuoto", async () => {
    const w = Keypair.generate();
    await airdrop(connection, w.publicKey, 5);
    const [ap] = findAgentPda(w.publicKey);
    const [sp] = findStatsPda(ap);
    const [pp] = findPricingPda(ap);

    await expectError(
      program.methods
        .registerAgent(
          "",
          "desc",
          [defaultCapability()],
          [defaultPricing()],
          ["x402"],
          null,
          null,
          null
        )
        .accountsStrict({
          wallet: w.publicKey,
          agent: ap,
          agentStats: sp,
          pricingMenu: pp,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: PROTOCOL_TREASURY, isSigner: false, isWritable: true },
        ])
        .signers([w])
        .rpc(),
      "EmptyName"
    );
  });

  it("Errore: nome > 64 bytes", async () => {
    const w = Keypair.generate();
    await airdrop(connection, w.publicKey, 5);
    const [ap] = findAgentPda(w.publicKey);
    const [sp] = findStatsPda(ap);
    const [pp] = findPricingPda(ap);

    await expectError(
      program.methods
        .registerAgent(
          "A".repeat(65),
          "desc",
          [defaultCapability()],
          [defaultPricing()],
          ["x402"],
          null,
          null,
          null
        )
        .accountsStrict({
          wallet: w.publicKey,
          agent: ap,
          agentStats: sp,
          pricingMenu: pp,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: PROTOCOL_TREASURY, isSigner: false, isWritable: true },
        ])
        .signers([w])
        .rpc(),
      "NameTooLong"
    );
  });

  it("Errore: description vuota", async () => {
    const w = Keypair.generate();
    await airdrop(connection, w.publicKey, 5);
    const [ap] = findAgentPda(w.publicKey);
    const [sp] = findStatsPda(ap);
    const [pp] = findPricingPda(ap);

    await expectError(
      program.methods
        .registerAgent(
          "ValidName",
          "",
          [defaultCapability()],
          [defaultPricing()],
          ["x402"],
          null,
          null,
          null
        )
        .accountsStrict({
          wallet: w.publicKey,
          agent: ap,
          agentStats: sp,
          pricingMenu: pp,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: PROTOCOL_TREASURY, isSigner: false, isWritable: true },
        ])
        .signers([w])
        .rpc(),
      "EmptyDescription"
    );
  });

  it("Errore: control char nel nome", async () => {
    const w = Keypair.generate();
    await airdrop(connection, w.publicKey, 5);
    const [ap] = findAgentPda(w.publicKey);
    const [sp] = findStatsPda(ap);
    const [pp] = findPricingPda(ap);

    await expectError(
      program.methods
        .registerAgent(
          "Bad\x00Name",
          "desc",
          [defaultCapability()],
          [defaultPricing()],
          ["x402"],
          null,
          null,
          null
        )
        .accountsStrict({
          wallet: w.publicKey,
          agent: ap,
          agentStats: sp,
          pricingMenu: pp,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: PROTOCOL_TREASURY, isSigner: false, isWritable: true },
        ])
        .signers([w])
        .rpc(),
      "ControlCharInName"
    );
  });

  it("Errore: capability format invalido (manca ':')", async () => {
    const w = Keypair.generate();
    await airdrop(connection, w.publicKey, 5);
    const [ap] = findAgentPda(w.publicKey);
    const [sp] = findStatsPda(ap);
    const [pp] = findPricingPda(ap);

    await expectError(
      program.methods
        .registerAgent(
          "FormatAgent",
          "cap format test",
          [{ ...defaultCapability(), id: "missingcolon" }],
          [defaultPricing()],
          ["x402"],
          null,
          null,
          null
        )
        .accountsStrict({
          wallet: w.publicKey,
          agent: ap,
          agentStats: sp,
          pricingMenu: pp,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: PROTOCOL_TREASURY, isSigner: false, isWritable: true },
        ])
        .signers([w])
        .rpc(),
      "InvalidCapabilityFormat"
    );
  });

  it("Errore: x402 endpoint non https", async () => {
    const w = Keypair.generate();
    await airdrop(connection, w.publicKey, 5);
    const [ap] = findAgentPda(w.publicKey);
    const [sp] = findStatsPda(ap);
    const [pp] = findPricingPda(ap);

    await expectError(
      program.methods
        .registerAgent(
          "X402Agent",
          "x402 test",
          [defaultCapability()],
          [defaultPricing()],
          ["x402"],
          null,
          null,
          "http://insecure.com/x402" // NOT https
        )
        .accountsStrict({
          wallet: w.publicKey,
          agent: ap,
          agentStats: sp,
          pricingMenu: pp,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: PROTOCOL_TREASURY, isSigner: false, isWritable: true },
        ])
        .signers([w])
        .rpc(),
      "InvalidX402Endpoint"
    );
  });

  it.skip("Errore: uptime > 100 (legacy: updateReputation removed in v0.7)", async () => {
    // Instruction `updateReputation` was removed; reputation is now derived
    // from on-chain feedback / settlements. Kept skipped for history.
  });

  // ═══════════════════════════════════════════════════════════════
  //  SELF-REVIEW / SELF-ATTESTATION
  // ═══════════════════════════════════════════════════════════════

  it("Errore: self-review bloccato (owner non può fare feedback su sé)", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, agentOwner.publicKey);

    await expectError(
      program.methods
        .giveFeedback(900, "self-review", null)
        .accountsStrict({
          reviewer: agentOwner.publicKey,
          feedback: feedbackPda,
          agent: agentPda,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([agentOwner])
        .rpc(),
      "SelfReviewNotAllowed"
    );
  });

  it("Errore: self-attestation bloccata", async () => {
    const [attestPda] = findAttestationPda(agentPda, agentOwner.publicKey);

    await expectError(
      program.methods
        .createAttestation("self-verified", randomHash(), new BN(0))
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

  // ═══════════════════════════════════════════════════════════════
  //  FEEDBACK GUARDS
  // ═══════════════════════════════════════════════════════════════

  it("Errore: feedback score > 1000", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, reviewer.publicKey);

    await expectError(
      program.methods
        .giveFeedback(1001, "too-high", null)
        .accountsStrict({
          reviewer: reviewer.publicKey,
          feedback: feedbackPda,
          agent: agentPda,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([reviewer])
        .rpc(),
      "InvalidFeedbackScore"
    );
  });

  it("Errore: feedback tag > 32 bytes", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, reviewer.publicKey);

    await expectError(
      program.methods
        .giveFeedback(500, "A".repeat(33), null)
        .accountsStrict({
          reviewer: reviewer.publicKey,
          feedback: feedbackPda,
          agent: agentPda,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([reviewer])
        .rpc(),
      "TagTooLong"
    );
  });

  it("Errore: double revoke", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, reviewer.publicKey);

    // First: give + revoke
    await program.methods
      .giveFeedback(500, "test", null)
      .accountsStrict({
        reviewer: reviewer.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([reviewer])
      .rpc();

    await program.methods
      .revokeFeedback()
      .accountsStrict({
        reviewer: reviewer.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
      })
      .signers([reviewer])
      .rpc();

    // Second revoke → error
    await expectError(
      program.methods
        .revokeFeedback()
        .accountsStrict({
          reviewer: reviewer.publicKey,
          feedback: feedbackPda,
          agent: agentPda,
        })
        .signers([reviewer])
        .rpc(),
      "FeedbackAlreadyRevoked"
    );

    // Cleanup
    await program.methods
      .closeFeedback()
      .accountsStrict({
        reviewer: reviewer.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
        globalRegistry: globalPda,
      })
      .signers([reviewer])
      .rpc();
  });

  it("Errore: close feedback non revocato", async () => {
    const [feedbackPda] = findFeedbackPda(agentPda, reviewer.publicKey);

    await program.methods
      .giveFeedback(700, "active-fb", null)
      .accountsStrict({
        reviewer: reviewer.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([reviewer])
      .rpc();

    await expectError(
      program.methods
        .closeFeedback()
        .accountsStrict({
          reviewer: reviewer.publicKey,
          feedback: feedbackPda,
          agent: agentPda,
          globalRegistry: globalPda,
        })
        .signers([reviewer])
        .rpc(),
      "FeedbackNotRevoked"
    );

    // Cleanup
    await program.methods
      .revokeFeedback()
      .accountsStrict({
        reviewer: reviewer.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
      })
      .signers([reviewer])
      .rpc();
    await program.methods
      .closeFeedback()
      .accountsStrict({
        reviewer: reviewer.publicKey,
        feedback: feedbackPda,
        agent: agentPda,
        globalRegistry: globalPda,
      })
      .signers([reviewer])
      .rpc();
  });

  // ═══════════════════════════════════════════════════════════════
  //  DEACTIVATE/REACTIVATE GUARDS
  // ═══════════════════════════════════════════════════════════════

  it("Errore: deactivate un agente già inattivo", async () => {
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
        .deactivateAgent()
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          globalRegistry: globalPda,
        })
        .signers([agentOwner])
        .rpc(),
      "AlreadyInactive"
    );

    // reactivate for remaining tests
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
  });

  it("Errore: feedback su agente inattivo bloccato", async () => {
    // Disattiva l'agente
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

    const outsider = Keypair.generate();
    await airdrop(connection, outsider.publicKey, 2);
    const [feedbackPda] = findFeedbackPda(agentPda, outsider.publicKey);

    await expectError(
      program.methods
        .giveFeedback(500, "inactive-target", null)
        .accountsStrict({
          reviewer: outsider.publicKey,
          feedback: feedbackPda,
          agent: agentPda,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([outsider])
        .rpc(),
      "AgentInactive"
    );

    // Riattiva per i test successivi
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
  });

  it("Errore: reactivate un agente già attivo", async () => {
    await expectError(
      program.methods
        .reactivateAgent()
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          globalRegistry: globalPda,
        })
        .signers([agentOwner])
        .rpc(),
      "AlreadyActive"
    );
  });

  // ═══════════════════════════════════════════════════════════════
  //  ESCROW SECURITY
  // ═══════════════════════════════════════════════════════════════

  it("Errore: settle con balance insufficiente", async () => {
    const smallDeposit = 1000;
    const { escrowPda, nonce } = await createSolCoSignedEscrowV2({
      pricePerCall: 1_000_000,
      maxCalls: 0,
      initialDeposit: smallDeposit,
    });

    const h620 = randomHash();
    await expectError(
      program.methods
        .settleCallsV2(new BN(nonce), new BN(1), h620)
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          escrow: escrowPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts(coSignedSettlementRemainingAccounts())
        .signers([agentOwner, client])
        .rpc(),
      "InsufficientEscrowBalance"
    );

    // Cleanup
    await program.methods
      .withdrawEscrowV2(new BN(smallDeposit))
      .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
      .signers([client])
      .rpc();
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

  it("Errore: settle con calls_to_settle = 0", async () => {
    const { escrowPda, nonce } = await createSolCoSignedEscrowV2({
      pricePerCall: 1_000_000,
      maxCalls: 0,
      initialDeposit: 1_000_000,
    });

    const h670 = randomHash();
    await expectError(
      program.methods
        .settleCallsV2(new BN(nonce), new BN(0), h670)
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          escrow: escrowPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts(coSignedSettlementRemainingAccounts())
        .signers([agentOwner, client])
        .rpc(),
      "InvalidSettlementCalls"
    );

    // Cleanup
    const escrow = await program.account.escrowAccountV2.fetch(escrowPda);
    await program.methods
      .withdrawEscrowV2(escrow.balance)
      .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
      .signers([client])
      .rpc();
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

  it("Errore: close escrow con saldo > 0", async () => {
    const { escrowPda } = await createSolCoSignedEscrowV2({
      pricePerCall: 1_000_000,
      maxCalls: 0,
      initialDeposit: 500_000,
    });

    await expectError(
      program.methods
        .closeEscrowV2()
        .accountsStrict({
          depositor: client.publicKey,
          escrow: escrowPda,
          agentStats: statsPda,
        })
        .signers([client])
        .rpc(),
      "EscrowNotEmpty"
    );

    // Cleanup
    await program.methods
      .withdrawEscrowV2(new BN(500_000))
      .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
      .signers([client])
      .rpc();
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

  it("Errore: escrow max_calls superato", async () => {
    const { escrowPda, nonce } = await createSolCoSignedEscrowV2({
      pricePerCall: 1_000_000,
      maxCalls: 2,
      initialDeposit: 3_000_000,
    });

    // Settle 2 → OK
    const h766 = randomHash();
    await program.methods
      .settleCallsV2(new BN(nonce), new BN(2), h766)
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        agentStats: statsPda,
        escrow: escrowPda,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(coSignedSettlementRemainingAccounts())
      .signers([agentOwner, client])
      .rpc();

    // Settle 1 more → FAIL (max exceeded)
    const h779 = randomHash();
    await expectError(
      program.methods
        .settleCallsV2(new BN(nonce), new BN(1), h779)
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          escrow: escrowPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts(coSignedSettlementRemainingAccounts())
        .signers([agentOwner, client])
        .rpc(),
      "EscrowMaxCallsExceeded"
    );

    // Cleanup
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

  it("V1 batch settlement non è più esposto dall'IDL", async () => {
    const names = program.idl.instructions.map((ix) => ix.name);
    expect(names).to.not.include("settle_batch");
    expect(names).to.not.include("create_escrow");
  });

  // ═══════════════════════════════════════════════════════════════
  //  TOOL GUARDS
  // ═══════════════════════════════════════════════════════════════

  it("Errore: tool name vuoto", async () => {
    const emptyNameHash = sha256("");
    const [toolPda] = findToolPda(agentPda, emptyNameHash);

    await expectError(
      program.methods
        .publishTool(
          "",
          Array.from(emptyNameHash),
          randomHash(),
          randomHash(),
          randomHash(),
          randomHash(),
          0,
          0,
          1,
          1,
          false
        )
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          tool: toolPda,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([agentOwner])
        .rpc(),
      "EmptyToolName"
    );
  });

  it("Errore: tool name > 32 bytes", async () => {
    const longName = "A".repeat(33);
    const nameHash = sha256(longName);
    const [toolPda] = findToolPda(agentPda, nameHash);

    await expectError(
      program.methods
        .publishTool(
          longName,
          Array.from(nameHash),
          randomHash(),
          randomHash(),
          randomHash(),
          randomHash(),
          0,
          0,
          1,
          1,
          false
        )
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          tool: toolPda,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([agentOwner])
        .rpc(),
      "ToolNameTooLong"
    );
  });

  it("Errore: update senza campi", async () => {
    const toolName = "secTool";
    const nameHash = sha256(toolName);
    const [toolPda] = findToolPda(agentPda, nameHash);

    await program.methods
      .publishTool(
        toolName,
        Array.from(nameHash),
        randomHash(),
        randomHash(),
        randomHash(),
        randomHash(),
        1,
        0,
        2,
        1,
        false
      )
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        tool: toolPda,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc();

    await expectError(
      program.methods
        .updateTool(null, null, null, null, null, null, null)
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          tool: toolPda,
        })
        .signers([agentOwner])
        .rpc(),
      "NoFieldsToUpdate"
    );

    // Cleanup
    await program.methods
      .closeTool()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        tool: toolPda,
        globalRegistry: globalPda,
      })
      .signers([agentOwner])
      .rpc();
  });

  // ═══════════════════════════════════════════════════════════════
  //  VAULT GUARDS
  // ═══════════════════════════════════════════════════════════════

  it("Errore: inscribe > 750 bytes", async () => {
    // Setup vault + session
    const [vaultPda] = findVaultPda(agentPda);
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

    const sessionHash = sha256("sec-session");
    const [sessionPda] = findSessionPda(vaultPda, sessionHash);
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

    const bigData = Buffer.alloc(751, 0x42);

    await expectError(
      program.methods
        .compactInscribe(
          0,
          bigData,
          randomNonce(),
          Array.from(sha256Bytes(bigData))
        )
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          vault: vaultPda,
          session: sessionPda,
        })
        .signers([agentOwner])
        .rpc(),
      "InscriptionTooLarge"
    );

    // Cleanup
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
  });

  // ═══════════════════════════════════════════════════════════════
  //  ATTESTATION GUARDS
  // ═══════════════════════════════════════════════════════════════

  it("Errore: attestation type vuoto", async () => {
    const [attestPda] = findAttestationPda(agentPda, reviewer.publicKey);

    await expectError(
      program.methods
        .createAttestation("", randomHash(), new BN(0))
        .accountsStrict({
          attester: reviewer.publicKey,
          agent: agentPda,
          attestation: attestPda,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([reviewer])
        .rpc(),
      "EmptyAttestationType"
    );
  });

  it("Errore: attestation type > 32 bytes", async () => {
    const [attestPda] = findAttestationPda(agentPda, reviewer.publicKey);

    await expectError(
      program.methods
        .createAttestation("A".repeat(33), randomHash(), new BN(0))
        .accountsStrict({
          attester: reviewer.publicKey,
          agent: agentPda,
          attestation: attestPda,
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([reviewer])
        .rpc(),
      "AttestationTypeTooLong"
    );
  });
});
