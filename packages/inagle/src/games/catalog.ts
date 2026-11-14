/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  INAZUMA_ERAS_GENERATED,
  INAZUMA_GAMES_GENERATED,
} from "./catalog.generated";
import type {
  InazumaCommunityWiki,
  InazumaEraInfo,
  InazumaGame,
  InazumaGameEra,
  InazumaGameStatus,
  InazumaLocale,
  InazumaModHub,
} from "./types";

export const SUPPORTED_GAME_LOCALES: readonly InazumaLocale[] = [
  "fr",
  "en",
  "ja",
  "es",
];

/** Canonical eras synchronized from `crates/nie-pkg/registry/games.toml`. */
export const INAZUMA_ERAS: readonly InazumaEraInfo[] = INAZUMA_ERAS_GENERATED;

export const GAME_STATUS_LABELS: Readonly<
  Record<
    InazumaGameStatus,
    {
      labelFr: string;
      labels: Readonly<Record<InazumaLocale, string>>;
      tone: "released" | "dev" | "muted";
    }
  >
> = {
  released: {
    labelFr: "Sorti / Disponible",
    labels: {
      fr: "Sorti / Disponible",
      en: "Released / Available",
      ja: "発売中・配信中",
      es: "Disponible",
    },
    tone: "released",
  },
  "in-development": {
    labelFr: "En développement",
    labels: {
      fr: "En développement",
      en: "In Development",
      ja: "開発中",
      es: "En desarrollo",
    },
    tone: "dev",
  },
  discontinued: {
    labelFr: "Service clos",
    labels: {
      fr: "Service clos",
      en: "Service Ended",
      ja: "サービス終了",
      es: "Servicio cerrado",
    },
    tone: "muted",
  },
  "cancelled-evolved": {
    labelFr: "Transformé en Victory Road",
    labels: {
      fr: "Transformé en Victory Road",
      en: "Evolved into Victory Road",
      ja: "ヴィクトリーロードへ統合",
      es: "Transformado en Victory Road",
    },
    tone: "muted",
  },
};

/**
 * Complete chronological catalog of Inazuma Eleven titles generated from
 * `crates/nie-pkg/registry/games.toml` via `bun run contract:sync`.
 */
export const INAZUMA_GAMES: readonly InazumaGame[] = INAZUMA_GAMES_GENERATED;

export function normalizeGameLocale(locale?: string | null): InazumaLocale {
  const code = locale?.trim().toLowerCase().slice(0, 2);
  if (code === "en" || code === "ja" || code === "es" || code === "fr") {
    return code;
  }
  return "fr";
}

export function getGameBySlug(slug: string): InazumaGame | undefined {
  return INAZUMA_GAMES.find(
    (g) => g.slug === slug || g.id === slug || g.aliases?.includes(slug),
  );
}

export function getGamesByEra(era: InazumaGameEra): InazumaGame[] {
  return INAZUMA_GAMES.filter((g) => g.era === era);
}

export function getGamesByStatus(status: InazumaGameStatus): InazumaGame[] {
  return INAZUMA_GAMES.filter((g) => g.status === status);
}

export function getDbSeriesKeysForGame(game: InazumaGame): string[] {
  if (game.relatedSeriesKeys?.length) {
    return [...game.relatedSeriesKeys];
  }
  return game.seriesFilterKey ? [game.seriesFilterKey] : [];
}

export function getLocalizedEra(
  era: InazumaEraInfo,
  locale: InazumaLocale = "fr",
) {
  return {
    id: era.id,
    label: era.labels[locale] ?? era.labelFr,
    description: era.descriptions[locale] ?? era.descriptionFr,
  };
}

export function getLocalizedStatusLabel(
  status: InazumaGameStatus,
  locale: InazumaLocale = "fr",
) {
  const meta = GAME_STATUS_LABELS[status];
  return {
    label: meta.labels[locale] ?? meta.labelFr,
    tone: meta.tone,
  };
}

export function getLocalizedGame(
  game: InazumaGame,
  locale: InazumaLocale = "fr",
) {
  return {
    id: game.id,
    slug: game.slug,
    order: game.order,
    series: game.series,
    era: game.era,
    status: game.status,
    title: game.titles[locale] ?? game.titleFr,
    statusDetail: game.statusDetail[locale] ?? game.statusDetailFr,
    summary: game.summaries[locale] ?? game.summaryFr,
    highlights: game.highlights[locale] ?? game.highlightsFr,
    internalLinks: game.internalLinks.map((link) => ({
      href: link.href,
      label: link.labels?.[locale] ?? link.label,
    })),
    platforms: game.platforms,
    developer: game.developer,
    protagonist: game.protagonist,
    releaseYear: game.releaseYear,
    releaseDateJa: game.releaseDateJa,
    releaseDateEu: game.releaseDateEu,
    thumbnail: game.thumbnail,
    cover: game.cover,
    officialUrl: game.officialUrl,
    storeUrl: game.storeUrl,
    seriesFilterKey: game.seriesFilterKey,
    relatedSeriesKeys: game.relatedSeriesKeys,
    communityWikis: game.communityWikis,
    modHubs: game.modHubs,
    features: game.features,
  };
}

export interface GameCommunityWikiEntry extends InazumaCommunityWiki {
  gameId: string;
  gameSlug: string;
  gameTitle: string;
}

