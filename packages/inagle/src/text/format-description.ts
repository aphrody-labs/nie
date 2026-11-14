/**
 * Linear-time single-pass parser and formatter for IEVR rich-text descriptions and ruby furigana.
 * Mirrors the Rust `aphrody_nie_data::text` AST (`text`, `furigana`, `tag`) with O(n) scanning
 * (zero regex backtracking) and zero runtime dependencies so browser/SSR/CLI callers stay pure.
 */

export type RichTextLang = "en" | "fr" | "es" | "jp" | "ja";

export type RichTextToken =
  | { kind: "text"; text: string }
  | { kind: "furigana"; base: string; reading: string }
  | { kind: "tag"; tagType: string; tagName: string };

export interface FuriganaPair {
  base: string;
  reading: string;
}

const VALID_TAG_TYPES = new Set(["FST", "FLC", "FLA", "FUL", "MNT"]);

type TagTranslation = { en?: string; fr?: string; es?: string; jp?: string };

const CHARACTER_NAME_TAGS: Record<string, TagTranslation> = {
  TENMA: { en: "Arion", fr: "Arion", es: "Arion", jp: "Tenma" },
  TSURUGI: { en: "Victor", fr: "Victor", es: "Victor", jp: "Kyosuke" },
  SHINDOU: { en: "Riccardo", fr: "Riccardo", es: "Riccardo", jp: "Takuto" },
  KIRINO: {
    en: "Jean-Pierre",
    fr: "Jean-Pierre",
    es: "Jean-Pierre",
    jp: "Ranmaru",
  },
  ENDO: { en: "Evans", fr: "Evans", es: "Evans", jp: "Endo" },
  GOENJI: { en: "Axel", fr: "Axel", es: "Axel", jp: "Shuya" },
  GOUENJI: { en: "Axel", fr: "Axel", es: "Axel", jp: "Shuya" },
  KIDOU: { en: "Jude", fr: "Jude", es: "Jude", jp: "Yuuto" },
  KAZEMARU: { en: "Nathan", fr: "Nathan", es: "Nathan", jp: "Ichirouta" },
  FUBUKI: { en: "Shawn", fr: "Shawn", es: "Shawn", jp: "Shirou" },
  SYU: { en: "Shuu", fr: "Shuu", es: "Shuu", jp: "Shuu" },
  ARTHUR: {
    en: "King Arthur",
    fr: "Roi Arthur",
    es: "Rey Arturo",
    jp: "Arthur",
  },
  BETA: { en: "Beta", fr: "Beta", es: "Beta", jp: "Beta" },
  RYUBI: { en: "Liu Bei", fr: "Liu Bei", es: "Liu Bei", jp: "Ryubi" },
  HAKURYU: { en: "Hakuryuu", fr: "Hakuryuu", es: "Bailong", jp: "Hakuryuu" },
  OZROCK: { en: "Ozrock", fr: "Ozrock", es: "Ozrock", jp: "Ozrock" },
  FEYZA: { en: "Fey", fr: "Fey", es: "Fey", jp: "Fei" },
  KINAKO: { en: "Goldie", fr: "Goldie", es: "Goldie", jp: "Kinako" },
  TETSUKADO: { en: "Iron", fr: "Iron", es: "Iron", jp: "Tetsukado" },
  MATATAGI: { en: "Matatagi", fr: "Matatagi", es: "Matatagi", jp: "Hayato" },
  IBUKI: { en: "Ibuki", fr: "Ibuki", es: "Ibuki", jp: "Munemasa" },
  MINAHO: { en: "Manabe", fr: "Manabe", es: "Manabe", jp: "Minaho" },
  KONOHA: { en: "Konoha", fr: "Konoha", es: "Konoha", jp: "Konoha" },
  ASUTO: { en: "Asuto", fr: "Asuto", es: "Asuto", jp: "Inamori" },
  NOSAKA: { en: "Nosaka", fr: "Nosaka", es: "Nosaka", jp: "Yuuma" },
  HAIZAKI: { en: "Haizaki", fr: "Haizaki", es: "Haizaki", jp: "Ryouhei" },
  KIRA: { en: "Kira", fr: "Kira", es: "Kira", jp: "Hiroto" },
};

const PLACE_NAME_TAGS: Record<string, TagTranslation> = {
  ALIEA: {
    en: "Aliea Academy",
    fr: "Académie Aliea",
    es: "Academia Alius",
    jp: "エイリア学園",
  },
  INAZUMAJAPAN: {
    en: "Inazuma Japan",
    fr: "Inazuma Japon",
    es: "Inazuma Japón",
    jp: "イナズマジャパン",
  },
  LITTLEGIGANTS: {
    en: "Little Gigant",
    fr: "Little Gigant",
    es: "Little Gigant",
    jp: "リトルギガント",
  },
  NAGUMOHARA: {
    en: "Nagumohara",
    fr: "Nagumohara",
    es: "Nagumohara",
    jp: "凪村原",
  },
  ORPHEUS: {
    en: "Orpheus",
    fr: "Orpheus",
    es: "Orfeo",
    jp: "オルフェウス",
  },
  RAIMON: { en: "Raimon", fr: "Raimon", es: "Raimon", jp: "雷門" },
  SHIROSHIKA: {
    en: "White Deer",
    fr: "Cerf Blanc",
    es: "Ciervo Blanco",
    jp: "白鹿",
  },
  TEAMK: { en: "Team K", fr: "Team K", es: "Equipo K", jp: "チームK" },
  TEIKOKU: {
    en: "Royal Academy",
    fr: "Académie Royale",
    es: "Royal Academy",
    jp: "帝国",
  },
  THEKINGDOM: {
    en: "The Kingdom",
    fr: "The Kingdom",
    es: "The Kingdom",
    jp: "ザ・キングダム",
  },
  UNICORN: { en: "Unicorn", fr: "Unicorn", es: "Unicorn", jp: "ユニコーン" },
  ZEUS: { en: "Zeus", fr: "Zeus", es: "Zeus", jp: "ゼウス" },
};

