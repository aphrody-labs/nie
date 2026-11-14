/**
 * Types + helpers PURS de l'index de texte de jeu, client-safe (pas de `bun:sqlite`).
 *
 * Importable depuis un composant `"use client"`. La couche data serveur vit dans
 * `game-text/index.ts` (sous-chemin serveur de la lib).
 */

/** Locales réellement décodées dans le dump (`ja`, `en`, `fr`, `es`). */
export type GameTextLocale = "fr" | "en" | "ja" | "es";

/** Ligne de l'index : un texte localisé pour un hashId + sa catégorie. */
export interface GameText {
  hashId: string;
  locale: GameTextLocale;
  category: string;
  value: string;
}

/** Texte résolu dans les 4 langues principales pour un hashId. */
export interface GameTextLocalized {
  hashId: string;
  category?: string;
  fr?: string;
  en?: string;
  ja?: string;
  es?: string;
}

/** Normalise un hashId entrant (`0xXXXX` MAJUSCULE 8 chiffres) pour le lookup. */
export function normalizeHashId(raw: string | number): string {
  if (typeof raw === "number") {
    return `0x${(raw >>> 0).toString(16).toUpperCase().padStart(8, "0")}`;
  }
  const s = raw.trim();
  if (/^-?\d+$/.test(s)) {
    // Entier décimal (signé) → hex non signé.
    return `0x${(Number.parseInt(s, 10) >>> 0).toString(16).toUpperCase().padStart(8, "0")}`;
  }
  const hex = s.replace(/^0x/i, "").toUpperCase().padStart(8, "0");
  return `0x${hex}`;
}

/** Choisit la meilleure langue disponible (`preferred` puis `fr -> en -> es -> ja`). */
export function pickLocale(
  t: GameTextLocalized,
  preferred?: GameTextLocale,
): string | undefined {
  if (preferred && t[preferred]) return t[preferred];
  return t.fr ?? t.en ?? t.es ?? t.ja;
}
