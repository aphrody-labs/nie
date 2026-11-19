// Builds the browser build of the GPUI apps into a content-hashed bundle that `nie serve` embeds.
//
//   bun gui/tools/pack-web.ts --out <dist dir> [--target-dir <dir>] [--no-opt]
//
// 1. cargo build --release --target wasm32-unknown-unknown -p nie-apps --lib (gpui_web, WebGPU with
//    the WebGL fallback chosen at runtime by gpui_web);
// 2. wasm-bindgen --target web at the version gui/Cargo.lock pins;
// 3. wasm-opt -Oz unless --no-opt;
// 4. every file but the entry page is renamed with the first 8 hex digits of its SHA-256;
// 5. a brotli copy of each file it shrinks, and manifest.json in the format crates/nie-app reads.
import { spawnSync } from "node:child_process";
import { brotliCompressSync, constants } from "node:zlib";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

const argv = process.argv.slice(2);
const opt = (name: string) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined);
const out = resolve(opt("--out") ?? "gui/dist");
const guiDir = resolve(dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..");
const targetDir = resolve(opt("--target-dir") ?? join(guiDir, "target"));
const optimise = !argv.includes("--no-opt");

function run(cmd: string, args: string[], env: Record<string, string> = {}) {
  const r = spawnSync(cmd, args, { cwd: guiDir, env: { ...process.env, ...env }, encoding: "utf8" });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(" ")} failed\n${r.stderr}`);
  return r.stdout;
}

console.log("build nie-apps for wasm32");
run("cargo", ["build", "--locked", "--release", "-p", "nie-apps", "--lib", "--target", "wasm32-unknown-unknown"], {
  CARGO_TARGET_DIR: targetDir,
  CARGO_PROFILE_RELEASE_OPT_LEVEL: "s",
  CARGO_PROFILE_RELEASE_LTO: "fat",
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS: "1",
  CARGO_PROFILE_RELEASE_PANIC: "abort",
  CARGO_PROFILE_RELEASE_DEBUG: "0",
});
const raw = join(targetDir, "wasm32-unknown-unknown/release/nie_apps.wasm");

const lock = readFileSync(join(guiDir, "Cargo.lock"), "utf8").replaceAll("\r\n", "\n");
const pinned = /name = "wasm-bindgen"\nversion = "([^"]+)"/.exec(lock)?.[1];
const installed = run("wasm-bindgen", ["--version"]).trim().split(/\s+/)[1];
if (pinned !== installed) throw new Error(`wasm-bindgen CLI ${installed} does not match ${pinned} in gui/Cargo.lock`);

const work = join(out, ".work");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });
run("wasm-bindgen", ["--target", "web", "--no-typescript", "--out-name", "nie_gui", "--out-dir", work, raw]);
if (optimise) {
  console.log("wasm-opt -Oz");
  run("wasm-opt", ["-Oz", "-all", "--strip-debug", "--strip-producers", "-o", join(work, "opt.wasm"), join(work, "nie_gui_bg.wasm")]);
  run("mv", [join(work, "opt.wasm"), join(work, "nie_gui_bg.wasm")]);
}

const hash = (b: Uint8Array) => createHash("sha256").update(b).digest("hex");
const short = (b: Uint8Array) => hash(b).slice(0, 8);
const hashed = (name: string, b: Uint8Array) => name.replace(/\.([^.]+)$/, `.${short(b)}.$1`);
const types: Record<string, string> = {
  html: "text/html; charset=utf-8", js: "text/javascript; charset=utf-8", css: "text/css; charset=utf-8",
  wasm: "application/wasm", json: "application/json",
};
const files: { name: string; bytes: Uint8Array; immutable: boolean }[] = [];

const wasm = readFileSync(join(work, "nie_gui_bg.wasm"));
const wasmName = hashed("nie_gui_bg.wasm", wasm);
files.push({ name: wasmName, bytes: wasm, immutable: true });
const glue = readFileSync(join(work, "nie_gui.js"), "utf8");
if (glue.split("nie_gui_bg.wasm").length !== 2) throw new Error("expected exactly one reference to nie_gui_bg.wasm in the glue");
const glueText = glue.replace("nie_gui_bg.wasm", wasmName);
const glueName = hashed("nie_gui.js", Buffer.from(glueText));
files.push({ name: glueName, bytes: Buffer.from(glueText), immutable: true });
const page = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, height=device-height, initial-scale=1">
<title>nie explorer</title>
<style>html,body{margin:0;height:100%;background:#000}</style>
</head>
<body>
<script type="module">import init from "./${glueName}"; await init();</script>
</body>
</html>
`;
files.push({ name: "index.html", bytes: Buffer.from(page), immutable: false });

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
let manifest = '{"files":[\n';
files.forEach((f, i) => {
  writeFileSync(join(out, f.name), f.bytes);
  const packed = brotliCompressSync(f.bytes, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } });
  if (packed.length * 10 < f.bytes.length * 9) writeFileSync(join(out, `${f.name}.br`), packed);
  const type = types[f.name.split(".").pop() ?? ""] ?? "application/octet-stream";
  manifest += `{"name":"${f.name}","type":"${type}","immutable":${f.immutable},"entry":${f.name === "index.html"},"bytes":${f.bytes.length},"brotli":${packed.length},"sha256":"${hash(f.bytes)}"}${i + 1 < files.length ? "," : ""}\n`;
  console.log(`${f.name}  ${f.bytes.length} bytes  br ${packed.length}`);
});
manifest += "]}\n";
writeFileSync(join(out, "manifest.json"), manifest);
console.log(`bundle in ${out}`);
