/**
 * SAP v2 — Test Helpers
 *
 * Utility functions shared across all test files.
 * PDA derivation, SHA-256 hashing, airdrop, common setup patterns.
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SynapseAgentSap } from "../target/types/synapse_agent_sap";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import { createHash } from "crypto";
import { BN } from "bn.js";

// ═══════════════════════════════════════════════════════════════════
//  Constants
// ═══════════════════════════════════════════════════════════════════

export const PROGRAM_ID = new PublicKey(
  "SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ"
);

/** Convenience: all seeds used by the program */
export const SEEDS = {
  GLOBAL: Buffer.from("sap_global"),
  AGENT: Buffer.from("sap_agent"),
  STATS: Buffer.from("sap_stats"),
  FEEDBACK: Buffer.from("sap_feedback"),
  CAP_IDX: Buffer.from("sap_cap_idx"),
  PROTO_IDX: Buffer.from("sap_proto_idx"),
  TOOL_CAT: Buffer.from("sap_tool_cat"),
  VAULT: Buffer.from("sap_vault"),
  SESSION: Buffer.from("sap_session"),
  EPOCH: Buffer.from("sap_epoch"),
  DELEGATE: Buffer.from("sap_delegate"),
  TOOL: Buffer.from("sap_tool"),
  CHECKPOINT: Buffer.from("sap_checkpoint"),
  ESCROW: Buffer.from("sap_escrow"),
  ATTEST: Buffer.from("sap_attest"),
  LEDGER: Buffer.from("sap_ledger"),
  PAGE: Buffer.from("sap_page"),
  PLUGIN: Buffer.from("sap_plugin"),
  MEMORY: Buffer.from("sap_memory"),
  MEM_CHUNK: Buffer.from("sap_mem_chunk"),
  BUFFER: Buffer.from("sap_buffer"),
  DIGEST: Buffer.from("sap_digest"),
} as const;

// ═══════════════════════════════════════════════════════════════════
//  SHA-256 Helper
// ═══════════════════════════════════════════════════════════════════

/** Compute sha256 of a UTF-8 string → 32-byte Buffer */
export function sha256(data: string): Buffer {
  return createHash("sha256").update(data, "utf8").digest();
}

/** Compute sha256 of raw bytes → 32-byte Buffer */
export function sha256Bytes(data: Buffer): Buffer {
  return createHash("sha256").update(data).digest();
}

// ═══════════════════════════════════════════════════════════════════
//  PDA Derivation
// ═══════════════════════════════════════════════════════════════════

export function findGlobalPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([SEEDS.GLOBAL], PROGRAM_ID);
}

export function findAgentPda(wallet: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.AGENT, wallet.toBuffer()],
    PROGRAM_ID
  );
}

export function findStatsPda(agentPda: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.STATS, agentPda.toBuffer()],
    PROGRAM_ID
  );
}

export function findFeedbackPda(
  agentPda: PublicKey,
  reviewer: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.FEEDBACK, agentPda.toBuffer(), reviewer.toBuffer()],
    PROGRAM_ID
  );
}

export function findCapabilityIndexPda(
  capabilityHash: Buffer
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.CAP_IDX, capabilityHash],
    PROGRAM_ID
  );
}

export function findProtocolIndexPda(
  protocolHash: Buffer
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.PROTO_IDX, protocolHash],
    PROGRAM_ID
  );
}

export function findToolCategoryIndexPda(
  category: number
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.TOOL_CAT, Buffer.from([category])],
    PROGRAM_ID
  );
}

export function findVaultPda(agentPda: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.VAULT, agentPda.toBuffer()],
    PROGRAM_ID
  );
}

export function findSessionPda(
  vaultPda: PublicKey,
  sessionHash: Buffer
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.SESSION, vaultPda.toBuffer(), sessionHash],
    PROGRAM_ID
  );
}

export function findEpochPagePda(
  sessionPda: PublicKey,
  epochIndex: number
): [PublicKey, number] {
  const buf = Buffer.alloc(4);
  buf.writeUInt32LE(epochIndex, 0);
  return PublicKey.findProgramAddressSync(
    [SEEDS.EPOCH, sessionPda.toBuffer(), buf],
    PROGRAM_ID
  );
}

export function findDelegatePda(
  vaultPda: PublicKey,
  delegate: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.DELEGATE, vaultPda.toBuffer(), delegate.toBuffer()],
    PROGRAM_ID
  );
}

