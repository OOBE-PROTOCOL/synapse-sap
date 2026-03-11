/**
 * SAP v2 — Test 03: Tool Registry
 *
 * Tool lifecycle: publish → inscribe schema → update → deactivate →
 * reactivate → report invocations → close.
 *
 * Best Practice: Gli schema dei tool vengono inscritti nei TX log
 * (permanenti, zero rent). Il ToolDescriptor PDA contiene solo gli hash.
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
  findToolPda,
  airdrop,
  ensureGlobalInitialized,
  registerAgent,
  sha256,
  randomHash,
} from "./helpers";

describe("03 — Tool Registry", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .synapseAgentSap as Program<SynapseAgentSap>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const agentOwner = Keypair.generate();

  let globalPda: PublicKey;
  let agentPda: PublicKey;
  let toolPda: PublicKey;

  const toolName = "getQuote";
  const toolNameHash = sha256(toolName);
  const protocolHash = Array.from(sha256("jupiter"));
  const descriptionHash = Array.from(sha256("Get a swap quote from Jupiter"));
  const inputSchemaContent = JSON.stringify({
    type: "object",
    properties: {
      inputMint: { type: "string" },
      outputMint: { type: "string" },
      amount: { type: "number" },
    },
    required: ["inputMint", "outputMint", "amount"],
  });
  const inputSchemaHash = Array.from(sha256(inputSchemaContent));
  const outputSchemaContent = JSON.stringify({
    type: "object",
    properties: {
      inAmount: { type: "string" },
      outAmount: { type: "string" },
      priceImpactPct: { type: "number" },
    },
  });
  const outputSchemaHash = Array.from(sha256(outputSchemaContent));

  before(async () => {
    await airdrop(connection, authority.publicKey, 20);
    await airdrop(connection, agentOwner.publicKey, 20);
    globalPda = await ensureGlobalInitialized(program, authority);
    const result = await registerAgent(program, agentOwner, globalPda, {
      name: "ToolAgent",
    });
    agentPda = result.agentPda;
  });

  // ── 1. Publish Tool ──
  it("Pubblica un tool 'getQuote' con schema hashes", async () => {
    [toolPda] = findToolPda(agentPda, toolNameHash);

    await program.methods
      .publishTool(
        toolName,
        Array.from(toolNameHash),
        protocolHash,
        descriptionHash,
        inputSchemaHash,
        outputSchemaHash,
        0, // GET
        0, // Swap
        3, // params_count
        3, // required_params
        false // not compound
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

    const tool = await program.account.toolDescriptor.fetch(toolPda);
    expect(tool.toolName).to.equal("getQuote");
    expect(tool.isActive).to.equal(true);
    expect(tool.version).to.equal(1);
    expect(tool.paramsCount).to.equal(3);
    expect(tool.requiredParams).to.equal(3);
    expect(tool.isCompound).to.equal(false);
    expect(tool.totalInvocations.toNumber()).to.equal(0);

    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.totalTools).to.equal(1);
  });

  // ── 2. Inscribe Input Schema (permanent TX log) ──
  it("Inscrive l'input schema nei TX log (permanente, zero rent)", async () => {
    const data = Buffer.from(inputSchemaContent);

    await program.methods
      .inscribeToolSchema(
        0, // input schema
        data,
        inputSchemaHash,
        0 // no compression
      )
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        tool: toolPda,
      })
      .signers([agentOwner])
      .rpc();

    // Il dato è permanentemente nei TX log — niente cambia nel PDA
    const tool = await program.account.toolDescriptor.fetch(toolPda);
    expect(tool.version).to.equal(1); // version non cambia con inscribe
  });

  // ── 3. Inscribe Output Schema ──
  it("Inscrive l'output schema nei TX log", async () => {
    const data = Buffer.from(outputSchemaContent);

    await program.methods
      .inscribeToolSchema(
        1, // output schema
        data,
        outputSchemaHash,
        0
      )
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        tool: toolPda,
      })
      .signers([agentOwner])
      .rpc();
  });

  // ── 4. Update Tool ──
  it("Aggiorna il tool — bumpa la version", async () => {
    const newDescHash = Array.from(sha256("Updated: get a swap quote v2"));

    await program.methods
      .updateTool(
        newDescHash,        // description_hash
        null,               // input_schema_hash
        null,               // output_schema_hash
        null,               // http_method
        null,               // category
        null,               // params_count
        null                // required_params
      )
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        tool: toolPda,
      })
      .signers([agentOwner])
      .rpc();

    const tool = await program.account.toolDescriptor.fetch(toolPda);
    expect(tool.version).to.equal(2); // version bumped
  });

  // ── 5. Report Invocations ──
  it("Report 100 invocazioni del tool", async () => {
    await program.methods
      .reportToolInvocations(new BN(100))
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        tool: toolPda,
      })
      .signers([agentOwner])
      .rpc();

    const tool = await program.account.toolDescriptor.fetch(toolPda);
    expect(tool.totalInvocations.toNumber()).to.equal(100);
  });

  // ── 6. Deactivate Tool ──
  it("Disattiva il tool", async () => {
    await program.methods
      .deactivateTool()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        tool: toolPda,
      })
      .signers([agentOwner])
      .rpc();

    const tool = await program.account.toolDescriptor.fetch(toolPda);
    expect(tool.isActive).to.equal(false);
  });

  // ── 7. Reactivate Tool ──
  it("Riattiva il tool", async () => {
    await program.methods
      .reactivateTool()
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent: agentPda,
        tool: toolPda,
      })
      .signers([agentOwner])
      .rpc();

    const tool = await program.account.toolDescriptor.fetch(toolPda);
    expect(tool.isActive).to.equal(true);
  });

  // ── 8. Close Tool ──
  it("Chiude il tool — rent rimborsato, contatore globale decrementato", async () => {
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

    const info = await connection.getAccountInfo(toolPda);
    expect(info).to.be.null;

    const global = await program.account.globalRegistry.fetch(globalPda);
    expect(global.totalTools).to.equal(0);
  });
});
