#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const ROOT = process.cwd();
const SOURCE = "target/idl/synapse_agent_sap.json";
const TARGETS = [
  "idl/synapse_agent_sap.json",
  "synapse-sap-sdk/idl/synapse_agent_sap.json",
  "synapse-sap-sdk/src/idl/synapse_agent_sap.json",
  "synapse-sap-sdk/src/idl.json",
];

const sourcePath = path.join(ROOT, SOURCE);
if (!fs.existsSync(sourcePath)) {
  console.error(
    `[sync-idl-artifacts] Missing ${SOURCE}. Run anchor build first.`
  );
  process.exit(1);
}

const source = fs.readFileSync(sourcePath);
for (const target of TARGETS) {
  const targetPath = path.join(ROOT, target);
  fs.mkdirSync(path.dirname(targetPath), { recursive: true });
  fs.writeFileSync(targetPath, source);
  console.log(`[sync-idl-artifacts] wrote ${target}`);
}
