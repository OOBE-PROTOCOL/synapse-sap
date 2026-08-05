#!/usr/bin/env node

import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import zlib from "node:zlib";
import anchor from "@coral-xyz/anchor";

const PROGRAM_ID = "SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ";
const EXPECTED_AUTHORITY = "GBLQznn1QMnx64zHXcDguP9yNW9ZfYCVdrY8eDovBvPk";
const PROGRAM_METADATA_PROGRAM_ID =
  "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S";
const BPF_UPGRADEABLE_LOADER_ID =
  "BPFLoaderUpgradeab1e11111111111111111111111";
const MAINNET_RPC = "https://api.mainnet-beta.solana.com";
const DEVNET_RPC = "https://api.devnet.solana.com";

const EXPECTED_IDL = {
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
  const opts = {
    network: "mainnet",
    rpc: null,
    expectedSdkVersion: "1.0.3",
  };

  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "--network") opts.network = args[++i];
    else if (arg === "--rpc") opts.rpc = args[++i];
    else if (arg === "--expected-sdk-version") opts.expectedSdkVersion = args[++i];
    else if (arg === "--help" || arg === "-h") {
      console.log(
        "Usage: node scripts/preflight-mainnet.mjs [--network mainnet|devnet] [--rpc URL] [--expected-sdk-version 1.0.3]"
      );
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  if (!["mainnet", "devnet"].includes(opts.network) && !opts.rpc) {
    throw new Error(`Unknown network '${opts.network}'. Pass --rpc for custom RPC.`);
  }

  return opts;
}

function ok(label, value = "") {
  console.log(`OK     ${label}${value ? `: ${value}` : ""}`);
}

function warn(label, value = "") {
  console.log(`WARN   ${label}${value ? `: ${value}` : ""}`);
}

function fail(failures, label, value = "") {
  failures.push(`${label}${value ? `: ${value}` : ""}`);
  console.log(`FAIL   ${label}${value ? `: ${value}` : ""}`);
}

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(process.cwd(), relativePath), "utf8"));
}

function sha256File(relativePath) {
  const bytes = fs.readFileSync(path.join(process.cwd(), relativePath));
  return createHash("sha256").update(bytes).digest("hex");
}

function fixedSeed(seed) {
  const out = Buffer.alloc(16);
  Buffer.from(seed, "utf8").copy(out);
  return out;
}

function deriveCanonicalMetadataPda() {
  return anchor.web3.PublicKey.findProgramAddressSync(
    [new anchor.web3.PublicKey(PROGRAM_ID).toBuffer(), fixedSeed("idl")],
    new anchor.web3.PublicKey(PROGRAM_METADATA_PROGRAM_ID)
  )[0];
}

function instructionNames(items = []) {
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
    createEscrowV2Accounts: instructionNames(createEscrow?.accounts),
  };
}

function decodeProgramDataAddress(programAccount) {
  const tag = programAccount.data.readUInt32LE(0);
  if (tag !== 2) {
    throw new Error(`program account is not UpgradeableLoader Program state: ${tag}`);
  }
  return new anchor.web3.PublicKey(programAccount.data.subarray(4, 36));
}

function decodeProgramData(programDataAccount) {
  const tag = programDataAccount.data.readUInt32LE(0);
  if (tag !== 3) {
    throw new Error(`programdata account is not UpgradeableLoader ProgramData state: ${tag}`);
  }

  const slot = programDataAccount.data.readBigUInt64LE(4).toString();
  const authorityOption = programDataAccount.data[12];
  const authority =
    authorityOption === 1
      ? new anchor.web3.PublicKey(programDataAccount.data.subarray(13, 45)).toBase58()
      : null;

  return { slot, authority };
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
  offset += 5;

  let payload = data.subarray(offset, offset + dataLength);
  if (compression === 1) payload = zlib.gunzipSync(payload);
  else if (compression === 2) payload = zlib.inflateSync(payload);
  else if (compression !== 0) throw new Error(`unsupported compression: ${compression}`);

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
    },
    idl: JSON.parse(payload.toString("utf8")),
  };
}

function compareSummary(summary) {
  return Object.entries(EXPECTED_IDL)
    .filter(([key, expected]) => JSON.stringify(summary[key]) !== JSON.stringify(expected))
    .map(([key, expected]) => ({
      key,
      expected,
      actual: summary[key],
    }));
}

