#!/usr/bin/env bun
// Project status line: branch, uncommitted file count, open tasks of the session. No network, no ssh.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const input = JSON.parse((await Bun.stdin.text()) || "{}") as { session_id?: string };
const git = (...a: string[]) => Bun.spawnSync(["git", ...a], { stdout: "pipe", stderr: "ignore" }).stdout.toString().trim();
const branch = git("branch", "--show-current") || "detached";
const dirty = git("status", "--porcelain").split("\n").filter(Boolean).length;
const dir = join(homedir(), ".claude", "tasks", `session-${(input.session_id ?? "").slice(0, 8)}`);
let open = 0;
let total = 0;
if (existsSync(dir)) {
  for (const f of readdirSync(dir).filter((n) => n.endsWith(".json"))) {
    try {
      const t = JSON.parse(readFileSync(join(dir, f), "utf8")) as { status: string };
      total++;
      if (t.status !== "completed" && t.status !== "deleted") open++;
    } catch {}
  }
}
console.log(`nie ${branch} ~${dirty} tasks ${open}/${total}`);