export function findToolPda(
  agentPda: PublicKey,
  toolNameHash: Buffer
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.TOOL, agentPda.toBuffer(), toolNameHash],
    PROGRAM_ID
  );
}

export function findCheckpointPda(
  sessionPda: PublicKey,
  index: number
): [PublicKey, number] {
  const buf = Buffer.alloc(4);
  buf.writeUInt32LE(index, 0);
  return PublicKey.findProgramAddressSync(
    [SEEDS.CHECKPOINT, sessionPda.toBuffer(), buf],
    PROGRAM_ID
  );
}

export function findEscrowPda(
  agentPda: PublicKey,
  depositor: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.ESCROW, agentPda.toBuffer(), depositor.toBuffer()],
    PROGRAM_ID
  );
}

export function findAttestationPda(
  agentPda: PublicKey,
  attester: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.ATTEST, agentPda.toBuffer(), attester.toBuffer()],
    PROGRAM_ID
  );
}

export function findLedgerPda(sessionPda: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.LEDGER, sessionPda.toBuffer()],
    PROGRAM_ID
  );
}

export function findLedgerPagePda(
  ledgerPda: PublicKey,
  pageIndex: number
): [PublicKey, number] {
  const buf = Buffer.alloc(4);
  buf.writeUInt32LE(pageIndex, 0);
  return PublicKey.findProgramAddressSync(
    [SEEDS.PAGE, ledgerPda.toBuffer(), buf],
    PROGRAM_ID
  );
}

export function findPluginPda(
  agentPda: PublicKey,
  pluginType: number
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.PLUGIN, agentPda.toBuffer(), Buffer.from([pluginType])],
    PROGRAM_ID
  );
}

// ═══════════════════════════════════════════════════════════════════
//  Airdrop Helper
// ═══════════════════════════════════════════════════════════════════

export async function airdrop(
  connection: anchor.web3.Connection,
  to: PublicKey,
  sol: number = 10
): Promise<void> {
  const sig = await connection.requestAirdrop(to, sol * LAMPORTS_PER_SOL);
  await connection.confirmTransaction(sig, "confirmed");
}

// ═══════════════════════════════════════════════════════════════════
//  Default Registration Data
// ═══════════════════════════════════════════════════════════════════

export function defaultCapability(id: string = "jupiter:swap") {
  return {
    id,
    description: "DeFi swap aggregator",
    protocolId: "jupiter",
    version: "1.0.0",
  };
}

export function defaultPricing(tierId: string = "standard") {
  return {
    tierId,
    pricePerCall: new BN(1_000_000), // 0.001 SOL
    minPricePerCall: null,
    maxPricePerCall: null,
    rateLimit: 100,
    maxCallsPerSession: 0,
    burstLimit: null,
    tokenType: { sol: {} },
    tokenMint: null,
    tokenDecimals: null,
    settlementMode: null,
    minEscrowDeposit: null,
    batchIntervalSec: null,
    volumeCurve: null,
  };
}

export function defaultRegistrationArgs(
  name: string = "TestAgent",
  description: string = "A test agent for SAP v2"
) {
  return {
    name,
    description,
    capabilities: [defaultCapability()],
    pricing: [defaultPricing()],
    protocols: ["x402"],
    agentId: null,
    agentUri: null,
    x402Endpoint: null,
  };
}

// ═══════════════════════════════════════════════════════════════════
//  Common Setup: Init Global + Register Agent
// ═══════════════════════════════════════════════════════════════════

/**
 * Initializes global registry (idempotent — catches "already in use").
 * Returns the global PDA.
 */
