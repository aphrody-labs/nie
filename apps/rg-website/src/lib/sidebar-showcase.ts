export type RgThemeId =
  "roy" | "gaelle" | "azalee-light" | "azalee-dark" | "achillea";

export interface SidebarMascotMeta {
  alt: string;
  image: string;
  name: string;
  role: string;
}

export const SIDEBAR_MASCOTS: Record<RgThemeId, SidebarMascotMeta> = {
  achillea: {
    alt: "Insigne Supernova Achilléa",
    image: "/achillea/ranks/supernova.webp",
    name: "Achilléa",
    role: "Circuit Classé",
  },
  "azalee-dark": {
    alt: "Portrait d'Azalée",
    image: "/emote/RG_Reika_zukan.webp",
    name: "Azalée",
    role: "Wiki & Archives",
  },
  "azalee-light": {
    alt: "Portrait d'Azalée",
    image: "/emote/RG_Reika_zukan.webp",
    name: "Azalée",
    role: "Wiki & Archives",
  },
  gaelle: {
    alt: "Portrait de Gaëlle, mascotte Rose Griffon",
    image: "/emote/RG_Gaelle_fiere.webp",
    name: "Gaëlle",
    role: "Pôle Création",
  },
  roy: {
    alt: "Portrait de Roy, mascotte Rose Griffon",
    image: "/emote/RG_Roy_salut.webp",
    name: "Roy",
    role: "Capitaine RG",
  },
};

export const SIDEBAR_FOOTER_COPY = {
  navAriaLabel: "Accès rapides atelier et jeu",
  playHref: "/play",
  playLabel: "IEVR Web",
  playTitle: "Ouvrir l'émulateur web Inazuma Eleven Victory Road",
  studioHref: "/wiki/demo",
  studioLabel: "Studio 3D",
  studioTitle: "Inspecter les modèles 3D et super-techniques",
  texturesHref: "/wiki/textures",
  texturesLabel: "Textures",
  texturesTitle: "Parcourir les textures décodées",
} as const;
