#!/usr/bin/env node
/**
 * Batch reclaim excess rent from all SAP program-owned PDA accounts on mainnet.
 *
 * Instructions per transaction: 30 (safe margin under 64 account limit)
 * Each instruction: reclaim_excess_rent for one PDA
 * Shared accounts: authority (signer), destination, global_registry, rent sysvar
 * Per-target account: target (writable)
 *
 * Authority: GBLQznn1QMnx64zHXcDguP9yNW9ZfYCVdrY8eDovBvPk (program deployer)
 * Destination: same wallet (where excess lamports land)
 */

const {
  Connection,
  PublicKey,
  Keypair,
  TransactionInstruction,
  Transaction,
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL,
} = require("@solana/web3.js");
const fs = require("fs");

// ── Config ──────────────────────────────────────────────────────
const RPC = "https://api.mainnet-beta.solana.com";
const PROGRAM_ID = new PublicKey("SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ");
const AUTHORITY_KEYPAIR_PATH = "/Users/keepeeto/.config/solana/id.json";
const DESTINATION = new PublicKey("GBLQznn1QMnx64zHXcDguP9yNW9ZfYCVdrY8eDovBvPk");
const RENT_SYSVAR = new PublicKey("SysvarRent111111111111111111111111111111111");
const CURRENT_LPB = 5080; // SIMD-0437-2 active
const TARGETS_PER_TX = 10;
const MAX_RETRIES = 2;

// ── Derived ─────────────────────────────────────────────────────
const [GLOBAL_REGISTRY_PDA] = PublicKey.findProgramAddressSync(
  [Buffer.from("sap_global")],
  PROGRAM_ID
);

// Discriminator for reclaim_excess_rent: 02047ff1e55aad39
const DISCRIMINATOR = Buffer.from([2, 4, 127, 241, 229, 90, 173, 57]);

// ── Main ────────────────────────────────────────────────────────
async function main() {
  const conn = new Connection(RPC, "confirmed");
  const authority = Keypair.fromSecretKey(
    Buffer.from(JSON.parse(fs.readFileSync(AUTHORITY_KEYPAIR_PATH, "utf-8")))
  );

  // Verify authority
  if (authority.publicKey.toBase58() !== "GBLQznn1QMnx64zHXcDguP9yNW9ZfYCVdrY8eDovBvPk") {
    console.error("ERROR: Keypair does not match expected authority wallet!");
    process.exit(1);
  }
  console.log("Authority:", authority.publicKey.toBase58());
  console.log("Destination:", DESTINATION.toBase58());
  console.log("GlobalRegistry PDA:", GLOBAL_REGISTRY_PDA.toBase58());

  // Fetch all program accounts
  console.log("\nFetching all program-owned accounts...");
  const accounts = await conn.getProgramAccounts(PROGRAM_ID, {
    encoding: "base64",
    commitment: "confirmed",
  });
  console.log(`Found ${accounts.length} accounts`);

  // Filter accounts with excess and sort by excess descending
  const reclaimable = [];
  for (const acc of accounts) {
    const len = acc.account.data.length;
    const rentFloor = (128 + len) * CURRENT_LPB;
    const excess = acc.account.lamports - rentFloor;
    if (excess > 0) {
      reclaimable.push({
        pubkey: new PublicKey(acc.pubkey),
        excess,
        balance: acc.account.lamports,
        dataLen: len,
      });
    }
  }
  reclaimable.sort((a, b) => b.excess - a.excess);

  const totalExcess = reclaimable.reduce((s, a) => s + a.excess, 0);
  console.log(`Accounts with excess: ${reclaimable.length}`);
  console.log(`Total excess: ${(totalExcess / LAMPORTS_PER_SOL).toFixed(6)} SOL`);

  // Build batches
  const numTx = Math.ceil(reclaimable.length / TARGETS_PER_TX);
  console.log(`\nBatching: ${reclaimable.length} accounts / ${TARGETS_PER_TX} per tx = ${numTx} transactions`);

  let reclaimed = 0;
  let txCount = 0;
  let failedCount = 0;

  for (let batch = 0; batch < numTx; batch++) {
    const start = batch * TARGETS_PER_TX;
    const end = Math.min(start + TARGETS_PER_TX, reclaimable.length);
    const batchAccounts = reclaimable.slice(start, end);
    const batchExcess = batchAccounts.reduce((s, a) => s + a.excess, 0);

    // Build instructions for this batch
    const instructions = batchAccounts.map((acc) => {
      const keys = [
        { pubkey: acc.pubkey, isSigner: false, isWritable: true },     // target
        { pubkey: authority.publicKey, isSigner: true, isWritable: true },  // authority
        { pubkey: DESTINATION, isSigner: false, isWritable: true },     // destination
        { pubkey: GLOBAL_REGISTRY_PDA, isSigner: false, isWritable: false }, // global_registry
        { pubkey: RENT_SYSVAR, isSigner: false, isWritable: false },   // rent sysvar
      ];
      return new TransactionInstruction({
        programId: PROGRAM_ID,
        keys,
        data: DISCRIMINATOR, // no args, just discriminator
      });
    });

    // Send with retries
    let success = false;
    for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
      try {
        const tx = new Transaction().add(...instructions);
        const sig = await sendAndConfirmTransaction(conn, tx, [authority], {
          commitment: "confirmed",
          skipPreflight: false,
        });
        console.log(`[TX ${batch + 1}/${numTx}] OK — ${batchAccounts.length} accounts, ${(batchExcess / LAMPORTS_PER_SOL).toFixed(6)} SOL — sig: ${sig.slice(0, 16)}...`);
        reclaimed += batchExcess;
        txCount++;
        success = true;
        // Rate limit: wait 500ms between txs
        await new Promise(r => setTimeout(r, 500));
        break;
      } catch (err) {
        if (attempt < MAX_RETRIES) {
          console.log(`[TX ${batch + 1}/${numTx}] Retry ${attempt + 1}/${MAX_RETRIES} — ${err.message?.slice(0, 80)}`);
          await new Promise(r => setTimeout(r, 2000 * (attempt + 1)));
        } else {
          console.error(`[TX ${batch + 1}/${numTx}] FAILED — ${batchAccounts.length} accounts — ${err.message?.slice(0, 120)}`);
          failedCount += batchAccounts.length;
        }
      }
    }
  }

  console.log("\n═══════════════════════════════════════");
  console.log(`Transactions sent: ${txCount}/${numTx}`);
  console.log(`Accounts processed: ${txCount * TARGETS_PER_TX} / ${reclaimable.length}`);
  console.log(`Accounts failed: ${failedCount}`);
  console.log(`Excess reclaimed: ${(reclaimed / LAMPORTS_PER_SOL).toFixed(6)} SOL`);
  console.log(`Cost (fees): ${(txCount * 5000 / LAMPORTS_PER_SOL).toFixed(8)} SOL`);
  console.log("═══════════════════════════════════════");

  // Check final balance
  const finalBalance = await conn.getBalance(DESTINATION);
  console.log(`\nFinal wallet balance: ${(finalBalance / LAMPORTS_PER_SOL).toFixed(6)} SOL`);

  process.exit(0);
}

main().catch(e => { console.error(e); process.exit(1); });