export async function ensureGlobalInitialized(
  program: Program<SynapseAgentSap>,
  authority: Keypair
): Promise<PublicKey> {
  const [globalPda] = findGlobalPda();
  try {
    await program.methods
      .initializeGlobal()
      .accountsStrict({
        authority: authority.publicKey,
        globalRegistry: globalPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();
  } catch {
    // Already initialized — that's fine
  }
  return globalPda;
}

/**
 * Registers an agent with default params.
 * Returns { agentPda, statsPda }.
 */
export async function registerAgent(
  program: Program<SynapseAgentSap>,
  wallet: Keypair,
  globalPda: PublicKey,
  overrides: Partial<ReturnType<typeof defaultRegistrationArgs>> = {}
): Promise<{ agentPda: PublicKey; statsPda: PublicKey }> {
  const [agentPda] = findAgentPda(wallet.publicKey);
  const [statsPda] = findStatsPda(agentPda);
  const args = { ...defaultRegistrationArgs(), ...overrides };

  await program.methods
    .registerAgent(
      args.name,
      args.description,
      args.capabilities,
      args.pricing,
      args.protocols,
      args.agentId,
      args.agentUri,
      args.x402Endpoint
    )
    .accountsStrict({
      wallet: wallet.publicKey,
      agent: agentPda,
      agentStats: statsPda,
      globalRegistry: globalPda,
      systemProgram: SystemProgram.programId,
    })
    .signers([wallet])
    .rpc();

  return { agentPda, statsPda };
}

// ═══════════════════════════════════════════════════════════════════
//  Random Data Generators
// ═══════════════════════════════════════════════════════════════════

/** Generate a random 32-byte hash */
export function randomHash(): number[] {
  return Array.from(Keypair.generate().publicKey.toBuffer());
}

/** Generate a zero 32-byte array */
export function zeroHash(): number[] {
  return new Array(32).fill(0);
}

/** Generate a random 12-byte nonce */
export function randomNonce(): number[] {
  return Array.from(Buffer.from(Keypair.generate().publicKey.toBuffer().subarray(0, 12)));
}

/** Generate a random 32-byte vault nonce */
export function randomVaultNonce(): number[] {
  return Array.from(Keypair.generate().publicKey.toBuffer());
}

/** Short sleep util */
export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ═══════════════════════════════════════════════════════════════════
//  Error Matchers
// ═══════════════════════════════════════════════════════════════════

/**
 * Asserts that a promise rejects with a specific Anchor error code.
 * Usage: await expectError(promise, "SelfReviewNotAllowed")
 */
export async function expectError(
  promise: Promise<any>,
  errorName: string
): Promise<void> {
  try {
    await promise;
    throw new Error(`Expected error "${errorName}" but TX succeeded`);
  } catch (err: any) {
    const msg = err?.message || err?.toString() || "";
    if (!msg.includes(errorName) && !msg.includes("Error Code:")) {
      // Try to match the short error message
      const errorMessages: Record<string, string> = {
        NameTooLong: "name>64",
        DescriptionTooLong: "desc>256",
        SelfReviewNotAllowed: "self review",
        InvalidFeedbackScore: "score 0-1000",
        FeedbackAlreadyRevoked: "already revoked",
        AlreadyActive: "already active",
        AlreadyInactive: "already inactive",
        InsufficientEscrowBalance: "low balance",
        EscrowExpired: "escrow expired",
        SelfAttestationNotAllowed: "self attest",
        AttestationAlreadyRevoked: "already revoked",
        AgentInactive: "agent inactive",
        Unauthorized: "unauthorized",
        EmptyName: "empty name",
        ControlCharInName: "ctrl char",
        InvalidCapabilityFormat: "cap format",
        EmptyDescription: "empty desc",
        ToolNameTooLong: "tool>32",
        EmptyToolName: "empty tool",
        NoFieldsToUpdate: "no fields",
        EscrowNotEmpty: "escrow!=0",
        InvalidSettlementCalls: "calls<1",
        EscrowMaxCallsExceeded: "max calls",
        FeedbackNotRevoked: "not revoked",
        SessionClosed: "session closed",
        InscriptionTooLarge: "data>750",
        EmptyInscription: "empty data",
        LedgerDataTooLarge: "ledger>750",
        LedgerRingEmpty: "ring empty",
        BatchEmpty: "batch empty",
        BatchTooLarge: "batch>10",
        ToolAlreadyInactive: "tool inactive",
        ToolAlreadyActive: "tool active",
        AttestationNotRevoked: "not revoked",
        AttestationTypeTooLong: "atype>32",
        EmptyAttestationType: "empty atype",
        ArithmeticOverflow: "overflow",
        InvalidX402Endpoint: "x402 https",
        InvalidUptimePercent: "uptime 0-100",
        TagTooLong: "tag>32",
        IndexNotEmpty: "idx not empty",
        CapabilityIndexFull: "cap idx full",
        ProtocolIndexFull: "proto idx full",
        SessionStillOpen: "session open",
      };
      const expected = errorMessages[errorName] || errorName;
      if (!msg.includes(expected) && !msg.includes(errorName)) {
        throw new Error(
          `Expected error "${errorName}" (${expected}) but got: ${msg.substring(0, 200)}`
        );
      }
    }
  }
}
