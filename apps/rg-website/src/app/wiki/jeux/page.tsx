import type { Metadata } from "next";
import { getInaglePgPool } from "@rosegriffon/db/pg";
import { getGamesByStatus, INAZUMA_GAMES } from "@rosegriffon/inagle/games";
import { WikiPageHeader } from "@/components/wiki/WikiPageHeader";
import { wiki } from "@rosegriffon/app-shell";
import { wikiTitle, withDefaultOgImage } from "@/lib/seo";
import { absoluteUrl } from "@/lib/site-url";
import { getServerLocale } from "@/lib/i18n/server-locale";
import { GAMES_PAGE_COPY, getGamesPageCopy } from "@/lib/wiki/games-copy";
import { GamesCatalogClient } from "./GamesCatalogClient";

export const revalidate = 1800;

export const metadata: Metadata = {
  alternates: { canonical: absoluteUrl(wiki("/jeux")) },
  description: GAMES_PAGE_COPY.subtitle,
  openGraph: withDefaultOgImage({
    description: GAMES_PAGE_COPY.subtitle,
    locale: "fr_FR",
    siteName: "Azalée Wiki",
    title: wikiTitle(GAMES_PAGE_COPY.title),
    type: "website",
    url: absoluteUrl(wiki("/jeux")),
  }),
  title: wikiTitle(GAMES_PAGE_COPY.title),
};

interface SeriesCountRow {
  series: string | null;
  total: number;
}

export default async function WikiGamesPage() {
  const locale = await getServerLocale();
  const copy = getGamesPageCopy(locale);
  const pool = getInaglePgPool();
  const { rows } = await pool
    .query<SeriesCountRow>(
      `SELECT series, COUNT(*)::int AS total FROM public.inagle_characters GROUP BY series`,
    )
    .catch(() => ({ rows: [] as SeriesCountRow[] }));

  const seriesCounts: Record<string, number> = {};
  let totalCharacters = 0;
  for (const r of rows) {
    totalCharacters += r.total;
    if (r.series) seriesCounts[r.series] = r.total;
  }

  const devCount = getGamesByStatus("in-development").length;
  const headerCount = `${INAZUMA_GAMES.length} ${copy.totalGamesLabel.toLowerCase()} · ${devCount} ${copy.activeDevLabel.toLowerCase()}`;

  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "CollectionPage",
    description: copy.subtitle,
    inLanguage: ["fr", "en", "ja", "es"],
    mainEntity: {
      "@type": "ItemList",
      itemListElement: INAZUMA_GAMES.map((game) => ({
        "@type": "ListItem",
        name: game.titles[locale] ?? game.titleFr,
        position: game.order,
        url: absoluteUrl(wiki(`/jeux/${game.slug}`)),
      })),
      numberOfItems: INAZUMA_GAMES.length,
    },
    name: copy.title,
    url: absoluteUrl(wiki("/jeux")),
  };

  return (
    <div className="space-y-6">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
      />
      <WikiPageHeader
        icon="sports_esports"
        title={copy.title}
        count={headerCount}
        description={copy.subtitle}
      />

      <GamesCatalogClient
        games={INAZUMA_GAMES}
        seriesCounts={seriesCounts}
        totalCharacters={totalCharacters}
      />
    </div>
  );
}
