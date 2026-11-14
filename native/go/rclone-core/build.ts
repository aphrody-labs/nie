#!/usr/bin/env bun
/**
 * Build script for the Rose Griffon Go rclone core (`rg_rclone_go.dll` / `.so` + `nie-rclone` CLI).
 */

import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

const GO_DIR = import.meta.dir;
const BUILD_DIR = join(GO_DIR, "build");
const isWin = process.platform === "win32";
const dllName = isWin
  ? "rg_rclone_go.dll"
  : process.platform === "darwin"
    ? "librg_rclone_go.dylib"
    : "librg_rclone_go.so";
const cliName = isWin ? "nie-rclone.exe" : "nie-rclone";
const runTest = process.argv.includes("--test");

mkdirSync(BUILD_DIR, { recursive: true });

function run(cmd: string[]) {
  const proc = Bun.spawnSync(cmd, {
    cwd: GO_DIR,
    stdout: "inherit",
    stderr: "inherit",
    env: { ...process.env, CGO_ENABLED: "1" },
  });
  if (proc.exitCode !== 0) {
    throw new Error(`Command failed (${proc.exitCode}): ${cmd.join(" ")}`);
  }
}

run([
  "go",
  "build",
  "-buildmode=c-shared",
  "-o",
  join(BUILD_DIR, dllName),
  ".",
]);
run(["go", "build", "-o", join(BUILD_DIR, cliName), "./cmd/nie-rclone"]);

if (runTest) {
  run(["go", "test", "-v", "./vfsbridge/..."]);
}

console.log(
  `Built ${resolve(BUILD_DIR, dllName)} and ${resolve(BUILD_DIR, cliName)}`,
);
