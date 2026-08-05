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
const {
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  transferChecked,
  getAccount,
} = require("@solana/spl-token");

const PROGRAM_ID = new PublicKey("SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ");
const TREASURY = new PublicKey("J7PyZAGKvprCz4SQ5DKBLAHstJxgVqZcz6kguUoWpP7P");
const USDC_DEVNET = new PublicKey(
  "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
);
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

function usdcPricing(pricePerCall) {
  return {
    tierId: "usdc-standard",
    pricePerCall: new BN(pricePerCall),
    minPricePerCall: null,
    maxPricePerCall: null,
    rateLimit: 100,
    maxCallsPerSession: 0,
    burstLimit: null,
    tokenType: { usdc: {} },
    tokenMint: USDC_DEVNET,
    tokenDecimals: 6,
    settlementMode: null,
    minEscrowDeposit: null,
    batchIntervalSec: null,
    volumeCurve: null,
  };
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
    Math.floor(0.08 * LAMPORTS)
  );

  const authorityAta = await getOrCreateAssociatedTokenAccount(
    connection,
    authority,
    USDC_DEVNET,
    authority.publicKey
  );
  const clientAta = await getOrCreateAssociatedTokenAccount(
    connection,
    authority,
    USDC_DEVNET,
    client.publicKey
  );
  const beforeAuthorityUsdc = await getAccount(
    connection,
    authorityAta.address
  );
  if (beforeAuthorityUsdc.amount < 2_000_000n) {
    throw new Error(`authority USDC too low: ${beforeAuthorityUsdc.amount}`);
  }
  const transferSig = await transferChecked(
    connection,
    authority,
    authorityAta.address,
    USDC_DEVNET,
    clientAta.address,
    authority,
    2_000_000,
    6
  );
  console.log("fundUsdcClient", transferSig);

  const global = pda([seeds.global]);
  const agent = pda([seeds.agent, agentOwner.publicKey.toBuffer()]);
  const stats = pda([seeds.stats, agent.toBuffer()]);
  const menu = pda([seeds.pricing, agent.toBuffer()]);
  const stake = pda([seeds.stake, agent.toBuffer()]);
  const vaultCheck = pda([seeds.vault, agent.toBuffer()]);

  const treasuryBeforeRegister = await connection.getBalance(TREASURY);
  const registerSig = await program.methods
    .registerAgent(
      `UsdcSmoke${Date.now()}`,
      "Devnet USDC smoke test agent",
      [
        {
          id: "smoke:usdc",
          description: "Devnet USDC smoke test capability",
          protocolId: "sap",
          version: "1.0.0",
        },
      ],
      [usdcPricing(100_000)],
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

  console.log(
    "initStake",
    await program.methods
      .initStake(new BN(100_000_000))
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent,
        stake,
        systemProgram: SystemProgram.programId,
      })
      .signers([agentOwner])
      .rpc()
  );

  const nonce = Math.floor(Date.now() / 1000);
  const escrow = pda([
    seeds.escrowV2,
    agent.toBuffer(),
    client.publicKey.toBuffer(),
    u64le(nonce),
  ]);
  const escrowAta = await getOrCreateAssociatedTokenAccount(
    connection,
    authority,
    USDC_DEVNET,
    escrow,
    true
  );
  const agentAta = await getOrCreateAssociatedTokenAccount(
    connection,
    authority,
    USDC_DEVNET,
    agentOwner.publicKey
  );
  const treasuryAta = await getOrCreateAssociatedTokenAccount(
    connection,
    authority,
    USDC_DEVNET,
    TREASURY,
    true
  );

  console.log(
    "createEscrowV2",
    await program.methods
      .createEscrowV2(
        new BN(nonce),
        new BN(100_000),
        new BN(5),
        new BN(1_000_000),
        new BN(0),
        [],
        USDC_DEVNET,
        6,
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
      .remainingAccounts([
        { pubkey: clientAta.address, isWritable: true, isSigner: false },
        { pubkey: escrowAta.address, isWritable: true, isSigner: false },
        { pubkey: TOKEN_PROGRAM_ID, isWritable: false, isSigner: false },
      ])
      .signers([client])
      .rpc()
  );

  const treasuryTokenBefore = (
    await getAccount(connection, treasuryAta.address)
  ).amount;
  console.log(
    "settleCallsV2",
    await program.methods
      .settleCallsV2(
        new BN(nonce),
        new BN(1),
        Array.from(Keypair.generate().publicKey.toBuffer())
      )
      .accountsStrict({
        wallet: agentOwner.publicKey,
        agent,
        agentStats: stats,
        escrow,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([
        { pubkey: escrowAta.address, isWritable: true, isSigner: false },
        { pubkey: agentAta.address, isWritable: true, isSigner: false },
        { pubkey: TOKEN_PROGRAM_ID, isWritable: false, isSigner: false },
        { pubkey: treasuryAta.address, isWritable: true, isSigner: false },
        { pubkey: client.publicKey, isWritable: false, isSigner: true },
      ])
      .signers([agentOwner, client])
      .rpc()
  );
  const treasuryTokenAfter = (await getAccount(connection, treasuryAta.address))
    .amount;
  console.log(
    "usdcTreasuryDelta",
    String(treasuryTokenAfter - treasuryTokenBefore)
  );
  if (treasuryTokenAfter - treasuryTokenBefore !== 500n) {
    throw new Error("USDC settlement fee did not reach treasury ATA");
  }

  const escrowAccount = await program.account.escrowAccountV2.fetch(escrow);
  if (escrowAccount.balance.gt(new BN(0))) {
    console.log(
      "withdrawEscrowV2",
      await program.methods
        .withdrawEscrowV2(escrowAccount.balance)
        .accountsStrict({ depositor: client.publicKey, escrow })
        .remainingAccounts([
          { pubkey: escrowAta.address, isWritable: true, isSigner: false },
          { pubkey: clientAta.address, isWritable: true, isSigner: false },
          { pubkey: TOKEN_PROGRAM_ID, isWritable: false, isSigner: false },
        ])
        .signers([client])
        .rpc()
    );
  }

  console.log(
    "closeEscrowV2",
    await program.methods
      .closeEscrowV2()
      .accountsStrict({
        depositor: client.publicKey,
        escrow,
        agentStats: stats,
      })
      .signers([client])
      .rpc()
  );

  const beforeClose = await connection.getBalance(agentOwner.publicKey);
  console.log(
    "closeAgent",
    await program.methods
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
      .rpc()
  );
  console.log(
    "closeAgentWalletDelta",
    (await connection.getBalance(agentOwner.publicKey)) - beforeClose
  );

  const [agentInfo, statsInfo, stakeInfo] = await Promise.all([
    connection.getAccountInfo(agent),
    connection.getAccountInfo(stats),
    connection.getAccountInfo(stake),
  ]);
  console.log("closedAccounts", {
    agent: agentInfo === null,
    stats: statsInfo === null,
    stake: stakeInfo === null,
  });
  if (agentInfo || statsInfo || stakeInfo)
    throw new Error("USDC agent cleanup incomplete");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