async function main() {
  const opts = parseArgs();
  const rpc = opts.rpc ?? (opts.network === "devnet" ? DEVNET_RPC : MAINNET_RPC);
  const failures = [];

  console.log("[sap-mainnet-preflight]");
  console.log(`network: ${opts.network}`);
  console.log(`rpc: ${rpc}`);
  console.log(`program: ${PROGRAM_ID}`);

  const rootPackage = readJson("package.json");
  const sdkPackage = readJson("synapse-sap-sdk/package.json");
  const localIdl = readJson("target/idl/synapse_agent_sap.json");
  const localSummary = summarizeIdl(localIdl);

  if (sdkPackage.version === opts.expectedSdkVersion) {
    ok("SDK package version", sdkPackage.version);
  } else {
    fail(
      failures,
      "SDK package version",
      `expected ${opts.expectedSdkVersion}, got ${sdkPackage.version}`
    );
  }

  if (rootPackage.dependencies?.["@coral-xyz/anchor"]) {
    ok("root Anchor client dependency", rootPackage.dependencies["@coral-xyz/anchor"]);
  } else {
    fail(failures, "root Anchor client dependency missing");
  }

  const localMismatches = compareSummary(localSummary);
  if (localMismatches.length === 0) {
    ok("local target IDL", `${localSummary.version}, ${localSummary.instructionCount} instructions`);
  } else {
    fail(failures, "local target IDL mismatch", JSON.stringify(localMismatches));
  }

  const programBinary = "target/deploy/synapse_agent_sap.so";
  if (fs.existsSync(path.join(process.cwd(), programBinary))) {
    ok("program binary sha256", sha256File(programBinary));
  } else {
    fail(failures, "program binary missing", programBinary);
  }

  const connection = new anchor.web3.Connection(rpc, "confirmed");
  const programKey = new anchor.web3.PublicKey(PROGRAM_ID);
  const programAccount = await connection.getAccountInfo(programKey, "confirmed");
  if (!programAccount) {
    fail(failures, "program account missing");
  } else {
    ok("program account owner", programAccount.owner.toBase58());
    if (programAccount.owner.toBase58() !== BPF_UPGRADEABLE_LOADER_ID) {
      fail(failures, "program owner", `expected ${BPF_UPGRADEABLE_LOADER_ID}`);
    }

    const programDataKey = decodeProgramDataAddress(programAccount);
    ok("programdata account", programDataKey.toBase58());

    const programDataAccount = await connection.getAccountInfo(programDataKey, "confirmed");
    if (!programDataAccount) {
      fail(failures, "programdata account missing");
    } else {
      const programData = decodeProgramData(programDataAccount);
      ok("programdata slot", programData.slot);
      ok(
        "programdata lamports",
        `${programDataAccount.lamports} (${programDataAccount.lamports / anchor.web3.LAMPORTS_PER_SOL} SOL rent reserve; do not withdraw while program is live)`
      );

      if (programData.authority === EXPECTED_AUTHORITY) {
        ok("upgrade authority", programData.authority);
      } else {
        fail(
          failures,
          "upgrade authority",
          `expected ${EXPECTED_AUTHORITY}, got ${programData.authority ?? "none"}`
        );
      }
    }
  }

  const metadataPda = deriveCanonicalMetadataPda();
  const metadataAccount = await connection.getAccountInfo(metadataPda, "confirmed");
  if (!metadataAccount) {
    fail(failures, "Program Metadata IDL missing", metadataPda.toBase58());
  } else if (metadataAccount.owner.toBase58() !== PROGRAM_METADATA_PROGRAM_ID) {
    fail(
      failures,
      "Program Metadata owner mismatch",
      `${metadataAccount.owner.toBase58()} at ${metadataPda.toBase58()}`
    );
  } else {
    try {
      const metadata = decodeProgramMetadataAccount(metadataAccount);
      const metadataSummary = summarizeIdl(metadata.idl);
      const metadataMismatches = compareSummary(metadataSummary);
      ok("Program Metadata PDA", metadataPda.toBase58());
      ok("Program Metadata header", JSON.stringify(metadata.header));

      if (metadataMismatches.length === 0) {
        ok(
          "Program Metadata IDL",
          `${metadataSummary.version}, ${metadataSummary.instructionCount} instructions`
        );
      } else {
        fail(
          failures,
          "Program Metadata IDL mismatch",
          JSON.stringify(metadataMismatches)
        );
      }
    } catch (error) {
      fail(failures, "Program Metadata decode failed", error.message);
    }
  }

  if (opts.network !== "mainnet") {
    warn("network", "this preflight is mainnet-oriented; use devnet only as a dry-run");
  }

  if (failures.length) {
    console.log("\nNO-GO");
    for (const item of failures) console.log(`- ${item}`);
    process.exit(1);
  }

  console.log("\nGO");
}

main().catch((error) => {
  console.error(`[sap-mainnet-preflight] ${error.stack ?? error.message}`);
  process.exit(1);
});
