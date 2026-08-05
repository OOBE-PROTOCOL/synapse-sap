#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import zlib from "node:zlib";
import anchor from "@coral-xyz/anchor";

const PROGRAM_ID = "SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ";
const PROGRAM_METADATA_PROGRAM_ID =
  "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S";
const IDL_METADATA_SEED = "idl";

const NETWORKS = {
  devnet: "https://api.devnet.solana.com",
  mainnet: "https://api.mainnet-beta.solana.com",
};

const EXPECTED = {
  version: "1.0.0",
  instructionCount: 79,
  hasMigratePricingMenu: true,
  createEscrowV2Accounts: [
    "depositor",
    "agent",
    "agent_stake",
    "agent_stats",
    "pricing_menu",
    "escrow",
    "system_program",
  ],
};

function parseArgs() {
  const args = process.argv.slice(2);
  const opts = { network: "devnet", rpc: null, strict: false };

  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "--network") opts.network = args[++i];
    else if (arg === "--rpc") opts.rpc = args[++i];
    else if (arg === "--strict") opts.strict = true;
    else if (arg === "--help" || arg === "-h") {
      console.log(
        "Usage: node scripts/audit-onchain-idl.mjs [--network devnet|mainnet] [--rpc URL] [--strict]"
      );
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  if (!NETWORKS[opts.network] && !opts.rpc) {
    throw new Error(
      `Unknown network '${opts.network}'. Pass --rpc for custom endpoints.`
    );
  }

  return opts;
}

function names(items = []) {
  return items.map((item) => item.name);
}

function summarizeIdl(idl) {
  const createEscrow = idl?.instructions?.find(
    (ix) => ix.name === "create_escrow_v2"
  );
  return {
    version: idl?.metadata?.version ?? idl?.version ?? null,
    instructionCount: idl?.instructions?.length ?? 0,
    hasMigratePricingMenu: Boolean(
      idl?.instructions?.find((ix) => ix.name === "migrate_pricing_menu")
    ),
    createEscrowV2Accounts: names(createEscrow?.accounts),
  };
}

function compare(label, summary, strict) {
  const mismatches = [];
  for (const [key, expected] of Object.entries(EXPECTED)) {
    const actual = summary[key];
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      mismatches.push({ key, expected, actual });
    }
  }

  if (!mismatches.length) {
    console.log(`${label}: OK`);
    return true;
  }

  console.log(`${label}: MISMATCH`);
  for (const mismatch of mismatches) {
    console.log(`  ${mismatch.key}`);
    console.log(`    expected: ${JSON.stringify(mismatch.expected)}`);
    console.log(`    actual:   ${JSON.stringify(mismatch.actual)}`);
  }

  if (strict) process.exitCode = 1;
  return false;
}

function fixedSeed(seed) {
  const out = Buffer.alloc(16);
  Buffer.from(seed, "utf8").copy(out);
  return out;
}

function deriveCanonicalMetadataPda() {
  return anchor.web3.PublicKey.findProgramAddressSync(
    [new anchor.web3.PublicKey(PROGRAM_ID).toBuffer(), fixedSeed(IDL_METADATA_SEED)],
    new anchor.web3.PublicKey(PROGRAM_METADATA_PROGRAM_ID)
  )[0];
}

function decodeProgramMetadataAccount(accountInfo) {
  const data = accountInfo.data;
  let offset = 0;

  const discriminator = data[offset];
  offset += 1;

  const program = new anchor.web3.PublicKey(
    data.subarray(offset, offset + 32)
  ).toBase58();
  offset += 32;

  const authorityBytes = data.subarray(offset, offset + 32);
  const authority = authorityBytes.every((byte) => byte === 0)
    ? null
    : new anchor.web3.PublicKey(authorityBytes).toBase58();
  offset += 32;

  const mutable = Boolean(data[offset]);
  offset += 1;

  const canonical = Boolean(data[offset]);
  offset += 1;

  const seed = data
    .subarray(offset, offset + 16)
    .toString("utf8")
    .replace(/\0+$/, "");
  offset += 16;

  const encoding = data[offset];
  offset += 1;

  const compression = data[offset];
  offset += 1;

  const format = data[offset];
  offset += 1;

  const dataSource = data[offset];
  offset += 1;

  const dataLength = data.readUInt32LE(offset);
  offset += 4;

  const padding = data.subarray(offset, offset + 5);
  offset += 5;

  const payload = data.subarray(offset, offset + dataLength);

  return {
    header: {
      discriminator,
      program,
      authority,
      mutable,
      canonical,
      seed,
      encoding,
      compression,
      format,
      dataSource,
      dataLength,
      padding: padding.toString("hex"),
      payloadBytes: payload.length,
      payloadPrefix: payload.subarray(0, 8).toString("hex"),
      payloadSuffix: payload.subarray(-8).toString("hex"),
    },
    payload,
  };
}