export function getCommunityWikis(
  locale?: InazumaLocale,
  gameSlug?: string,
  catalogLocale: InazumaLocale = "fr",
): GameCommunityWikiEntry[] {
  const out: GameCommunityWikiEntry[] = [];
  for (const game of INAZUMA_GAMES) {
    if (gameSlug && game.slug !== gameSlug && game.id !== gameSlug) continue;
    for (const wiki of game.communityWikis) {
      if (locale && wiki.lang !== locale) continue;
      out.push({
        ...wiki,
        gameId: game.id,
        gameSlug: game.slug,
        gameTitle: game.titles[catalogLocale] ?? game.titleFr,
      });
    }
  }
  return out;
}

export interface GameModHubEntry extends InazumaModHub {
  gameId: string;
  gameSlug: string;
  gameTitle: string;
  era: InazumaGameEra;
  thumbnail: string;
}

export function getModHubs(
  gameSlug?: string,
  catalogLocale: InazumaLocale = "fr",
): GameModHubEntry[] {
  const out: GameModHubEntry[] = [];
  for (const game of INAZUMA_GAMES) {
    if (gameSlug && game.slug !== gameSlug && game.id !== gameSlug) continue;
    for (const hub of game.modHubs) {
      out.push({
        ...hub,
        gameId: game.id,
        gameSlug: game.slug,
        gameTitle: game.titles[catalogLocale] ?? game.titleFr,
        era: game.era,
        thumbnail: game.thumbnail,
      });
    }
  }
  return out;
}

export interface GamesCatalogQueryOptions {
  locale?: string | null;
  era?: string | null;
  status?: string | null;
  q?: string | null;
}

export function queryGamesCatalog(options: GamesCatalogQueryOptions = {}) {
  const locale = normalizeGameLocale(options.locale);
  const eraFilter = options.era?.trim() || null;
  const statusFilter = options.status?.trim() || null;
  const query = options.q?.trim().toLowerCase() || "";

  const filtered = INAZUMA_GAMES.filter((game) => {
    if (eraFilter && game.era !== eraFilter) return false;
    if (statusFilter && game.status !== statusFilter) return false;
    if (!query) return true;
    const localized = game.titles[locale] ?? game.titleFr;
    const haystack = [
      game.id,
      game.slug,
      localized,
      game.titleFr,
      game.titleEn,
      game.titleJa,
      game.titleEs,
      game.protagonist,
      ...game.platforms,
      ...(game.aliases ?? []),
    ]
      .join(" ")
      .toLowerCase();
    return haystack.includes(query);
  });

  return {
    schemaVersion: 1,
    locale,
    total: filtered.length,
    eras: INAZUMA_ERAS.map((era) => getLocalizedEra(era, locale)),
    statuses: (Object.keys(GAME_STATUS_LABELS) as InazumaGameStatus[]).map(
      (id) => ({
        id,
        ...getLocalizedStatusLabel(id, locale),
      }),
    ),
    games: filtered.map((game) => getLocalizedGame(game, locale)),
  };
}

export function queryGameDetail(slug: string, localeInput?: string | null) {
  const game = getGameBySlug(slug);
  if (!game) return null;
  const locale = normalizeGameLocale(localeInput);
  const eraInfo = INAZUMA_ERAS.find((item) => item.id === game.era);
  return {
    schemaVersion: 1,
    locale,
    game: getLocalizedGame(game, locale),
    era: eraInfo ? getLocalizedEra(eraInfo, locale) : null,
    statusMeta: getLocalizedStatusLabel(game.status, locale),
  };
}

export function queryCommunityWikis(
  options: {
    lang?: string | null;
    gameSlug?: string | null;
    q?: string | null;
    limit?: number;
    locale?: string | null;
  } = {},
) {
  const locale = normalizeGameLocale(options.locale);
  const langFilter = options.lang?.trim()
    ? normalizeGameLocale(options.lang)
    : undefined;
  const gameSlug = options.gameSlug?.trim() || undefined;
  const query = options.q?.trim().toLowerCase() || "";
  const limit = Math.max(1, Math.min(options.limit ?? 100, 500));

  const entries = getCommunityWikis(langFilter, gameSlug, locale).filter(
    (entry) => {
      if (!query) return true;
      return `${entry.name} ${entry.lang} ${entry.kind} ${entry.gameTitle} ${entry.gameSlug} ${entry.url}`
        .toLowerCase()
        .includes(query);
    },
  );

  return entries.slice(0, limit);
}

export function queryModHubs(
  options: {
    gameSlug?: string | null;
    q?: string | null;
    limit?: number;
    locale?: string | null;
  } = {},
) {
  const locale = normalizeGameLocale(options.locale);
  const gameSlug = options.gameSlug?.trim() || undefined;
  const query = options.q?.trim().toLowerCase() || "";
  const limit = Math.max(1, Math.min(options.limit ?? 100, 500));

  const entries = getModHubs(gameSlug, locale).filter((entry) => {
    if (!query) return true;
    return `${entry.name} ${entry.platform} ${entry.gameTitle} ${entry.gameSlug} ${entry.url}`
      .toLowerCase()
      .includes(query);
  });

  return entries.slice(0, limit);
}
