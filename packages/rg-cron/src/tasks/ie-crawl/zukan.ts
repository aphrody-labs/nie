/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { cache } from "@rosegriffon/db/redis";
import {
  buildOrderMap,
  crawlZukanOrderPages,
  type ZukanOrderCharacter,
} from "@rosegriffon/inagle/zukan/order";
import { setWebCdpPageFactory } from "@rosegriffon/inagle/zukan/web-cdp";
import { inagleSql, sql } from "../../lib/db.js";
import { fetchHtmlResilient } from "../../lib/http-crawl";
import { PageWebView } from "../../lib/webview";

const REDIS_ORDERS_KEY = "crawl:zukan:orders";

const ELEMENT_EN_TO_FR: Readonly<Record<string, string>> = {
  Fire: "Feu",
  Wind: "Vent",
  Forest: "Forêt",
  Mountain: "Montagne",
  Void: "Néant",
  None: "Aucun",
};

const POSITION_EN_TO_FR: Readonly<Record<string, string>> = {
  GK: "Gardien",
  DF: "Défenseur",
  MF: "Milieu",
  FW: "Attaquant",
  Coach: "Entraîneur",
  COACH: "Entraîneur",
};

function normalizeSlugPart(raw: string): string {
  return raw
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
}

function mapGameToSeries(game: string | null | undefined): string {
  const g = (game ?? "").trim();
  if (!g) return "Victory Road";
  if (g === "Inazuma Eleven" || g.startsWith("Inazuma Eleven 1"))
    return "Inazuma Eleven";
  if (g.startsWith("Inazuma Eleven 2")) return "Inazuma Eleven 2";
  if (g.startsWith("Inazuma Eleven 3")) return "Inazuma Eleven 3";
  if (
    g.startsWith("Inazuma Eleven GO") &&
    !g.includes("Chrono") &&
    !g.includes("Galaxy")
  ) {
    return "Inazuma Eleven GO";
  }
  if (g.includes("Chrono Stone")) return "Chrono Stone";
  if (g.includes("Galaxy")) return "Galaxy";
  if (g.includes("Ares")) return "Ares";
  if (g.includes("Orion")) return "Orion";
  return "Victory Road";
}

function makeDeterministicId(hash: string): string {
  const leaf =
    hash
      .split("/")
      .pop()
      ?.replace(/[^a-zA-Z0-9_-]/g, "") ?? hash;
  return `zukan_${leaf}`.slice(0, 40);
}

async function fetchViaHttpSsr(): Promise<ZukanOrderCharacter[]> {
  let lastHtml = "";
  setWebCdpPageFactory(async () => ({
    goto: async (url: string) => {
      const res = await fetchHtmlResilient(url, {
        allowWebViewFallback: false,
        timeoutMs: 25_000,
      });
      lastHtml = res.html;
      return { status: res.status, ok: res.ok };
    },
    evaluate: async <T>() =>
      (lastHtml.includes("charaListBox") ? 1 : 0) as unknown as T,
    content: async () => lastHtml,
    close: async () => {},
  }));
  try {
    return await crawlZukanOrderPages();
  } finally {
    setWebCdpPageFactory(null);
  }
}

async function fetchViaWebView(): Promise<ZukanOrderCharacter[]> {
  setWebCdpPageFactory(() => PageWebView.connect());
  try {
    return await crawlZukanOrderPages();
  } finally {
    setWebCdpPageFactory(null);
  }
}

