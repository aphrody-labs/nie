import { Database } from "bun:sqlite";
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";

export type FrontierStatus =
  "pending" | "processing" | "done" | "failed" | "challenged";

export interface FrontierEnqueueInput {
  targetSlug: string;
  url: string;
  engineHint?: string;
  sourceKind?: string;
  priority?: number;
  maxRetries?: number;
}

export interface FrontierRow {
  id: number;
  target_slug: string;
  url: string;
  normalized_url: string;
  host: string;
  engine_hint: string;
  source_kind: string;
  priority: number;
  status: FrontierStatus;
  attempts: number;
  max_retries: number;
  last_error: string | null;
  content_hash: string | null;
  next_retry_at: number;
  updated_at: number;
}

export function normalizeCrawlUrl(rawUrl: string): {
  normalized: string;
  host: string;
} {
  const parsed = new URL(rawUrl);
  parsed.hash = "";
  const entries = [...parsed.searchParams.entries()].sort(([a], [b]) =>
    a.localeCompare(b),
  );
  parsed.search = "";
  for (const [k, v] of entries) {
    parsed.searchParams.append(k, v);
  }
  return {
    normalized: parsed.href,
    host: parsed.hostname.toLowerCase(),
  };
}

let crawleeSupportChecked: boolean | null = null;
export async function isCrawleeCoreAvailable(): Promise<boolean> {
  if (crawleeSupportChecked !== null) return crawleeSupportChecked;
  try {
    const req = createOptionalRequire();
    if (!req) {
      crawleeSupportChecked = false;
      return false;
    }
    req.resolve("@crawlee/core");
    crawleeSupportChecked = true;
    return true;
  } catch {
    crawleeSupportChecked = false;
    return false;
  }
}

function createOptionalRequire(): NodeRequire | null {
  try {
    return Bun.main ? (createRequireCompat() ?? null) : null;
  } catch {
    return null;
  }
}

function createRequireCompat(): NodeRequire | null {
  try {
    const mod = process.getBuiltinModule?.("node:module") as
      { createRequire?: (id: string) => NodeRequire } | undefined;
    return mod?.createRequire?.(import.meta.url) ?? null;
  } catch {
    return null;
  }
}

/**
 * Durable SQLite-backed request frontier queue following Crawlee RequestQueue semantics:
 * - canonical URL normalization & deduplication
 * - per-host rate limiting / cooldown windows
 * - exponential retry backoff
 * - SHA-256 content hash deduplication
 */
export class CrawleeFrontierQueue {
  private readonly db: Database;

  constructor(
    sqlitePath = Bun.env.RG_CRAWL_SQLITE_PATH || "var/character-crawl.sqlite",
  ) {
    if (sqlitePath !== ":memory:") {
      mkdirSync(dirname(sqlitePath), { recursive: true });
    }
    this.db = new Database(sqlitePath);
    this.db.run("PRAGMA journal_mode = WAL;");
    this.db.run("PRAGMA busy_timeout = 5000;");
    this.initSchema();
  }

  private initSchema(): void {
    this.db.run(`
      CREATE TABLE IF NOT EXISTS crawl_frontier (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        target_slug TEXT NOT NULL,
        url TEXT NOT NULL,
        normalized_url TEXT NOT NULL,
        host TEXT NOT NULL,
        engine_hint TEXT NOT NULL DEFAULT 'auto',
        source_kind TEXT NOT NULL DEFAULT 'web',
        priority INTEGER NOT NULL DEFAULT 50,
        status TEXT NOT NULL DEFAULT 'pending',
        attempts INTEGER NOT NULL DEFAULT 0,
        max_retries INTEGER NOT NULL DEFAULT 3,
        last_error TEXT,
        content_hash TEXT,
        next_retry_at INTEGER NOT NULL DEFAULT 0,
        updated_at INTEGER NOT NULL DEFAULT 0,
        UNIQUE(target_slug, normalized_url)
      );
      CREATE INDEX IF NOT EXISTS idx_frontier_claim
        ON crawl_frontier (target_slug, status, next_retry_at, priority DESC);
      CREATE TABLE IF NOT EXISTS host_rate_limits (
        host TEXT PRIMARY KEY,
        last_request_at INTEGER NOT NULL
      );
    `);
  }

