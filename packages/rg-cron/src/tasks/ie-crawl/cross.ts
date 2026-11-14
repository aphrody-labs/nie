/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { sql } from "../../lib/db";
import {
  downloadAssetIfMissing,
  fetchHtmlResilient,
  resolveRepoRoot,
} from "../../lib/http-crawl";
import { crawlDataPath } from "../../lib/shared-path";
import { extraireCrossPortal } from "./portal-extract";

const CROSS_URLS = {
  home: "https://www.inazuma-cross.jp/",
  guide: "https://www.inazuma-cross.jp/guide/",
  vr2026: "https://www.inazuma-cross.jp/victory-road-2026/",
  samuraiBlue: "https://www.inazuma-cross.jp/samuraiblue2026/",
  cbtReport: "https://www.inazuma-cross.jp/cbt-report/",
} as const;

const KNOWN_FR_NEWS: ReadonlyArray<{
  match: string;
  titleFr: string;
  tagFr: string;
}> = [
  {
    match: "コラボ開催決定",
    titleFr:
      "Collaboration « Inazuma Eleven: Victory Road » annoncée (17 nov. 2026 – 31 mars 2027)",
    tagFr: "Collaboration",
  },
  {
    match: "初心者ガイド公開",
    titleFr: "Guide du débutant Inazuma Eleven Cross publié",
    tagFr: "Guide",
  },
  {
    match: "SAMURAI BLUE 2026",
    titleFr: "Événement spécial SAMURAI BLUE 2026 annoncé",
    tagFr: "Collaboration",
  },
  {
    match: "サイン色紙",
    titleFr: "Campagne concours d'illustrations dédicacées",
    tagFr: "Campagne",
  },
  {
    match: "公式サイト公開",
    titleFr: "Ouverture du site officiel d'Inazuma Eleven Cross",
    tagFr: "Info",
  },
];

const VR_CHARA_FR: Readonly<Record<string, string>> = {
  "笹波 雲明": "Destin Billows (Unmei Sasanami)",
  "桜咲 丈二": "Briar Bloomhurst (Sakurazaki Jouji)",
  "木曽路 兵太": "Cedric Freud (Kisoji Heita)",
  "忍原 來夏": "Raika Shinohara",
  "古道飼 亀雄": "Cade Shelby (Kameo Kodoukai)",
  "四川堂 我流": "Garyuu Shisendou",
  "柳生 駿河": "Thierry Bison (Suruga Yagyuu)",
  "空宮 征": "Tadashi Soramiya",
  "品 真士": "Masashi Shina",
  "円堂 ハル": "Harper Evans (Haru Endou)",
  "月影 蓮": "Ren Tsukikage",
};

const TAG_FR_MAP: Readonly<Record<string, string>> = {
  COLLABORATION: "Collaboration",
  GUIDE: "Guide",
  CAMPAIGN: "Campagne",
  INFO: "Info",
  NEWS: "Actualité",
};

function inferNewsTitleFr(titleJa: string, existing?: string): string {
  if (existing && existing !== titleJa) return existing;
  for (const rule of KNOWN_FR_NEWS) {
    if (titleJa.includes(rule.match)) return rule.titleFr;
  }
  return titleJa;
}

function safeAssetFilename(url: string, fallbackPrefix: string): string {
  try {
    const clean = basename(new URL(url).pathname).replace(
      /[^a-zA-Z0-9._-]/g,
      "",
    );
    return clean || `${fallbackPrefix}.webp`;
  } catch {
    return `${fallbackPrefix}.webp`;
  }
}

async function writeSnapshotSafe(
  relDir: string,
  filename: string,
  content: string,
): Promise<void> {
  try {
    const outDir = crawlDataPath("inazuma-cross.jp", relDir);
    await mkdir(outDir, { recursive: true });
    await writeFile(join(outDir, filename), content, "utf8");
  } catch {
    // Disque partage optionnel hors VPS
  }
}

