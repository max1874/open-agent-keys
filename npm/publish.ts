// Publish a release to npm the way esbuild and Biome do: one small package per platform holding
// its binary, and an `open-agent-keys` package that depends on all of them optionally, so npm
// downloads only the one that matches the machine. No install script runs.
//
//   node npm/publish.ts <version> <dir with the release's .tar.gz assets>
//
// Run by .github/workflows/release.yml with NODE_AUTH_TOKEN set. A version already on npm is skipped,
// so a rerun after a partial failure finishes the job.
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [version, assets] = process.argv.slice(2);
if (!version || !assets) throw new Error("usage: publish.ts <version> <assets dir>");

// Release asset suffix (uname -s, uname -m) → Node's process.platform and process.arch.
const platforms = { "Darwin-arm64": ["darwin", "arm64"], "Darwin-x86_64": ["darwin", "x64"],
  "Linux-aarch64": ["linux", "arm64"], "Linux-x86_64": ["linux", "x64"] } as const;
const common = { version, license: "MIT", repository: { type: "git", url: "git+https://github.com/max1874/open-agent-keys.git" } };
const out = join(import.meta.dirname, "..", "target", "npm");

function publish(dir: string, name: string) {
  try {
    execFileSync("npm", ["view", `${name}@${version}`, "version"], { stdio: "ignore" });
    return console.log(`${name}@${version} is already on npm`);
  } catch {}
  execFileSync("npm", ["publish", "--access", "public", "--provenance"], { cwd: dir, stdio: "inherit" });
}

const names: string[] = [];
for (const [asset, [os, cpu]] of Object.entries(platforms)) {
  const name = `open-agent-keys-${os}-${cpu}`, dir = join(out, name);
  mkdirSync(dir, { recursive: true });
  execFileSync("tar", ["-xzf", join(assets, `open-agent-keys-${asset}.tar.gz`), "-C", dir]);
  chmodSync(join(dir, "open-agent-keys"), 0o755);
  writeFileSync(join(dir, "package.json"), JSON.stringify({
    name, ...common, description: `The open-agent-keys binary for ${os} ${cpu}; install open-agent-keys instead.`,
    os: [os], cpu: [cpu], files: ["open-agent-keys"],
  }, null, 2));
  publish(dir, name);
  names.push(name);
}

const dir = join(out, "open-agent-keys");
mkdirSync(dir, { recursive: true });
copyFileSync(join(import.meta.dirname, "bin.js"), join(dir, "bin.js"));
copyFileSync(join(import.meta.dirname, "..", "README.md"), join(dir, "README.md"));
writeFileSync(join(dir, "package.json"), JSON.stringify({
  name: "open-agent-keys", ...common,
  description: "Give AI agents API keys from 1Password or Bitwarden without putting values in the transcript",
  keywords: ["ai-agents", "secrets", "1password", "bitwarden", "cli"],
  bin: { "open-agent-keys": "bin.js" }, files: ["bin.js"],
  optionalDependencies: Object.fromEntries(names.map((n) => [n, version])),
}, null, 2));
publish(dir, "open-agent-keys");
