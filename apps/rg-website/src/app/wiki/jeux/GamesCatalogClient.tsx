"use client";

import Image from "next/image";
import Link from "@/components/shared/Link";
import { useMemo, useState } from "react";
import {
  ArrowRight,
  BookOpen,
  Calendar,
  ExternalLink,
  Gamepad2,
  Package,
  Search,
  ShoppingBag,
  Sparkles,
  Users,
} from "lucide-react";
import {
  getDbSeriesKeysForGame,
  getGamesByStatus,
  getLocalizedEra,
  getLocalizedGame,
  getLocalizedStatusLabel,
  INAZUMA_ERAS,
  normalizeGameLocale,
  type InazumaGame,
  type InazumaGameEra,
  type InazumaGameStatus,
} from "@rosegriffon/inagle/games";
import { useLanguage } from "@/components/providers/language-provider";
import { getGamesPageCopy } from "@/lib/wiki/games-copy";

export interface GamesCatalogClientProps {
  games: readonly InazumaGame[];
  seriesCounts: Record<string, number>;
  totalCharacters?: number;
}

const STATUS_BADGE_CLASSES = {
  dev: "bg-tertiary-container text-on-tertiary-container border-tertiary/30",
  released: "bg-primary-container text-on-primary-container border-primary/30",
  muted:
    "bg-surface-container-highest text-on-surface-variant border-outline-variant/30",
} as const;