export async function crawlCross(): Promise<{
  success: boolean;
  error?: string;
}> {
  console.log(
    "[Crawl Cross] Recuperation du portail officiel inazuma-cross.jp...",
  );

  try {
    const [homeRes, guideRes, vrRes, sbRes, cbtRes] = await Promise.all([
      fetchHtmlResilient(CROSS_URLS.home),
      fetchHtmlResilient(CROSS_URLS.guide),
      fetchHtmlResilient(CROSS_URLS.vr2026),
      fetchHtmlResilient(CROSS_URLS.samuraiBlue),
      fetchHtmlResilient(CROSS_URLS.cbtReport),
    ]);

    if (!homeRes.ok || homeRes.html.length < 500) {
      return {
        success: false,
        error: `Echec chargement inazuma-cross.jp (status ${homeRes.status})`,
      };
    }

    await Promise.all([
      writeSnapshotSafe("ja", "index.html", homeRes.html),
      guideRes.ok
        ? writeSnapshotSafe("guide", "index.html", guideRes.html)
        : Promise.resolve(),
      vrRes.ok
        ? writeSnapshotSafe("victory-road-2026", "index.html", vrRes.html)
        : Promise.resolve(),
      sbRes.ok
        ? writeSnapshotSafe("samuraiblue2026", "index.html", sbRes.html)
        : Promise.resolve(),
      cbtRes.ok
        ? writeSnapshotSafe("cbt-report", "index.html", cbtRes.html)
        : Promise.resolve(),
    ]);

    const extracted = extraireCrossPortal(
      homeRes.html,
      guideRes.ok ? guideRes.html : undefined,
      vrRes.ok ? vrRes.html : undefined,
    );

    await writeSnapshotSafe(
      "ja",
      "meta.json",
      JSON.stringify(
        {
          crawledAt: new Date().toISOString(),
          transport: homeRes.transport,
          extracted,
        },
        null,
        2,
      ),
    );

    const repoRoot = resolveRepoRoot();
    if (repoRoot) {
      const jsonPath = join(
        repoRoot,
        "apps/rg-website/data/inazuma-cross.json",
      );
      const rawJson = await readFile(jsonPath, "utf8").catch(() => "{}");
      const current = JSON.parse(rawJson) as Record<string, unknown>;

      const prevStatus =
        (current.status as Record<string, unknown> | undefined) ?? {};
      const nextStatus: Record<string, unknown> = {
        ...prevStatus,
        phase: extracted.phase,
        phaseJa: extracted.phaseLabelJa,
        phaseFr:
          extracted.phase === "released"
            ? "Disponible maintenant (iOS / Android)"
            : "Pré-enregistrement ouvert",
      };

      const prevLinks =
        (current.links as Record<string, unknown> | undefined) ?? {};
      const nextLinks: Record<string, unknown> = {
        ...prevLinks,
        official: CROSS_URLS.home,
        guide: extracted.guideUrl ?? CROSS_URLS.guide,
        victoryRoad2026: extracted.vrCollabUrl ?? CROSS_URLS.vr2026,
        ...(extracted.storeUrl ? { store: extracted.storeUrl } : {}),
      };

      const prevNews = Array.isArray(current.news)
        ? (current.news as Array<Record<string, string>>)
        : [];
      const prevByKey = new Map<string, Record<string, string>>();
      for (const item of prevNews) {
        if (item.url) prevByKey.set(item.url, item);
        if (item.date && item.titleJa)
          prevByKey.set(`${item.date}:${item.titleJa}`, item);
      }

      const mergedNewsMap = new Map<string, Record<string, string>>();
      for (const item of extracted.news) {
        const old =
          prevByKey.get(item.url) ??
          prevByKey.get(`${item.date}:${item.titleJa}`);
        const mappedRule = KNOWN_FR_NEWS.find((r) =>
          item.titleJa.includes(r.match),
        );
        const normalized = {
          date: item.date,
          titleJa: item.titleJa,
          titleFr: inferNewsTitleFr(item.titleJa, old?.titleFr),
          url: item.url,
          tagJa: item.tagJa,
          tagFr:
            mappedRule?.tagFr ??
            old?.tagFr ??
            TAG_FR_MAP[item.tagJa] ??
            "Actualité",
        };
        mergedNewsMap.set(`${normalized.date}:${normalized.url}`, normalized);
      }
      for (const old of prevNews) {
        const k = `${old.date}:${old.url}`;
        if (!mergedNewsMap.has(k)) {
          mergedNewsMap.set(k, old);
        }
      }
      const nextNews = Array.from(mergedNewsMap.values()).sort((a, b) =>
        (b.date ?? "").localeCompare(a.date ?? ""),
      );

      let victoryRoad2026 = current.victoryRoad2026 as
        Record<string, unknown> | undefined;
      if (extracted.vrCollab) {
        const vrPublicDir = join(
          repoRoot,
          "apps/rg-website/public/inazuma-cross/vr2026",
        );
        let localKvImage: string | null = null;
        if (extracted.vrCollab.kvImageUrl) {
          const fileName = safeAssetFilename(
            extracted.vrCollab.kvImageUrl,
            "kv",
          );
          const res = await downloadAssetIfMissing(
            extracted.vrCollab.kvImageUrl,
            join(vrPublicDir, fileName),
          );
          if (res !== "failed") {
            localKvImage = `/inazuma-cross/vr2026/${fileName}`;
          }
        }

        const charasWithAssets = [];
        for (let i = 0; i < extracted.vrCollab.characters.length; i++) {
          const ch = extracted.vrCollab.characters[i]!;
          let localThumb: string | null = null;
          let localFullImage: string | null = null;
          let localHissatsuImage: string | null = null;

          if (ch.thumbUrl) {
            const name = safeAssetFilename(ch.thumbUrl, `chara-thumb-${i + 1}`);
            const st = await downloadAssetIfMissing(
              ch.thumbUrl,
              join(vrPublicDir, name),
            );
            if (st !== "failed") localThumb = `/inazuma-cross/vr2026/${name}`;
          }
          if (ch.fullImageUrl) {
            const name = safeAssetFilename(ch.fullImageUrl, `chara-${i + 1}`);
            const st = await downloadAssetIfMissing(
              ch.fullImageUrl,
              join(vrPublicDir, name),
            );
            if (st !== "failed")
              localFullImage = `/inazuma-cross/vr2026/${name}`;
          }
          if (ch.hissatsuImageUrl) {
            const name = safeAssetFilename(
              ch.hissatsuImageUrl,
              `hissatsu-${i + 1}`,
            );
            const st = await downloadAssetIfMissing(
              ch.hissatsuImageUrl,
              join(vrPublicDir, name),
            );
            if (st !== "failed")
              localHissatsuImage = `/inazuma-cross/vr2026/${name}`;
          }

          charasWithAssets.push({
            ...ch,
            nameFr: VR_CHARA_FR[ch.nameJa] ?? ch.nameJa,
            localThumb,
            localFullImage,
            localHissatsuImage,
          });
        }

        victoryRoad2026 = {
          url: CROSS_URLS.vr2026,
          period: extracted.vrCollab.period ?? "2026.11.17 - 2027.03.31",
          storyJa: extracted.vrCollab.storyJa,
          storyFr:
            "Collaboration spéciale entre Inazuma Eleven Cross et Inazuma Eleven: Victory Road réunissant Unmei Sasanami (Destin Billows) et son équipe.",
          kvImageUrl: extracted.vrCollab.kvImageUrl,
          localKvImage,
          characters: charasWithAssets,
        };
      }

      // Enrichissement facultatif des tweets @inazuma_cross récents depuis PostgreSQL
      let nextTweets = Array.isArray(current.tweets)
        ? (current.tweets as Array<Record<string, unknown>>)
        : [];
      try {
        const dbTweets = await sql<
          Array<{
            id: string;
            text: string | null;
            translation: string | null;
            created_at: Date | string | null;
          }>
        >`
          SELECT id, text, translation, created_at
          FROM tweets
          WHERE lower(author_username) = 'inazuma_cross'
          ORDER BY created_at DESC
          LIMIT 16
        `;
        if (dbTweets.length > 0) {
          const tweetById = new Map<string, Record<string, unknown>>();
          for (const t of nextTweets) {
            if (typeof t.id === "string") tweetById.set(t.id, t);
          }
          for (const row of dbTweets) {
            if (!tweetById.has(row.id) && row.text) {
              const isoDate =
                row.created_at instanceof Date
                  ? row.created_at.toISOString()
                  : String(row.created_at ?? "");
              tweetById.set(row.id, {
                id: row.id,
                date: isoDate.slice(0, 10),
                text: row.translation || row.text,
                url: `https://x.com/inazuma_cross/status/${row.id}`,
              });
            }
          }
          nextTweets = Array.from(tweetById.values())
            .sort((a, b) =>
              String(b.date ?? "").localeCompare(String(a.date ?? "")),
            )
            .slice(0, 16);
        }
      } catch {
        // Base de tweets facultative hors ligne
      }

      const updated = {
        ...current,
        crawledAt: new Date().toISOString(),
        status: nextStatus,
        links: nextLinks,
        news: nextNews,
        ...(victoryRoad2026 ? { victoryRoad2026 } : {}),
        ...(extracted.guidePoints
          ? { guidePoints: extracted.guidePoints }
          : {}),
        tweets: nextTweets,
      };

      await writeFile(
        jsonPath,
        `${JSON.stringify(updated, null, 2)}\n`,
        "utf8",
      );
      console.log(
        `[Crawl Cross] inazuma-cross.json synchronisé (${nextStatus.phase}, ${nextNews.length} actus, ${victoryRoad2026 ? (victoryRoad2026.characters as unknown[]).length : 0} persos collab VR).`,
      );
    }

    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error("[Crawl Cross] Erreur critique :", msg);
    return { success: false, error: msg };
  }
}
