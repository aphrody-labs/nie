/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import * as cheerio from "cheerio";

export interface ExtractedCrossNewsItem {
  date: string;
  titleJa: string;
  url: string;
  tagJa: string;
}

export interface ExtractedCrossVrChara {
  nameJa: string;
  roleOrPosition: string;
  elementJa: string | null;
  cv: string | null;
  descJa: string;
  thumbUrl: string | null;
  fullImageUrl: string | null;
  hissatsuNameJa: string | null;
  hissatsuImageUrl: string | null;
}

export interface ExtractedCrossPortal {
  phase: "released" | "pre-registration";
  phaseLabelJa: string;
  storeUrl: string | null;
  guideUrl: string | null;
  vrCollabUrl: string | null;
  news: ExtractedCrossNewsItem[];
  vrCollab?: {
    period: string | null;
    storyJa: string | null;
    kvImageUrl: string | null;
    characters: ExtractedCrossVrChara[];
  };
  guidePoints?: Array<{ titleJa: string; bodyJa: string }>;
}

export interface ExtractedPatchNoteDetail {
  title: string;
  date: string;
  contentHtml: string;
}

export interface ExtractedRePortal {
  title: string;
  genre: string;
  platforms: string;
  languages: string;
  releaseDate: string;
  teaserImageUrl: string | null;
  logoImageUrl: string | null;
}

export interface ExtractedSeriesGame {
  href: string;
  platforms: string;
  titleJa: string;
  thumbnailUrl: string | null;
}

const ORIGIN_INAZUMA = "https://www.inazuma.jp";
const ORIGIN_CROSS = "https://www.inazuma-cross.jp";

export function toAbsoluteUrl(base: string, raw: string | undefined): string {
  if (!raw) return base;
  try {
    return new URL(raw, base).toString();
  } catch {
    return raw;
  }
}

/** Normalise une date `2026.11.6`, `2026/11/12` ou `2026年11月12日` en `YYYY.MM.DD`. */
export function normalizeDotDate(raw: string): string {
  const m = raw.match(/(\d{4})[./年-](\d{1,2})[./月-](\d{1,2})/);
  if (!m) return raw.trim();
  const year = m[1]!;
  const month = m[2]!.padStart(2, "0");
  const day = m[3]!.padStart(2, "0");
  return `${year}.${month}.${day}`;
}

export function normalizeIsoDate(raw: string): string {
  return normalizeDotDate(raw).replaceAll(".", "-");
}

function inferCrossTag(titleJa: string): string {
  if (titleJa.includes("コラボ")) return "COLLABORATION";
  if (titleJa.includes("初心者") || titleJa.includes("ガイド")) return "GUIDE";
  if (titleJa.includes("キャンペーン")) return "CAMPAIGN";
  if (titleJa.includes("公式サイト") || titleJa.includes("公開")) return "INFO";
  return "NEWS";
}

