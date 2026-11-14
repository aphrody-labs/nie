/**
 * IEVR VFS multilingual glossary + 2026 Japanese NLP/translation pipeline.
 *
 * Combines:
 * - Official LEVEL-5 IEVR VFS dictionary (`packages/inagle/data/ievr-i18n.json`, Zod-validated)
 * - Longest-match bucketed Japanese entity matcher preserving canonical game nouns
 * - BudouX phrase segmentation (`budoux`)
 * - Kuromoji + Kuroshiro kanji-to-kana/romaji analysis with WanaKana fallback
 * - `google-translate-api-x` contextual neural translation with entity placeholder protection
 */

import { loadDefaultJapaneseParser } from "budoux";
import { isKana, toHiragana, toRomaji as wanakanaToRomaji } from "wanakana";
import { z } from "zod";
import { formatJapaneseName } from "./format-description";
import { containsJapanese, stripRubyAnnotations } from "./japanese-detect";

export const IEVR_VFS_LOCALES = [
  "ja",
  "en",
  "fr",
  "es",
  "de",
  "it",
  "pt",
  "zh_hans",
  "zh_hant",
] as const;

export type IevrVfsLocale = (typeof IEVR_VFS_LOCALES)[number];
export type TranslateTargetLang = "fr" | "en" | "es";

export const IevrTermEntrySchema = z.object({
  category: z.string(),
  hash_id: z.string().optional(),
  ja: z.string().optional(),
  ja_raw: z.string().optional(),
  romaji: z.string().optional(),
  en: z.string().optional(),
  fr: z.string().optional(),
  es: z.string().optional(),
  de: z.string().optional(),
  it: z.string().optional(),
  pt: z.string().optional(),
  zh_hans: z.string().optional(),
  zh_hant: z.string().optional(),
});

export type IevrTermEntry = z.infer<typeof IevrTermEntrySchema>;

export const IevrI18nBundleSchema = z.object({
  source: z.string().optional(),
  langs: z.array(z.string()),
  hash_entry_count: z.number().optional(),
  ja_term_count: z.number().optional(),
  canonical_hits: z.number().optional(),
  categories: z.record(z.string(), z.number()).optional(),
  hash_entries: z.record(z.string(), IevrTermEntrySchema),
  ja_to_translations: z.record(z.string(), IevrTermEntrySchema),
});

export type IevrI18nBundle = z.infer<typeof IevrI18nBundleSchema>;

export interface MatchedIevrEntity {
  source: string;
  target: string;
  category: string;
  hashId?: string;
  romaji?: string;
  start: number;
  end: number;
  translations: Partial<Record<TranslateTargetLang | "ja", string>>;
}

export interface TranslateJpResult {
  source: string;
  sourceLang: "ja" | "en";
  targetLang: TranslateTargetLang;
  translated: string;
  reading: string;
  romaji: string;
  matchedEntities: MatchedIevrEntity[];
  segments: string[];
  usedRemote: boolean;
}

export interface IevrGlossaryEntry {
  key: string;
  target: string;
  category: string;
  hashId?: string;
  romaji?: string;
  translations: Partial<Record<TranslateTargetLang | "ja", string>>;
}

export interface IevrGlossaryIndex {
  readonly byFirstChar: ReadonlyMap<string, readonly IevrGlossaryEntry[]>;
  readonly exactJa: ReadonlyMap<string, IevrGlossaryEntry>;
  readonly exactCleanJa: ReadonlyMap<string, IevrGlossaryEntry>;
  readonly targetLang: TranslateTargetLang;
  readonly termCount: number;
}

let cachedBudouxParser: ReturnType<typeof loadDefaultJapaneseParser> | null =
  null;
const bundleCache = new Map<string, Promise<IevrI18nBundle | null>>();
const indexCache = new WeakMap<
  IevrI18nBundle,
  Partial<Record<TranslateTargetLang, IevrGlossaryIndex>>
>();

function getBudouxParser() {
  if (!cachedBudouxParser) {
    cachedBudouxParser = loadDefaultJapaneseParser();
  }
  return cachedBudouxParser;
}

function pickEntryTranslation(
  entry: Pick<IevrTermEntry, "fr" | "en" | "es" | "ja">,
  targetLang: TranslateTargetLang,
): string | undefined {
  if (targetLang === "es") return entry.es || entry.en || entry.fr;
  if (targetLang === "en") return entry.en || entry.fr || entry.es;
  return entry.fr || entry.en || entry.es;
}

