import * as cheerio from "cheerio";
import CATALOG from "./catalog.json";
import { normalizeImagePath } from "./parser.js";

const ZUKAN_BASE_URL = "https://zukan.inazuma.jp";
const CLOUDFRONT_CDN_BASE = "https://dxi4wb638ujep.cloudfront.net/1";

export interface CharacterIntelTarget {
  slug: string;
  canonicalName: string;
  localizedNames: Record<string, string>;
  aliases: string[];
  zukanIds: string[];
  zukanHashes: string[];
  seedUrls: string[];
  searchQueries: string[];
}

export interface ZukanLiveCharacterEntry {
  characterId: string | null;
  name: string;
  nickname: string | null;
  element: string | null;
  position: string | null;
  paramUrl: string | null;
  imageUrl: string | null;
  sourceUrl: string;
}

export interface CatalogCharacterSnapshot {
  name: string;
  nickname: string | null;
  zukanHash: string | null;
  faceUrl: string | null;
  position: string | null;
  element: string | null;
  game: string | null;
  gender: string | null;
  stats: Record<string, number> | null;
}

export interface InagleCharacterIntelResult {
  slug: string;
  canonicalName: string;
  fetchedAt: string;
  catalogMatches: CatalogCharacterSnapshot[];
  liveEntries: ZukanLiveCharacterEntry[];
  discoveredAssetUrls: string[];
  discoveredPageUrls: string[];
  errors: string[];
}

export const DEFAULT_CHARACTER_TARGETS: Record<string, CharacterIntelTarget> = {
  "afuro-terumi": {
    slug: "afuro-terumi",
    canonicalName: "Afuro Terumi",
    localizedNames: {
      en: "Byron Love",
      fr: "Byron Love",
      ja: "亜風炉照美",
      nickname_en: "Aphrodi",
      nickname_ja: "アフロディ",
      reading: "亜風炉あふろ 照美てるみ",
    },
    aliases: [
      "Afuro Terumi",
      "Byron Love",
      "Aphrodi",
      "Aphrody",
      "亜風炉照美",
      "亜風炉 照美",
      "アフロディ",
      "아프로디",
    ],
    zukanIds: ["c01001900", "c05026590", "c07080010"],
    zukanHashes: [
      "k/d/y/dykb3jbxeis",
      "k/g/l/gltgpqjnrn0",
      "k/6/q/6q1af7euf9m",
    ],
    seedUrls: [
      "https://inazuma-eleven.fandom.com/wiki/Afuro_Terumi",
      "https://inazuma-eleven.fandom.com/fr/wiki/Byron_Love",
      "https://baike.baidu.com/fr/item/Afuro%20Terumi/4316218",
      "https://manga-encyclopedie.fandom.com/fr/wiki/Afuro_Terumi_(Inazuma_Eleven)",
      "https://zukan.inazuma.jp/en/chara_list/?q=hN2RnpKaoIyano2cl93FpN2-j5eNkJuG392igg==",
    ],
    searchQueries: [
      "byron love",
      '"Byron Love" inazuma eleven',
      '"Afuro Terumi" inazuma eleven',
      '"Aphrodi" inazuma eleven',
      "亜風炉照美 イナズマイレブン",
    ],
  },
};

export function resolveDefaultCharacterTarget(
  slugOrQuery?: string,
): CharacterIntelTarget {
  const normalized = (slugOrQuery ?? "afuro-terumi").trim().toLowerCase();
  if (!normalized || normalized === "watch") {
    return DEFAULT_CHARACTER_TARGETS["afuro-terumi"]!;
  }
  const direct = DEFAULT_CHARACTER_TARGETS[normalized];
  if (direct) return direct;

  for (const target of Object.values(DEFAULT_CHARACTER_TARGETS)) {
    if (
      target.canonicalName.toLowerCase() === normalized ||
      target.aliases.some((alias) => alias.toLowerCase() === normalized)
    ) {
      return target;
    }
  }

  const slug =
    normalized.replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") ||
    "custom-character";
  const label = slugOrQuery!.trim();
  return {
    slug,
    canonicalName: label,
    localizedNames: { en: label },
    aliases: [label],
    zukanIds: [],
    zukanHashes: [],
    seedUrls: [
      `https://zukan.inazuma.jp/en/chara_list/?q=${encodeZukanQuery({
        name_search: label,
      })}`,
    ],
    searchQueries: [label, `"${label}" inazuma eleven`],
  };
}

/** Encode a JSON filter object into the bitwise-NOT base64url `?q=` parameter used by Zukan. */
export function encodeZukanQuery(payload: unknown): string {
  const json = typeof payload === "string" ? payload : JSON.stringify(payload);
  const bytes = new TextEncoder().encode(json);
  const inverted = new Uint8Array(bytes.length);
  for (let i = 0; i < bytes.length; i++) {
    inverted[i] = ~bytes[i]! & 0xff;
  }
  return Buffer.from(inverted).toString("base64url");
}