async function syncZukanToPg(
  db: typeof sql,
  label: string,
  allCharacters: ZukanOrderCharacter[],
  charByHash: Map<string, ZukanOrderCharacter>,
): Promise<{ updated: number; inserted: number }> {
  const dbChars = await db<
    Array<{
      id: string;
      zukan_hash: string | null;
      zukan_order: number | null;
      age_group: string | null;
      school_year: string | null;
      nickname: string | null;
    }>
  >`SELECT id, zukan_hash, zukan_order, age_group, school_year, nickname FROM inagle_characters`;

  const knownHashes = new Set(
    dbChars.map((r) => r.zukan_hash).filter((h): h is string => Boolean(h)),
  );

  let updated = 0;
  for (const row of dbChars) {
    if (!row.zukan_hash || row.zukan_hash.includes("icn_secret_character")) {
      continue;
    }
    const zukan = charByHash.get(row.zukan_hash);
    if (!zukan) continue;

    const nextAge = zukan.ageGroup || row.age_group;
    const nextYear = zukan.schoolYear || row.school_year;
    const nextNick = zukan.nickname || row.nickname;

    const changed =
      row.zukan_order !== zukan.order ||
      nextAge !== row.age_group ||
      nextYear !== row.school_year ||
      nextNick !== row.nickname;

    if (!changed) continue;

    await db`
      UPDATE inagle_characters
      SET zukan_order = ${zukan.order},
          age_group = ${nextAge},
          school_year = ${nextYear},
          nickname = ${nextNick},
          updated_at = NOW()
      WHERE id = ${row.id}
    `;
    updated++;
  }

  let inserted = 0;
  for (const ch of allCharacters) {
    if (!ch.zukanHash || ch.zukanHash.includes("icn_secret_character"))
      continue;
    if (knownHashes.has(ch.zukanHash)) continue;
    knownHashes.add(ch.zukanHash);

    const id = makeDeterministicId(ch.zukanHash);
    const leaf = ch.zukanHash.split("/").pop() ?? id;
    const slug = `${normalizeSlugPart(ch.name) || "player"}-${leaf.slice(0, 12)}`;
    const element = ch.element
      ? (ELEMENT_EN_TO_FR[ch.element] ?? ch.element)
      : null;
    const position = ch.position
      ? (POSITION_EN_TO_FR[ch.position] ?? ch.position)
      : null;
    const series = mapGameToSeries(ch.game);
    const imageUrl = `https://dxi4wb638ujep.cloudfront.net/1/${ch.zukanHash}.webp`;

    await db`
      INSERT INTO inagle_characters (
        id, chara_id, name_fr, name_en, element, position,
        image_url, series, slug, zukan_hash, zukan_order,
        age_group, school_year, nickname, is_primary, updated_at
      )
      VALUES (
        ${id}, ${id}, ${ch.name}, ${ch.name}, ${element}, ${position},
        ${imageUrl}, ${series}, ${slug}, ${ch.zukanHash}, ${ch.order},
        ${ch.ageGroup ?? null}, ${ch.schoolYear ?? null}, ${ch.nickname ?? null},
        true, NOW()
      )
      ON CONFLICT (id) DO UPDATE SET
        zukan_order = EXCLUDED.zukan_order,
        age_group = COALESCE(EXCLUDED.age_group, inagle_characters.age_group),
        school_year = COALESCE(EXCLUDED.school_year, inagle_characters.school_year),
        nickname = COALESCE(EXCLUDED.nickname, inagle_characters.nickname),
        updated_at = NOW()
    `;
    inserted++;
  }

  console.log(
    `[Crawl Zukan] ${label}: ${updated} mis à jour, ${inserted} nouveaux insérés.`,
  );
  return { updated, inserted };
}

export async function crawlZukanOrder(): Promise<{
  success: boolean;
  error?: string;
}> {
  console.log("[Crawl Zukan] Récupération de l'index Zukan officiel...");

  try {
    let allCharacters: ZukanOrderCharacter[];
    try {
      allCharacters = await fetchViaHttpSsr();
    } catch (ssrErr) {
      console.warn(
        "[Crawl Zukan] Repli WebView Chrome suite à erreur HTTP SSR :",
        ssrErr instanceof Error ? ssrErr.message : String(ssrErr),
      );
      allCharacters = await fetchViaWebView();
    }

    if (allCharacters.length === 0) {
      return { success: false, error: "Aucun personnage retourné par Zukan" };
    }

    const currentSnapshot = buildOrderMap(allCharacters);
    const charByHash = new Map<string, ZukanOrderCharacter>();
    for (const ch of allCharacters) {
      if (ch.zukanHash && !charByHash.has(ch.zukanHash)) {
        charByHash.set(ch.zukanHash, ch);
      }
    }
    const previousSnapshot =
      (await cache
        .get<Record<string, number>>(REDIS_ORDERS_KEY)
        .catch(() => null)) ?? {};
    let deltaCount = 0;
    for (const [hash, ord] of Object.entries(currentSnapshot)) {
      if (previousSnapshot[hash] !== ord) deltaCount++;
    }
    console.log(
      `[Crawl Zukan] ${allCharacters.length} personnages extraits (${deltaCount} évolutions vs cache Redis).`,
    );

    await syncZukanToPg(sql, "DATABASE_URL", allCharacters, charByHash);
    try {
      await syncZukanToPg(
        inagleSql,
        "INAGLE_DATABASE_URL",
        allCharacters,
        charByHash,
      );
    } catch (e) {
      console.warn(
        "[Crawl Zukan] Sync secondaire INAGLE_DATABASE_URL ignoré :",
        e,
      );
    }

    await cache
      .set(REDIS_ORDERS_KEY, currentSnapshot, 60 * 60 * 24 * 30)
      .catch(() => {});

    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error("[Crawl Zukan] Erreur critique :", msg);
    return { success: false, error: msg };
  }
}
