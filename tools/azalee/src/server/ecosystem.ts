/**
 * Shared server helpers for saga games, community mods/wikis snapshots,
 * Material 3 design manifest, website locale dictionaries, and IEVR translation.
 */

import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { STACK_CONTRACT } from "@rosegriffon/config/contract";
import rgM3Manifest from "@rosegriffon/config/rg-m3.json";
import {
  getCommunityWikis,
  getModHubs,
  normalizeGameLocale,
  queryCommunityWikis,
  queryModHubs,
  type GameCommunityWikiEntry,
  type GameModHubEntry,
  type InazumaLocale,
} from "@rosegriffon/inagle/games";
import {
  loadIevrI18n,
  translateJp,
  type IevrI18nBundle,
  type TranslateTargetLang,
} from "@rosegriffon/inagle/text/ievr-i18n";

export interface IndexedCommunityMod {
  id: string;
  source: string;
  gameSlug: string;
  gameId: number | null;
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

export interface IndexedCommunityWikiPage {
  id?: string;
  url: string;
  title: string;
  lang: string;
  sourceKind: string;
  excerpt?: string;
}

export interface CommunityModsSnapshot {
  updatedAt: string | null;
  snapshotPath: string | null;
  mods: IndexedCommunityMod[];
  wikis: IndexedCommunityWikiPage[];
  curatedWikis: GameCommunityWikiEntry[];
  modHubs: GameModHubEntry[];
}

interface SnapshotFileShape {
  updatedAt?: string;
  mods?: IndexedCommunityMod[];
  wikis?: IndexedCommunityWikiPage[];
}

let cachedSnapshot: {
  expiresAt: number;
  updatedAt: string | null;
  path: string | null;
  mods: IndexedCommunityMod[];
  wikis: IndexedCommunityWikiPage[];
} | null = null;

let cachedRepoRoot: string | null | undefined;
let cachedIevrBundlePromise: Promise<IevrI18nBundle | null> | null = null;
const localeDictCache = new Map<string, Record<string, string>>();

function findUpwardForContract(startDir: string): string | null {
  let current = resolve(startDir);
  for (let depth = 0; depth < 8; depth++) {
    if (existsSync(join(current, "contract", "stack.yml"))) {
      return current;
    }
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return null;
}

function resolveModuleDir(): string | null {
  if (typeof import.meta.dir === "string") return import.meta.dir;
  if (
    typeof import.meta.url === "string" &&
    import.meta.url.startsWith("file://")
  ) {
    return dirname(fileURLToPath(import.meta.url));
  }
  return null;
}

export function resolveMonorepoRoot(): string | null {
  if (cachedRepoRoot !== undefined) return cachedRepoRoot;
  const moduleDir = resolveModuleDir();
  const candidates = [
    process.cwd(),
    ...(moduleDir ? [dirname(moduleDir)] : []),
  ];
  for (const start of candidates) {
    const found = findUpwardForContract(start);
    if (found) {
      cachedRepoRoot = found;
      return found;
    }
  }
  cachedRepoRoot = null;
  return null;
}

function resolveSnapshotPath(): string | null {
  const repoRoot = resolveMonorepoRoot();
  const candidates = [
    process.env.RG_CRAWL_DATA_ROOT
      ? join(process.env.RG_CRAWL_DATA_ROOT, "community-mods-index.json")
      : null,
    process.env.RG_CRON_SHARED_ROOT
      ? join(
          process.env.RG_CRON_SHARED_ROOT,
          "ie-crawl",
          "community-mods-index.json",
        )
      : null,
    repoRoot
      ? join(repoRoot, "data", "ie-crawl", "community-mods-index.json")
      : null,
    process.env.RG_SRV_ROOT
      ? join(
          process.env.RG_SRV_ROOT,
          "shared",
          "cron",
          "ie-crawl",
          "community-mods-index.json",
        )
      : null,
  ];
  for (const path of candidates) {
    if (path && existsSync(path)) return path;
  }
  return null;
}

export function loadCommunityModsSnapshot(
  localeInput?: string | null,
): CommunityModsSnapshot {
  const locale: InazumaLocale = normalizeGameLocale(localeInput);
  const now = Date.now();
  if (!cachedSnapshot || now >= cachedSnapshot.expiresAt) {
    const path = resolveSnapshotPath();
    let parsed: SnapshotFileShape | null = null;
    if (path) {
      try {
        parsed = JSON.parse(readFileSync(path, "utf8")) as SnapshotFileShape;
      } catch {
        parsed = null;
      }
    }
    cachedSnapshot = {
      expiresAt: now + 60_000,
      updatedAt: parsed?.updatedAt ?? null,
      path,
      mods: Array.isArray(parsed?.mods) ? parsed.mods : [],
      wikis: Array.isArray(parsed?.wikis) ? parsed.wikis : [],
    };
  }

  return {
    updatedAt: cachedSnapshot.updatedAt,
    snapshotPath: cachedSnapshot.path,
    mods: cachedSnapshot.mods,
    wikis: cachedSnapshot.wikis,
    curatedWikis: getCommunityWikis(undefined, undefined, locale),
    modHubs: getModHubs(undefined, locale),
  };
}

export function queryCommunityModsEndpoint(
  options: {
    gameSlug?: string | null;
    q?: string | null;
    limit?: number;
    locale?: string | null;
  } = {},
) {
  const locale = normalizeGameLocale(options.locale);
  const snapshot = loadCommunityModsSnapshot(locale);
  const limit = Math.max(1, Math.min(options.limit ?? 50, 500));
  const query = options.q?.trim().toLowerCase() || "";
  const gameFilter = options.gameSlug?.trim().toLowerCase() || "";

  const mods = snapshot.mods
    .filter((mod) => {
      if (
        gameFilter &&
        mod.gameSlug.toLowerCase() !== gameFilter &&
        !mod.gameTitle.toLowerCase().includes(gameFilter)
      ) {
        return false;
      }
      if (!query) return true;
      return [
        mod.name,
        mod.author ?? "",
        mod.category,
        mod.gameTitle,
        mod.gameSlug,
        mod.description ?? "",
      ]
        .join(" ")
        .toLowerCase()
        .includes(query);
    })
    .slice(0, limit);

  const hubs = queryModHubs({
    gameSlug: options.gameSlug,
    q: options.q,
    limit,
    locale,
  });

  return {
    schemaVersion: 1,
    locale,
    updatedAt: snapshot.updatedAt,
    totalMods: mods.length,
    totalHubs: hubs.length,
    mods,
    hubs,
  };
}

export function queryCommunityWikisEndpoint(
  options: {
    lang?: string | null;
    gameSlug?: string | null;
    q?: string | null;
    limit?: number;
    locale?: string | null;
  } = {},
) {
  const locale = normalizeGameLocale(options.locale);
  const snapshot = loadCommunityModsSnapshot(locale);
  const limit = Math.max(1, Math.min(options.limit ?? 50, 500));
  const query = options.q?.trim().toLowerCase() || "";
  const langFilter = options.lang?.trim()
    ? normalizeGameLocale(options.lang)
    : null;

  const pages = snapshot.wikis
    .filter((page) => {
      if (langFilter && page.lang.toLowerCase() !== langFilter) return false;
      if (!query) return true;
      return [page.title, page.url, page.sourceKind, page.excerpt ?? ""]
        .join(" ")
        .toLowerCase()
        .includes(query);
    })
    .slice(0, limit);

  const curated = queryCommunityWikis({
    lang: options.lang,
    gameSlug: options.gameSlug,
    q: options.q,
    limit,
    locale,
  });

  return {
    schemaVersion: 1,
    locale,
    updatedAt: snapshot.updatedAt,
    totalPages: pages.length,
    totalSources: curated.length,
    sources: curated,
    pages,
  };
}

export function getDesignThemesManifest() {
  return {
    schemaVersion: rgM3Manifest.schemaVersion,
    generator: rgM3Manifest.generator,
    variant: rgM3Manifest.variant,
    contrast: rgM3Manifest.contrast,
    defaultTheme: rgM3Manifest.defaultTheme,
    framework: STACK_CONTRACT.ui.framework,
    primitives: STACK_CONTRACT.ui.primitives,
    themes: rgM3Manifest.themes,
  };
}

/**
 * Dictionnaire d'interface du site pour une langue (`apps/rg-website/src/lib/i18n`).
 *
 * Les textes du jeu ne sont pas servis ici : ils vivent dans `game-text` et
 * sortent par `/api/text`. `scope` est conservé pour les clients existants, les
 * deux valeurs rendent le dictionnaire d'interface.
 */
export function loadWebsiteLocaleDictionary(
  langInput: string,
  scope?: string | null,
): {
  lang: InazumaLocale;
  scope: "all" | "interface";
  keyCount: number;
  dictionary: Record<string, string>;
} | null {
  const raw = langInput.trim().toLowerCase();
  if (!(STACK_CONTRACT.project.locales as readonly string[]).includes(raw)) {
    return null;
  }
  const lang = raw as InazumaLocale;
  let dictionary = localeDictCache.get(lang);
  if (!dictionary) {
    const repoRoot = resolveMonorepoRoot();
    const candidates = [
      repoRoot
        ? join(
            repoRoot,
            "apps",
            "rg-website",
            "src",
            "lib",
            "i18n",
            `${lang}-interface.json`,
          )
        : null,
      join(process.cwd(), "src", "lib", "i18n", `${lang}-interface.json`),
    ];
    const filePath = candidates.find((p) => p && existsSync(p));
    if (!filePath) return null;
    dictionary = JSON.parse(readFileSync(filePath, "utf8")) as Record<
      string,
      string
    >;
    localeDictCache.set(lang, dictionary);
  }

  return {
    lang,
    scope: scope?.trim().toLowerCase() === "interface" ? "interface" : "all",
    keyCount: Object.keys(dictionary).length,
    dictionary,
  };
}

async function resolveSharedIevrBundle(): Promise<IevrI18nBundle | null> {
  if (!cachedIevrBundlePromise) {
    cachedIevrBundlePromise = (async () => {
      const direct = await loadIevrI18n();
      if (direct) return direct;
      const repoRoot = resolveMonorepoRoot();
      if (!repoRoot) return null;
      return loadIevrI18n(
        join(repoRoot, "packages", "inagle", "data", "ievr-i18n.json"),
      );
    })();
  }
  return cachedIevrBundlePromise;
}

export async function translateIevrQuery(options: {
  text: string;
  targetLang?: string | null;
  skipRemote?: boolean;
}) {
  const target = normalizeGameLocale(options.targetLang);
  const bundle = await resolveSharedIevrBundle();

  if (target === "ja") {
    const analysis = await translateJp(options.text, "en", {
      bundle,
      skipRemote: true,
    });
    return {
      ...analysis,
      targetLang: "ja" as const,
      translated: analysis.source,
    };
  }

  return translateJp(options.text, target as TranslateTargetLang, {
    bundle,
    skipRemote: options.skipRemote,
  });
}