export function extraireCrossPortal(
  homeHtml: string,
  guideHtml?: string,
  vrCollabHtml?: string,
): ExtractedCrossPortal {
  const $ = cheerio.load(homeHtml);
  const firstViewText = $(".firstview").text().replace(/\s+/g, " ").trim();
  const bodyText = $("body").text().replace(/\s+/g, " ");

  const isReleased =
    firstViewText.includes("好評配信中") || bodyText.includes("好評配信中");

  const phaseLabelJa = isReleased ? "好評配信中！" : "事前登録受付中";

  const news: ExtractedCrossNewsItem[] = [];
  const seen = new Set<string>();
  $(".news-list li, .news-items > li, .news-items > div, .news-items > a").each(
    (_, el) => {
      const node = $(el);
      const rawText = node.text().replace(/\s+/g, " ").trim();
      const dateMatch = rawText.match(/\d{4}\/\d{1,2}\/\d{1,2}/);
      if (!dateMatch) return;
      const date = normalizeIsoDate(dateMatch[0]);
      const titleJa = rawText
        .slice(rawText.indexOf(dateMatch[0]) + dateMatch[0].length)
        .trim();
      if (!titleJa) return;
      const href = node.is("a")
        ? node.attr("href")
        : node.find("a[href]").first().attr("href");
      const url = href ? toAbsoluteUrl(ORIGIN_CROSS, href) : ORIGIN_CROSS;
      const key = `${date}:${titleJa}`;
      if (seen.has(key)) return;
      seen.add(key);
      news.push({
        date,
        titleJa,
        url,
        tagJa: inferCrossTag(titleJa),
      });
    },
  );

  const links = $("a[href]")
    .map((_, el) => $(el).attr("href") ?? "")
    .get();
  const storeUrl =
    links.find((h) => h.includes("store.inazuma-cross.jp")) ?? null;
  const guideHref = links.find((h) => h.includes("/guide"));
  const vrHref = links.find((h) => h.includes("/victory-road-2026"));

  const result: ExtractedCrossPortal = {
    phase: isReleased ? "released" : "pre-registration",
    phaseLabelJa,
    storeUrl,
    guideUrl: guideHref ? toAbsoluteUrl(ORIGIN_CROSS, guideHref) : null,
    vrCollabUrl: vrHref ? toAbsoluteUrl(ORIGIN_CROSS, vrHref) : null,
    news,
  };

  if (guideHtml) {
    const $g = cheerio.load(guideHtml);
    const guidePoints: Array<{ titleJa: string; bodyJa: string }> = [];
    $g(".point").each((_, el) => {
      const point = $g(el);
      const titleJa = point
        .find(".point-title")
        .first()
        .text()
        .replace(/\s+/g, " ")
        .trim();
      const bodyJa = point.text().replace(/\s+/g, " ").trim().slice(0, 320);
      if (titleJa) guidePoints.push({ titleJa, bodyJa });
    });
    if (guidePoints.length > 0) result.guidePoints = guidePoints;
  }

  if (vrCollabHtml) {
    const $v = cheerio.load(vrCollabHtml);
    const kvText = $v(".kv").text().replace(/\s+/g, " ").trim();
    const periodMatch = kvText.match(
      /(\d{4}\.\d{2}\.\d{2})[^\d]{0,12}(\d{4}\.\d{2}\.\d{2})/,
    );
    const period = periodMatch ? `${periodMatch[1]} - ${periodMatch[2]}` : null;
    const storyJa =
      $v(".story")
        .text()
        .replace(/\s+/g, " ")
        .replace(/^コラボオリジナルストーリー\s*STORY\s*あらすじ\s*/, "")
        .trim() || null;
    const kvImgRaw = $v(".kv img").first().attr("src");
    const kvImageUrl = kvImgRaw ? toAbsoluteUrl(ORIGIN_CROSS, kvImgRaw) : null;

    const characters: ExtractedCrossVrChara[] = [];
    $v(
      ".character .swiper-slide, .character [class*='chara-card'], .character li",
    ).each((_, el) => {
      const card = $v(el);
      const nameJa = card
        .find(".profile-name")
        .first()
        .text()
        .replace(/\s+/g, " ")
        .trim();
      if (!nameJa) return;
      const pos = card
        .find("[class*='pos'], [class*='position']")
        .first()
        .text()
        .replace(/\s+/g, " ")
        .trim();
      const cvMatch = card.text().match(/CV[：:]\s*\[?([^\]\n]+)\]?/);
      const thumbRaw = card.find("img[src*='chara-thumb']").first().attr("src");
      const fullRaw = card
        .find("img[src*='quality100/chara']")
        .first()
        .attr("src");
      const hisImg = card.find("img[src*='hissatsu-']").first();
      const hisAlt = hisImg.attr("alt") ?? "";
      const hisNameMatch = hisAlt.match(/必殺技\s*(.+)$/);
      characters.push({
        nameJa,
        roleOrPosition: pos || "Victory Road",
        elementJa: null,
        cv: cvMatch ? cvMatch[1]!.trim() : null,
        descJa: card.find("p").last().text().replace(/\s+/g, " ").trim(),
        thumbUrl: thumbRaw ? toAbsoluteUrl(ORIGIN_CROSS, thumbRaw) : null,
        fullImageUrl: fullRaw ? toAbsoluteUrl(ORIGIN_CROSS, fullRaw) : null,
        hissatsuNameJa: hisNameMatch ? hisNameMatch[1]!.trim() : null,
        hissatsuImageUrl: hisImg.attr("src")
          ? toAbsoluteUrl(ORIGIN_CROSS, hisImg.attr("src"))
          : null,
      });
    });

    // Secours par les miniatures .chara-thumb quand les slides détaillées sont clonées/séparées
    if (characters.length === 0) {
      $v("img[src*='chara-thumb-'][alt]").each((_, el) => {
        const alt = ($v(el).attr("alt") ?? "").trim();
        const src = $v(el).attr("src") ?? "";
        if (!alt || src.includes("-hover") || src.includes("-current")) return;
        const m = alt.match(/^(.+?)[（(]([^）)]+)[）)]$/);
        const nameJa = (m?.[1] ?? alt).trim();
        const roleOrPosition = (m?.[2] ?? "").trim();
        const idxMatch = src.match(/chara-thumb-(\d+)\.webp/);
        const idx = idxMatch?.[1];
        const hissatsuNode = idx
          ? $v(`img[src*='hissatsu-${idx}.webp']`).first()
          : null;
        const hissatsuAlt = hissatsuNode?.attr("alt") ?? "";
        const hissatsuMatch = hissatsuAlt.match(/必殺技\s*(.+)$/);
        const fullNode = idx
          ? $v(`img[src*='quality100/chara-${idx}.webp']`).first()
          : null;

        characters.push({
          nameJa,
          roleOrPosition,
          elementJa: null,
          cv: null,
          descJa: "",
          thumbUrl: toAbsoluteUrl(ORIGIN_CROSS, src),
          fullImageUrl: fullNode?.attr("src")
            ? toAbsoluteUrl(ORIGIN_CROSS, fullNode.attr("src"))
            : null,
          hissatsuNameJa: hissatsuMatch ? hissatsuMatch[1]!.trim() : null,
          hissatsuImageUrl: hissatsuNode?.attr("src")
            ? toAbsoluteUrl(ORIGIN_CROSS, hissatsuNode.attr("src"))
            : null,
        });
      });
    }

    result.vrCollab = {
      period,
      storyJa,
      kvImageUrl,
      characters,
    };
  }

  return result;
}

