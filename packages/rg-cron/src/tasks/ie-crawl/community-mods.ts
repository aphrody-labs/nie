/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { CrawlEngineKind } from "@rosegriffon/db/schema";
import { CharacterCrawlStore } from "../../lib/character-store";
import {
  type CrawleeBatchItemResult,
  runCrawleeCheerioBatch,
} from "../../lib/crawlee-runner";
import { impitFetchText } from "../../lib/impit-client";
import { CrawlOramaIndex } from "../../lib/orama-index";
import { downloadAndMirrorAsset } from "../../lib/r2-assets";
import { cronSharedPath } from "../../lib/shared-path";

export interface GameBananaGameRef {
  readonly id: number;
  readonly gameSlug: string;
  readonly name: string;
}

/** All 14 live-verified GameBanana Inazuma Eleven game IDs. */
export const GAMEBANANA_INAZUMA_GAMES: readonly GameBananaGameRef[] = [
  {
    id: 20069,
    gameSlug: "victory-road",
    name: "Inazuma Eleven: Victory Road (PC)",
  },
  {
    id: 20068,
    gameSlug: "victory-road",
    name: "Inazuma Eleven: Victory Road (Switch)",
  },
  {
    id: 15421,
    gameSlug: "go-strikers-2013",
    name: "Inazuma Eleven GO Strikers 2013",
  },
  { id: 19165, gameSlug: "go-galaxy", name: "Inazuma Eleven GO Galaxy" },
  {
    id: 19164,
    gameSlug: "go-chrono-stones",
    name: "Inazuma Eleven GO 2: Chrono Stone",
  },
  { id: 19163, gameSlug: "go", name: "Inazuma Eleven GO Shadow" },
  { id: 19162, gameSlug: "go", name: "Inazuma Eleven GO Light" },
  { id: 19155, gameSlug: "ie1", name: "Inazuma Eleven" },
  {
    id: 24334,
    gameSlug: "strikers-2012-xtreme",
    name: "Inazuma Eleven Strikers 2012 Xtreme",
  },
  { id: 19160, gameSlug: "ie3", name: "Inazuma Eleven 3: Team Ogre Attacks!" },
  { id: 19159, gameSlug: "ie3", name: "Inazuma Eleven 3: Bomb Blast" },
  { id: 19158, gameSlug: "ie3", name: "Inazuma Eleven 3: Lightning Bolt" },
  { id: 19157, gameSlug: "ie2", name: "Inazuma Eleven 2: Blizzard" },
  { id: 19156, gameSlug: "ie2", name: "Inazuma Eleven 2: Firestorm" },
] as const;

export const GAMEBANANA_TOOL_IEVR_MANAGER =
  "https://gamebanana.com/tools/21354";
export const INAZUMA_MODS_API_URL = "https://inazuma-eleven.fr/mods/api/mods";
export const TRADUSQUARE_GALAXY_URL =
  "https://tradusquare.es/proyectos/inazuma-eleven-go-galaxy/";

export const COMMUNITY_FANDOM_OPENSEARCH = [
  {
    id: "fandom-en",
    lang: "en",
    apiEndpoint: "https://inazuma-eleven.fandom.com/api.php",
    search: "Inazuma Eleven",
  },
  {
    id: "fandom-fr",
    lang: "fr",
    apiEndpoint: "https://inazuma-eleven.fandom.com/fr/api.php",
    search: "Inazuma Eleven",
  },
  {
    id: "fandom-es",
    lang: "es",
    apiEndpoint: "https://inazuma.fandom.com/es/api.php",
    search: "Inazuma Eleven",
  },
] as const;

export const COMMUNITY_HTML_WIKI_URLS = [
  { url: "https://wikiwiki.jp/inazuma/", lang: "ja", sourceKind: "wiki" },
  {
    url: "https://game8.jp/inazuma-victory-road",
    lang: "ja",
    sourceKind: "guide",
  },
  { url: "https://w.atwiki.jp/inazuma3/", lang: "ja", sourceKind: "wiki" },
  { url: "https://w.atwiki.jp/inazumago/", lang: "ja", sourceKind: "wiki" },
  { url: "https://w.atwiki.jp/inazumago2cs/", lang: "ja", sourceKind: "wiki" },
  { url: "https://w.atwiki.jp/inazumago3/", lang: "ja", sourceKind: "wiki" },
  {
    url: "https://www.guiasnintendo.com/0_NINTENDO_DS/inazuma_eleven/inazuma_eleven_sp/welcome.html",
    lang: "es",
    sourceKind: "guide",
  },
  {
    url: TRADUSQUARE_GALAXY_URL,
    lang: "es",
    sourceKind: "translation",
  },
  {
    url: GAMEBANANA_TOOL_IEVR_MANAGER,
    lang: "en",
    sourceKind: "mod-tool",
  },
] as const;