/** Decode a Zukan `?q=` query parameter back into UTF-8 JSON (or parsed object). */
export function decodeZukanQuery<T = unknown>(q: string): T | null {
  try {
    const clean = decodeURIComponent(q).trim();
    const base64 = clean.replace(/-/g, "+").replace(/_/g, "/");
    const padded = base64.padEnd(Math.ceil(base64.length / 4) * 4, "=");
    const inverted = Buffer.from(padded, "base64");
    const restored = new Uint8Array(inverted.length);
    for (let i = 0; i < inverted.length; i++) {
      restored[i] = ~inverted[i]! & 0xff;
    }
    const text = new TextDecoder().decode(restored);
    return JSON.parse(text) as T;
  } catch {
    return null;
  }
}

export function buildZukanFaceUrl(
  zukanHash: string | null | undefined,
): string | null {
  if (!zukanHash) return null;
  const clean = zukanHash.replace(/^\/+/, "");
  if (!clean) return null;
  if (clean.startsWith("http://") || clean.startsWith("https://")) {
    return clean;
  }
  return `${CLOUDFRONT_CDN_BASE}/${clean}.webp`;
}

export function resolveCatalogCharacterMatches(
  aliases: readonly string[],
): CatalogCharacterSnapshot[] {
  const normalizedAliases = new Set(
    aliases.map((a) => a.trim().toLowerCase()).filter(Boolean),
  );
  const catalogRaw = CATALOG as unknown;
  const items = Array.isArray(catalogRaw)
    ? (catalogRaw as Array<Record<string, unknown>>)
    : Array.isArray((catalogRaw as { characters?: unknown })?.characters)
      ? (catalogRaw as { characters: Array<Record<string, unknown>> })
          .characters
      : [];

  const matches: CatalogCharacterSnapshot[] = [];
  const seen = new Set<string>();

  for (const raw of items) {
    const name = typeof raw.name === "string" ? raw.name.trim() : "";
    const nickname =
      typeof raw.nickname === "string" ? raw.nickname.trim() : null;
    if (!name) continue;
    const hit =
      normalizedAliases.has(name.toLowerCase()) ||
      (nickname ? normalizedAliases.has(nickname.toLowerCase()) : false);
    if (!hit) continue;

    const zukanHash = typeof raw.zukanHash === "string" ? raw.zukanHash : null;
    const game = typeof raw.game === "string" ? raw.game : null;
    const dedupKey = `${name}::${zukanHash ?? ""}::${game ?? ""}`;
    if (seen.has(dedupKey)) continue;
    seen.add(dedupKey);

    matches.push({
      name,
      nickname,
      zukanHash,
      faceUrl: buildZukanFaceUrl(zukanHash),
      position: typeof raw.position === "string" ? raw.position : null,
      element: typeof raw.element === "string" ? raw.element : null,
      game,
      gender: typeof raw.gender === "string" ? raw.gender : null,
      stats:
        raw.stats && typeof raw.stats === "object"
          ? (raw.stats as Record<string, number>)
          : null,
    });
  }

  return matches;
}

export function extractCharacterIdFromZukanHref(href: string): string | null {
  try {
    const parsed = new URL(href, ZUKAN_BASE_URL);
    const q = parsed.searchParams.get("q");
    if (!q) return null;
    const decoded = decodeZukanQuery<Record<string, unknown>>(q);
    if (!decoded) return null;
    const candidate =
      decoded.filter_chara_id_str ?? decoded.character_id ?? decoded.chara_id;
    if (typeof candidate === "string" && candidate.trim()) {
      return candidate.trim();
    }
    if (Array.isArray(candidate) && typeof candidate[0] === "string") {
      return candidate[0].trim();
    }
    return null;
  } catch {
    return null;
  }
}

