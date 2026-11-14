import type { GameIconKey } from "@/config/game-icons";
import type { SpriteCommonKey } from "@/config/sprites-common";

export interface HeroActionLink {
  href: string;
  icon: string;
  iconAlt: string;
  label: string;
  variant: "primary" | "outline" | "ghost";
}

export interface ElementAssetItem {
  href: string;
  id: string;
  image: string;
  label: string;
}

export interface RoleAssetItem {
  code: string;
  href: string;
  image: string;
  label: string;
}

export interface RarityAssetItem {
  id: string;
  image: string;
  label: string;
}

export interface SpriteShortcutItem {
  darkBadge?: boolean;
  href: string;
  key: SpriteCommonKey;
  label: string;
  subtitle: string;
}

export interface CategoryShortcutItem {
  href: string;
  key: GameIconKey;
  label: string;
  subtitle: string;
}

export interface RankTierAsset {
  href: string;
  image: string;
  label: string;
}

export const HERO_SHOWCASE_DATA = {
  ariaLabel: "Présentation de Rose Griffon",
  artwork: {
    alt: "Mark Evans, Roy et Gaëlle sur le terrain aux couleurs de Rose Griffon",
    caption: "Illustration officielle Rose Griffon",
    src: "/da/afficheRG.webp",
  },
  backdrop: {
    alt: "Ciel du stade Rose Griffon",
    src: "/sky.webp",
  },
  badge: {
    alt: "Emblème officiel Rose Griffon",
    logo: "/RG_Logo_V2.5.webp",
    text: "Association Inazuma Eleven France",
  },
  charactersChip: [
    {
      alt: "Roy, mascotte Rose Griffon",
      href: "/notre-equipe",
      image: "/emote/RG_Roy_salut.webp",
      name: "Roy",
      role: "Communauté",
    },
    {
      alt: "Gaëlle, mascotte Rose Griffon",
      href: "/projets",
      image: "/emote/RG_Gaelle_fiere.webp",
      name: "Gaëlle",
      role: "Projets",
    },
  ],
  ctas: [
    {
      href: "/wiki",
      icon: "/azale.webp",
      iconAlt: "Logo Azalée Wiki",
      label: "Explorer le Wiki Azalée",
      variant: "primary",
    },
    {
      href: "/achillea",
      icon: "/achillea/ranks/supernova.webp",
      iconAlt: "Rang Supernova Achilléa",
      label: "Circuit Achilléa",
      variant: "outline",
    },
    {
      href: "/play",
      icon: "/icon.webp",
      iconAlt: "Logo Rose Griffon",
      label: "Lancer IEVR Web",
      variant: "ghost",
    },
  ] satisfies HeroActionLink[],
  description:
    "Association loi 1901 dédiée à Inazuma Eleven : base encyclopédique Azalée, tournois et classement Achilléa, Web TV et outils communautaires.",
  metricsAriaLabel: "Chiffres clés en direct",
  metricsLabels: {
    achillea: "compétiteurs classés",
    achilleaFallbackTitle: "Achilléa",
    clans: "clans",
    rgtv: "abonnés Twitch",
    rgtvFallback: "Chaîne communautaire",
    rgtvNextPrefix: "Prochain live :",
    rgtvTitle: "RG TV",
    skills: "techniques",
    wiki: "fiches wiki",
    wikiFallbackTitle: "Azalée",
  },
  title: "Rose Griffon",
} as const;

export const ELEMENT_ASSETS: ElementAssetItem[] = [
  {
    href: "/wiki/chara?element=Feu",
    id: "Feu",
    image: "/spirit_type/fire.webp",
    label: "Feu",
  },
  {
    href: "/wiki/chara?element=Vent",
    id: "Vent",
    image: "/spirit_type/wind.webp",
    label: "Vent",
  },
  {
    href: "/wiki/chara?element=Montagne",
    id: "Montagne",
    image: "/spirit_type/mountain.webp",
    label: "Montagne",
  },
  {
    href: "/wiki/chara?element=Forêt",
    id: "Forêt",
    image: "/spirit_type/forest.webp",
    label: "Forêt",
  },
];

export const ELEMENT_ICON_BY_NAME: Record<string, string> = {
  Eau: "/spirit_type/wind.webp",
  Feu: "/spirit_type/fire.webp",
  Fire: "/spirit_type/fire.webp",
  Forêt: "/spirit_type/forest.webp",
  Forest: "/spirit_type/forest.webp",
  Montagne: "/spirit_type/mountain.webp",
  Mountain: "/spirit_type/mountain.webp",
  Vent: "/spirit_type/wind.webp",
  Wind: "/spirit_type/wind.webp",
};

