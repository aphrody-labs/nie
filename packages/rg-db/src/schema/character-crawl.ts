import type { Generated } from "kysely";

export type CrawlEngineKind =
  | "mediawiki-api"
  | "impit"
  | "patchright"
  | "crawlee-cheerio"
  | "crawlee-playwright"
  | "crawlee-fetch"
  | "obscura"
  | "lightpanda"
  | "agent-browser"
  | "zukan-inagle"
  | "google-news-rss"
  | "google-cse"
  | "duckduckgo"
  | "gamebanana-api"
  | "inazuma-mods-api"
  | "atwiki"
  | "game8";

export type CrawlPageStatus = "ok" | "challenged" | "error" | "redirect";

export interface CharacterCrawlTargetTable {
  slug: string;
  canonical_name: string;
  localized_names: Record<string, string>;
  aliases: string[];
  zukan_ids: string[];
  seed_urls: string[];
  search_queries: string[];
  crawl_interval_sec: Generated<number>;
  enabled: Generated<boolean>;
  last_crawled_at: Date | string | null;
  last_summary: Record<string, unknown>;
  created_at: Generated<Date | string>;
  updated_at: Generated<Date | string>;
}

export interface CharacterCrawlPageTable {
  id: Generated<number>;
  target_slug: string;
  url: string;
  normalized_url: string;
  engine: CrawlEngineKind | string;
  source_kind: string;
  status: CrawlPageStatus | string;
  http_status: number | null;
  title: string | null;
  language: string | null;
  content_hash: string | null;
  excerpt: string | null;
  body_text: string | null;
  metadata: Record<string, unknown>;
  discovered_links: string[];
  r2_snapshot_key: string | null;
  r2_snapshot_url: string | null;
  depth: Generated<number>;
  parent_url: string | null;
  worker_host: string | null;
  worker_id: string | null;
  fetched_at: Generated<Date | string>;
  updated_at: Generated<Date | string>;
}

export interface CharacterCrawlAssetTable {
  id: Generated<number>;
  target_slug: string;
  page_url: string | null;
  source_url: string;
  sha256: string;
  content_type: string;
  byte_size: number;
  r2_bucket: string | null;
  r2_key: string;
  r2_url: string | null;
  storage_mode: "r2" | "rclone" | "local-staging" | string;
  metadata: Record<string, unknown>;
  created_at: Generated<Date | string>;
  updated_at: Generated<Date | string>;
}
