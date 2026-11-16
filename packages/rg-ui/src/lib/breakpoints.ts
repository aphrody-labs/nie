/**
 * Classes de taille de fenêtre Material Design 3 (largeur en dp/px).
 * Seuils officiels 600 / 840 / 1200 / 1600 (m3.material.io, Foundations > Layout > Window size classes).
 */
export type WindowSizeClass =
  | "compact"
  | "medium"
  | "expanded"
  | "large"
  | "extra-large";

/** Borne basse (dp) de chaque classe. */
export const BREAKPOINTS = {
  compact: 0,
  medium: 600,
  expanded: 840,
  large: 1200,
  "extra-large": 1600,
} as const satisfies Record<WindowSizeClass, number>;

/** Classe M3 d'une largeur en dp/px. */
export function classifyWidth(width: number): WindowSizeClass {
  if (width < BREAKPOINTS.medium) return "compact";
  if (width < BREAKPOINTS.expanded) return "medium";
  if (width < BREAKPOINTS.large) return "expanded";
  if (width < BREAKPOINTS["extra-large"]) return "large";
  return "extra-large";
}