export const ROLE_ASSETS: RoleAssetItem[] = [
  {
    code: "ATT",
    href: "/wiki/chara?position=FW",
    image: "/move/tir.webp",
    label: "Attaquant",
  },
  {
    code: "MIL",
    href: "/wiki/chara?position=MF",
    image: "/move/dribble.webp",
    label: "Milieu",
  },
  {
    code: "DEF",
    href: "/wiki/chara?position=DF",
    image: "/move/defense.webp",
    label: "Défenseur",
  },
  {
    code: "GAR",
    href: "/wiki/chara?position=GK",
    image: "/move/gardien.webp",
    label: "Gardien",
  },
];

export const RARITY_ASSETS: RarityAssetItem[] = [
  { id: "normal", image: "/rarity/normal.webp", label: "Normal" },
  {
    id: "experimente",
    image: "/rarity/experimente.webp",
    label: "Expérimenté",
  },
  { id: "emerite", image: "/rarity/emerite.webp", label: "Émérite" },
  { id: "legendaire", image: "/rarity/legendaire.webp", label: "Légendaire" },
  { id: "heros", image: "/rarity/heros.webp", label: "Héros" },
  { id: "basara", image: "/rarity/basara.webp", label: "Basara" },
];

export const COMMON_SPRITE_SHORTCUTS: SpriteShortcutItem[] = [
  {
    href: "/wiki/chara",
    key: "chara",
    label: "Joueurs",
    subtitle: "Base & profils",
  },
  {
    href: "/wiki/skill",
    key: "skill",
    label: "Techniques",
    subtitle: "Hissatsu",
  },
  {
    href: "/wiki/aura/esprits-guerriers",
    key: "keshin",
    label: "Keshin",
    subtitle: "Esprits guerriers",
  },
  {
    href: "/wiki/aura/totems",
    key: "soul",
    label: "Totems",
    subtitle: "Souls",
  },
  {
    href: "/wiki/aura/miximax",
    key: "miximax",
    label: "Miximax",
    subtitle: "Fusions",
  },
  {
    href: "/wiki/aura/eveil",
    key: "eveil",
    label: "Éveil",
    subtitle: "Super-états",
  },
  {
    darkBadge: true,
    href: "/wiki/tactic",
    key: "justice",
    label: "Tactiques",
    subtitle: "Spéciales",
  },
  {
    darkBadge: true,
    href: "/wiki/passive",
    key: "tension",
    label: "Passifs",
    subtitle: "Systèmes",
  },
];

export const CATEGORY_ICON_SHORTCUTS: CategoryShortcutItem[] = [
  {
    href: "/wiki/equipe",
    key: "emblem",
    label: "Équipes",
    subtitle: "Clubs & écoles",
  },
  {
    href: "/wiki/stade",
    key: "stadium",
    label: "Stades",
    subtitle: "Terrains",
  },
  {
    href: "/wiki/item",
    key: "chest",
    label: "Objets",
    subtitle: "Équipements",
  },
  {
    href: "/wiki/capsule",
    key: "ticket",
    label: "Capsules",
    subtitle: "Boutique",
  },
  {
    href: "/wiki/invocation",
    key: "star-token",
    label: "Invocations",
    subtitle: "Gacha",
  },
  {
    href: "/wiki/drops",
    key: "creature",
    label: "Drops",
    subtitle: "Récompenses",
  },
];

export const RANK_TIER_ASSETS: RankTierAsset[] = [
  {
    href: "/achillea/ranked/tiers",
    image: "/achillea/ranks/supernova.webp",
    label: "Supernova",
  },
  {
    href: "/achillea/ranked/tiers",
    image: "/achillea/ranks/legendaire.webp",
    label: "Légendaire",
  },
  {
    href: "/achillea/ranked/tiers",
    image: "/achillea/ranks/divin.webp",
    label: "Divin",
  },
  {
    href: "/achillea/ranked/tiers",
    image: "/achillea/ranks/diamant.webp",
    label: "Diamant",
  },
  {
    href: "/achillea/ranked/tiers",
    image: "/achillea/ranks/or.webp",
    label: "Or",
  },
];

