import { Database } from "bun:sqlite";
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";
import type { CharacterIntelTarget } from "@rosegriffon/inagle";
import { cache, crawlerTracker, getRedisClient } from "@rosegriffon/db/redis";
import type { CrawlFetchOutcome } from "./crawl-engines";
import { sql } from "./db";
import { CrawlOramaIndex } from "./orama-index";
import type { R2UploadResult } from "./r2-assets";

export const CHARACTER_CRAWL_EVENTS_CHANNEL = "rg:crawl:character:events";

function isPostgresConfigured(): boolean {
  return Boolean(
    Bun.env.DATABASE_URL?.trim() || Bun.env.RG_DATABASE_URL?.trim(),
  );
}

function isRedisConfigured(): boolean {
  return Boolean(Bun.env.REDIS_URL?.trim() || Bun.env.REDIS_HOST?.trim());
}

async function withTimeout<T>(
  promise: Promise<T>,
  fallback: T,
  ms = 1200,
): Promise<T> {
  return Promise.race([
    promise.catch(() => fallback),
    new Promise<T>((resolve) => setTimeout(() => resolve(fallback), ms)),
  ]);
}

export function computeContentHash(text: string | null | undefined): string {
  const hasher = new Bun.CryptoHasher("sha256");
  hasher.update(text ?? "");
  return hasher.digest("hex");
}

export interface StoredCharacterPageSummary {
  url: string;
  engine: string;
  status: string;
  title: string | null;
  contentHash: string;
  fetchedAt: number;
}

export interface StoredCharacterAssetSummary {
  sourceUrl: string;
  sha256: string;
  contentType: string;
  byteSize: number;
  r2Key: string;
  r2Url: string | null;
  storageMode: string;
}

export class CharacterCrawlStore {
  private readonly sqlite: Database;
  private readonly persistOrama: boolean;
  private readonly oramaIndex: CrawlOramaIndex;

  constructor(
    sqlitePath = Bun.env.RG_CRAWL_SQLITE_PATH || "var/character-crawl.sqlite",
  ) {
    this.persistOrama = sqlitePath !== ":memory:";
    if (this.persistOrama) {
      mkdirSync(dirname(sqlitePath), { recursive: true });
    }
    this.sqlite = new Database(sqlitePath);
    this.sqlite.run("PRAGMA journal_mode = WAL;");
    this.sqlite.run("PRAGMA busy_timeout = 5000;");
    this.oramaIndex = new CrawlOramaIndex(
      this.persistOrama ? `${sqlitePath}.orama.json` : ":memory:",
    );
    this.initSchema();
  }

  private initSchema(): void {
    this.sqlite.run(`
      CREATE TABLE IF NOT EXISTS character_targets (
        slug TEXT PRIMARY KEY,
        canonical_name TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        last_crawled_at INTEGER,
        last_summary_json TEXT
      );
      CREATE TABLE IF NOT EXISTS character_pages (
        target_slug TEXT NOT NULL,
        normalized_url TEXT NOT NULL,
        url TEXT NOT NULL,
        engine TEXT NOT NULL,
        source_kind TEXT NOT NULL,
        status TEXT NOT NULL,
        http_status INTEGER,
        title TEXT,
        language TEXT,
        content_hash TEXT NOT NULL,
        excerpt TEXT,
        body_text TEXT,
        metadata_json TEXT NOT NULL,
        discovered_links_json TEXT NOT NULL,
        depth INTEGER NOT NULL DEFAULT 0,
        parent_url TEXT,
        worker_host TEXT,
        worker_id TEXT,
        fetched_at INTEGER NOT NULL,
        PRIMARY KEY (target_slug, normalized_url)
      );
      CREATE TABLE IF NOT EXISTS character_assets (
        target_slug TEXT NOT NULL,
        sha256 TEXT NOT NULL,
        page_url TEXT,
        source_url TEXT NOT NULL,
        content_type TEXT NOT NULL,
        byte_size INTEGER NOT NULL,
        r2_bucket TEXT,
        r2_key TEXT NOT NULL,
        r2_url TEXT,
        storage_mode TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        PRIMARY KEY (target_slug, sha256)
      );
    `);
    const cols = this.sqlite
      .query(`PRAGMA table_info(character_pages)`)
      .all() as Array<{ name: string }>;
    const colNames = new Set(cols.map((c) => c.name));
    if (!colNames.has("depth")) {
      this.sqlite.run(
        `ALTER TABLE character_pages ADD COLUMN depth INTEGER NOT NULL DEFAULT 0`,
      );
    }
    if (!colNames.has("parent_url")) {
      this.sqlite.run(`ALTER TABLE character_pages ADD COLUMN parent_url TEXT`);
    }
    if (!colNames.has("worker_host")) {
      this.sqlite.run(
        `ALTER TABLE character_pages ADD COLUMN worker_host TEXT`,
      );
    }
    if (!colNames.has("worker_id")) {
      this.sqlite.run(`ALTER TABLE character_pages ADD COLUMN worker_id TEXT`);
    }
  }

