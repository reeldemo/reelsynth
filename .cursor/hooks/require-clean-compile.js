#!/usr/bin/env node
/**
 * Mark (or verify) a clean compile stamp for the push gate.
 *
 *   node .cursor/hooks/require-clean-compile.js          # cargo check + write stamp
 *   node .cursor/hooks/require-clean-compile.js --mark-only  # write stamp only (after you already checked)
 *
 * Used by agents before `git push`. The beforeShellExecution hook only *reads*
 * the stamp (Cursor's hook host cannot spawn cargo/shell).
 */
const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");
const STAMP = path.join(ROOT, ".cursor", "compile-clean.stamp");
const CRATES = ["reelsynth", "reelsynth-ui", "reelsynth-app"];
const markOnly = process.argv.includes("--mark-only");

function runCargoCheck() {
  const env = { ...process.env, RUSTFLAGS: "-D warnings" };
  const args = ["check", ...CRATES.flatMap((c) => ["-p", c])];
  const result = spawnSync("cargo", args, {
    cwd: ROOT,
    env,
    encoding: "utf8",
    shell: process.platform === "win32",
    maxBuffer: 8 * 1024 * 1024,
  });
  if (result.status !== 0) {
    const tail = [result.stderr, result.stdout]
      .filter(Boolean)
      .join("\n")
      .trim()
      .split(/\r?\n/)
      .slice(-40)
      .join("\n");
    console.error("cargo check failed (RUSTFLAGS=-D warnings):\n" + tail);
    process.exit(1);
  }
}

function writeStamp() {
  const payload = {
    ok: true,
    at: new Date().toISOString(),
    epochMs: Date.now(),
    crates: CRATES,
    rustflags: "-D warnings",
  };
  fs.mkdirSync(path.dirname(STAMP), { recursive: true });
  fs.writeFileSync(STAMP, JSON.stringify(payload, null, 2) + "\n", "utf8");
  console.log("Wrote " + path.relative(ROOT, STAMP));
}

if (!markOnly) {
  runCargoCheck();
}
writeStamp();
process.exit(0);
