#!/usr/bin/env bun
// Stop hook: keeps the session working while the plan has open tasks. Reads the session's task store,
// blocks the stop with a minimal JSON reason, gives up after MAX consecutive blocks with no task change.
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";

const MAX = 25;
const input = JSON.parse((await Bun.stdin.text()) || "{}") as { session_id?: string };
const sid = (input.session_id ?? "").slice(0, 8);
const dir = join(homedir(), ".claude", "tasks", `session-${sid}`);
if (!sid || !existsSync(dir)) process.exit(0);

const tasks = readdirSync(dir)
  .filter((f) => f.endsWith(".json"))
  .map((f) => JSON.parse(readFileSync(join(dir, f), "utf8")) as { id: string; status: string; blockedBy?: string[] });
const open = tasks.filter((t) => t.status !== "completed" && t.status !== "deleted");
if (!open.length) process.exit(0);

const sig = open.map((t) => `${t.id}:${t.status}`).join(",");
const state = join(tmpdir(), `nie-goal-${sid}.json`);
const prev = existsSync(state) ? (JSON.parse(readFileSync(state, "utf8")) as { sig: string; n: number }) : { sig: "", n: 0 };
const n = prev.sig === sig ? prev.n + 1 : 1;
writeFileSync(state, JSON.stringify({ sig, n }));
if (n > MAX) process.exit(0);

const ready = open.filter((t) => t.status === "in_progress" || !(t.blockedBy ?? []).length).map((t) => t.id);
console.log(JSON.stringify({ decision: "block", reason: JSON.stringify({ goal: "complete the plan", open: open.length, next: ready.slice(0, 8), rule: "act, verify, continue; no questions" }) }));