export interface CommunityModRecord {
  id: string;
  source: "gamebanana" | "inazuma-mods" | "tradusquare";
  gameSlug: string;
  gameId?: number;
  gameTitle: string;
  name: string;
  author: string | null;
  category: string;
  url: string;
  thumbnailUrl: string | null;
  downloads: number | null;
  likes: number | null;
  updatedAt: number | null;
  description: string | null;
}

export interface CommunityWikiPageRecord {
  url: string;
  lang: string;
  title: string;
  sourceKind: string;
  engine: CrawlEngineKind;
  excerpt: string;
}

export type FetchTextFn = (url: string) => Promise<{
  ok: boolean;
  status: number;
  text: string;
}>;

export type CrawleeBatchFn = (
  urls: readonly string[],
) => Promise<CrawleeBatchItemResult[]>;

export function normalizeGameBananaSubfeed(
  game: GameBananaGameRef,
  rawPayload: unknown,
): CommunityModRecord[] {
  if (!rawPayload || typeof rawPayload !== "object") return [];
  const obj = rawPayload as Record<string, unknown>;
  const rawRecords = Array.isArray(obj._aRecords)
    ? obj._aRecords
    : Array.isArray(rawPayload)
      ? rawPayload
      : [];

  const mods: CommunityModRecord[] = [];
  for (const item of rawRecords) {
    if (!item || typeof item !== "object") continue;
    const rec = item as Record<string, unknown>;
    const rawId = rec._idRow ?? rec.id;
    const name = typeof rec._sName === "string" ? rec._sName.trim() : "";
    const url =
      typeof rec._sProfileUrl === "string" ? rec._sProfileUrl.trim() : "";
    if (!name || !url) continue;

    const submitter = rec._aSubmitter as Record<string, unknown> | undefined;
    const media = rec._aPreviewMedia as Record<string, unknown> | undefined;
    const images = Array.isArray(media?._aImages)
      ? (media!._aImages as Array<Record<string, unknown>>)
      : [];
    const firstImg = images[0];
    const thumb =
      firstImg &&
      typeof firstImg._sBaseUrl === "string" &&
      typeof firstImg._sFile === "string"
        ? `${firstImg._sBaseUrl}/${firstImg._sFile}`
        : null;

    const tsRaw = Number(rec._tsDateModified ?? rec._tsDateAdded ?? 0);
    mods.push({
      id: `gb:${game.id}:${String(rawId ?? name)}`,
      source: "gamebanana",
      gameSlug: game.gameSlug,
      gameId: game.id,
      gameTitle: game.name,
      name,
      author: typeof submitter?._sName === "string" ? submitter._sName : null,
      category: typeof rec._sModelName === "string" ? rec._sModelName : "Mod",
      url,
      thumbnailUrl: thumb,
      downloads:
        typeof rec._nDownloadCount === "number" ? rec._nDownloadCount : null,
      likes: typeof rec._nLikeCount === "number" ? rec._nLikeCount : null,
      updatedAt: Number.isFinite(tsRaw) && tsRaw > 0 ? tsRaw * 1000 : null,
      description:
        typeof rec._sText === "string" ? rec._sText.slice(0, 400) : null,
    });
  }
  return mods;
}

