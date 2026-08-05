const fs = require("fs");
const os = require("os");
const path = require("path");
const anchor = require("@coral-xyz/anchor");
const { BN } = anchor;
const {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} = require("@solana/web3.js");

const PROGRAM_ID = new PublicKey("SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ");
const TREASURY = new PublicKey("J7PyZAGKvprCz4SQ5DKBLAHstJxgVqZcz6kguUoWpP7P");
const RPC = process.env.SAP_RPC || "https://api.devnet.solana.com";
const LAMPORTS = 1_000_000_000;

const seeds = {
  global: Buffer.from("sap_global"),
  agent: Buffer.from("sap_agent"),
  stats: Buffer.from("sap_stats"),
  pricing: Buffer.from("sap_pricing"),
  stake: Buffer.from("sap_stake"),
  escrowV2: Buffer.from("sap_escrow_v2"),
  vault: Buffer.from("sap_vault"),
};

function kp(file) {
  return Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(file, "utf8")))
  );
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function installRpcRetry(connection) {
  const original = connection._rpcRequest.bind(connection);
  connection._rpcRequest = async (method, args) => {
    let lastError;
    for (let attempt = 1; attempt <= 8; attempt += 1) {
      try {
        return await original(method, args);
      } catch (error) {
        lastError = error;
        const message = String(error?.message || error);
        if (
          !message.includes("429") &&
          !message.includes("Too Many Requests")
        ) {
          throw error;
        }
        await sleep(Math.min(20_000, 500 * 2 ** (attempt - 1)));
      }
    }
    throw lastError;
  };
}

function pda(parts) {
  return PublicKey.findProgramAddressSync(parts, PROGRAM_ID)[0];
}

function u64le(n) {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(n));
  return b;
}

function capability(id) {
  return {
    id,
    description: "Devnet smoke test capability",
    protocolId: "sap",
    version: "1.0.0",
  };
}

function pricing() {
  return {
    tierId: "standard",
    pricePerCall: new BN(1_000_000),
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

async function fund(connection, payer, to, lamports) {
  const tx = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey: payer.publicKey,
      toPubkey: to,
      lamports,
    })
  );
  const sig = await sendAndConfirmTransaction(connection, tx, [payer], {
    commitment: "confirmed",
  });
  console.log("fund", to.toBase58(), lamports, sig);
}

