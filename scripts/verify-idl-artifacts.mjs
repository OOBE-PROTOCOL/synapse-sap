#!/usr/bin/env node

import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const ROOT = process.cwd();
const PROGRAM_ID = "SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ";

const IDL_PATHS = [
  "target/idl/synapse_agent_sap.json",
  "idl/synapse_agent_sap.json",
  "synapse-sap-sdk/idl/synapse_agent_sap.json",
  "synapse-sap-sdk/src/idl/synapse_agent_sap.json",
  "synapse-sap-sdk/src/idl.json",
];

const CRITICAL_INSTRUCTIONS = {
  register_agent: {
    accounts: [
      "wallet",
      "agent",
      "agent_stats",
      "pricing_menu",
      "global_registry",
      "system_program",
    ],
    args: [
      "name",
      "description",
      "capabilities",
      "pricing",
      "protocols",
      "agent_id",
      "agent_uri",
      "x402_endpoint",
    ],
  },
  migrate_pricing_menu: {
    accounts: ["wallet", "agent", "pricing_menu", "system_program"],
    args: [],
  },
  init_stake: {
    accounts: ["wallet", "agent", "stake", "system_program"],
    args: ["initial_deposit"],
  },
  create_escrow_v2: {
    accounts: [
      "depositor",
      "agent",
      "agent_stake",
      "agent_stats",
      "pricing_menu",
      "escrow",
      "system_program",
    ],
    args: [
      "escrow_nonce",
      "price_per_call",
      "max_calls",
      "initial_deposit",
      "expires_at",
      "volume_curve",
      "token_mint",
      "token_decimals",
      "settlement_security",
      "dispute_window_slots",
      "co_signer",
      "arbiter",
    ],
  },
  settle_calls_v2: {
    accounts: ["wallet", "agent", "agent_stats", "escrow", "system_program"],
    args: ["escrow_nonce", "calls_to_settle", "service_hash"],
  },
  close_escrow_v2: {
    accounts: ["depositor", "escrow", "agent_stats"],
    args: [],
  },
  close_agent: {
    accounts: [
      "wallet",
      "agent",
      "agent_stats",
      "vault_check",
      "pricing_menu",
      "stake",
      "global_registry",
    ],
    args: [],
  },
};

const FORBIDDEN_IDL_TOKENS = [
  "settlement_receipt",
  "settlementReceipt",
  "receipt_merkle_root",
  "receiptMerkleRoot",
];

function fail(message) {
  console.error(`\n[verify-idl-artifacts] ${message}`);
  process.exitCode = 1;
}

function readJson(relativePath) {
  const absolute = path.join(ROOT, relativePath);
  if (!fs.existsSync(absolute)) {
    fail(`Missing IDL artifact: ${relativePath}. Run anchor build first.`);
    return null;
  }

  try {
    return JSON.parse(fs.readFileSync(absolute, "utf8"));
  } catch (error) {
    fail(`Invalid JSON in ${relativePath}: ${error.message}`);
    return null;
  }
}

function canonicalHash(value) {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex");
}

function listNames(items = []) {
  return items.map((item) => item.name);
}

function assertArrayEqual(label, actual, expected) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(
      `${label} mismatch.\n  expected: ${expected.join(
        ","
      )}\n  actual:   ${actual.join(",")}`
    );
  }
}

function assertInstruction(idl, name, spec) {
  const ix = idl.instructions?.find((candidate) => candidate.name === name);
  if (!ix) {
    fail(`Missing critical instruction: ${name}`);
    return;
  }

  assertArrayEqual(`${name}.accounts`, listNames(ix.accounts), spec.accounts);
  assertArrayEqual(`${name}.args`, listNames(ix.args), spec.args);
}

function assertNoForbiddenInstructionTokens(relativePath, idl) {
  for (const ix of idl.instructions ?? []) {
    const instructionSurface = JSON.stringify({
      name: ix.name,
      accounts: ix.accounts,
      args: ix.args,
    });
    for (const token of FORBIDDEN_IDL_TOKENS) {
      if (instructionSurface.includes(token)) {
        fail(
          `${relativePath} instruction ${ix.name} contains stale V2 token: ${token}`
        );
      }
    }
  }
}

const loaded = new Map();
for (const relativePath of IDL_PATHS) {
  const idl = readJson(relativePath);
  if (idl) loaded.set(relativePath, idl);
}

if (loaded.size !== IDL_PATHS.length) {
  process.exit(process.exitCode || 1);
}

const canonical = loaded.get(IDL_PATHS[0]);
const canonicalDigest = canonicalHash(canonical);

if (canonical.address !== PROGRAM_ID) {
  fail(`Canonical IDL address mismatch: ${canonical.address}`);
}

if (canonical.metadata?.version !== "1.0.0") {
  fail(
    `Canonical IDL metadata.version must be 1.0.0, got ${canonical.metadata?.version}`
  );
}

if (canonical.instructions?.length !== 79) {
  fail(
    `Canonical IDL must expose 79 instructions, got ${canonical.instructions?.length}`
  );
}

for (const [name, spec] of Object.entries(CRITICAL_INSTRUCTIONS)) {
  assertInstruction(canonical, name, spec);
}

for (const [relativePath, idl] of loaded.entries()) {
  if (idl.address !== PROGRAM_ID) {
    fail(`${relativePath} address mismatch: ${idl.address}`);
  }

  if (idl.metadata?.version !== canonical.metadata?.version) {
    fail(`${relativePath} metadata.version mismatch: ${idl.metadata?.version}`);
  }

  if (canonicalHash(idl) !== canonicalDigest) {
    fail(
      `${relativePath} is not byte-for-byte equivalent after JSON parse to ${IDL_PATHS[0]}`
    );
  }

  assertNoForbiddenInstructionTokens(relativePath, idl);
}

if (process.exitCode) {
  process.exit(process.exitCode);
}

console.log("[verify-idl-artifacts] OK");
console.log(`  version: ${canonical.metadata.version}`);
console.log(`  instructions: ${canonical.instructions.length}`);
console.log(`  sha256: ${canonicalDigest}`);