export function normalizeInazumaModsPayload(
  rawPayload: unknown,
): CommunityModRecord[] {
  const list = Array.isArray(rawPayload)
    ? rawPayload
    : rawPayload && typeof rawPayload === "object"
      ? ((rawPayload as Record<string, unknown>).mods ??
        (rawPayload as Record<string, unknown>).items ??
        (rawPayload as Record<string, unknown>).data ??
        [])
      : [];
  if (!Array.isArray(list)) return [];

  const mods: CommunityModRecord[] = [];
  for (const item of list) {
    if (!item || typeof item !== "object") continue;
    const rec = item as Record<string, unknown>;
    const rawId = rec.id ?? rec.slug ?? rec.uuid;
    const name = String(rec.title ?? rec.name ?? "").trim();
    if (!name) continue;
    const slug = String(rec.slug ?? rawId ?? name);
    const url =
      typeof rec.url === "string" && rec.url.startsWith("http")
        ? rec.url
        : `https://inazuma-eleven.fr/mods/${encodeURIComponent(slug)}`;

    const rawThumb = rec.thumbnail ?? rec.image ?? rec.coverUrl ?? rec.icon;
    const thumbnailUrl =
      typeof rawThumb === "string" && rawThumb.startsWith("http")
        ? rawThumb
        : null;

    mods.push({
      id: `iemods:${slug}`,
      source: "inazuma-mods",
      gameSlug: String(rec.game ?? rec.gameSlug ?? "victory-road"),
      gameTitle: String(rec.gameName ?? "Inazuma Eleven: Victory Road"),
      name,
      author:
        typeof rec.author === "string"
          ? rec.author
          : typeof rec.creator === "string"
            ? rec.creator
            : null,
      category: String(rec.category ?? rec.type ?? "Mod"),
      url,
      thumbnailUrl,
      downloads: typeof rec.downloads === "number" ? rec.downloads : null,
      likes: typeof rec.likes === "number" ? rec.likes : null,
      updatedAt: null,
      description:
        typeof rec.description === "string"
          ? rec.description.slice(0, 400)
          : null,
    });
  }
  return mods;
}

export interface CommunityModsCrawlOptions {
  fetchText?: FetchTextFn;
  crawleeBatch?: CrawleeBatchFn;
  sqlitePath?: string;
  oramaPath?: string;
  outputDir?: string;
  r2StagingDir?: string;
  mirrorAssets?: boolean;
  maxAssets?: number;
  maxGameBananaGames?: number;
}

export interface CommunityModsCrawlResult {
  success: boolean;
  modsCount: number;
  wikisCount: number;
  assetsMirrored: number;
  oramaCount: number;
  snapshotPath: string;
  errors: string[];
}