export const BENTO_COPY = {
  achillea: {
    astralAlt: "Logo Astral League",
    astralHref: "/achillea/leagues/astral",
    astralImage: "/achillea/leagues/astral-logo-mark.png",
    astralLabel: "Astral League",
    badge: "Compétition",
    ctaHref: "/achillea",
    ctaLabel: "Ouvrir l'arène",
    eloSuffix: "ELO",
    fallbackClanAvatar: "/achillea/default-clan-avatar.svg",
    logoAlt: "Emblème Achilléa",
    logoSrc: "/achillea.webp",
    matchesSuffix: "matchs",
    subtitle:
      "Classement 1v1, clans et tournois homologués Inazuma Eleven Victory Road.",
    title: "Achilléa E-Sport",
    winsLabel: "V",
  },
  ariaLabel: "Univers Rose Griffon",
  azalee: {
    badge: "Encyclopédie",
    ctaHref: "/wiki",
    ctaLabel: "Ouvrir le Wiki",
    logoAlt: "Logo Wiki Azalée",
    logoSrc: "/azale.webp",
    subtitle:
      "Données extraites du jeu : personnages, techniques, statistiques et textures.",
    title: "Wiki Azalée",
  },
  heading: "Trois pôles complémentaires",
  rgtv: {
    artworkAlt: "Roy et Gaëlle manettes en main pour Rose Griffon TV",
    artworkSrc: "/rgtv/affiche.webp",
    badge: "Web TV",
    ctaHref: "/rose-griffon-tv",
    ctaLabel: "Voir la programmation",
    liveBadge: "En direct",
    logoAlt: "Logo Rose Griffon TV",
    logoSrc: "/rgtv/rgtv.webp",
    nextStreamFallback: "Casts tournois & soirées communauté sur Twitch.",
    offlineBadge: "Programme TV",
    subtitle:
      "Retransmissions de tournois, interviews et soirées création communautaire.",
    title: "Rose Griffon TV",
  },
  subHeading:
    "Encyclopédie technique, circuit compétitif et diffusion en direct reliés au même compte.",
} as const;

export const ASSET_SHOWCASE_COPY = {
  ariaLabel: "Explorateur de ressources et studio 3D",
  badge: "Ressources authentiques du jeu",
  heading: "Sprites, textures et modèles 3D en temps réel",
  subHeading:
    "Chaque icône, poste, palier et maillage provient directement des archives décodées d'Inazuma Eleven Victory Road.",
  tabs: [
    { id: "atlas", label: "Atlas & Postes" },
    { id: "elements", label: "Éléments & Raretés" },
    { id: "tiers", label: "Paliers & Catégories" },
  ],
  tabsAriaLabel: "Catégories de ressources du jeu",
  texturesCta: {
    href: "/wiki/textures",
    label: "Galerie de textures",
  },
  viewer3d: {
    badge: "Modèle 3D GLB (420 Ko)",
    defaultName: "Super-technique Victory Road (ev60_00340)",
    demoHref: "/wiki/demo",
    demoLabel: "Atelier 3D complet",
    glbUrl: "https://cdn.rosegriffon.fr/model-chr/waza/ev60_00340.glb",
    loadButtonLabel: "Charger l'aperçu 3D interactif",
    modelAlt: "Rendu 3D interactif de la super-technique Victory Road",
    modelsHref: "/wiki/modeles",
    modelsLabel: "Catalogue 3D",
    posterAlt: "Visuel clé Inazuma Eleven et Byron Love",
    posterAvatarAlt: "Portrait 3D de Byron Love",
    posterAvatarSrc:
      "https://cdn.rosegriffon.fr/dx11/menu/200_icon/10_icon_chr/face/c01001900_l.png?w=256&format=webp",
    posterBgSrc: "/inazuma-cross/home-keyvisual.webp",
    subtitle:
      "Orbit, zoom et inspection directe du maillage sur le navigateur.",
    title: "Aperçu cut-in Waza 3D",
  },
} as const;

export const DISCORD_HOME_COPY = {
  ariaLabel: "Serveurs Discord de la communauté",
  emptyNote: "Aucun membre affiché pour l'instant.",
  heading: "Rejoindre la communauté sur Discord",
  joinLabel: "Rejoindre",
  memberCountSuffix: "en ligne",
  openDiscordAriaPrefix: "Rejoindre le serveur Discord",
  subHeading:
    "Discussions quotidiennes, recherche de partenaires et support des tournois.",
} as const;

export function cdnFacePortraitUrl(internalCode: string): string {
  return `https://cdn.rosegriffon.fr/dx11/menu/200_icon/10_icon_chr/face/${internalCode}_l.png?w=256&format=webp`;
}

/** Retire les emojis du nom de guilde renvoyé par le widget public Discord. */
export function sanitiserNomServeurDiscord(name: string): string {
  return name
    .replace(/\p{Extended_Pictographic}/gu, "")
    .replace(/\s{2,}/g, " ")
    .trim();
}
