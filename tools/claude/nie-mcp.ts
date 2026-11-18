// Stdio launcher for the native nie MCP server (`crates/nie-cli/src/bin/nie-mcp.rs`).
//
// Resolution order, first hit wins:
//   1. NIE_MCP_BIN                      explicit binary
//   2. <cargo target dir>/{release,debug}/nie-mcp[.exe], newest first
//   3. the build host over SSH          NIE_BUILD_HOST (default fr1), NIE_BUILD_DIR (default /srv/xbuild/nie)
// Heavy builds stay off the PC: build with
//   ssh fr1 'cd /srv/xbuild/nie && cargo build -p nie-cli --bin nie-mcp --features host'
// Paths passed to tools resolve on the machine that runs the binary.

import { existsSync, statSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "..", "..");
const exe = process.platform === "win32" ? "nie-mcp.exe" : "nie-mcp";

function targetDir(): string | undefined {
  if (process.env.CARGO_TARGET_DIR) return process.env.CARGO_TARGET_DIR;
  const meta = Bun.spawnSync(["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"], {
    cwd: root,
    stdout: "pipe",
    stderr: "ignore",
  });
  if (!meta.success) return undefined;
  return (JSON.parse(meta.stdout.toString()) as { target_directory: string }).target_directory;
}

function localBinary(): string | undefined {
  if (process.env.NIE_MCP_BIN) return process.env.NIE_MCP_BIN;
  const dir = targetDir();
  if (!dir) return undefined;
  return ["release", "debug"]
    .map((profile) => join(dir, profile, exe))
    .filter((path) => existsSync(path))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
}

function command(): string[] {
  const local = localBinary();
  if (local) return [local];
  const host = process.env.NIE_BUILD_HOST ?? "fr1";
  const dir = process.env.NIE_BUILD_DIR ?? "/srv/xbuild/nie";
  const remote = `cd ${dir} && bin=$(ls -t target/release/nie-mcp target/debug/nie-mcp 2>/dev/null | head -n1) && [ -n "$bin" ] && exec "$bin" || { echo "nie-mcp not built in ${host}:${dir}" >&2; exit 127; }`;
  return ["ssh", "-T", "-o", "BatchMode=yes", host, remote];
}

const child = Bun.spawn(command(), { cwd: root, stdin: "inherit", stdout: "inherit", stderr: "inherit" });
for (const signal of ["SIGINT", "SIGTERM"] as const) process.on(signal, () => child.kill(signal));
process.exit(await child.exited);