export function GamesCatalogClient({
  games,
  seriesCounts,
  totalCharacters = 0,
}: GamesCatalogClientProps) {
  const { language } = useLanguage();
  const locale = normalizeGameLocale(language);
  const copy = useMemo(() => getGamesPageCopy(locale), [locale]);
  const intlLocale =
    locale === "ja"
      ? "ja-JP"
      : locale === "en"
        ? "en-US"
        : locale === "es"
          ? "es-ES"
          : "fr-FR";

  const [selectedEra, setSelectedEra] = useState<InazumaGameEra | "all">("all");
  const [selectedStatus, setSelectedStatus] = useState<
    InazumaGameStatus | "all"
  >("all");
  const [query, setQuery] = useState("");

  const upcomingGames = useMemo(
    () => games.filter((g) => g.status === "in-development"),
    [games],
  );
  const devCount = getGamesByStatus("in-development").length;

  const filteredGames = useMemo(() => {
    const q = query.trim().toLowerCase();
    return games.filter((g) => {
      if (selectedEra !== "all" && g.era !== selectedEra) return false;
      if (selectedStatus !== "all" && g.status !== selectedStatus) return false;
      if (!q) return true;
      const haystack = [
        g.titleFr,
        g.titleEn,
        g.titleJa,
        g.titleEs,
        g.summaries[locale],
        g.protagonist,
        g.releaseYear,
        ...g.platforms,
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(q);
    });
  }, [games, locale, query, selectedEra, selectedStatus]);

  const computePlayerCount = (game: InazumaGame): number => {
    if (game.seriesFilterKey) {
      return seriesCounts[game.seriesFilterKey] ?? 0;
    }
    let total = 0;
    for (const key of getDbSeriesKeysForGame(game)) {
      total += seriesCounts[key] ?? 0;
    }
    return total;
  };

  return (
    <div className="space-y-8">
      <div className="grid grid-cols-1 sm:grid-cols-4 gap-3">
        <div className="rounded-2xl border border-outline-variant/20 bg-surface-container p-4">
          <div className="text-xs font-medium text-on-surface-variant">
            {copy.totalGamesLabel}
          </div>
          <div className="text-2xl font-black text-on-surface tabular-nums mt-0.5">
            {games.length}
          </div>
        </div>
        <div className="rounded-2xl border border-outline-variant/20 bg-surface-container p-4">
          <div className="text-xs font-medium text-on-surface-variant">
            {copy.activeDevLabel}
          </div>
          <div className="text-2xl font-black text-primary tabular-nums mt-0.5">
            {devCount}
          </div>
        </div>
        <div className="rounded-2xl border border-outline-variant/20 bg-surface-container p-4">
          <div className="text-xs font-medium text-on-surface-variant">
            {copy.playableCharactersLabel}
          </div>
          <div className="text-2xl font-black text-on-surface tabular-nums mt-0.5">
            {totalCharacters > 0
              ? totalCharacters.toLocaleString(intlLocale)
              : "5 459+"}
          </div>
        </div>
        <Link
          href="/wiki/mods"
          className="rounded-2xl border border-primary/30 bg-primary-container/60 p-4 flex flex-col justify-between hover:border-primary transition"
        >
          <span className="inline-flex items-center gap-1.5 text-xs font-bold text-on-primary-container">
            <Package className="size-4" />
            <span>{copy.modsHubCta}</span>
          </span>
          <span className="inline-flex items-center gap-1 text-xs font-bold text-primary mt-2">
            <span>{copy.viewDetailsLabel}</span>
            <ArrowRight className="size-3.5" />
          </span>
        </Link>
      </div>

      {/* Spotlight : Jeux en développement */}
      <section className="rounded-3xl border border-outline-variant/20 bg-surface-container p-5 md:p-7 space-y-5">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div>
            <h2 className="text-lg md:text-xl font-black text-on-surface flex items-center gap-2">
              <Sparkles className="size-5 text-primary" />
              <span>{copy.upcomingSpotlightHeading}</span>
            </h2>
            <p className="text-xs md:text-sm text-on-surface-variant mt-0.5">
              {copy.upcomingSpotlightSubtitle}
            </p>
          </div>
          <span className="inline-flex items-center px-3 py-1 rounded-full text-xs font-bold bg-tertiary-container text-on-tertiary-container self-start">
            {`${upcomingGames.length} ${copy.activeDevLabel.toLowerCase()}`}
          </span>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {upcomingGames.map((game) => {
            const loc = getLocalizedGame(game, locale);
            const devBadge = getLocalizedStatusLabel(game.status, locale);
            const badgeClass = STATUS_BADGE_CLASSES[devBadge.tone];
            const platformsLine = game.platforms.join(" · ");
            return (
              <Link
                key={game.slug}
                href={`/wiki/jeux/${game.slug}`}
                className="group flex flex-col sm:flex-row gap-4 rounded-2xl border border-outline-variant/20 bg-surface-container-low p-4 hover:border-primary/40 hover:bg-surface-container-high transition"
              >
                <div className="relative w-full sm:w-44 aspect-[16/10] shrink-0 rounded-xl overflow-hidden bg-surface-container-highest">
                  <Image
                    src={game.cover || game.thumbnail}
                    alt={loc.title}
                    fill
                    sizes="(max-width: 640px) 100vw, 176px"
                    className="object-cover transition-transform duration-300 group-hover:scale-105"
                  />
                </div>
                <div className="flex flex-col justify-between flex-1 min-w-0">
                  <div>
                    <div className="flex items-center gap-2 flex-wrap mb-1.5">
                      <span
                        className={`text-xs font-bold px-2.5 py-0.5 rounded-full border ${badgeClass}`}
                      >
                        {devBadge.label}
                      </span>
                      <span className="text-xs font-semibold text-on-surface-variant">
                        {game.releaseYear}
                      </span>
                    </div>
                    <h3 className="text-base md:text-lg font-black text-on-surface group-hover:text-primary transition-colors">
                      {loc.title}
                    </h3>
                    <p className="text-xs text-on-surface-variant mt-1 line-clamp-2">
                      {loc.summary}
                    </p>
                  </div>
                  <div className="mt-3 pt-2.5 border-t border-outline-variant/15 flex items-center justify-between gap-2 text-xs text-on-surface-variant">
                    <span className="truncate">{platformsLine}</span>
                    <span className="inline-flex items-center gap-1 font-bold text-primary shrink-0">
                      {copy.viewDetailsLabel}
                      <ArrowRight className="size-3.5" />
                    </span>
                  </div>
                </div>
              </Link>
            );
          })}
        </div>
      </section>

      {/* Filtres */}
      <div className="rounded-2xl border border-outline-variant/20 bg-surface-container p-4 space-y-4">
        <div className="flex flex-col md:flex-row gap-3 items-stretch md:items-center justify-between">
          <div className="relative flex-1 max-w-md">
            <Search className="size-4 text-on-surface-variant absolute left-3.5 top-1/2 -translate-y-1/2 pointer-events-none" />
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={copy.searchPlaceholder}
              aria-label={copy.searchPlaceholder}
              className="w-full pl-10 pr-4 py-2.5 rounded-xl bg-surface-container-low border border-outline-variant/25 text-sm text-on-surface placeholder:text-on-surface-variant focus:outline-none focus:border-primary"
            />
          </div>

          <div className="flex flex-wrap gap-2">
            {(
              [
                { key: "all", label: copy.allStatusesLabel },
                {
                  key: "in-development",
                  label: getLocalizedStatusLabel("in-development", locale)
                    .label,
                },
                {
                  key: "released",
                  label: getLocalizedStatusLabel("released", locale).label,
                },
                {
                  key: "discontinued",
                  label: getLocalizedStatusLabel("discontinued", locale).label,
                },
                {
                  key: "cancelled-evolved",
                  label: getLocalizedStatusLabel("cancelled-evolved", locale)
                    .label,
                },
              ] as const
            ).map((opt) => {
              const active = selectedStatus === opt.key;
              return (
                <button
                  key={opt.key}
                  type="button"
                  onClick={() => setSelectedStatus(opt.key)}
                  className={`px-3 py-1.5 rounded-full text-xs font-bold border transition ${
                    active
                      ? "bg-primary text-on-primary border-primary"
                      : "bg-surface-container-low text-on-surface-variant border-outline-variant/25 hover:border-primary/40"
                  }`}
                >
                  {opt.label}
                </button>
              );
            })}
          </div>
        </div>

        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => setSelectedEra("all")}
            className={`px-3 py-1.5 rounded-xl text-xs font-bold border transition ${
              selectedEra === "all"
                ? "bg-secondary-container text-on-secondary-container border-secondary/30"
                : "bg-surface-container-low text-on-surface-variant border-outline-variant/20 hover:border-primary/30"
            }`}
          >
            {copy.allErasLabel}
          </button>
          {INAZUMA_ERAS.map((era) => {
            const locEra = getLocalizedEra(era, locale);
            const active = selectedEra === era.id;
            return (
              <button
                key={era.id}
                type="button"
                onClick={() => setSelectedEra(era.id)}
                className={`px-3 py-1.5 rounded-xl text-xs font-bold border transition ${
                  active
                    ? "bg-secondary-container text-on-secondary-container border-secondary/30"
                    : "bg-surface-container-low text-on-surface-variant border-outline-variant/20 hover:border-primary/30"
                }`}
              >
                {locEra.label}
              </button>
            );
          })}
        </div>
      </div>

      {/* Grille chronologique */}
      {filteredGames.length === 0 ? (
        <div className="rounded-2xl border border-outline-variant/20 bg-surface-container p-10 text-center space-y-3">
          <p className="text-sm text-on-surface-variant">{copy.emptyResults}</p>
          <button
            type="button"
            onClick={() => {
              setSelectedEra("all");
              setSelectedStatus("all");
              setQuery("");
            }}
            className="px-4 py-2 rounded-xl bg-primary text-on-primary text-xs font-bold"
          >
            {copy.resetFiltersLabel}
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
          {filteredGames.map((game) => {
            const loc = getLocalizedGame(game, locale);
            const statusMeta = getLocalizedStatusLabel(game.status, locale);
            const statusClass = STATUS_BADGE_CLASSES[statusMeta.tone];
            const playerCount = computePlayerCount(game);
            const countLabel =
              playerCount > 0
                ? `${playerCount.toLocaleString(intlLocale)} ${copy.playersCountSuffix}`
                : copy.noPlayersDirectNote;
            const orderBadge = `#${String(game.order).padStart(2, "0")}`;
            const platformsSummary = game.platforms.join(" · ");
            const subtitleAlt = locale === "ja" ? game.titleEn : game.titleJa;

            return (
              <article
                key={game.slug}
                className="group flex flex-col rounded-2xl border border-outline-variant/20 bg-surface-container overflow-hidden hover:border-primary/40 hover:shadow-lg transition-all"
              >
                <Link
                  href={`/wiki/jeux/${game.slug}`}
                  className="relative aspect-[16/9] w-full bg-surface-container-highest overflow-hidden block"
                >
                  <Image
                    src={game.thumbnail}
                    alt={loc.title}
                    fill
                    sizes="(max-width: 768px) 100vw, (max-width: 1200px) 50vw, 33vw"
                    className="object-cover transition-transform duration-300 group-hover:scale-105"
                  />
                  <div className="absolute inset-0 bg-linear-to-t from-surface/90 via-transparent to-transparent" />
                  <div className="absolute top-3 left-3 flex items-center gap-2">
                    <span className="px-2 py-0.5 rounded-md text-xs font-black bg-surface/85 text-on-surface backdrop-blur-sm border border-outline-variant/30 tabular-nums">
                      {orderBadge}
                    </span>
                    <span
                      className={`px-2.5 py-0.5 rounded-full text-xs font-bold border backdrop-blur-sm ${statusClass}`}
                    >
                      {statusMeta.label}
                    </span>
                  </div>
                  <div className="absolute bottom-3 left-3 right-3 flex items-center justify-between gap-2 text-xs font-bold text-on-surface">
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-surface/80 backdrop-blur-sm">
                      <Calendar className="size-3.5 text-primary" />
                      {game.releaseYear}
                    </span>
                  </div>
                </Link>

                <div className="p-4 flex-1 flex flex-col justify-between gap-4">
                  <div className="space-y-2">
                    <div>
                      <Link
                        href={`/wiki/jeux/${game.slug}`}
                        className="text-base font-black text-on-surface group-hover:text-primary transition-colors line-clamp-2 block"
                      >
                        {loc.title}
                      </Link>
                      <p className="text-xs text-on-surface-variant line-clamp-1 mt-0.5">
                        {subtitleAlt}
                      </p>
                    </div>
                    <p className="text-xs text-on-surface-variant leading-relaxed line-clamp-3">
                      {loc.summary}
                    </p>
                    {(game.communityWikis.length > 0 ||
                      game.modHubs.length > 0) && (
                      <div className="flex flex-wrap gap-1.5 pt-1">
                        {game.communityWikis.length > 0 && (
                          <span className="inline-flex items-center gap-1 rounded-md bg-surface-container-high px-2 py-0.5 text-xs font-medium text-on-surface-variant">
                            <BookOpen className="size-3 text-primary" />
                            <span>{`${game.communityWikis.length} ${copy.wikisBadgeLabel}`}</span>
                          </span>
                        )}
                        {game.modHubs.length > 0 && (
                          <span className="inline-flex items-center gap-1 rounded-md bg-surface-container-high px-2 py-0.5 text-xs font-medium text-on-surface-variant">
                            <Package className="size-3 text-primary" />
                            <span>{`${game.modHubs.length} ${copy.modsBadgeLabel}`}</span>
                          </span>
                        )}
                      </div>
                    )}
                  </div>

                  <div className="space-y-2.5 pt-3 border-t border-outline-variant/15 text-xs text-on-surface-variant">
                    <div className="flex items-center gap-1.5">
                      <Gamepad2 className="size-3.5 text-primary shrink-0" />
                      <span className="truncate">{platformsSummary}</span>
                    </div>
                    <div className="flex items-center gap-1.5">
                      <Users className="size-3.5 text-primary shrink-0" />
                      <span className="truncate">{countLabel}</span>
                    </div>
                    <div className="flex items-center justify-between gap-2 pt-1">
                      <Link
                        href={`/wiki/jeux/${game.slug}`}
                        className="inline-flex items-center gap-1 font-bold text-primary hover:underline"
                      >
                        <span>{copy.viewDetailsLabel}</span>
                        <ArrowRight className="size-3.5" />
                      </Link>
                      <div className="flex items-center gap-2">
                        {game.storeUrl && (
                          <a
                            href={game.storeUrl}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-surface-container-high hover:text-primary transition"
                            aria-label={copy.storeLabel}
                          >
                            <ShoppingBag className="size-3.5" />
                          </a>
                        )}
                        <a
                          href={game.officialUrl}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-surface-container-high hover:text-primary transition"
                          aria-label={copy.officialSiteLabel}
                        >
                          <ExternalLink className="size-3.5" />
                        </a>
                      </div>
                    </div>
                  </div>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </div>
  );
}