async function fetchRawProgramMetadataIdl(rpc) {
  const connection = new anchor.web3.Connection(rpc, "confirmed");
  const pda = deriveCanonicalMetadataPda();
  const accountInfo = await connection.getAccountInfo(pda, "confirmed");

  if (!accountInfo) {
    return {
      pda: pda.toBase58(),
      error: "canonical Program Metadata account not found",
    };
  }

  if (!accountInfo.owner.equals(new anchor.web3.PublicKey(PROGRAM_METADATA_PROGRAM_ID))) {
    return {
      pda: pda.toBase58(),
      error: `Program Metadata account owner mismatch: ${accountInfo.owner.toBase58()}`,
    };
  }

  const decoded = decodeProgramMetadataAccount(accountInfo);
  try {
    let bytes = decoded.payload;
    if (decoded.header.compression === 1) bytes = zlib.gunzipSync(bytes);
    else if (decoded.header.compression === 2) bytes = zlib.inflateSync(bytes);
    else if (decoded.header.compression !== 0) {
      throw new Error(`unsupported compression: ${decoded.header.compression}`);
    }

    const text = bytes.toString("utf8");
    return {
      pda: pda.toBase58(),
      account: {
        lamports: accountInfo.lamports,
        space: accountInfo.data.length,
        owner: accountInfo.owner.toBase58(),
      },
      header: decoded.header,
      idl: JSON.parse(text),
      jsonBytes: bytes.length,
    };
  } catch (error) {
    return {
      pda: pda.toBase58(),
      account: {
        lamports: accountInfo.lamports,
        space: accountInfo.data.length,
        owner: accountInfo.owner.toBase58(),
      },
      header: decoded.header,
      error: `failed to decode Program Metadata payload: ${error.message}`,
    };
  }
}

async function fetchLegacyIdl(rpc) {
  const connection = new anchor.web3.Connection(rpc, "confirmed");
  return anchor.Program.fetchIdl(PROGRAM_ID, { connection });
}

function fetchProgramMetadataIdl(rpc) {
  const out = path.join(
    os.tmpdir(),
    `sap-program-metadata-${Date.now()}-${Math.random()
      .toString(16)
      .slice(2)}.json`
  );

  const result = spawnSync(
    "anchor",
    ["idl", "fetch", PROGRAM_ID, "--provider.cluster", rpc, "--out", out],
    { encoding: "utf8" }
  );

  if (result.status !== 0) {
    return {
      error:
        [result.stderr, result.stdout].filter(Boolean).join("\n").trim() ||
        "anchor idl fetch failed",
    };
  }

  try {
    return { idl: JSON.parse(fs.readFileSync(out, "utf8")) };
  } catch (error) {
    return {
      error: `anchor idl fetch returned non-JSON output: ${error.message}`,
    };
  } finally {
    fs.rmSync(out, { force: true });
  }
}

async function main() {
  const opts = parseArgs();
  const rpc = opts.rpc ?? NETWORKS[opts.network];

  console.log("[audit-onchain-idl]");
  console.log(`  network: ${opts.network}`);
  console.log(`  rpc: ${rpc}`);
  console.log(`  strict: ${opts.strict}`);

  let legacySummary = null;
  try {
    const legacy = await fetchLegacyIdl(rpc);
    legacySummary = summarizeIdl(legacy);
    console.log("\nLegacy Anchor JS IDL:");
    console.log(JSON.stringify(legacySummary, null, 2));
    compare("legacy", legacySummary, false);
  } catch (error) {
    console.log("\nLegacy Anchor JS IDL: ERROR");
    console.log(`  ${error.message}`);
  }

  const metadata = fetchProgramMetadataIdl(rpc);
  console.log("\nAnchor 1.x Program Metadata IDL:");
  if (metadata.error) {
    console.log("  ERROR");
    console.log(`  ${metadata.error}`);
    if (opts.strict) process.exitCode = 1;
  } else {
    const metadataSummary = summarizeIdl(metadata.idl);
    console.log(JSON.stringify(metadataSummary, null, 2));
    compare("program-metadata", metadataSummary, opts.strict);
  }

  const rawMetadata = await fetchRawProgramMetadataIdl(rpc);
  console.log("\nRaw Program Metadata account:");
  console.log(`  pda: ${rawMetadata.pda}`);
  if (rawMetadata.account) {
    console.log(`  owner: ${rawMetadata.account.owner}`);
    console.log(`  space: ${rawMetadata.account.space}`);
    console.log(`  lamports: ${rawMetadata.account.lamports}`);
  }
  if (rawMetadata.header) {
    console.log("  header:");
    console.log(JSON.stringify(rawMetadata.header, null, 2));
  }
  if (rawMetadata.error) {
    console.log("  ERROR");
    console.log(`  ${rawMetadata.error}`);
    if (opts.strict) process.exitCode = 1;
  } else {
    const rawSummary = summarizeIdl(rawMetadata.idl);
    console.log("  decoded IDL:");
    console.log(JSON.stringify(rawSummary, null, 2));
    compare("raw-program-metadata", rawSummary, opts.strict);
  }

  if (legacySummary && legacySummary.version !== EXPECTED.version) {
    console.log(
      "\nNote: Anchor 1.x removed legacy IDL instructions. A stale legacy IDL account can remain readable by older clients but is not a production source of truth."
    );
  }

  process.exit(process.exitCode ?? 0);
}

main().catch((error) => {
  console.error(`[audit-onchain-idl] ${error.message}`);
  process.exit(1);
});
