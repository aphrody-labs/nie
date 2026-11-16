# Full Bun migration plan

Goal: every JS/TS tool and library that Bun ships natively is replaced by Bun; one driver per data
store; one version policy. Inventory measured on 2026-11-16 (`docs/proofs/stack/matrix.md`).

## 0. Version policy

- Machine runs `bun 1.4.5-canary.1+9098897a1`; the latest upstream release on GitHub is `bun-v1.4.4`
  (2026-10-22). `packageManager: bun@1.4.5`, `engines.bun >=1.4.5` and the `oven/bun:${BUN_VERSION}`
  images name a version that is not a release.
- Step: pin `packageManager`, `engines` and `BUN_VERSION` to the latest release read live
  (`gh api repos/oven-sh/bun/releases/latest`), keep canary only on the workstation for `bun lint`.
- Proof: `bun --version` inside the website and core images equals the pinned release.

## 1. Tooling (no runtime impact)

| Today | Where | Bun replacement | Proof |
| --- | --- | --- | --- |
| `oxlint` (22 workspace `lint` scripts, `rg-config/oxlintrc.json`) | apps, packages | `bun lint`, rules ported from `oxlintrc.json` | `bun run lint` green on all workspaces, oxlint removed from the root manifest |
| `turbo` 2.11.12-canary.3 (`turbo:*` scripts) | root | `bun run --filter '*'` (already the default scripts) | `turbo` removed, `bun run build/typecheck/test` unchanged |
| `typescript` (3 manifests) | tooling | `bun check` (17 workspaces already use it) | no `typescript` in any manifest, `bun run typecheck` green |
| `@types/bun` (20), `@types/node` (4), `@types/pg` (3) | devDependencies | `bun check` built-in types | removed, typecheck green |
| `picocolors` (3 files) | scripts | `Bun.color(c, "ansi")` | removed, script output unchanged |
| formatting | 2 workspaces use `bun fmt` | `bun fmt --check` at root in CI | CI step `bun fmt --check .` |

## 2. Data drivers (one per store)

| Store | Today | Target | Files to move |
| --- | --- | --- | --- |
| Postgres | `pg` (rg-db/pg.ts, inagle), `Bun.SQL` (rg-db, rg-cron, inagle, azalee), drizzle (`inagle/src/sql/drizzle.ts`), Supabase Data API (rg-db/browser.ts and 13 site files) | `Bun.SQL` behind the existing rg-db facade, Kysely on top where a query builder is used | rg-db/pg.ts, inagle drizzle.ts, the 13 `supabase.from/rpc` call sites |
| SQLite | `bun:sqlite` (inagle 14, azalee 6, rg-cron 2) | unchanged | none |
| Redis | `Bun.redis` / `RedisClient` (rg-bot cache, rg-db) | unchanged (server side: Valkey decision is separate) | none |
| Object storage | Supabase Storage (3 call sites) | `Bun.S3Client` on R2 | admin/media, image-upload, rg-db browser |

Proof per row: the package's `bun test <path>` green, `pg`, `drizzle-orm`, `@supabase/*`,
`kysely-supabase` absent from every manifest, and a real request on prod that reads and writes
through the new driver (content checked).

## 3. Runtime libraries

| Today | Target | Notes |
| --- | --- | --- |
| `sharp` (2 manifests, 1 file) | `Bun.Image` | check the formats the file uses are supported by `Bun.Image` first |
| Elysia (rg-bot, 64 files) | keep | Elysia is a thin layer over `Bun.serve`; a rewrite to bare `Bun.serve` is not worth it. Replace `@elysiajs/swagger` by `@elysiajs/openapi` |
| Next.js (site) | keep | AGENTS.md: the site stays as is; Next runs on Bun |

## 4. Order of commits

1. Version pin (section 0).
2. turbo, typescript, @types/*, picocolors removal.
3. oxlint -> `bun lint` (port rules, then remove).
4. Postgres: drizzle and `pg` -> `Bun.SQL`; then Supabase Data API -> rg-db facade.
5. Supabase Storage -> `Bun.S3Client` on R2; sharp -> `Bun.Image`.
6. `@elysiajs/swagger` -> `@elysiajs/openapi`.

Each commit: `bun check`, `bun lint`, `bun test <package>` for every touched package, gitleaks.
Deploy only after steps 4 and 5, through `just deploy`, with screenshots of the touched pages in
`docs/proofs/`.
