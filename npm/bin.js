#!/usr/bin/env node
// The `open-agent-keys` command from npm: run the native binary that npm installed for this
// platform as an optional dependency. JavaScript, not TypeScript: Node does not strip types
// inside node_modules.
const { spawnSync } = require("node:child_process");

const pkg = `open-agent-keys-${process.platform}-${process.arch}`;
let binary;
try {
  binary = require.resolve(`${pkg}/open-agent-keys`);
} catch {
  console.error(`open-agent-keys: no binary for ${process.platform}-${process.arch} (${pkg} is not installed). ` +
    "Supported: macOS and Linux on arm64 and x64. If npm skipped optional dependencies, reinstall without --omit=optional.");
  process.exit(1);
}
const run = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (run.error) throw run.error;
if (run.signal) process.kill(process.pid, run.signal);
process.exit(run.status ?? 1);
