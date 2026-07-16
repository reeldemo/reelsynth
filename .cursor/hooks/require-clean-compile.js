#!/usr/bin/env node
/**
 * beforeShellExecution hook — block `git push` unless cargo check is clean
 * with warnings denied (RUSTFLAGS=-D warnings).
 *
 * Input: JSON on stdin (Cursor hook payload with `.command`)
 * Output: JSON on stdout `{ permission, user_message?, agent_message? }`
 */
const { spawnSync } = require("child_process");

function readStdin() {
  try {
    return require("fs").readFileSync(0, "utf8");
  } catch {
    return "";
  }
}

function allow(extra = {}) {
  process.stdout.write(JSON.stringify({ permission: "allow", ...extra }));
  process.exit(0);
}

function deny(userMessage, agentMessage) {
  process.stdout.write(
    JSON.stringify({
      permission: "deny",
      user_message: userMessage,
      agent_message: agentMessage,
    })
  );
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
  allow();
}

const crates = ["reelsynth", "reelsynth-ui", "reelsynth-app"];
const args = ["check", ...crates.flatMap((c) => ["-p", c])];
const env = { ...process.env, RUSTFLAGS: "-D warnings" };

const result = spawnSync("cargo", args, {
  env,
  encoding: "utf8",
  shell: process.platform === "win32",
  maxBuffer: 8 * 1024 * 1024,
});

if (result.status === 0) {
  allow({
    agent_message:
      "Compile clean (RUSTFLAGS=-D warnings) — git push allowed.",
  });
}

const tail = [result.stderr, result.stdout]
  .filter(Boolean)
  .join("\n")
  .trim()
  .split(/\r?\n/)
  .slice(-40)
  .join("\n");

deny(
  "Push blocked: cargo check failed with warnings treated as errors. Fix compile errors/warnings, then push again.",
  `Push blocked — clean compile required (cargo check -p reelsynth -p reelsynth-ui -p reelsynth-app with RUSTFLAGS=-D warnings).\n\n${tail}`
);
