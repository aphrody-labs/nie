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
  depth?: number;
  parentUrl?: string | null;
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
  depth: number;
  parent_url: string | null;
  status: FrontierStatus;
  attempts: number;
  max_retries: number;
  last_error: string | null;
  content_hash: string | null;
  lease_worker_id: string | null;
  lease_expires_at: number;
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
 * Durable SQLite-backed recursive request frontier queue following Crawlee semantics:
 * - canonical URL normalization & deduplication
 * - recursive depth & parent_url tracking
 * - distributed multi-worker lease tracking & expired lease recovery
 * - per-host rate limiting / cooldown windows
 * - exponential retry backoff
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
        depth INTEGER NOT NULL DEFAULT 0,
        parent_url TEXT,
        status TEXT NOT NULL DEFAULT 'pending',
        attempts INTEGER NOT NULL DEFAULT 0,
        max_retries INTEGER NOT NULL DEFAULT 3,
        last_error TEXT,
        content_hash TEXT,
        lease_worker_id TEXT,
        lease_expires_at INTEGER NOT NULL DEFAULT 0,
        next_retry_at INTEGER NOT NULL DEFAULT 0,
        updated_at INTEGER NOT NULL DEFAULT 0,
        UNIQUE(target_slug, normalized_url)
      );
      CREATE INDEX IF NOT EXISTS idx_frontier_claim
        ON crawl_frontier (target_slug, status, depth, next_retry_at, priority DESC);
      CREATE TABLE IF NOT EXISTS host_rate_limits (
        host TEXT PRIMARY KEY,
        last_request_at INTEGER NOT NULL
      );
    `);

    const cols = this.db
      .query(`PRAGMA table_info(crawl_frontier)`)
      .all() as Array<{ name: string }>;
    const names = new Set(cols.map((c) => c.name));
    if (!names.has("depth")) {
      this.db.run(
        `ALTER TABLE crawl_frontier ADD COLUMN depth INTEGER NOT NULL DEFAULT 0`,
      );
    }
    if (!names.has("parent_url")) {
      this.db.run(`ALTER TABLE crawl_frontier ADD COLUMN parent_url TEXT`);
    }
    if (!names.has("lease_worker_id")) {
      this.db.run(`ALTER TABLE crawl_frontier ADD COLUMN lease_worker_id TEXT`);
    }
    if (!names.has("lease_expires_at")) {
      this.db.run(
        `ALTER TABLE crawl_frontier ADD COLUMN lease_expires_at INTEGER NOT NULL DEFAULT 0`,
      );
    }
  }

  enqueue(input: FrontierEnqueueInput): boolean {
    const { normalized, host } = normalizeCrawlUrl(input.url);
    const now = Date.now();
    const res = this.db
      .query(
        `INSERT INTO crawl_frontier (
           target_slug, url, normalized_url, host, engine_hint, source_kind,
           priority, depth, parent_url, status, max_retries, next_retry_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, 0, ?)
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
        input.depth ?? 0,
        input.parentUrl ?? null,
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

  requeueExpired(targetSlug: string, ttlMs: number): number {
    const cutoff = Date.now() - ttlMs;
    const res = this.db
      .query(
        `UPDATE crawl_frontier
         SET status = 'pending', next_retry_at = 0, lease_worker_id = NULL, lease_expires_at = 0, updated_at = ?
         WHERE target_slug = ? AND status = 'done' AND updated_at < ?`,
      )
      .run(Date.now(), targetSlug, cutoff);
    return res.changes;
  }

  reclaimExpiredLeases(targetSlug: string): number {
    const now = Date.now();
    const res = this.db
      .query(
        `UPDATE crawl_frontier
         SET status = 'pending', lease_worker_id = NULL, lease_expires_at = 0, updated_at = ?
         WHERE target_slug = ? AND status = 'processing' AND lease_expires_at > 0 AND lease_expires_at < ?`,
      )
      .run(now, targetSlug, now);
    return res.changes;
  }

  claimBatch(
    targetSlug: string,
    options: {
      limit?: number;
      maxDepth?: number;
      workerId?: string;
      leaseMs?: number;
      hostCooldownMs?: number;
    } = {},
  ): FrontierRow[] {
    const limit = Math.max(1, options.limit ?? 1);
    const maxDepth = options.maxDepth ?? 3;
    const workerId = options.workerId ?? "local";
    const leaseMs = options.leaseMs ?? 90_000;
    const hostCooldownMs = options.hostCooldownMs ?? 500;
    const now = Date.now();

    this.reclaimExpiredLeases(targetSlug);

    const claimed: FrontierRow[] = [];
    const seenHosts = new Set<string>();

    const candidates = this.db
      .query(
        `SELECT f.*
         FROM crawl_frontier f
         LEFT JOIN host_rate_limits h ON h.host = f.host
         WHERE f.target_slug = ?
           AND f.status = 'pending'
           AND f.depth <= ?
           AND f.next_retry_at <= ?
           AND (h.last_request_at IS NULL OR (? - h.last_request_at) >= ?)
         ORDER BY f.depth ASC, f.priority DESC, f.id ASC
         LIMIT ?`,
      )
      .all(
        targetSlug,
        maxDepth,
        now,
        now,
        hostCooldownMs,
        limit * 4,
      ) as FrontierRow[];

    for (const row of candidates) {
      if (claimed.length >= limit) break;
      if (hostCooldownMs > 0 && seenHosts.has(row.host)) continue;
      seenHosts.add(row.host);

      const leaseExpires = now + leaseMs;
      this.db.run(
        `UPDATE crawl_frontier
         SET status = 'processing',
             attempts = attempts + 1,
             lease_worker_id = ?,
             lease_expires_at = ?,
             updated_at = ?
         WHERE id = ? AND status = 'pending'`,
        [workerId, leaseExpires, now, row.id],
      );
      this.db.run(
        `INSERT INTO host_rate_limits (host, last_request_at) VALUES (?, ?)
         ON CONFLICT(host) DO UPDATE SET last_request_at = excluded.last_request_at`,
        [row.host, now],
      );
      claimed.push({
        ...row,
        status: "processing",
        attempts: row.attempts + 1,
        lease_worker_id: workerId,
        lease_expires_at: leaseExpires,
      });
    }

    return claimed;
  }

  claimNext(targetSlug: string, hostCooldownMs = 800): FrontierRow | null {
    return this.claimBatch(targetSlug, { limit: 1, hostCooldownMs })[0] ?? null;
  }

  markDone(id: number, contentHash: string): void {
    this.db.run(
      `UPDATE crawl_frontier
       SET status = 'done', content_hash = ?, last_error = NULL, lease_worker_id = NULL, lease_expires_at = 0, updated_at = ?
       WHERE id = ?`,
      [contentHash, Date.now(), id],
    );
  }

  markChallenged(id: number, reason: string): void {
    this.db.run(
      `UPDATE crawl_frontier
       SET status = 'challenged', last_error = ?, lease_worker_id = NULL, lease_expires_at = 0, updated_at = ?
       WHERE id = ?`,
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
        `UPDATE crawl_frontier
         SET status = 'pending', last_error = ?, lease_worker_id = NULL, lease_expires_at = 0, next_retry_at = ?, updated_at = ?
         WHERE id = ?`,
        [error, now + backoffMs, now, id],
      );
      return;
    }
    this.db.run(
      `UPDATE crawl_frontier
       SET status = 'failed', last_error = ?, lease_worker_id = NULL, lease_expires_at = 0, updated_at = ?
       WHERE id = ?`,
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

  depthHistogram(targetSlug: string): Record<string, number> {
    const rows = this.db
      .query(
        `SELECT depth, COUNT(*) as cnt FROM crawl_frontier WHERE target_slug = ? AND status = 'done' GROUP BY depth ORDER BY depth ASC`,
      )
      .all(targetSlug) as Array<{ depth: number; cnt: number }>;
    const hist: Record<string, number> = {};
    for (const r of rows) {
      hist[`depth_${r.depth}`] = Number(r.cnt);
    }
    return hist;
  }

  close(): void {
    this.db.close();
  }
}
