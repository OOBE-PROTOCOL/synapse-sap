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
  findFeedbackPda,
  findEscrowPda,
  findVaultPda,
  findSessionPda,
  findAttestationPda,
  findToolPda,
  findLedgerPda,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  defaultRegistrationArgs,
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
  const program = anchor.workspace
    .synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();
  const attacker = Keypair.generate();
  const reviewer = Keypair.generate();
  const client = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;
  let statsPda: PublicKey;

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
  });

  // ═══════════════════════════════════════════════════════════════
  //  INPUT VALIDATION
  // ═══════════════════════════════════════════════════════════════

  it("Errore: nome vuoto", async () => {
    const w = Keypair.generate();
    await airdrop(connection, w.publicKey, 5);
    const [ap] = findAgentPda(w.publicKey);
    const [sp] = findStatsPda(ap);

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
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
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
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
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
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
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
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
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
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
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
          globalRegistry: globalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([w])
        .rpc(),
      "InvalidX402Endpoint"
    );
  });

  it("Errore: uptime > 100", async () => {
    await expectError(
      program.methods
        .updateReputation(150, 101) // uptime > 100
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
        })
        .signers([agentOwner])
        .rpc(),
      "InvalidUptimePercent"
    );
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
    const [escrowPda] = findEscrowPda(agentPda, client.publicKey);
    const smallDeposit = 1000; // Very small

    await program.methods
      .createEscrow(
        new BN(1_000_000), // 1M per call
        new BN(0),
        new BN(smallDeposit),
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
      "InsufficientEscrowBalance"
    );

    // Cleanup
    await program.methods
      .withdrawEscrow(new BN(smallDeposit))
      .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
      .signers([client])
      .rpc();
    await program.methods
      .closeEscrow()
      .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
      .signers([client])
      .rpc();
  });

  it("Errore: settle con calls_to_settle = 0", async () => {
    const [escrowPda] = findEscrowPda(agentPda, client.publicKey);

    await program.methods
      .createEscrow(
        new BN(100_000),
        new BN(0),
        new BN(1_000_000),
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

    await expectError(
      program.methods
        .settleCalls(new BN(0), randomHash())
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          escrow: escrowPda,
        })
        .signers([agentOwner])
        .rpc(),
      "InvalidSettlementCalls"
    );

    // Cleanup
    const escrow = await program.account.escrowAccount.fetch(escrowPda);
    await program.methods
      .withdrawEscrow(escrow.balance)
      .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
      .signers([client])
      .rpc();
    await program.methods
      .closeEscrow()
      .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
      .signers([client])
      .rpc();
  });

  it("Errore: close escrow con saldo > 0", async () => {
    const [escrowPda] = findEscrowPda(agentPda, client.publicKey);

    await program.methods
      .createEscrow(
        new BN(100_000),
        new BN(0),
        new BN(500_000),
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

    await expectError(
      program.methods
        .closeEscrow()
        .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
        .signers([client])
        .rpc(),
      "EscrowNotEmpty"
    );

    // Cleanup
    await program.methods
      .withdrawEscrow(new BN(500_000))
      .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
      .signers([client])
      .rpc();
    await program.methods
      .closeEscrow()
      .accountsStrict({ depositor: client.publicKey, escrow: escrowPda })
      .signers([client])
      .rpc();
  });

  it("Errore: escrow max_calls superato", async () => {
    const [escrowPda] = findEscrowPda(agentPda, client.publicKey);

    await program.methods
      .createEscrow(
        new BN(1000),
        new BN(2), // max 2 calls
        new BN(100_000),
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

    // Settle 2 → OK
    await program.methods
      .settleCalls(new BN(2), randomHash())
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        agentStats: statsPda,
        escrow: escrowPda,
      })
      .signers([agentOwner])
      .rpc();

    // Settle 1 more → FAIL (max exceeded)
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
      "EscrowMaxCallsExceeded"
    );

    // Cleanup
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

  it("Errore: batch settlement vuoto", async () => {
    const [escrowPda] = findEscrowPda(agentPda, client.publicKey);

    await program.methods
      .createEscrow(
        new BN(1000),
        new BN(0),
        new BN(100_000),
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

    await expectError(
      program.methods
        .settleBatch([])
        .accountsStrict({
          wallet: agentOwner.publicKey,
          agent: agentPda,
          agentStats: statsPda,
          escrow: escrowPda,
        })
        .signers([agentOwner])
        .rpc(),
      "BatchEmpty"
    );

    // Cleanup
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
          0, 0, 1, 1, false
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
          0, 0, 1, 1, false
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
        1, 0, 2, 1, false
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
