/**
 * Palette Rose Griffon — référence officielle.
 *
 * `icon` pointe vers un fichier de `public/emote/` : les quatre visuels
 * sont réutilisés entre les cinq thèmes. `RG_Reika_zukan.webp` est l'icône
 * zukan de Reika, la mascotte d'Azalée.
 */
import type { ThemeId } from "@rosegriffon/types/preferences";

import type { ThemeSeed } from "./color-engine";

export const THEMES = {
  roy: {
    name: "Roy",
    icon: "/emote/RG_Roy_salut.webp",
    colors: {
      primary: "#344E8C", // Bleu Roy
      secondary: "#243D5B", // Bleu foncé
      accent: "#FFC46E", // Doré
      background: "#F2F6F7", // Blanc
      text: "#373434", // Noir
    },
  },
  gaelle: {
    name: "Gaëlle",
    icon: "/emote/RG_Gaelle_salut.webp",
    colors: {
      primary: "#B03519", // Vermillon
      secondary: "#770007", // Rouge
      accent: "#FFC46E", // Doré
      background: "#F2F6F7", // Blanc
      text: "#373434", // Noir
    },
  },
  "azalee-light": {
    name: "Azalée clair",
    icon: "/emote/RG_Reika_zukan.webp",
    colors: {
      primary: "#F2A93B",
      secondary: "#A92335",
      accent: "#F2A93B",
      background: "#FFF8F5",
      text: "#241A15",
    },
  },
  "azalee-dark": {
    name: "Azalée",
    icon: "/emote/RG_Reika_zukan.webp",
    colors: {
      primary: "#FFC66C",
      secondary: "#FFB2BE",
      accent: "#FFDEA6",
      background: "#1A120D",
      text: "#EDE0DB",
    },
  },
  achillea: {
    name: "Achilléa",
    icon: "/achillea/logo1.svg",
    colors: {
      primary: "#6D00CE",
      secondary: "#DFFF57",
      accent: "#DFFF57",
      background: "#0D0814",
      text: "#F3EDF7",
    },
  },
  inazuma: {
    name: "Inazuma",
    icon: "/ie.webp",
    colors: {
      primary: "#082475",
      secondary: "#3EC4E9",
      accent: "#FFE900",
      background: "#F6F9FF",
      text: "#0C1730",
    },
  },
  nie: {
    name: "NIE Studio",
    icon: "/ievr.webp",
    colors: {
      primary: "#97C0FC",
      secondary: "#E6C961",
      accent: "#5F9EFC",
      background: "#131420",
      text: "#FCFAF8",
    },
  },
} as const satisfies Record<
  ThemeId,
  {
    name: string;
    icon: string;
    colors: {
      primary: string;
      secondary: string;
      accent: string;
      background: string;
      text: string;
    };
  }
>;

export type ThemeType = ThemeId;

/**
 * Pack de thème : ce que le changement de thème remplace d'un coup, au-delà
 * des couleurs. Le chrome lit ce manifeste au lieu de chemins codés en dur.
 *
 * `seed` est la graine des couleurs calculées (teinte et chroma OKLCH,
 * mode clair ou sombre) ; les palettes écrites à la main des blocs CSS
 * migrent vers elle.
 */
export interface ThemePack {
  /** Nom de marque affiché dans la barre d'application. */
  title: string;
  logo: string;
  wallpaper: string;
  /** Compagnon du thème ; `null` tant qu'aucune mascotte n'existe. */
  pet: {
    name: string;
    emotes: { hello: string; proud?: string; surprised?: string };
  } | null;
  seed: ThemeSeed;
}

export const THEME_PACKS = {
  roy: {
    title: "Rose Griffon",
    logo: "/RG_Logo_V2.5.webp",
    wallpaper: "/roy_bg.webp",
    pet: {
      name: "Roy",
      emotes: {
        hello: "/emote/RG_Roy_salut.webp",
        proud: "/emote/RG_roy_fine.webp",
      },
    },
    seed: { hue: 265, chroma: 0.107, mode: "light" },
  },
  gaelle: {
    title: "Rose Griffon",
    logo: "/RG_Logo_V2.5.webp",
    wallpaper: "/gaelle_bg.webp",
    pet: {
      name: "Gaëlle",
      emotes: {
        hello: "/emote/RG_Gaelle_salut.webp",
        proud: "/emote/RG_Gaelle_fiere.webp",
        surprised: "/emote/RG_Gaelle_surprise.webp",
      },
    },
    seed: { hue: 34, chroma: 0.164, mode: "light" },
  },
  "azalee-light": {
    title: "Azalée",
    logo: "/RG_Azalee-logo.webp",
    wallpaper: "/azalee-bg.webp",
    pet: { name: "Reika", emotes: { hello: "/emote/RG_Reika_zukan.webp" } },
    seed: { hue: 73, chroma: 0.147, mode: "light" },
  },
  "azalee-dark": {
    title: "Azalée",
    logo: "/RG_Azalee-logo.webp",
    wallpaper: "/azalee_fond.webp",
    pet: { name: "Reika", emotes: { hello: "/emote/RG_Reika_zukan.webp" } },
    seed: { hue: 77, chroma: 0.126, mode: "dark" },
  },
  achillea: {
    title: "Achilléa",
    logo: "/achillea/logo1.svg",
    wallpaper: "/fond_achilea.webp",
    pet: null,
    // Charte legacy (achillea-web, preset PrimeNG) : violet #9039ff, or
    // #fbbf24 en accent, surfaces en gris neutres (#131313, #1e1e1e).
    seed: {
      hue: 297,
      chroma: 0.267,
      mode: "dark",
      secondary: { hue: 84, chroma: 0.164 },
      neutralChroma: 0,
    },
  },
  inazuma: {
    title: "Azalée",
    logo: "/ie.webp",
    wallpaper: "/sky.webp",
    pet: { name: "Mark", emotes: { hello: "/markvr.webp" } },
    seed: {
      hue: 264,
      chroma: 0.142,
      mode: "light",
      secondary: { hue: 221, chroma: 0.123 },
    },
  },
  nie: {
    title: "NIE Azalée",
    logo: "/ievr.webp",
    wallpaper: "/markogvr.webp",
    pet: { name: "Unmei", emotes: { hello: "/vr.webp" } },
    seed: {
      hue: 258,
      chroma: 0.096,
      mode: "dark",
      secondary: { hue: 94, chroma: 0.128 },
    },
  },
} as const satisfies Record<ThemeId, ThemePack>;

/**
 * Thème imposé par une section du site (posé sur le conteneur, sans toucher
 * au choix enregistré de l'utilisateur). `undefined` : la section suit le
 * thème choisi.
 */
export function sectionTheme(
  section: "association" | "wiki" | "esport",
): ThemeId | undefined {
  if (section === "esport") return "achillea";
  if (section === "wiki") return "azalee-dark";
  return undefined;
}

/** Clé `localStorage` où `ThemeProvider` enregistre le thème choisi. */
export const THEME_STORAGE_KEY = "rg-theme";

/**
 * Thème d'une section compte tenu du choix de l'utilisateur : un thème
 * choisi explicitement s'applique partout, wiki et e-sport compris ; sans
 * choix, la section garde son thème par défaut (`sectionTheme`).
 */
export function sectionThemeSelonChoix(
  section: "association" | "wiki" | "esport",
  themeChoisi: boolean,
): ThemeId | undefined {
  return themeChoisi ? undefined : sectionTheme(section);
}