  async upsertTarget(
    target: CharacterIntelTarget,
    lastSummary?: Record<string, unknown>,
  ): Promise<void> {
    const now = Date.now();
    this.sqlite.run(
      `INSERT INTO character_targets (slug, canonical_name, payload_json, last_crawled_at, last_summary_json)
       VALUES (?, ?, ?, ?, ?)
       ON CONFLICT(slug) DO UPDATE SET
         canonical_name = excluded.canonical_name,
         payload_json = excluded.payload_json,
         last_crawled_at = COALESCE(excluded.last_crawled_at, character_targets.last_crawled_at),
         last_summary_json = COALESCE(excluded.last_summary_json, character_targets.last_summary_json)`,
      [
        target.slug,
        target.canonicalName,
        JSON.stringify(target),
        lastSummary ? now : null,
        lastSummary ? JSON.stringify(lastSummary) : null,
      ],
    );

    if (isPostgresConfigured()) {
      try {
        await sql`
          insert into public.character_crawl_targets (
            slug, canonical_name, localized_names, aliases, zukan_ids,
            seed_urls, search_queries, last_crawled_at, last_summary, updated_at
          ) values (
            ${target.slug},
            ${target.canonicalName},
            ${JSON.stringify(target.localizedNames)}::jsonb,
            ${target.aliases},
            ${target.zukanIds},
            ${target.seedUrls},
            ${target.searchQueries},
            ${lastSummary ? new Date(now).toISOString() : null},
            ${JSON.stringify(lastSummary ?? {})}::jsonb,
            now()
          )
          on conflict (slug) do update set
            canonical_name = excluded.canonical_name,
            localized_names = excluded.localized_names,
            aliases = excluded.aliases,
            zukan_ids = excluded.zukan_ids,
            seed_urls = excluded.seed_urls,
            search_queries = excluded.search_queries,
            last_crawled_at = coalesce(excluded.last_crawled_at, public.character_crawl_targets.last_crawled_at),
            last_summary = case
              when excluded.last_summary = '{}'::jsonb then public.character_crawl_targets.last_summary
              else excluded.last_summary
            end,
            updated_at = now()
        `;
      } catch (err) {
        console.warn("[character-store] Postgres target upsert warning:", err);
      }
    }
  }