export async function runCommunityModsCrawl(
  options: CommunityModsCrawlOptions = {},
): Promise<CommunityModsCrawlResult> {
  const fetchText: FetchTextFn =
    options.fetchText ??
    (async (url: string) => {
      const res = await impitFetchText(url, {
        headers: { accept: "application/json,text/html" },
        timeoutMs: 15_000,
      });
      return { ok: res.ok, status: res.status, text: res.text };
    });

  const crawleeBatch: CrawleeBatchFn =
    options.crawleeBatch ??
    ((urls: readonly string[]) =>
      runCrawleeCheerioBatch(urls, {
        maxConcurrency: 4,
        maxRequestsPerCrawl: urls.length,
        timeoutSecs: 20,
      }));

  const store = new CharacterCrawlStore(options.sqlitePath ?? ":memory:");
  const orama = new CrawlOramaIndex(options.oramaPath ?? ":memory:");
  const outputDir = options.outputDir ?? cronSharedPath("ie-crawl");
  const errors: string[] = [];
  const mods: CommunityModRecord[] = [];
  const wikis: CommunityWikiPageRecord[] = [];

  await store.upsertTarget({
    slug: "community-mods",
    canonicalName: "Inazuma Eleven Community Mods & Wikis",
    localizedNames: {
      en: "Inazuma Eleven Community Mods & Wikis",
      fr: "Mods et Wikis Communautaires Inazuma Eleven",
      es: "Mods y Wikis Comunitarios de Inazuma Eleven",
      ja: "イナズマイレブン コミュニティMOD・攻略Wiki",
    },
    aliases: ["community:mods", "gamebanana", "inazuma-mods", "tradusquare"],
    zukanIds: [],
    zukanHashes: [],
    seedUrls: [
      INAZUMA_MODS_API_URL,
      GAMEBANANA_TOOL_IEVR_MANAGER,
      ...COMMUNITY_HTML_WIKI_URLS.map((w) => w.url),
    ],
    searchQueries: ["Inazuma Eleven mods", "Victory Road mods"],
  });

  // 1. inazuma-eleven.fr/mods/api/mods
  try {
    const res = await fetchText(INAZUMA_MODS_API_URL);
    if (res.ok) {
      const parsed = JSON.parse(res.text) as unknown;
      mods.push(...normalizeInazumaModsPayload(parsed));
    } else {
      errors.push(`inazuma-mods HTTP ${res.status}`);
    }
  } catch (err) {
    errors.push(
      `inazuma-mods: ${err instanceof Error ? err.message : String(err)}`,
    );
  }

  // 2. GameBanana all 14 Inazuma Eleven titles
  const gamesToQuery = GAMEBANANA_INAZUMA_GAMES.slice(
    0,
    options.maxGameBananaGames ?? GAMEBANANA_INAZUMA_GAMES.length,
  );
  for (const game of gamesToQuery) {
    const apiUrl = `https://gamebanana.com/apiv11/Game/${game.id}/Subfeed?_nPage=1&_nPerpage=15`;
    try {
      const res = await fetchText(apiUrl);
      if (!res.ok) {
        errors.push(`gamebanana:${game.id} HTTP ${res.status}`);
        continue;
      }
      const parsed = JSON.parse(res.text) as unknown;
      mods.push(...normalizeGameBananaSubfeed(game, parsed));
    } catch (err) {
      errors.push(
        `gamebanana:${game.id}: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  }

  // Persist mods to CharacterCrawlStore + Orama
  for (const mod of mods) {
    await store.savePage("community-mods", {
      url: mod.url,
      normalizedUrl: mod.url,
      engine:
        mod.source === "gamebanana" ? "gamebanana-api" : "inazuma-mods-api",
      sourceKind: mod.source,
      status: "ok",
      httpStatus: 200,
      title: mod.name,
      language: mod.source === "tradusquare" ? "es" : "en",
      excerpt: mod.description ?? `${mod.gameTitle} (${mod.category})`,
      bodyText:
        `${mod.name} ${mod.gameTitle} ${mod.author ?? ""} ${mod.description ?? ""}`.trim(),
      discoveredLinks: [mod.url],
      discoveredAssets: mod.thumbnailUrl ? [mod.thumbnailUrl] : [],
      metadata: {
        gameId: mod.gameId,
        gameSlug: mod.gameSlug,
        category: mod.category,
      },
    });

    orama.indexDocument({
      id: mod.id,
      targetSlug: "community-mods",
      url: mod.url,
      normalizedUrl: mod.url,
      title: mod.name,
      excerpt: mod.description ?? mod.gameTitle,
      bodyText: `${mod.name} ${mod.gameTitle} ${mod.author ?? ""} ${mod.category} ${mod.description ?? ""}`,
      language: mod.source === "tradusquare" ? "es" : "en",
      sourceKind: mod.source,
      engine:
        mod.source === "gamebanana" ? "gamebanana-api" : "inazuma-mods-api",
      depth: 0,
      tags: ["mod", mod.source, mod.gameSlug],
      fetchedAt: Date.now(),
    });
  }

  // 3. Fandom OpenSearch (EN, FR, ES)
  for (const wiki of COMMUNITY_FANDOM_OPENSEARCH) {
    const apiUrl = `${wiki.apiEndpoint}?action=opensearch&search=${encodeURIComponent(wiki.search)}&limit=8&format=json`;
    try {
      const res = await fetchText(apiUrl);
      if (!res.ok) continue;
      const payload = JSON.parse(res.text) as [
        string?,
        string[]?,
        string[]?,
        string[]?,
      ];
      const titles = payload[1] ?? [];
      const snippets = payload[2] ?? [];
      const urls = payload[3] ?? [];
      for (let i = 0; i < urls.length; i++) {
        const pageUrl = urls[i];
        const title = titles[i] ?? pageUrl ?? "";
        if (!pageUrl) continue;
        wikis.push({
          url: pageUrl,
          lang: wiki.lang,
          title,
          sourceKind: "wiki",
          engine: "mediawiki-api",
          excerpt: snippets[i] ?? "",
        });
        orama.indexDocument({
          id: `${wiki.id}:${pageUrl}`,
          targetSlug: "community-mods",
          url: pageUrl,
          normalizedUrl: pageUrl,
          title,
          excerpt: snippets[i] ?? title,
          bodyText: `${title} ${snippets[i] ?? ""}`,
          language: wiki.lang,
          sourceKind: "wiki",
          engine: "mediawiki-api",
          depth: 0,
          tags: ["wiki", wiki.lang, "fandom"],
          fetchedAt: Date.now(),
        });
      }
    } catch (err) {
      errors.push(
        `wiki:${wiki.id}: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  }

  // 4. Crawlee Cheerio + Impit across Japanese wikis, Spanish guides & TraduSquare
  try {
    const batchResults = await crawleeBatch(
      COMMUNITY_HTML_WIKI_URLS.map((item) => item.url),
    );
    const metaByUrl = new Map<
      string,
      (typeof COMMUNITY_HTML_WIKI_URLS)[number]
    >(COMMUNITY_HTML_WIKI_URLS.map((item) => [item.url, item]));
    for (const item of batchResults) {
      const meta = metaByUrl.get(item.url);
      const lang = meta?.lang ?? item.language ?? "und";
      const sourceKind = meta?.sourceKind ?? "wiki";
      const title = item.title ?? item.url;
      const excerpt = item.text.slice(0, 320);

      if (item.status >= 200 && item.status < 400) {
        wikis.push({
          url: item.finalUrl || item.url,
          lang,
          title,
          sourceKind,
          engine: item.engine,
          excerpt,
        });
        await store.savePage("community-mods", {
          url: item.url,
          normalizedUrl: item.finalUrl || item.url,
          engine: item.engine,
          sourceKind,
          status: "ok",
          httpStatus: item.status,
          title,
          language: lang,
          excerpt,
          bodyText: item.text.slice(0, 12_000),
          discoveredLinks: item.links.slice(0, 20),
          discoveredAssets: item.assets.slice(0, 10),
          metadata: { sourceKind },
        });
        orama.indexDocument({
          id: `crawlee:${item.url}`,
          targetSlug: "community-mods",
          url: item.url,
          normalizedUrl: item.finalUrl || item.url,
          title,
          excerpt,
          bodyText: item.text.slice(0, 12_000),
          language: lang,
          sourceKind,
          engine: item.engine,
          depth: 0,
          tags: [sourceKind, lang],
          fetchedAt: Date.now(),
        });
      }
    }
  } catch (err) {
    errors.push(
      `crawlee-wikis: ${err instanceof Error ? err.message : String(err)}`,
    );
  }

  // 5. Optional R2 thumbnail staging
  let assetsMirrored = 0;
  if (options.mirrorAssets) {
    const thumbUrls = [
      ...new Set(
        mods.map((m) => m.thumbnailUrl).filter((u): u is string => Boolean(u)),
      ),
    ].slice(0, options.maxAssets ?? 12);
    for (const assetUrl of thumbUrls) {
      try {
        const uploaded = await downloadAndMirrorAsset(
          "community-mods",
          assetUrl,
          { stagingDir: options.r2StagingDir },
        );
        if (uploaded) {
          await store.saveAsset("community-mods", null, uploaded);
          assetsMirrored += 1;
        }
      } catch {
        // Non-fatal individual thumbnail mirror error.
      }
    }
  }

  mkdirSync(outputDir, { recursive: true });
  const snapshotPath = join(outputDir, "community-mods-index.json");
  writeFileSync(
    snapshotPath,
    JSON.stringify(
      {
        updatedAt: new Date().toISOString(),
        gameBananaGames: GAMEBANANA_INAZUMA_GAMES,
        modsCount: mods.length,
        wikisCount: wikis.length,
        mods,
        wikis,
      },
      null,
      2,
    ),
    "utf8",
  );

  if (options.oramaPath && options.oramaPath !== ":memory:") {
    orama.persist();
  }
  const oramaCount = orama.count();
  store.close();

  return {
    success: mods.length > 0 || wikis.length > 0 || errors.length === 0,
    modsCount: mods.length,
    wikisCount: wikis.length,
    assetsMirrored,
    oramaCount,
    snapshotPath,
    errors,
  };
}

export async function crawlCommunityMods(): Promise<{
  success: boolean;
  count?: number;
  error?: string;
}> {
  const res = await runCommunityModsCrawl();
  return {
    success: res.success,
    count: res.modsCount + res.wikisCount,
    error:
      res.errors.length > 0 ? res.errors.slice(0, 3).join("; ") : undefined,
  };
}
