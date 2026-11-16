"use client";

import { useSyncExternalStore } from "react";

import { THEME_STORAGE_KEY } from "./themes";

function abonner(rappel: () => void): () => void {
  // next-themes réécrit la classe de <html> à chaque changement de thème ;
  // l'événement `storage` couvre les autres onglets.
  const observateur = new MutationObserver(rappel);
  observateur.observe(document.documentElement, {
    attributeFilter: ["class"],
    attributes: true,
  });
  addEventListener("storage", rappel);
  return () => {
    observateur.disconnect();
    removeEventListener("storage", rappel);
  };
}

const lire = () => localStorage.getItem(THEME_STORAGE_KEY) !== null;

/** Vrai quand l'utilisateur a choisi un thème (utilisable hors de `ThemeProvider`). */
export function useThemeChoisi(): boolean {
  return useSyncExternalStore(abonner, lire, () => false);
}