  async savePage(
    targetSlug: string,
    outcome: CrawlFetchOutcome,
    context: {
      depth?: number;
      parentUrl?: string | null;
      workerHost?: string | null;
      workerId?: string | null;
    } = {},
  ): Promise<{ contentHash: string; changed: boolean }> {
    const depth = context.depth ?? 0;
    const parentUrl = context.parentUrl ?? null;
    const workerHost = context.workerHost ?? null;
    const workerId = context.workerId ?? null;
    const contentHash = computeContentHash(
      `${outcome.title ?? ""}\n${outcome.bodyText ?? outcome.status}`,
    );

    const existing = this.sqlite
      .query(
        `SELECT content_hash FROM character_pages WHERE target_slug = ? AND normalized_url = ?`,
      )
      .get(targetSlug, outcome.normalizedUrl) as {
      content_hash: string;
    } | null;

    let changed = existing?.content_hash !== contentHash;
    if (isRedisConfigured()) {
      const redisChanged = await withTimeout(
        crawlerTracker.shouldProcess(outcome.normalizedUrl, contentHash),
        changed,
      );
      changed = changed || redisChanged;
    }

    const now = Date.now();
    this.sqlite.run(
      `INSERT INTO character_pages (
         target_slug, normalized_url, url, engine, source_kind, status, http_status,
         title, language, content_hash, excerpt, body_text, metadata_json,
         discovered_links_json, depth, parent_url, worker_host, worker_id, fetched_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(target_slug, normalized_url) DO UPDATE SET
         url = excluded.url,
         engine = excluded.engine,
         source_kind = excluded.source_kind,
         status = excluded.status,
         http_status = excluded.http_status,
         title = excluded.title,
         language = excluded.language,
         content_hash = excluded.content_hash,
         excerpt = excluded.excerpt,
         body_text = excluded.body_text,
         metadata_json = excluded.metadata_json,
         discovered_links_json = excluded.discovered_links_json,
         depth = excluded.depth,
         parent_url = excluded.parent_url,
         worker_host = excluded.worker_host,
         worker_id = excluded.worker_id,
         fetched_at = excluded.fetched_at`,
      [
        targetSlug,
        outcome.normalizedUrl,
        outcome.url,
        outcome.engine,
        outcome.sourceKind,
        outcome.status,
        outcome.httpStatus,
        outcome.title,
        outcome.language,
        contentHash,
        outcome.excerpt,
        outcome.bodyText,
        JSON.stringify(outcome.metadata),
        JSON.stringify(outcome.discoveredLinks),
        depth,
        parentUrl,
        workerHost,
        workerId,
        now,
      ],
    );

    if (isRedisConfigured()) {
      await withTimeout(
        crawlerTracker.markProcessed(outcome.normalizedUrl, contentHash),
        undefined,
      );
      await withTimeout(
        cache.set(
          `rg:crawl:page:${targetSlug}:${contentHash.slice(0, 16)}`,
          {
            url: outcome.normalizedUrl,
            engine: outcome.engine,
            status: outcome.status,
            title: outcome.title,
            excerpt: outcome.excerpt,
            depth,
            workerHost,
            workerId,
            fetchedAt: now,
          },
          3600 * 6,
        ),
        false,
      );
      await this.publishRealtimeEvent({
        type: "page",
        targetSlug,
        url: outcome.normalizedUrl,
        engine: outcome.engine,
        status: outcome.status,
        depth,
        workerHost,
        workerId,
        changed,
        timestamp: now,
      });
    }

    if (isPostgresConfigured()) {
      try {
        await sql`
          insert into public.character_crawl_pages (
            target_slug, url, normalized_url, engine, source_kind, status,
            http_status, title, language, content_hash, excerpt, body_text,
            metadata, discovered_links, depth, parent_url, worker_host, worker_id,
            fetched_at, updated_at
          ) values (
            ${targetSlug},
            ${outcome.url},
            ${outcome.normalizedUrl},
            ${outcome.engine},
            ${outcome.sourceKind},
            ${outcome.status},
            ${outcome.httpStatus},
            ${outcome.title},
            ${outcome.language},
            ${contentHash},
            ${outcome.excerpt},
            ${outcome.bodyText},
            ${JSON.stringify(outcome.metadata)}::jsonb,
            ${JSON.stringify(outcome.discoveredLinks)}::jsonb,
            ${depth},
            ${parentUrl},
            ${workerHost},
            ${workerId},
            ${new Date(now).toISOString()},
            now()
          )
          on conflict (target_slug, normalized_url) do update set
            url = excluded.url,
            engine = excluded.engine,
            source_kind = excluded.source_kind,
            status = excluded.status,
            http_status = excluded.http_status,
            title = excluded.title,
            language = excluded.language,
            content_hash = excluded.content_hash,
            excerpt = excluded.excerpt,
            body_text = excluded.body_text,
            metadata = excluded.metadata,
            discovered_links = excluded.discovered_links,
            depth = excluded.depth,
            parent_url = excluded.parent_url,
            worker_host = excluded.worker_host,
            worker_id = excluded.worker_id,
            fetched_at = excluded.fetched_at,
            updated_at = now()
        `;
      } catch (err) {
        console.warn("[character-store] Postgres page upsert warning:", err);
      }
    }

    if (outcome.status === "ok" && (outcome.title || outcome.bodyText)) {
      this.oramaIndex.indexDocument({
        id: `${targetSlug}:${contentHash.slice(0, 16)}:${outcome.normalizedUrl}`,
        targetSlug,
        url: outcome.url,
        normalizedUrl: outcome.normalizedUrl,
        title: outcome.title ?? outcome.normalizedUrl,
        excerpt: outcome.excerpt ?? "",
        bodyText: outcome.bodyText ?? "",
        language: outcome.language ?? "und",
        sourceKind: outcome.sourceKind,
        engine: outcome.engine,
        depth,
        tags: [targetSlug, outcome.sourceKind, outcome.engine],
        fetchedAt: now,
      });
    }

    return { contentHash, changed };
  }

