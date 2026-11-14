/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Multilingual BM25 + faceted full-text search index powered by `@orama/orama` (`3.1.18`).
 * Used alongside SQLite, Redis, and R2 in `ie-crawl` / `character-store` so crawled
 * characters, community wikis (FR/EN/JA/ES), and community mods are instantly searchable.
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import {
  type AnyOrama,
  create,
  insert,
  load,
  save,
  search,
  update,
} from "@orama/orama";
import { cronSharedPath } from "./shared-path";

export interface IndexedCrawlDocument {
  id: string;
  targetSlug: string;
  url: string;
  normalizedUrl: string;
  title: string;
  excerpt: string;
  bodyText: string;
  language: string;
  sourceKind: string;
  engine: string;
  depth: number;
  tags: string[];
  fetchedAt: number;
}

const ORAMA_SCHEMA = {
  id: "string",
  targetSlug: "string",
  url: "string",
  normalizedUrl: "string",
  title: "string",
  excerpt: "string",
  bodyText: "string",
  language: "string",
  sourceKind: "string",
  engine: "string",
  depth: "number",
  tags: "string[]",
  fetchedAt: "number",
} as const;

export function resolveOramaIndexPath(customPath?: string): string {
  return (
    customPath ??
    Bun.env.RG_ORAMA_INDEX_PATH ??
    join(cronSharedPath("indexes"), "ie-crawl-orama.json")
  );
}

export class CrawlOramaIndex {
  private readonly dbPath: string;
  private readonly indexedIds = new Set<string>();
  private readonly db: AnyOrama;

  constructor(indexPath?: string) {
    this.dbPath =
      indexPath === ":memory:" ? ":memory:" : resolveOramaIndexPath(indexPath);
    this.db = create({ schema: ORAMA_SCHEMA });
    if (this.dbPath !== ":memory:" && existsSync(this.dbPath)) {
      try {
        const raw = JSON.parse(readFileSync(this.dbPath, "utf8"));
        load(this.db, raw);
        const docs = raw?.docs?.docs;
        if (docs && typeof docs === "object") {
          for (const key of Object.keys(docs)) {
            this.indexedIds.add(key);
          }
        }
      } catch {
        // Start clean if persisted snapshot is corrupt or schema-mismatched.
      }
    }
  }

  indexDocument(doc: IndexedCrawlDocument): void {
    const payload = {
      id: doc.id,
      targetSlug: doc.targetSlug,
      url: doc.url,
      normalizedUrl: doc.normalizedUrl,
      title: doc.title,
      excerpt: doc.excerpt,
      bodyText: doc.bodyText.slice(0, 24_000),
      language: doc.language || "und",
      sourceKind: doc.sourceKind || "web",
      engine: doc.engine,
      depth: doc.depth,
      tags: doc.tags,
      fetchedAt: doc.fetchedAt,
    };

    if (this.indexedIds.has(doc.id)) {
      update(this.db, doc.id, payload);
      return;
    }
    insert(this.db, payload);
    this.indexedIds.add(doc.id);
  }

  searchDocuments(
    term: string,
    options: {
      language?: string;
      sourceKind?: string;
      targetSlug?: string;
      limit?: number;
    } = {},
  ): Array<{ score: number; document: IndexedCrawlDocument }> {
    const where: Record<string, string> = {};
    if (options.language) where.language = options.language;
    if (options.sourceKind) where.sourceKind = options.sourceKind;
    if (options.targetSlug) where.targetSlug = options.targetSlug;

    const results = search(this.db, {
      term,
      limit: options.limit ?? 20,
      ...(Object.keys(where).length > 0 ? { where } : {}),
      boost: {
        title: 2.5,
        excerpt: 1.5,
        tags: 2,
      },
    }) as {
      hits: Array<{ score: number; document: IndexedCrawlDocument }>;
    };

    return results.hits.map((hit) => ({
      score: hit.score,
      document: hit.document,
    }));
  }

  persist(): string {
    mkdirSync(dirname(this.dbPath), { recursive: true });
    const raw = save(this.db);
    writeFileSync(this.dbPath, JSON.stringify(raw));
    return this.dbPath;
  }

  count(): number {
    return this.indexedIds.size;
  }
}
