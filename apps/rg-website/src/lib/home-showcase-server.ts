import "server-only";

import {
  lireChiffresEsport,
  lireClansActifs,
  lireClassement,
  type ChiffresEsport,
  type LigneClassement,
  type PreviewClan,
} from "@/lib/achillea/classement";
import { getPgPoolOrNull } from "@/lib/db/pg";
import { cdnFacePortraitUrl } from "@/lib/home-showcase";

export interface FeaturedCharaShowcase {
  element: string;
  href: string;
  internalCode: string;
  name: string;
  nameJa: string | null;
  portraitUrl: string;
}

export interface WikiStatsShowcase {
  characters: number;
  items: number;
  skills: number;
  teams: number;
}

export interface NextStreamSlot {
  category: string | null;
  startTime: string | null;
  streamerName: string;
}

export interface HomeShowcaseData {
  achillea: {
    clans: PreviewClan[];
    leaderboard: LigneClassement[];
    stats: ChiffresEsport;
  };
  characters: FeaturedCharaShowcase[];
  nextStream: NextStreamSlot | null;
  wikiStats: WikiStatsShowcase;
}

const FALLBACK_CHARACTERS: FeaturedCharaShowcase[] = [
  {
    element: "Montagne",
    href: "/wiki/chara",
    internalCode: "c01000010",
    name: "Mark Evans",
    nameJa: "円堂 守",
    portraitUrl: cdnFacePortraitUrl("c01000010"),
  },
  {
    element: "Forêt",
    href: "/wiki/chara",
    internalCode: "c01001900",
    name: "Byron Love",
    nameJa: "亜風炉 照美",
    portraitUrl: cdnFacePortraitUrl("c01001900"),
  },
  {
    element: "Feu",
    href: "/wiki/chara",
    internalCode: "c05024710",
    name: "Axel Blaze",
    nameJa: "豪炎寺 修也",
    portraitUrl: cdnFacePortraitUrl("c05024710"),
  },
  {
    element: "Vent",
    href: "/wiki/chara",
    internalCode: "c01010970",
    name: "Jude Sharp",
    nameJa: "鬼道 有人",
    portraitUrl: cdnFacePortraitUrl("c01010970"),
  },
];

async function chargerWikiVitrine(): Promise<{
  characters: FeaturedCharaShowcase[];
  nextStream: NextStreamSlot | null;
  stats: WikiStatsShowcase;
}> {
  const pool = getPgPoolOrNull();
  if (!pool) {
    return {
      characters: FALLBACK_CHARACTERS,
      nextStream: null,
      stats: { characters: 0, items: 0, skills: 0, teams: 0 },
    };
  }

  try {
    const today = new Date().getDay();
    const nowTime = new Date().toLocaleTimeString("fr-FR", {
      hour: "2-digit",
      minute: "2-digit",
    });

    const [countsRes, charaRes, streamRes] = await Promise.all([
      pool.query<{ key: string; count: number }>(
        `SELECT 'characters'::text AS key, COUNT(*)::int AS count FROM public.inagle_characters
         UNION ALL SELECT 'skills', COUNT(*)::int FROM public.inagle_skills
         UNION ALL SELECT 'items', COUNT(*)::int FROM public.inagle_items
         UNION ALL SELECT 'teams', COUNT(*)::int FROM public.inagle_teams`,
      ),
      pool.query<{
        element: string;
        id: string;
        internal_code: string;
        name_en: string;
        name_fr: string | null;
        name_ja: string | null;
        slug: string | null;
      }>(
        `SELECT id, slug, name_fr, name_en, name_ja, element, internal_code
         FROM public.inagle_characters
         WHERE is_primary = true
           AND internal_code IN ('c01000010', 'c01001900', 'c05024710', 'c01010970')
         ORDER BY CASE internal_code
           WHEN 'c01000010' THEN 1
           WHEN 'c01001900' THEN 2
           WHEN 'c05024710' THEN 3
           WHEN 'c01010970' THEN 4
           ELSE 5 END`,
      ),
      pool
        .query<{
          category: string | null;
          start_time: string | null;
          streamer_name: string;
        }>(
          `SELECT start_time, streamer_name, category
           FROM stream_schedules
           WHERE day_of_week = $1 AND status = 'active' AND start_time >= $2
           ORDER BY start_time ASC
           LIMIT 1`,
          [today, nowTime],
        )
        .catch(() => ({ rows: [] })),
    ]);

    const countMap = new Map(countsRes.rows.map((r) => [r.key, r.count]));
    const characters: FeaturedCharaShowcase[] =
      charaRes.rows.length > 0
        ? charaRes.rows.map((c) => ({
            element: c.element || "Vent",
            href: c.slug ? `/wiki/chara/${c.slug}` : "/wiki/chara",
            internalCode: c.internal_code,
            name: c.name_fr || c.name_en,
            nameJa: c.name_ja,
            portraitUrl: cdnFacePortraitUrl(c.internal_code),
          }))
        : FALLBACK_CHARACTERS;

    const firstStream = streamRes.rows[0];
    return {
      characters,
      nextStream: firstStream
        ? {
            category: firstStream.category,
            startTime: firstStream.start_time,
            streamerName: firstStream.streamer_name,
          }
        : null,
      stats: {
        characters: countMap.get("characters") ?? 0,
        items: countMap.get("items") ?? 0,
        skills: countMap.get("skills") ?? 0,
        teams: countMap.get("teams") ?? 0,
      },
    };
  } catch (err) {
    console.error("[home-showcase] Erreur chargement vitrine:", err);
    return {
      characters: FALLBACK_CHARACTERS,
      nextStream: null,
      stats: { characters: 0, items: 0, skills: 0, teams: 0 },
    };
  }
}

export async function chargerVitrineAccueil(): Promise<HomeShowcaseData> {
  const [wiki, esportStats, leaderboard, clans] = await Promise.all([
    chargerWikiVitrine(),
    lireChiffresEsport(),
    lireClassement(2, 3),
    lireClansActifs(3),
  ]);

  return {
    achillea: {
      clans,
      leaderboard,
      stats: esportStats,
    },
    characters: wiki.characters,
    nextStream: wiki.nextStream,
    wikiStats: wiki.stats,
  };
}