  async saveAsset(
    targetSlug: string,
    pageUrl: string | null,
    asset: R2UploadResult,
  ): Promise<void> {
    const now = Date.now();
    this.sqlite.run(
      `INSERT INTO character_assets (
         target_slug, sha256, page_url, source_url, content_type, byte_size,
         r2_bucket, r2_key, r2_url, storage_mode, created_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(target_slug, sha256) DO UPDATE SET
         page_url = COALESCE(excluded.page_url, character_assets.page_url),
         r2_bucket = excluded.r2_bucket,
         r2_key = excluded.r2_key,
         r2_url = excluded.r2_url,
         storage_mode = excluded.storage_mode`,
      [
        targetSlug,
        asset.sha256,
        pageUrl,
        asset.sourceUrl,
        asset.contentType,
        asset.byteSize,
        asset.r2Bucket,
        asset.r2Key,
        asset.r2Url,
        asset.storageMode,
        now,
      ],
    );

    if (isRedisConfigured()) {
      await this.publishRealtimeEvent({
        type: "asset",
        targetSlug,
        sha256: asset.sha256,
        r2Key: asset.r2Key,
        storageMode: asset.storageMode,
        timestamp: now,
      });
    }

    if (isPostgresConfigured()) {
      try {
        await sql`
          insert into public.character_crawl_assets (
            target_slug, page_url, source_url, sha256, content_type,
            byte_size, r2_bucket, r2_key, r2_url, storage_mode, updated_at
          ) values (
            ${targetSlug},
            ${pageUrl},
            ${asset.sourceUrl},
            ${asset.sha256},
            ${asset.contentType},
            ${asset.byteSize},
            ${asset.r2Bucket},
            ${asset.r2Key},
            ${asset.r2Url},
            ${asset.storageMode},
            now()
          )
          on conflict (target_slug, sha256) do update set
            page_url = coalesce(excluded.page_url, public.character_crawl_assets.page_url),
            r2_bucket = excluded.r2_bucket,
            r2_key = excluded.r2_key,
            r2_url = excluded.r2_url,
            storage_mode = excluded.storage_mode,
            updated_at = now()
        `;
      } catch (err) {
        console.warn("[character-store] Postgres asset upsert warning:", err);
      }
    }
  }

  async publishCycleSummary(
    targetSlug: string,
    summary: Record<string, unknown>,
  ): Promise<void> {
    if (!isRedisConfigured()) return;
    await withTimeout(
      cache.set(`rg:crawl:character:${targetSlug}:latest`, summary, 3600),
      false,
    );
    await this.publishRealtimeEvent({
      type: "cycle",
      targetSlug,
      summary,
      timestamp: Date.now(),
    });
  }

  private async publishRealtimeEvent(
    payload: Record<string, unknown>,
  ): Promise<void> {
    if (!isRedisConfigured()) return;
    const redis = getRedisClient();
    await withTimeout(
      redis.publish(CHARACTER_CRAWL_EVENTS_CHANNEL, JSON.stringify(payload)),
      0,
      800,
    );
  }

  listPages(targetSlug: string): StoredCharacterPageSummary[] {
    return this.sqlite
      .query(
        `SELECT url, engine, status, title, content_hash as contentHash, fetched_at as fetchedAt
         FROM character_pages
         WHERE target_slug = ?
         ORDER BY fetched_at DESC`,
      )
      .all(targetSlug) as StoredCharacterPageSummary[];
  }

  listAssets(targetSlug: string): StoredCharacterAssetSummary[] {
    return this.sqlite
      .query(
        `SELECT source_url as sourceUrl, sha256, content_type as contentType,
                byte_size as byteSize, r2_key as r2Key, r2_url as r2Url,
                storage_mode as storageMode
         FROM character_assets
         WHERE target_slug = ?
         ORDER BY created_at DESC`,
      )
      .all(targetSlug) as StoredCharacterAssetSummary[];
  }

  searchIndex(
    term: string,
    options: {
      language?: string;
      sourceKind?: string;
      targetSlug?: string;
      limit?: number;
    } = {},
  ) {
    return this.oramaIndex.searchDocuments(term, options);
  }

  close(): void {
    if (this.persistOrama) {
      this.oramaIndex.persist();
    }
    this.sqlite.close();
  }
}
