"use client";

import Image from "next/image";
import Link from "@/components/shared/Link";
import { useMemo, useState } from "react";
import {
  ArrowLeft,
  BookOpen,
  Download,
  ExternalLink,
  Globe,
  Package,
  Search,
} from "lucide-react";
import { normalizeGameLocale } from "@rosegriffon/inagle/games";
import { useLanguage } from "@/components/providers/language-provider";
import { getModsPageCopy } from "@/lib/wiki/games-copy";
import type { CommunityModsSnapshot } from "@/lib/wiki/community-mods-server";

export interface ModsDirectoryClientProps {
  snapshot: CommunityModsSnapshot;
}

type DirectoryTab = "mods" | "wikis" | "hubs";

const LANG_OPTIONS = ["all", "ja", "es", "en", "fr"] as const;

export function ModsDirectoryClient({ snapshot }: ModsDirectoryClientProps) {
  const { language } = useLanguage();
  const locale = normalizeGameLocale(language);
  const copy = useMemo(() => getModsPageCopy(locale), [locale]);
  const intlLocale =
    locale === "ja"
      ? "ja-JP"
      : locale === "en"
        ? "en-US"
        : locale === "es"
          ? "es-ES"
          : "fr-FR";

  const [tab, setTab] = useState<DirectoryTab>("mods");
  const [selectedLang, setSelectedLang] =
    useState<(typeof LANG_OPTIONS)[number]>("all");
  const [selectedGame, setSelectedGame] = useState<string>("all");
  const [query, setQuery] = useState("");

  const gameOptions = useMemo(() => {
    const map = new Map<string, string>();
    for (const hub of snapshot.modHubs) {
      map.set(hub.gameSlug, hub.gameTitle);
    }
    for (const m of snapshot.mods) {
      if (!map.has(m.gameSlug)) map.set(m.gameSlug, m.gameTitle);
    }
    return [...map.entries()].map(([slug, title]) => ({ slug, title }));
  }, [snapshot.modHubs, snapshot.mods]);

  const filteredMods = useMemo(() => {
    const q = query.trim().toLowerCase();
    return snapshot.mods.filter((mod) => {
      if (selectedGame !== "all" && mod.gameSlug !== selectedGame) return false;
      if (!q) return true;
      return [
        mod.name,
        mod.author,
        mod.gameTitle,
        mod.category,
        mod.description,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
        .includes(q);
    });
  }, [query, selectedGame, snapshot.mods]);

  const mergedWikis = useMemo(() => {
    const seen = new Set<string>();
    const combined: Array<{
      id: string;
      name: string;
      lang: string;
      url: string;
      kind: string;
      subtitle: string;
    }> = [];
    for (const w of snapshot.wikis) {
      if (seen.has(w.url)) continue;
      seen.add(w.url);
      combined.push({
        id: w.id ?? w.url,
        name: w.title,
        lang: w.lang,
        url: w.url,
        kind: w.sourceKind,
        subtitle: w.excerpt ?? w.url,
      });
    }
    for (const cw of snapshot.curatedWikis) {
      if (seen.has(cw.url)) continue;
      seen.add(cw.url);
      combined.push({
        id: `${cw.gameSlug}-${cw.lang}-${cw.url}`,
        name: cw.name,
        lang: cw.lang,
        url: cw.url,
        kind: cw.kind,
        subtitle: cw.gameTitle,
      });
    }
    const q = query.trim().toLowerCase();
    return combined.filter((item) => {
      if (selectedLang !== "all" && item.lang !== selectedLang) return false;
      if (!q) return true;
      return [item.name, item.lang, item.kind, item.subtitle, item.url]
        .join(" ")
        .toLowerCase()
        .includes(q);
    });
  }, [query, selectedLang, snapshot.curatedWikis, snapshot.wikis]);

  const filteredHubs = useMemo(() => {
    const q = query.trim().toLowerCase();
    return snapshot.modHubs.filter((hub) => {
      if (selectedGame !== "all" && hub.gameSlug !== selectedGame) return false;
      if (!q) return true;
      return [hub.name, hub.gameTitle, hub.platform]
        .join(" ")
        .toLowerCase()
        .includes(q);
    });
  }, [query, selectedGame, snapshot.modHubs]);

  return (
    <div className="space-y-8">
      {/* Stat cards */}
      <div className="grid grid-cols-1 sm:grid-cols-4 gap-3">
        <div className="rounded-2xl border border-outline-variant/20 bg-surface-container p-4">
          <div className="text-xs font-medium text-on-surface-variant">
            {copy.statsModsLabel}
          </div>
          <div className="text-2xl font-black text-on-surface tabular-nums mt-0.5">
            {snapshot.mods.length}
          </div>
        </div>
        <div className="rounded-2xl border border-outline-variant/20 bg-surface-container p-4">
          <div className="text-xs font-medium text-on-surface-variant">
            {copy.statsWikisLabel}
          </div>
          <div className="text-2xl font-black text-primary tabular-nums mt-0.5">
            {mergedWikis.length}
          </div>
        </div>
        <div className="rounded-2xl border border-outline-variant/20 bg-surface-container p-4">
          <div className="text-xs font-medium text-on-surface-variant">
            {copy.statsHubsLabel}
          </div>
          <div className="text-2xl font-black text-on-surface tabular-nums mt-0.5">
            {snapshot.modHubs.length}
          </div>
        </div>
        <Link
          href="/wiki/jeux"
          className="rounded-2xl border border-outline-variant/20 bg-surface-container-low p-4 flex flex-col justify-between hover:border-primary/40 transition"
        >
          <span className="inline-flex items-center gap-1.5 text-xs font-bold text-primary">
            <ArrowLeft className="size-3.5" />
            <span>{copy.gamesBackCta}</span>
          </span>
          {snapshot.updatedAt && (
            <span className="text-xs text-on-surface-variant mt-2 truncate">
              {`${copy.snapshotUpdatedPrefix} ${new Date(snapshot.updatedAt).toLocaleDateString(intlLocale)}`}
            </span>
          )}
        </Link>
      </div>

      {/* Filters */}
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
                {
                  id: "mods",
                  label: `${copy.modsTabLabel} (${filteredMods.length})`,
                },
                {
                  id: "wikis",
                  label: `${copy.wikisTabLabel} (${mergedWikis.length})`,
                },
                {
                  id: "hubs",
                  label: `${copy.hubsTabLabel} (${filteredHubs.length})`,
                },
              ] as const
            ).map((opt) => (
              <button
                key={opt.id}
                type="button"
                onClick={() => setTab(opt.id)}
                className={`px-3.5 py-1.5 rounded-full text-xs font-bold border transition ${
                  tab === opt.id
                    ? "bg-primary text-on-primary border-primary"
                    : "bg-surface-container-low text-on-surface-variant border-outline-variant/25 hover:border-primary/40"
                }`}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>

        {tab === "wikis" ? (
          <div className="flex flex-wrap gap-2">
            {LANG_OPTIONS.map((langCode) => (
              <button
                key={langCode}
                type="button"
                onClick={() => setSelectedLang(langCode)}
                className={`px-3 py-1.5 rounded-xl text-xs font-bold uppercase border transition ${
                  selectedLang === langCode
                    ? "bg-secondary-container text-on-secondary-container border-secondary/30"
                    : "bg-surface-container-low text-on-surface-variant border-outline-variant/20 hover:border-primary/30"
                }`}
              >
                {langCode === "all" ? copy.allLangsLabel : langCode}
              </button>
            ))}
          </div>
        ) : (
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => setSelectedGame("all")}
              className={`px-3 py-1.5 rounded-xl text-xs font-bold border transition ${
                selectedGame === "all"
                  ? "bg-secondary-container text-on-secondary-container border-secondary/30"
                  : "bg-surface-container-low text-on-surface-variant border-outline-variant/20 hover:border-primary/30"
              }`}
            >
              {copy.allGamesLabel}
            </button>
            {gameOptions.slice(0, 10).map((g) => (
              <button
                key={g.slug}
                type="button"
                onClick={() => setSelectedGame(g.slug)}
                className={`px-3 py-1.5 rounded-xl text-xs font-bold border transition ${
                  selectedGame === g.slug
                    ? "bg-secondary-container text-on-secondary-container border-secondary/30"
                    : "bg-surface-container-low text-on-surface-variant border-outline-variant/20 hover:border-primary/30"
                }`}
              >
                {g.title}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Content */}
      {tab === "mods" &&
        (filteredMods.length === 0 ? (
          <div className="rounded-2xl border border-outline-variant/20 bg-surface-container p-10 text-center text-sm text-on-surface-variant">
            {copy.emptyModsMessage}
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
            {filteredMods.map((mod) => (
              <article
                key={mod.id}
                className="rounded-2xl border border-outline-variant/20 bg-surface-container overflow-hidden flex flex-col justify-between hover:border-primary/40 transition"
              >
                <div>
                  {mod.thumbnailUrl && (
                    <div className="relative aspect-[16/9] w-full bg-surface-container-highest overflow-hidden">
                      <Image
                        src={mod.thumbnailUrl}
                        alt={mod.name}
                        fill
                        unoptimized
                        sizes="(max-width: 768px) 100vw, 33vw"
                        className="object-cover"
                      />
                    </div>
                  )}
                  <div className="p-4 space-y-2">
                    <div className="flex items-center justify-between gap-2">
                      <span className="px-2 py-0.5 rounded-md text-xs font-bold bg-secondary-container text-on-secondary-container">
                        {mod.category}
                      </span>
                      <span className="text-xs text-on-surface-variant truncate">
                        {mod.gameTitle}
                      </span>
                    </div>
                    <h3 className="text-base font-black text-on-surface line-clamp-2">
                      {mod.name}
                    </h3>
                    {mod.author && (
                      <p className="text-xs text-on-surface-variant">
                        {`${copy.byAuthorPrefix} ${mod.author}`}
                      </p>
                    )}
                    {mod.description && (
                      <p className="text-xs text-on-surface-variant line-clamp-2">
                        {mod.description}
                      </p>
                    )}
                  </div>
                </div>
                <div className="px-4 py-3 border-t border-outline-variant/15 flex items-center justify-between gap-2 text-xs">
                  <span className="inline-flex items-center gap-1 text-on-surface-variant tabular-nums">
                    {typeof mod.downloads === "number" && (
                      <>
                        <Download className="size-3.5 text-primary" />
                        <span>{`${mod.downloads.toLocaleString(intlLocale)} ${copy.downloadsLabel}`}</span>
                      </>
                    )}
                  </span>
                  <a
                    href={mod.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center gap-1 font-bold text-primary hover:underline"
                  >
                    <span>{copy.openModCta}</span>
                    <ExternalLink className="size-3.5" />
                  </a>
                </div>
              </article>
            ))}
          </div>
        ))}

      {tab === "wikis" &&
        (mergedWikis.length === 0 ? (
          <div className="rounded-2xl border border-outline-variant/20 bg-surface-container p-10 text-center text-sm text-on-surface-variant">
            {copy.emptyWikisMessage}
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {mergedWikis.map((wikiItem) => (
              <a
                key={wikiItem.id}
                href={wikiItem.url}
                target="_blank"
                rel="noopener noreferrer"
                className="group rounded-2xl border border-outline-variant/20 bg-surface-container p-4 flex items-start justify-between gap-4 hover:border-primary/40 transition"
              >
                <div className="space-y-1.5 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-xs font-black uppercase bg-primary-container text-on-primary-container">
                      <Globe className="size-3" />
                      <span>{wikiItem.lang}</span>
                    </span>
                    <span className="px-2 py-0.5 rounded-md text-xs font-semibold bg-surface-container-high text-on-surface-variant">
                      {wikiItem.kind}
                    </span>
                  </div>
                  <h3 className="text-sm font-black text-on-surface group-hover:text-primary line-clamp-1">
                    {wikiItem.name}
                  </h3>
                  <p className="text-xs text-on-surface-variant line-clamp-2">
                    {wikiItem.subtitle}
                  </p>
                </div>
                <span className="inline-flex items-center gap-1 text-xs font-bold text-primary shrink-0 mt-1">
                  <BookOpen className="size-3.5" />
                  <span>{copy.openWikiCta}</span>
                </span>
              </a>
            ))}
          </div>
        ))}

      {tab === "hubs" && (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredHubs.map((hub) => (
            <a
              key={hub.url}
              href={hub.url}
              target="_blank"
              rel="noopener noreferrer"
              className="group rounded-2xl border border-outline-variant/20 bg-surface-container p-4 flex items-center justify-between gap-3 hover:border-primary/40 transition"
            >
              <div className="min-w-0 space-y-1">
                <div className="text-xs font-semibold text-primary">
                  {hub.gameTitle}
                </div>
                <div className="text-sm font-black text-on-surface group-hover:text-primary line-clamp-1">
                  {hub.name}
                </div>
              </div>
              <span className="inline-flex items-center gap-1 text-xs font-bold text-on-surface-variant group-hover:text-primary shrink-0">
                <Package className="size-4" />
                <span>{copy.openHubCta}</span>
              </span>
            </a>
          ))}
        </div>
      )}
    </div>
  );
}