export function parseZukanCharaListHtml(
  html: string,
  sourceUrl = `${ZUKAN_BASE_URL}/en/chara_list/`,
): ZukanLiveCharacterEntry[] {
  const $ = cheerio.load(html);
  const results: ZukanLiveCharacterEntry[] = [];
  const seen = new Set<string>();

  $('a[href*="/chara_param/?q="]').each((_, anchor) => {
    const el = $(anchor);
    const href = el.attr("href");
    if (!href) return;

    const characterId = extractCharacterIdFromZukanHref(href);
    const paramUrl = new URL(href, ZUKAN_BASE_URL).href;
    const container = el.closest(
      "li, tr, tbody, .charaListBox, .chara_list_box, article, div",
    );
    const rawName =
      el.find(".name, .charaName, p, span").first().text().trim() ||
      el.text().trim() ||
      container.find(".name, .charaName, td.name").first().text().trim();
    const name = rawName.replace(/\s+/g, " ").trim();
    if (!name && !characterId) return;

    const imgEl =
      el.find("img").first().length > 0
        ? el.find("img").first()
        : container.find("img").first();
    const imgRaw = imgEl.attr("data-src") || imgEl.attr("src") || null;
    const imageUrl = normalizeImagePath(imgRaw);

    const nickname =
      container.find(".nickname, .kana, .sub").first().text().trim() || null;
    const element =
      container.find("[data-element], .element").first().text().trim() || null;
    const position =
      container.find("[data-position], .position").first().text().trim() ||
      null;

    const key = `${characterId ?? name}::${paramUrl}`;
    if (seen.has(key)) return;
    seen.add(key);

    results.push({
      characterId,
      name: name || characterId || "Unknown",
      nickname,
      element,
      position,
      paramUrl,
      imageUrl,
      sourceUrl,
    });
  });

  return results;
}

export async function fetchZukanCharacterIntel(
  target: CharacterIntelTarget,
  options: { fetchImpl?: typeof fetch; timeoutMs?: number } = {},
): Promise<InagleCharacterIntelResult> {
  const fetchImpl = options.fetchImpl ?? fetch;
  const timeoutMs = options.timeoutMs ?? 15_000;
  const catalogMatches = resolveCatalogCharacterMatches(target.aliases);

  const queryUrls = new Set<string>();
  for (const url of target.seedUrls) {
    if (url.includes("zukan.inazuma.jp")) queryUrls.add(url);
  }
  const primaryAlias = target.localizedNames.en ?? target.canonicalName;
  queryUrls.add(
    `${ZUKAN_BASE_URL}/en/chara_list/?q=${encodeZukanQuery({
      name_search: primaryAlias,
    })}`,
  );
  if (target.localizedNames.ja) {
    queryUrls.add(
      `${ZUKAN_BASE_URL}/ja/chara_list/?q=${encodeZukanQuery({
        name_search: target.localizedNames.ja,
      })}`,
    );
  }

  const liveEntries: ZukanLiveCharacterEntry[] = [];
  const discoveredAssetUrls = new Set<string>();
  const discoveredPageUrls = new Set<string>();
  const errors: string[] = [];

  for (const m of catalogMatches) {
    if (m.faceUrl) discoveredAssetUrls.add(m.faceUrl);
  }
  for (const hash of target.zukanHashes) {
    const faceUrl = buildZukanFaceUrl(hash);
    if (faceUrl) discoveredAssetUrls.add(faceUrl);
  }

  for (const url of queryUrls) {
    try {
      const response = await fetchImpl(url, {
        headers: {
          "user-agent":
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36",
          accept: "text/html,application/xhtml+xml",
        },
        signal: AbortSignal.timeout(timeoutMs),
      });
      if (!response.ok) {
        errors.push(`HTTP ${response.status} on ${url}`);
        continue;
      }
      const html = await response.text();
      const rawEntries = parseZukanCharaListHtml(html, url);
      const aliasSet = new Set(target.aliases.map((a) => a.toLowerCase()));
      const idSet = new Set(target.zukanIds.map((id) => id.toLowerCase()));
      const matchingEntries = rawEntries.filter((entry) => {
        if (entry.characterId && idSet.has(entry.characterId.toLowerCase())) {
          return true;
        }
        const n = entry.name.toLowerCase();
        const nick = entry.nickname?.toLowerCase() ?? "";
        for (const alias of aliasSet) {
          if (n.includes(alias) || (nick && nick.includes(alias))) return true;
        }
        return false;
      });
      const selectedEntries =
        matchingEntries.length > 0 ? matchingEntries : rawEntries.slice(0, 10);
      for (const entry of selectedEntries) {
        liveEntries.push(entry);
        if (entry.imageUrl) discoveredAssetUrls.add(entry.imageUrl);
        if (entry.paramUrl) discoveredPageUrls.add(entry.paramUrl);
      }
    } catch (error) {
      errors.push(
        `Fetch error on ${url}: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }

  for (const charId of target.zukanIds) {
    const paramQ = encodeZukanQuery({ filter_chara_id_str: charId });
    discoveredPageUrls.add(`${ZUKAN_BASE_URL}/en/chara_param/?q=${paramQ}`);
  }

  return {
    slug: target.slug,
    canonicalName: target.canonicalName,
    fetchedAt: new Date().toISOString(),
    catalogMatches,
    liveEntries,
    discoveredAssetUrls: [...discoveredAssetUrls],
    discoveredPageUrls: [...discoveredPageUrls],
    errors,
  };
}