function isTagIdentChar(code: number): boolean {
  return (
    (code >= 48 && code <= 57) || // 0-9
    (code >= 65 && code <= 90) || // A-Z
    (code >= 97 && code <= 122) || // a-z
    code === 95 // _
  );
}

function tryParseTagAt(
  input: string,
  start: number,
): { token: RichTextToken; nextIndex: number } | null {
  const colonIdx = input.indexOf(":", start + 1);
  if (colonIdx === -1 || colonIdx - start - 1 > 6) return null;
  const tagType = input.slice(start + 1, colonIdx).toUpperCase();
  if (!VALID_TAG_TYPES.has(tagType)) return null;

  const closeIdx = input.indexOf(">", colonIdx + 1);
  if (closeIdx === -1 || closeIdx === colonIdx + 1) return null;

  for (let i = colonIdx + 1; i < closeIdx; i++) {
    if (!isTagIdentChar(input.charCodeAt(i))) return null;
  }

  return {
    token: {
      kind: "tag",
      tagType,
      tagName: input.slice(colonIdx + 1, closeIdx),
    },
    nextIndex: closeIdx + 1,
  };
}

function tryParseFuriganaAt(
  input: string,
  start: number,
): { token: RichTextToken; nextIndex: number } | null {
  const closeIdx = input.indexOf("]", start + 1);
  if (closeIdx === -1) return null;

  const inside = input.slice(start + 1, closeIdx);
  if (inside.includes("[") || inside.includes("\n")) return null;

  const slashIdx = inside.indexOf("/");
  if (slashIdx <= 0 || slashIdx === inside.length - 1) return null;

  return {
    token: {
      kind: "furigana",
      base: inside.slice(0, slashIdx),
      reading: inside.slice(slashIdx + 1),
    },
    nextIndex: closeIdx + 1,
  };
}

/**
 * Parses an IEVR rich-text string into structured AST tokens in a single O(n) pass.
 */
export function parseIevrRichText(input: string): RichTextToken[] {
  if (!input) return [];
  const tokens: RichTextToken[] = [];
  let textStart = 0;
  let i = 0;

  while (i < input.length) {
    const ch = input[i];
    let parsed: { token: RichTextToken; nextIndex: number } | null = null;

    if (ch === "<") {
      parsed = tryParseTagAt(input, i);
    } else if (ch === "[") {
      parsed = tryParseFuriganaAt(input, i);
    }

    if (parsed) {
      if (i > textStart) {
        tokens.push({ kind: "text", text: input.slice(textStart, i) });
      }
      tokens.push(parsed.token);
      i = parsed.nextIndex;
      textStart = i;
    } else {
      i += 1;
    }
  }

  if (textStart < input.length) {
    tokens.push({ kind: "text", text: input.slice(textStart) });
  }

  return tokens;
}

/**
 * Extracts `[base/reading]` furigana pairs from a Japanese string.
 */
export function extractFuriganaPairs(text: string): FuriganaPair[] {
  if (!text) return [];
  const pairs: FuriganaPair[] = [];
  for (const token of parseIevrRichText(text)) {
    if (token.kind === "furigana") {
      pairs.push({ base: token.base, reading: token.reading });
    }
  }
  return pairs;
}

function resolveTag(
  _tagType: string,
  tagName: string,
  lang: RichTextLang = "fr",
): string {
  const upperName = tagName.toUpperCase();
  const normalizedLang = lang === "ja" ? "jp" : lang;

  const charMapping = CHARACTER_NAME_TAGS[upperName];
  if (charMapping) {
    return charMapping[normalizedLang] || charMapping.en || tagName;
  }

  const placeMapping = PLACE_NAME_TAGS[upperName];
  if (placeMapping) {
    return placeMapping[normalizedLang] || placeMapping.en || tagName;
  }

  return tagName.charAt(0) + tagName.slice(1).toLowerCase();
}

/**
 * Resolves name/place tags and strips furigana ruby brackets in one linear pass.
 */
export function formatDescription(
  description: string | undefined | null,
  lang: RichTextLang = "fr",
): string {
  if (!description) return "";
  let out = "";
  for (const token of parseIevrRichText(description)) {
    if (token.kind === "text") {
      out += token.text;
    } else if (token.kind === "furigana") {
      out += token.base;
    } else {
      out += resolveTag(token.tagType, token.tagName, lang);
    }
  }
  return out;
}

/**
 * Removes `[kanji/reading]` furigana annotations while preserving base text.
 */
export function formatJapaneseName(name: string | undefined | null): string {
  if (!name) return "";
  let out = "";
  for (const token of parseIevrRichText(name)) {
    if (token.kind === "text") {
      out += token.text;
    } else if (token.kind === "furigana") {
      out += token.base;
    } else {
      out += `<${token.tagType}:${token.tagName}>`;
    }
  }
  return out;
}

export { japaneseToRomaji } from "./japanese-romaji";

/**
 * Returns `true` when the string still contains `<FST:...>`-style description tags.
 */
export function hasUnresolvedTags(description: string): boolean {
  return parseIevrRichText(description).some((t) => t.kind === "tag");
}