async function main() {
  const walletPath =
    process.env.ANCHOR_WALLET ||
    path.join(os.homedir(), ".config", "solana", "id.json");
  const authority = kp(walletPath);
  const connection = new Connection(RPC, "confirmed");
  installRpcRetry(connection);
  const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(authority),
    { commitment: "confirmed", preflightCommitment: "confirmed" }
  );
  anchor.setProvider(provider);
  const idl = JSON.parse(
    fs.readFileSync("target/idl/synapse_agent_sap.json", "utf8")
  );
  const program = new anchor.Program(idl, provider);

  const agentOwner = Keypair.generate();
  const client = Keypair.generate();
  await fund(
    connection,
    authority,
    agentOwner.publicKey,
    Math.floor(0.55 * LAMPORTS)
  );
  await fund(
    connection,
    authority,
    client.publicKey,
    Math.floor(0.2 * LAMPORTS)
  );

  const global = pda([seeds.global]);
  try {
    const sig = await program.methods
      .initializeGlobal()
      .accountsStrict({
        authority: authority.publicKey,
        globalRegistry: global,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();
    console.log("initializeGlobal", sig);
  } catch (e) {
    console.log(
      "initializeGlobal skipped",
      e.error?.errorCode?.code || e.message.split("\n")[0]
    );
  }

  const agent = pda([seeds.agent, agentOwner.publicKey.toBuffer()]);
  const stats = pda([seeds.stats, agent.toBuffer()]);
  const menu = pda([seeds.pricing, agent.toBuffer()]);
  const stake = pda([seeds.stake, agent.toBuffer()]);
  const vaultCheck = pda([seeds.vault, agent.toBuffer()]);

  const treasuryBeforeRegister = await connection.getBalance(TREASURY);
  const registerSig = await program.methods
    .registerAgent(
      `SmokeAgent${Date.now()}`,
      "Devnet smoke test agent",
      [capability("smoke:test")],
      [pricing()],
      ["x402"],
      null,
      null,
      null
    )
    .accountsStrict({
      wallet: agentOwner.publicKey,
      agent,
      agentStats: stats,
      pricingMenu: menu,
      globalRegistry: global,
      systemProgram: SystemProgram.programId,
    })
    .remainingAccounts([
      { pubkey: TREASURY, isSigner: false, isWritable: true },
    ])
    .signers([agentOwner])
    .rpc();
  const treasuryAfterRegister = await connection.getBalance(TREASURY);
  console.log("registerAgent", registerSig);
  console.log(
    "registerTreasuryDelta",
    treasuryAfterRegister - treasuryBeforeRegister
  );
  if (treasuryAfterRegister - treasuryBeforeRegister !== 100_000_000) {
    throw new Error("registration fee did not reach treasury");
  }

  const stakeSig = await program.methods
    .initStake(new BN(100_000_000))
    .accountsStrict({
      wallet: agentOwner.publicKey,
      agent,
      stake,
      systemProgram: SystemProgram.programId,
    })
    .signers([agentOwner])
    .rpc();
  console.log("initStake", stakeSig);

  const migrateSig = await program.methods
    .migratePricingMenu()
    .accountsStrict({
      wallet: agentOwner.publicKey,
      agent,
      pricingMenu: menu,
      systemProgram: SystemProgram.programId,
    })
    .signers([agentOwner])
    .rpc();
  console.log("migratePricingMenu", migrateSig);

  const nonce = Math.floor(Date.now() / 1000);
  const escrow = pda([
    seeds.escrowV2,
    agent.toBuffer(),
    client.publicKey.toBuffer(),
    u64le(nonce),
  ]);
  const createSig = await program.methods
    .createEscrowV2(
      new BN(nonce),
      new BN(1_000_000),
      new BN(5),
      new BN(5_000_000),
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
      agent,
      agentStake: stake,
      agentStats: stats,
      pricingMenu: menu,
      escrow,
      systemProgram: SystemProgram.programId,
    })
    .signers([client])
    .rpc();
  console.log("createEscrowV2", createSig);

  const treasuryBeforeSettle = await connection.getBalance(TREASURY);
  const serviceHash = Array.from(Keypair.generate().publicKey.toBuffer());
  const settleSig = await program.methods
    .settleCallsV2(new BN(nonce), new BN(1), serviceHash)
    .accountsStrict({
      wallet: agentOwner.publicKey,
      agent,
      agentStats: stats,
      escrow,
      systemProgram: SystemProgram.programId,
    })
    .remainingAccounts([
      { pubkey: TREASURY, isWritable: true, isSigner: false },
      { pubkey: client.publicKey, isWritable: false, isSigner: true },
    ])
    .signers([agentOwner, client])
    .rpc();
  const treasuryAfterSettle = await connection.getBalance(TREASURY);
  console.log("settleCallsV2", settleSig);
  console.log(
    "settleTreasuryDelta",
    treasuryAfterSettle - treasuryBeforeSettle
  );
  if (treasuryAfterSettle - treasuryBeforeSettle !== 5_000) {
    throw new Error("settlement fee did not reach treasury");
  }

  let escrowAccount = await program.account.escrowAccountV2.fetch(escrow);
  if (escrowAccount.balance.gt(new BN(0))) {
    const withdrawSig = await program.methods
      .withdrawEscrowV2(escrowAccount.balance)
      .accountsStrict({ depositor: client.publicKey, escrow })
      .signers([client])
      .rpc();
    console.log("withdrawEscrowV2", withdrawSig);
  }

  const closeEscrowSig = await program.methods
    .closeEscrowV2()
    .accountsStrict({ depositor: client.publicKey, escrow, agentStats: stats })
    .signers([client])
    .rpc();
  console.log("closeEscrowV2", closeEscrowSig);

  const agentOwnerBeforeClose = await connection.getBalance(
    agentOwner.publicKey
  );
  const closeAgentSig = await program.methods
    .closeAgent()
    .accountsStrict({
      wallet: agentOwner.publicKey,
      agent,
      agentStats: stats,
      vaultCheck,
      pricingMenu: menu,
      stake,
      globalRegistry: global,
    })
    .signers([agentOwner])
    .rpc();
  const agentOwnerAfterClose = await connection.getBalance(
    agentOwner.publicKey
  );
  console.log("closeAgent", closeAgentSig);
  console.log(
    "closeAgentWalletDelta",
    agentOwnerAfterClose - agentOwnerBeforeClose
  );

  const agentInfo = await connection.getAccountInfo(agent);
  const statsInfo = await connection.getAccountInfo(stats);
  const stakeInfo = await connection.getAccountInfo(stake);
  console.log("closedAccounts", {
    agent: agentInfo === null,
    stats: statsInfo === null,
    stake: stakeInfo === null,
  });
  if (agentInfo || statsInfo || stakeInfo)
    throw new Error("agent close cleanup incomplete");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
