/**
 * @license
 * Copyright 2026 Aphrody Code
 * SPDX-License-Identifier: Apache-2.0
 */

// The ambient module declarations must not become runtime imports.
// eslint-disable-next-line @typescript-eslint/triple-slash-reference
/// <reference path="../types/kuroshiro.d.ts" />
import Kuroshiro from "kuroshiro";
import KuromojiAnalyzer from "kuroshiro-analyzer-kuromoji";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

let instance: Kuroshiro | null = null;
let initPromise: Promise<Kuroshiro> | null = null;

function resolveKuromojiDictPath(): string {
  let dir = resolve(process.cwd());
  for (let depth = 0; depth < 8; depth++) {
    const candidate = join(dir, "node_modules", "kuromoji", "dict");
    if (existsSync(candidate)) return candidate;
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  if (typeof import.meta.dir === "string") {
    const fallback = resolve(
      import.meta.dir,
      "../../../../node_modules/kuromoji/dict",
    );
    if (existsSync(fallback)) return fallback;
  }
  return join(process.cwd(), "node_modules", "kuromoji", "dict");
}

/**
 * Initializes the Kuroshiro singleton with Kuromoji analyzer.
 */
export async function initRomaji(): Promise<Kuroshiro> {
  if (instance) return instance;

  if (initPromise) return initPromise;

  initPromise = (async () => {
    const kuro = new Kuroshiro();
    const dictPath = resolveKuromojiDictPath();

    await kuro.init(
      new KuromojiAnalyzer({
        dictPath,
      }),
    );

    instance = kuro;
    return kuro;
  })();

  return initPromise;
}

/**
 * Converts Japanese text (Kanji, Hiragana, Katakana) to Romaji.
 */
export async function toRomaji(
  text: string,
  options?: {
    mode?: "normal" | "spaced" | "okurigana" | "furigana";
    system?: "nippon" | "passport" | "hepburn";
  },
): Promise<string> {
  const kuro = await initRomaji();
  return kuro.convert(text, {
    to: "romaji",
    mode: options?.mode || "normal",
    romajiSystem: options?.system || "hepburn",
  });
}

/**
 * Converts Japanese text to Romaji in title case with spaces (ideal for character names).
 * Example: "円堂守" -> "Endō Mamoru"
 */
export async function toRomajiTitleCase(text: string): Promise<string> {
  const rawRomaji = await toRomaji(text, { mode: "spaced" });

  // Split by space or dot and capitalize each part
  return rawRomaji
    .split(/[・\s]+/)
    .filter(Boolean)
    .map(
      (part) =>
        part.charAt(0).toUpperCase() +
        part.slice(part.length > 1 ? 1 : 0).toLowerCase(),
    )
    .join(" ");
}

/**
 * Converts Japanese text to Hiragana or Katakana.
 */
export async function toKana(
  text: string,
  target: "hiragana" | "katakana",
): Promise<string> {
  const kuro = await initRomaji();
  return kuro.convert(text, {
    to: target,
    mode: "normal",
  });
}
