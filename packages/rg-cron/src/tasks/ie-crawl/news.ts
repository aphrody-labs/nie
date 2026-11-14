/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import * as cheerio from "cheerio";
import { sql } from "../../lib/db";
import { fetchHtmlResilient, resolveRepoRoot } from "../../lib/http-crawl";
import { crawlDataPath } from "../../lib/shared-path";
import { extrairePatchNotes, extraireTopics } from "./html-extract";
import { extrairePatchNoteDetail, normalizeDotDate } from "./portal-extract";

const TOPICS_SOURCES = [
  { lang: "fr", url: "https://www.inazuma.jp/victory-road/fr/topics/" },
  { lang: "en", url: "https://www.inazuma.jp/victory-road/en/topics/" },
  { lang: "ja", url: "https://www.inazuma.jp/victory-road/topics/" },
] as const;

const PATCH_PLATFORMS: ReadonlyArray<{
  slug: "ps-steam" | "switch" | "xbox";
  platforms: string[];
}> = [
  { slug: "ps-steam", platforms: ["ps4", "ps5", "steam"] },
  { slug: "switch", platforms: ["switch"] },
  { slug: "xbox", platforms: ["xbox"] },
];

async function writeSafe(
  dir: string,
  file: string,
  content: string,
): Promise<void> {
  try {
    await mkdir(dir, { recursive: true });
    await writeFile(join(dir, file), content, "utf8");
  } catch {
    // Disque partage optionnel hors VPS
  }
}

function versionSortScore(verId: string): number {
  const m = verId.match(/ver_(\d+)_(\d+)_(\d+)/i);
  if (!m) return 0;
  return (
    Number.parseInt(m[1]!, 10) * 10_000 +
    Number.parseInt(m[2]!, 10) * 100 +
    Number.parseInt(m[3]!, 10)
  );
}