export function extrairePatchNoteDetail(
  html: string,
  _url: string,
  fallbackTitle: string,
): ExtractedPatchNoteDetail {
  const $ = cheerio.load(html);
  const dateRaw =
    $(".dateBox, .date").first().text().replace(/\s+/g, " ").trim() || "";
  const dateMatch = dateRaw.match(/\d{4}[./-]\d{1,2}[./-]\d{1,2}/);
  const date = dateMatch ? normalizeDotDate(dateMatch[0]) : "";

  const detail = $(".patchnote .detail").first().clone();
  detail.find("script, style, .anchorLink, .updateBtn").remove();
  // Absolutiser les images relatives
  detail.find("img[src]").each((_, el) => {
    const src = $(el).attr("src");
    if (src && !src.startsWith("http")) {
      $(el).attr("src", toAbsoluteUrl(ORIGIN_INAZUMA, src));
    }
  });
  detail.find("a[href]").each((_, el) => {
    const href = $(el).attr("href");
    if (href && href.startsWith("/")) {
      $(el).attr("href", toAbsoluteUrl(ORIGIN_INAZUMA, href));
    }
  });

  const leadText =
    $(".patchnote .lead, .patchnote .detail > p")
      .first()
      .text()
      .replace(/\s+/g, " ")
      .trim() || "";
  const pageTitle = $("title")
    .text()
    .replace(/\s*\|.*$/, "")
    .trim();
  const title =
    fallbackTitle ||
    pageTitle ||
    leadText.slice(0, 120) ||
    "Update Information";

  const contentHtml = detail.html()?.trim() || "";
  return { title, date, contentHtml };
}

export function extraireRePortal(html: string): ExtractedRePortal {
  const $ = cheerio.load(html);
  const specMap = new Map<string, string>();
  $("dt").each((_, el) => {
    const k = $(el).text().replace(/\s+/g, " ").trim().toLowerCase();
    const v = $(el).next("dd").text().replace(/\s+/g, " ").trim();
    if (k && v) specMap.set(k, v);
  });
  $("tr").each((_, el) => {
    const k = $(el).find("th").text().replace(/\s+/g, " ").trim().toLowerCase();
    const v = $(el).find("td").text().replace(/\s+/g, " ").trim();
    if (k && v) specMap.set(k, v);
  });

  const teaserSrc =
    $("img[src*='img_pv_pc']").first().attr("src") ||
    $("meta[property='og:image']").attr("content");
  const logoSrc = $("img[src*='img_logo.png']").first().attr("src");

  return {
    title:
      specMap.get("title") ??
      specMap.get("タイトル") ??
      $("title").text().trim(),
    genre: specMap.get("genre") ?? specMap.get("ジャンル") ?? "",
    platforms:
      specMap.get("supported platform") ?? specMap.get("対応機種") ?? "",
    languages:
      specMap.get("supported languages") ?? specMap.get("対応言語") ?? "",
    releaseDate: specMap.get("release date") ?? specMap.get("発売日") ?? "2026",
    teaserImageUrl: teaserSrc ? toAbsoluteUrl(ORIGIN_INAZUMA, teaserSrc) : null,
    logoImageUrl: logoSrc ? toAbsoluteUrl(ORIGIN_INAZUMA, logoSrc) : null,
  };
}

export function extraireSeriesPortal(homeHtml: string): ExtractedSeriesGame[] {
  const $ = cheerio.load(homeHtml);
  const items: ExtractedSeriesGame[] = [];
  const seen = new Set<string>();

  $(".gameList li").each((_, el) => {
    const node = $(el);
    const href = node.find("a").first().attr("href") ?? "";
    const img = node.find("img").first();
    const src = img.attr("src") ?? "";
    const alt = (img.attr("alt") ?? "").trim();
    const fullText = node.text().replace(/\s+/g, " ").trim();
    if (!href && !alt) return;
    const key = `${href}:${alt}`;
    if (seen.has(key)) return;
    seen.add(key);

    items.push({
      href: href ? toAbsoluteUrl(ORIGIN_INAZUMA, href) : ORIGIN_INAZUMA,
      platforms:
        alt && fullText.endsWith(alt)
          ? fullText.slice(0, fullText.length - alt.length).trim()
          : fullText,
      titleJa: alt || fullText,
      thumbnailUrl: src ? toAbsoluteUrl(ORIGIN_INAZUMA, src) : null,
    });
  });

  return items;
}
