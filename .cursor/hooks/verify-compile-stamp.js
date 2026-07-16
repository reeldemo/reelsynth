#!/usr/bin/env node
/**
 * beforeShellExecution — gate `git push` on a fresh compile-clean stamp.
 *
 * Cursor's hook host cannot spawn cargo/shell, so this script only reads
 * `.cursor/compile-clean.stamp` (written by require-clean-compile.js via a
 * normal Shell tool / terminal).
 *
 * Refresh the stamp before pushing:
 *   node .cursor/hooks/require-clean-compile.js
 */
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");
const STAMP = path.join(ROOT, ".cursor", "compile-clean.stamp");
/** Stamp older than this is rejected (ms). */
const MAX_AGE_MS = 30 * 60 * 1000;

function readStdin() {
  try {
    return fs.readFileSync(0, "utf8");
  } catch {
    return "";
  }
}

function reply(obj) {
  process.stdout.write(JSON.stringify(obj));
  process.exit(0);
}

const raw = readStdin();
let payload = {};
try {
  payload = JSON.parse(raw || "{}");
} catch {
  payload = {};
}

const command = String(payload.command || "");
if (!/\bgit(\.exe)?(\s+-C\s+\S+)?\s+push\b/i.test(command)) {
  reply({ permission: "allow" });
}

if (!fs.existsSync(STAMP)) {
  reply({
    permission: "deny",
    user_message:
      "Push blocked: missing compile-clean stamp. Run `node .cursor/hooks/require-clean-compile.js` first.",
    agent_message:
      "Push blocked — no .cursor/compile-clean.stamp. From the repo root run: node .cursor/hooks/require-clean-compile.js (cargo check with -D warnings), then push again.",
  });
}

let stamp;
try {
  stamp = JSON.parse(fs.readFileSync(STAMP, "utf8"));
} catch (e) {
  reply({
    permission: "deny",
    user_message: "Push blocked: compile-clean stamp is unreadable.",
    agent_message: "Push blocked — could not parse .cursor/compile-clean.stamp: " + e,
  });
}

if (!stamp || stamp.ok !== true || typeof stamp.epochMs !== "number") {
  reply({
    permission: "deny",
    user_message: "Push blocked: compile-clean stamp is invalid.",
    agent_message:
      "Push blocked — .cursor/compile-clean.stamp missing ok/epochMs. Re-run: node .cursor/hooks/require-clean-compile.js",
  });
}

const age = Date.now() - stamp.epochMs;
if (age < 0 || age > MAX_AGE_MS) {
  reply({
    permission: "deny",
    user_message:
      "Push blocked: compile-clean stamp expired. Re-run `node .cursor/hooks/require-clean-compile.js`.",
    agent_message:
      "Push blocked — stamp age " +
      Math.round(age / 1000) +
      "s exceeds 30m. Refresh with: node .cursor/hooks/require-clean-compile.js",
  });
}

reply({
  permission: "allow",
  agent_message:
    "Compile-clean stamp ok (age " +
    Math.round(age / 1000) +
    "s) — git push allowed.",
});