export async function crawlNews(): Promise<{
  success: boolean;
  error?: string;
}> {
  console.log("[Crawl News] Récupération Topics + Patch Notes Victory Road...");

  try {
    const repoRoot = resolveRepoRoot();
    const manifestTopics: Array<{
      id: string;
      lang: string;
      date: string;
      title: string;
      url: string;
      summary: string;
      imageUrl: string | null;
    }> = [];
    const seenTopicIds = new Set<string>();

    // 1. Topics FR / EN / JA
    for (const src of TOPICS_SOURCES) {
      const res = await fetchHtmlResilient(src.url);
      if (!res.ok || res.html.length < 400) continue;

      await writeSafe(
        crawlDataPath("inazuma.jp", "victory-road", src.lang, "topics"),
        "index.html",
        res.html,
      );

      const topics = extraireTopics(res.html);
      for (const topic of topics.slice(0, 12)) {
        const key = `${src.lang}:${topic.id}`;
        if (seenTopicIds.has(key)) continue;
        seenTopicIds.add(key);

        const detailRes = await fetchHtmlResilient(topic.url);
        let summary = "";
        let imageUrl: string | null = null;
        if (detailRes.ok && detailRes.html.length > 400) {
          const $d = cheerio.load(detailRes.html);
          summary = $d("article p, .topicsDetail p, main p")
            .slice(0, 3)
            .text()
            .replace(/\s+/g, " ")
            .trim()
            .slice(0, 360);
          const ogImg = $d("meta[property='og:image']").attr("content");
          const bodyImg = $d("article img, .topicsDetail img")
            .first()
            .attr("src");
          const rawImg = bodyImg || ogImg;
          if (rawImg) {
            imageUrl = rawImg.startsWith("http")
              ? rawImg
              : new URL(rawImg, "https://www.inazuma.jp").toString();
          }

          await writeSafe(
            crawlDataPath(
              "inazuma.jp",
              "victory-road",
              src.lang,
              "topics",
              topic.id,
            ),
            "index.html",
            detailRes.html,
          );
        }

        manifestTopics.push({
          id: topic.id,
          lang: src.lang,
          date: normalizeDotDate(topic.date),
          title: topic.title,
          url: topic.url,
          summary,
          imageUrl,
        });
      }
    }

    if (repoRoot && manifestTopics.length > 0) {
      const target = join(
        repoRoot,
        "packages/inagle/data/official-topics.json",
      );
      const existingRaw = await readFile(target, "utf8").catch(() => "[]");
      const existing = JSON.parse(existingRaw) as typeof manifestTopics;
      const byKey = new Map<string, (typeof manifestTopics)[number]>();
      for (const t of existing) byKey.set(`${t.lang}:${t.id}`, t);
      for (const t of manifestTopics) byKey.set(`${t.lang}:${t.id}`, t);
      const merged = Array.from(byKey.values()).sort((a, b) =>
        b.date.localeCompare(a.date),
      );
      await writeFile(
        target,
        `${JSON.stringify({ updatedAt: new Date().toISOString(), topics: merged }, null, 2)}\n`,
        "utf8",
      );
    }

    // 2. Patch Notes EN + JA fallback pour les dernières versions non traduites (ex: ver_7_2_2)
    let existingIds = new Set<string>();
    try {
      const existingRows = await sql<
        Array<{ id: string; content_html: string | null }>
      >`
        SELECT id, content_html FROM patch_notes
      `;
      existingIds = new Set(
        existingRows
          .filter((r) => (r.content_html?.length ?? 0) > 40)
          .map((r) => r.id),
      );
      // Normaliser les anciennes dates non paddées (`2025.12.6` -> `2025.12.06`)
      const allDates = await sql<Array<{ id: string; date: string }>>`
        SELECT id, date FROM patch_notes
      `;
      for (const row of allDates) {
        const norm = normalizeDotDate(row.date);
        if (norm && norm !== row.date) {
          await sql`UPDATE patch_notes SET date = ${norm} WHERE id = ${row.id}`;
        }
      }
    } catch (e) {
      console.warn("[Crawl News] Connexion patch_notes indisponible :", e);
    }

    const candidates = new Map<
      string,
      {
        dbId: string;
        platSlug: string;
        platforms: string[];
        verId: string;
        url: string;
        title: string;
        date: string;
        lang: "en" | "ja";
      }
    >();

    for (const lang of ["en", "ja"] as const) {
      const langPrefix = lang === "en" ? "/victory-road/en" : "/victory-road";
      for (const plat of PATCH_PLATFORMS) {
        const indexUrl = `https://www.inazuma.jp${langPrefix}/patchnote/${plat.slug}/`;
        const indexRes = await fetchHtmlResilient(indexUrl);
        if (!indexRes.ok || indexRes.html.length < 400) continue;

        await writeSafe(
          crawlDataPath(
            "inazuma.jp",
            "victory-road",
            lang,
            "patchnote",
            plat.slug,
          ),
          "index.html",
          indexRes.html,
        );

        const list = extrairePatchNotes(indexRes.html);
        for (const item of list) {
          // Exclure les liens croisés d'autres plateformes présents dans la navigation
          if (!item.url.includes(`/patchnote/${plat.slug}/`)) continue;
          const dbId = `${plat.slug}_${item.id}`;
          if (!candidates.has(dbId)) {
            candidates.set(dbId, {
              dbId,
              platSlug: plat.slug,
              platforms: plat.platforms,
              verId: item.id,
              url: item.url,
              title: item.title,
              date: normalizeDotDate(item.date),
              lang,
            });
          }
        }
      }
    }

    // Trier par version décroissante et synchroniser les manquants ou plus récents
    const sortedCandidates = Array.from(candidates.values()).sort(
      (a, b) => versionSortScore(b.verId) - versionSortScore(a.verId),
    );

    let upsertedCount = 0;
    for (const item of sortedCandidates) {
      const needsFetch = !existingIds.has(item.dbId) || upsertedCount < 6;
      if (!needsFetch) continue;

      const detailRes = await fetchHtmlResilient(item.url);
      if (!detailRes.ok || detailRes.html.length < 300) continue;

      const detail = extrairePatchNoteDetail(
        detailRes.html,
        item.url,
        item.title,
      );
      const normDate = detail.date || item.date;

      await writeSafe(
        crawlDataPath(
          "inazuma.jp",
          "victory-road",
          item.lang,
          "patchnote",
          item.platSlug,
          item.verId,
        ),
        "index.html",
        detailRes.html,
      );

      try {
        await sql`
          INSERT INTO patch_notes (id, title, date, platform, url, content_html, updated_at)
          VALUES (
            ${item.dbId},
            ${detail.title || item.title},
            ${normDate},
            ${JSON.stringify(item.platforms)}::jsonb,
            ${item.url},
            ${detail.contentHtml},
            NOW()
          )
          ON CONFLICT (id) DO UPDATE SET
            title = EXCLUDED.title,
            date = CASE WHEN EXCLUDED.date <> '' THEN EXCLUDED.date ELSE patch_notes.date END,
            platform = EXCLUDED.platform,
            url = EXCLUDED.url,
            content_html = CASE
              WHEN length(EXCLUDED.content_html) > 40 THEN EXCLUDED.content_html
              ELSE patch_notes.content_html
            END,
            updated_at = NOW()
        `;
        upsertedCount++;
      } catch (e) {
        console.warn(`[Crawl News] Upsert ${item.dbId} ignoré :`, e);
      }
    }

    console.log(
      `[Crawl News] Termine (${manifestTopics.length} topics, ${sortedCandidates.length} patchs détectés, ${upsertedCount} synchronisés en DB).`,
    );
    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error("[Crawl News] Erreur critique :", msg);
    return { success: false, error: msg };
  }
}
