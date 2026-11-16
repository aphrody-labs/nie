#!/usr/bin/env bun
// Build nie-wasm (web-craft profile, wasm32-unknown-unknown) and generate
// wasm-bindgen --target web bindings into infra/cloudflare/edge/pkg for Wrangler & tests.

import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";

const edgeDir = dirname(
  new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1"),
);
const repoRoot = resolve(edgeDir, "../../..");
const pkgDir = resolve(edgeDir, "pkg");
const wasmOut = resolve(
  repoRoot,
  "target/wasm32-unknown-unknown/web-craft/nie_wasm.wasm",
);
const force = process.argv.includes("--force");

if (!force && existsSync(resolve(pkgDir, "nie_wasm_bg.wasm"))) {
  console.log(
    "[nie-edge] wasm bindings already present in pkg/ (pass --force to rebuild)",
  );
  process.exit(0);
}

const env = { ...process.env, RUSTC_WRAPPER: "" };

const cargo = Bun.spawnSync(
  [
    "cargo",
    "build",
    "--locked",
    "-p",
    "nie-wasm",
    "--target",
    "wasm32-unknown-unknown",
    "--profile",
    "web-craft",
  ],
  { cwd: repoRoot, env, stdout: "inherit", stderr: "inherit" },
);

if (cargo.exitCode !== 0) {
  process.exit(cargo.exitCode ?? 1);
}

const bindgen = Bun.spawnSync(
  ["wasm-bindgen", "--target", "web", "--out-dir", pkgDir, wasmOut],
  { cwd: repoRoot, env, stdout: "inherit", stderr: "inherit" },
);

process.exit(bindgen.exitCode ?? 0);