  enqueue(input: FrontierEnqueueInput): boolean {
    const { normalized, host } = normalizeCrawlUrl(input.url);
    const now = Date.now();
    const res = this.db
      .query(
        `INSERT INTO crawl_frontier (
           target_slug, url, normalized_url, host, engine_hint, source_kind,
           priority, status, max_retries, next_retry_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?, 0, ?)
         ON CONFLICT(target_slug, normalized_url) DO NOTHING`,
      )
      .run(
        input.targetSlug,
        input.url,
        normalized,
        host,
        input.engineHint ?? "auto",
        input.sourceKind ?? "web",
        input.priority ?? 50,
        input.maxRetries ?? 3,
        now,
      );
    return res.changes > 0;
  }

  enqueueBatch(items: readonly FrontierEnqueueInput[]): number {
    let added = 0;
    const tx = this.db.transaction(() => {
      for (const item of items) {
        if (this.enqueue(item)) added++;
      }
    });
    tx();
    return added;
  }

  /** Re-queue finished URLs whose TTL expired so continuous watch mode refreshes them. */
  requeueExpired(targetSlug: string, ttlMs: number): number {
    const cutoff = Date.now() - ttlMs;
    const res = this.db
      .query(
        `UPDATE crawl_frontier
         SET status = 'pending', next_retry_at = 0, updated_at = ?
         WHERE target_slug = ? AND status = 'done' AND updated_at < ?`,
      )
      .run(Date.now(), targetSlug, cutoff);
    return res.changes;
  }

  claimNext(targetSlug: string, hostCooldownMs = 800): FrontierRow | null {
    const now = Date.now();
    const candidates = this.db
      .query(
        `SELECT f.*
         FROM crawl_frontier f
         LEFT JOIN host_rate_limits h ON h.host = f.host
         WHERE f.target_slug = ?
           AND f.status = 'pending'
           AND f.next_retry_at <= ?
           AND (h.last_request_at IS NULL OR (? - h.last_request_at) >= ?)
         ORDER BY f.priority DESC, f.id ASC
         LIMIT 1`,
      )
      .all(targetSlug, now, now, hostCooldownMs) as FrontierRow[];

    const row = candidates[0];
    if (!row) return null;

    this.db.run(
      `UPDATE crawl_frontier SET status = 'processing', attempts = attempts + 1, updated_at = ? WHERE id = ?`,
      [now, row.id],
    );
    this.db.run(
      `INSERT INTO host_rate_limits (host, last_request_at) VALUES (?, ?)
       ON CONFLICT(host) DO UPDATE SET last_request_at = excluded.last_request_at`,
      [row.host, now],
    );
    return { ...row, status: "processing", attempts: row.attempts + 1 };
  }

  markDone(id: number, contentHash: string): void {
    this.db.run(
      `UPDATE crawl_frontier SET status = 'done', content_hash = ?, last_error = NULL, updated_at = ? WHERE id = ?`,
      [contentHash, Date.now(), id],
    );
  }

  markChallenged(id: number, reason: string): void {
    this.db.run(
      `UPDATE crawl_frontier SET status = 'challenged', last_error = ?, updated_at = ? WHERE id = ?`,
      [reason, Date.now(), id],
    );
  }

  markFailed(id: number, error: string): void {
    const row = this.db
      .query(`SELECT attempts, max_retries FROM crawl_frontier WHERE id = ?`)
      .get(id) as { attempts: number; max_retries: number } | null;
    const now = Date.now();
    if (row && row.attempts < row.max_retries) {
      const backoffMs = Math.min(300_000, 2_000 * 2 ** row.attempts);
      this.db.run(
        `UPDATE crawl_frontier SET status = 'pending', last_error = ?, next_retry_at = ?, updated_at = ? WHERE id = ?`,
        [error, now + backoffMs, now, id],
      );
      return;
    }
    this.db.run(
      `UPDATE crawl_frontier SET status = 'failed', last_error = ?, updated_at = ? WHERE id = ?`,
      [error, now, id],
    );
  }

  stats(targetSlug: string): Record<FrontierStatus, number> {
    const rows = this.db
      .query(
        `SELECT status, COUNT(*) as cnt FROM crawl_frontier WHERE target_slug = ? GROUP BY status`,
      )
      .all(targetSlug) as Array<{ status: FrontierStatus; cnt: number }>;
    const summary: Record<FrontierStatus, number> = {
      pending: 0,
      processing: 0,
      done: 0,
      failed: 0,
      challenged: 0,
    };
    for (const r of rows) {
      if (r.status in summary) summary[r.status] = Number(r.cnt);
    }
    return summary;
  }

  close(): void {
    this.db.close();
  }
}
