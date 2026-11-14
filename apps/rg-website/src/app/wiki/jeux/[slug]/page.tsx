import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { getInaglePgPool } from "@rosegriffon/db/pg";
import {
  getDbSeriesKeysForGame,
  getGameBySlug,
  INAZUMA_GAMES,
} from "@rosegriffon/inagle/games";
import { getCharacterImageUrl } from "@rosegriffon/azalee/images";
import { wiki } from "@rosegriffon/app-shell";
import { wikiTitle, withDefaultOgImage } from "@/lib/seo";
import { absoluteUrl } from "@/lib/site-url";
import { GAME_DETAIL_COPY } from "@/lib/wiki/games-copy";
import {
  GameDetailClient,
  type FeaturedGameCharacter,
} from "./GameDetailClient";

export const revalidate = 1800;

export function generateStaticParams() {
  return INAZUMA_GAMES.map((game) => ({ slug: game.slug }));
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ slug: string }>;
}): Promise<Metadata> {
  const { slug } = await params;
  const game = getGameBySlug(slug);
  if (!game) {
    return { title: wikiTitle(GAME_DETAIL_COPY.gamesIndex) };
  }
  const title = wikiTitle(game.titleFr, GAME_DETAIL_COPY.gamesIndex);
  return {
    alternates: { canonical: absoluteUrl(wiki(`/jeux/${game.slug}`)) },
    description: game.summaryFr,
    openGraph: withDefaultOgImage({
      description: game.summaryFr,
      images: [
        {
          alt: game.titleFr,
          height: 630,
          url: game.cover || game.thumbnail,
          width: 1200,
        },
      ],
      locale: "fr_FR",
      siteName: "Azalée Wiki",
      title,
      type: "article",
      url: absoluteUrl(wiki(`/jeux/${game.slug}`)),
    }),
    title,
  };
}

interface GameCharaRow {
  id: string;
  slug: string | null;
  internal_code: string | null;
  name_fr: string | null;
  name_en: string | null;
  element: string | null;
  position: string | null;
  image_url: string | null;
}

export default async function WikiGameDetailPage({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;
  const game = getGameBySlug(slug);
  if (!game) notFound();

  const seriesKeys = getDbSeriesKeysForGame(game);

  const pool = getInaglePgPool();
  const { rows: rawChars } =
    seriesKeys.length > 0
      ? await pool
          .query<GameCharaRow>(
            `SELECT id, slug, internal_code, name_fr, name_en, element, position, image_url
             FROM public.inagle_characters
             WHERE series = ANY($1::text[])
             ORDER BY rarity_code DESC NULLS LAST, zukan_order ASC NULLS LAST
             LIMIT 12`,
            [seriesKeys],
          )
          .catch(() => ({ rows: [] as GameCharaRow[] }))
      : { rows: [] as GameCharaRow[] };

  const featuredChars: FeaturedGameCharacter[] = rawChars.map((ch) => ({
    id: ch.id,
    slug: ch.slug,
    displayName: ch.name_fr || ch.name_en || ch.id,
    avatar: ch.internal_code
      ? getCharacterImageUrl(ch.internal_code)
      : ch.image_url || "/images/placeholder-character.svg",
    subtitle: ch.position || ch.element || game.seriesFilterKey,
  }));

  const primarySeriesFilter = game.seriesFilterKey ?? seriesKeys[0] ?? null;
  const playersHref = primarySeriesFilter
    ? `/wiki/chara?series=${encodeURIComponent(primarySeriesFilter)}`
    : "/wiki/chara";

  const otherGames = INAZUMA_GAMES.filter((g) => g.slug !== game.slug)
    .slice(-4)
    .reverse();

  const videoGameJsonLd = {
    "@context": "https://schema.org",
    "@type": "VideoGame",
    alternateName: [game.titleEn, game.titleJa, game.titleEs],
    applicationCategory: "Game",
    datePublished: game.releaseDateJa || game.releaseYear,
    description: game.summaryFr,
    developer: { "@type": "Organization", name: game.developer },
    gamePlatform: [...game.platforms],
    image: absoluteUrl(game.cover || game.thumbnail),
    inLanguage: ["fr", "en", "ja", "es"],
    name: game.titleFr,
    url: absoluteUrl(wiki(`/jeux/${game.slug}`)),
  };

  return (
    <div className="space-y-8">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(videoGameJsonLd) }}
      />
      <GameDetailClient
        game={game}
        otherGames={otherGames}
        playersHref={playersHref}
        featuredChars={featuredChars}
      />
    </div>
  );
}