async function resolveDefaultBundlePath(): Promise<string | null> {
  const { existsSync } = await import("node:fs");
  const { dirname, join, resolve } = await import("node:path");
  if (typeof import.meta.dir === "string") {
    const direct = resolve(import.meta.dir, "../../data/ievr-i18n.json");
    if (existsSync(direct)) return direct;
  }
  let dir = resolve(process.cwd());
  for (let depth = 0; depth < 6; depth++) {
    const monoPath = join(dir, "packages", "inagle", "data", "ievr-i18n.json");
    if (existsSync(monoPath)) return monoPath;
    const localPath = join(dir, "data", "ievr-i18n.json");
    if (existsSync(localPath)) return localPath;
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return null;
}

/**
 * Loads and Zod-validates `packages/inagle/data/ievr-i18n.json`.
 * Returns `null` when unavailable (e.g. client browser bundle or missing export).
 */
export async function loadIevrI18n(
  customPath?: string,
): Promise<IevrI18nBundle | null> {
  if (typeof process === "undefined") return null;
  const resolvedPath = customPath || (await resolveDefaultBundlePath());
  if (!resolvedPath) return null;
  const hit = bundleCache.get(resolvedPath);
  if (hit) return hit;

  const task = (async () => {
    try {
      const fs = await import("node:fs/promises");
      const raw = await fs.readFile(resolvedPath, "utf8");
      const parsed = JSON.parse(raw) as unknown;
      return IevrI18nBundleSchema.parse(parsed);
    } catch {
      return null;
    }
  })();

  bundleCache.set(resolvedPath, task);
  return task;
}

/**
 * Builds a bucketed longest-match index from an IEVR i18n bundle + optional extra terms.
 */
export function createIevrGlossaryIndex(
  bundle: IevrI18nBundle | null,
  targetLang: TranslateTargetLang = "fr",
  extraEntries: Array<{
    ja: string;
    fr?: string;
    en?: string;
    es?: string;
    category?: string;
    hashId?: string;
  }> = [],
): IevrGlossaryIndex {
  if (bundle && extraEntries.length === 0) {
    const perLang = indexCache.get(bundle);
    if (perLang?.[targetLang]) return perLang[targetLang]!;
  }

  const exactJa = new Map<string, IevrGlossaryEntry>();
  const exactCleanJa = new Map<string, IevrGlossaryEntry>();
  const buckets = new Map<string, IevrGlossaryEntry[]>();

  const register = (rawJa: string, entry: IevrTermEntry) => {
    const clean = stripRubyAnnotations(formatJapaneseName(rawJa)).trim();
    if (!clean) return;
    const target = pickEntryTranslation(entry, targetLang);
    if (!target) return;

    const record: IevrGlossaryEntry = {
      key: clean,
      target,
      category: entry.category,
      hashId: entry.hash_id,
      romaji: entry.romaji,
      translations: {
        ja: clean,
        fr: entry.fr,
        en: entry.en,
        es: entry.es,
      },
    };

    if (!exactJa.has(rawJa)) exactJa.set(rawJa, record);
    if (!exactCleanJa.has(clean)) exactCleanJa.set(clean, record);

    const first = clean[0];
    if (!first || clean.length < 2) return;
    let bucket = buckets.get(first);
    if (!bucket) {
      bucket = [];
      buckets.set(first, bucket);
    }
    bucket.push(record);
  };

  if (bundle) {
    for (const [rawJa, entry] of Object.entries(bundle.ja_to_translations)) {
      register(rawJa, entry);
    }
  }

  for (const extra of extraEntries) {
    register(extra.ja, {
      category: extra.category ?? "glossary",
      hash_id: extra.hashId,
      ja: extra.ja,
      fr: extra.fr,
      en: extra.en,
      es: extra.es,
    });
  }

  for (const bucket of buckets.values()) {
    bucket.sort((a, b) => b.key.length - a.key.length);
  }

  const index: IevrGlossaryIndex = {
    byFirstChar: buckets,
    exactJa,
    exactCleanJa,
    targetLang,
    termCount: exactCleanJa.size,
  };

  if (bundle && extraEntries.length === 0) {
    const existing = indexCache.get(bundle) ?? {};
    existing[targetLang] = index;
    indexCache.set(bundle, existing);
  }

  return index;
}

/**
 * Looks up a single exact Japanese term or hash ID inside the IEVR dictionary.
 */
export async function lookupIevrTerm(
  query: string,
  targetLang: TranslateTargetLang = "fr",
  bundleArg?: IevrI18nBundle | null,
): Promise<{
  term: string;
  target?: string;
  entry: IevrTermEntry;
} | null> {
  const bundle = bundleArg === undefined ? await loadIevrI18n() : bundleArg;
  if (!bundle) return null;

  const trimmed = stripRubyAnnotations(formatJapaneseName(query)).trim();
  if (!trimmed) return null;

  if (/^0x[0-9a-f]{1,8}$/i.test(trimmed)) {
    const hex = `0x${trimmed.slice(2).toUpperCase().padStart(8, "0")}`;
    const byHash = bundle.hash_entries[hex];
    if (byHash) {
      return {
        term: byHash.ja ?? hex,
        target: pickEntryTranslation(byHash, targetLang),
        entry: { ...byHash, hash_id: hex },
      };
    }
  }

  const direct =
    bundle.ja_to_translations[trimmed] ??
    bundle.ja_to_translations[query.trim()];
  if (direct) {
    return {
      term: trimmed,
      target: pickEntryTranslation(direct, targetLang),
      entry: direct,
    };
  }

  return null;
}

/**
 * Finds non-overlapping longest matches of canonical IEVR terms inside `text`.
 */
export function matchIevrEntities(
  text: string,
  index: IevrGlossaryIndex,
): MatchedIevrEntity[] {
  const clean = stripRubyAnnotations(formatJapaneseName(text));
  if (!clean) return [];

  const exactHit = index.exactCleanJa.get(clean.trim());
  if (exactHit && clean.trim() === clean) {
    return [
      {
        source: exactHit.key,
        target: exactHit.target,
        category: exactHit.category,
        hashId: exactHit.hashId,
        romaji: exactHit.romaji,
        start: 0,
        end: clean.length,
        translations: exactHit.translations,
      },
    ];
  }

  const matches: MatchedIevrEntity[] = [];
  let i = 0;

  while (i < clean.length) {
    const ch = clean[i];
    const bucket = ch ? index.byFirstChar.get(ch) : undefined;
    let matched: IevrGlossaryEntry | null = null;

    if (bucket) {
      for (const candidate of bucket) {
        if (clean.startsWith(candidate.key, i)) {
          matched = candidate;
          break;
        }
      }
    }

    if (matched) {
      matches.push({
        source: matched.key,
        target: matched.target,
        category: matched.category,
        hashId: matched.hashId,
        romaji: matched.romaji,
        start: i,
        end: i + matched.key.length,
        translations: matched.translations,
      });
      i += matched.key.length;
    } else {
      i += 1;
    }
  }

  return matches;
}

/**
 * Segments Japanese text into natural phrase chunks using BudouX.
 */
export function segmentJapanese(text: string): string[] {
  const clean = stripRubyAnnotations(formatJapaneseName(text)).trim();
  if (!clean) return [];
  return getBudouxParser().parse(clean);
}

/**
 * Computes Japanese hiragana reading and Hepburn romaji using Kuromoji/Kuroshiro when
 * available in the runtime, with deterministic WanaKana fallback for pure kana/offline hosts.
 */
export async function analyzeJapaneseReading(
  text: string,
): Promise<{ reading: string; romaji: string }> {
  const clean = stripRubyAnnotations(formatJapaneseName(text)).trim();
  if (!clean) return { reading: "", romaji: "" };

  const allKanaOrPunct = [...clean].every(
    (c) => isKana(c) || c === "ー" || c === "・" || c === " " || c === "　",
  );
  if (allKanaOrPunct) {
    const reading = toHiragana(clean);
    return {
      reading,
      romaji: wanakanaToRomaji(reading, { upcaseKatakana: false }).trim(),
    };
  }

  if (typeof process !== "undefined") {
    try {
      const romajiMod = await import("../utils/romaji.js");
      const [reading, romaji] = await Promise.all([
        romajiMod.toKana(clean, "hiragana"),
        romajiMod.toRomaji(clean, { mode: "spaced", system: "hepburn" }),
      ]);
      if (reading || romaji) {
        return {
          reading: reading.trim(),
          romaji: romaji.trim(),
        };
      }
    } catch {
      // Fallback below when Kuromoji dictionary isn't accessible in this environment.
    }
  }

  const hira = toHiragana(clean);
  return {
    reading: hira,
    romaji: wanakanaToRomaji(hira).trim(),
  };
}

function maskAndRestoreEntities(
  cleanText: string,
  matches: MatchedIevrEntity[],
): {
  masked: string;
  restore: (translated: string) => string;
} {
  if (matches.length === 0) {
    return { masked: cleanText, restore: (t) => t };
  }

  let masked = "";
  let cursor = 0;
  const placeholders: Array<{ token: string; target: string; num: number }> =
    [];

  matches.forEach((m, idx) => {
    masked += cleanText.slice(cursor, m.start);
    const token = `__IEVR_${idx}__`;
    masked += ` ${token} `;
    placeholders.push({ token, target: m.target, num: idx });
    cursor = m.end;
  });
  masked += cleanText.slice(cursor);

  const restore = (translated: string): string => {
    let out = translated;
    for (const { token, target, num } of placeholders) {
      if (out.includes(token)) {
        out = out.replaceAll(token, target);
      }
      const lax = new RegExp(`_+\\s*ievr\\s*_?\\s*${num}\\s*_*`, "gi");
      out = out.replace(lax, target);
    }
    return out.replace(/\s+/g, " ").trim();
  };

  return { masked: masked.replace(/\s+/g, " ").trim(), restore };
}

/**
 * Translates Japanese (or English) text into `targetLang` (`fr` | `en` | `es`),
 * grounding canonical Inazuma Eleven terms from the live IEVR VFS dictionary first,
 * analyzing kana reading & romaji, segmenting via BudouX, and using `google-translate-api-x`
 * only for remaining non-glossary phrasing.
 */
export async function translateJp(
  text: string,
  targetLang: TranslateTargetLang = "fr",
  options: {
    bundle?: IevrI18nBundle | null;
    extraEntities?: Array<{
      ja: string;
      fr?: string;
      en?: string;
      es?: string;
      category?: string;
      hashId?: string;
    }>;
    skipRemote?: boolean;
  } = {},
): Promise<TranslateJpResult> {
  const clean = stripRubyAnnotations(formatJapaneseName(text)).trim();
  const isJa = containsJapanese(clean);
  const sourceLang: "ja" | "en" = isJa ? "ja" : "en";

  const bundle =
    options.bundle === undefined ? await loadIevrI18n() : options.bundle;
  const index = createIevrGlossaryIndex(
    bundle,
    targetLang,
    options.extraEntities ?? [],
  );

  const matchedEntities = isJa ? matchIevrEntities(clean, index) : [];
  const segments = isJa
    ? segmentJapanese(clean)
    : clean.split(/\s+/).filter(Boolean);
  const { reading, romaji } = isJa
    ? await analyzeJapaneseReading(clean)
    : { reading: "", romaji: "" };

  // Fast path: the entire string is a single canonical IEVR term.
  const firstMatch = matchedEntities[0];
  if (
    matchedEntities.length === 1 &&
    firstMatch &&
    firstMatch.start === 0 &&
    firstMatch.end === clean.length
  ) {
    return {
      source: text,
      sourceLang,
      targetLang,
      translated: firstMatch.target,
      reading,
      romaji: romaji || firstMatch.romaji || "",
      matchedEntities,
      segments,
      usedRemote: false,
    };
  }

  const { masked, restore } = maskAndRestoreEntities(clean, matchedEntities);

  // Check if everything remaining outside placeholders has no translatable letters/kana/kanji.
  const strippedRemaining = masked
    .replace(/__IEVR_\d+__/g, "")
    .replace(/[\s、。・「」『』【】！？!?,.・/()\[\]]+/g, "");

  if (options.skipRemote || !strippedRemaining) {
    return {
      source: text,
      sourceLang,
      targetLang,
      translated: restore(masked),
      reading,
      romaji,
      matchedEntities,
      segments,
      usedRemote: false,
    };
  }

  let usedRemote = false;
  let translated = restore(masked);

  try {
    const mod = await import("google-translate-api-x");
    const translateFn = mod.default;
    const res = await translateFn(masked, {
      from: sourceLang,
      to: targetLang,
    });
    if (res?.text) {
      translated = restore(res.text);
      usedRemote = true;
    }
  } catch {
    translated = restore(masked);
  }

  return {
    source: text,
    sourceLang,
    targetLang,
    translated,
    reading,
    romaji,
    matchedEntities,
    segments,
    usedRemote,
  };
}
