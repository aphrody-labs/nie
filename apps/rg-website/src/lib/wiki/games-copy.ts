/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  normalizeGameLocale,
  type InazumaLocale,
} from "@rosegriffon/inagle/games";

export interface GamesPageCopy {
  title: string;
  subtitle: string;
  upcomingSpotlightHeading: string;
  upcomingSpotlightSubtitle: string;
  allErasLabel: string;
  allStatusesLabel: string;
  searchPlaceholder: string;
  protagonistLabel: string;
  releaseLabel: string;
  platformsLabel: string;
  playersCountSuffix: string;
  noPlayersDirectNote: string;
  officialSiteLabel: string;
  viewDetailsLabel: string;
  storeLabel: string;
  totalGamesLabel: string;
  activeDevLabel: string;
  playableCharactersLabel: string;
  modsHubCta: string;
  wikisBadgeLabel: string;
  modsBadgeLabel: string;
  emptyResults: string;
  resetFiltersLabel: string;
}

export const GAMES_PAGE_COPY_MAP: Readonly<
  Record<InazumaLocale, GamesPageCopy>
> = {
  fr: {
    title: "Jeux Inazuma Eleven",
    subtitle:
      "Chronologie complète de la licence : du tout premier Inazuma Eleven (2008) jusqu'à Victory Road, Cross et les opus en développement (RE, Bold Revolution).",
    upcomingSpotlightHeading: "Jeux annoncés & en développement",
    upcomingSpotlightSubtitle:
      "Projets officiellement dévoilés par LEVEL-5 sur inazuma.jp",
    allErasLabel: "Toutes les époques",
    allStatusesLabel: "Tous les statuts",
    searchPlaceholder: "Rechercher un jeu, une console ou un protagoniste...",
    protagonistLabel: "Protagoniste",
    releaseLabel: "Sortie",
    platformsLabel: "Plateformes",
    playersCountSuffix: "joueurs indexés",
    noPlayersDirectNote: "Casting transverse / spin-off",
    officialSiteLabel: "Site officiel",
    viewDetailsLabel: "Voir la fiche du jeu",
    storeLabel: "Boutique officielle",
    totalGamesLabel: "Jeux référencés",
    activeDevLabel: "En développement",
    playableCharactersLabel: "Joueurs en base",
    modsHubCta: "Explorer les mods & wikis (JA/EN/FR/ES)",
    wikisBadgeLabel: "wikis",
    modsBadgeLabel: "hubs mods",
    emptyResults: "Aucun épisode ne correspond aux filtres sélectionnés.",
    resetFiltersLabel: "Réinitialiser les filtres",
  },
  en: {
    title: "Inazuma Eleven Games",
    subtitle:
      "Complete chronological franchise archive: from the first Nintendo DS entry (2008) to Victory Road, Cross, RE, and Bold Revolution.",
    upcomingSpotlightHeading: "Announced & In Development",
    upcomingSpotlightSubtitle:
      "Upcoming chapters officially revealed by LEVEL-5 on inazuma.jp",
    allErasLabel: "All Eras",
    allStatusesLabel: "All Statuses",
    searchPlaceholder: "Search a game, platform, or protagonist...",
    protagonistLabel: "Protagonist",
    releaseLabel: "Release",
    platformsLabel: "Platforms",
    playersCountSuffix: "indexed players",
    noPlayersDirectNote: "Crossover roster / spin-off",
    officialSiteLabel: "Official Site",
    viewDetailsLabel: "View game details",
    storeLabel: "Official Web Store",
    totalGamesLabel: "Cataloged Titles",
    activeDevLabel: "In Development",
    playableCharactersLabel: "Players Indexed",
    modsHubCta: "Browse Mods & Community Wikis (JA/EN/FR/ES)",
    wikisBadgeLabel: "wikis",
    modsBadgeLabel: "mod hubs",
    emptyResults: "No titles match the selected filters.",
    resetFiltersLabel: "Reset filters",
  },
  ja: {
    title: "イナズマイレブン シリーズ一覧",
    subtitle:
      "初代ニンテンドーDS作品（2008年）から『ヴィクトリーロード』『クロス』『RE』『英雄たちのグレートロード後継作』まで全タイトルを網羅。",
    upcomingSpotlightHeading: "開発中・最新発表タイトル",
    upcomingSpotlightSubtitle:
      "LEVEL-5公式ポータル inazuma.jp 掲載中の新作プロジェクト",
    allErasLabel: "すべての時代",
    allStatusesLabel: "すべての状態",
    searchPlaceholder: "タイトル・機種・主人公名で検索...",
    protagonistLabel: "主人公",
    releaseLabel: "発売・配信",
    platformsLabel: "対応機種",
    playersCountSuffix: "名 登録済み",
    noPlayersDirectNote: "クロスオーバー／外伝作品",
    officialSiteLabel: "公式サイト",
    viewDetailsLabel: "作品詳細を見る",
    storeLabel: "公式ストア",
    totalGamesLabel: "収録タイトル数",
    activeDevLabel: "開発中",
    playableCharactersLabel: "登録選手数",
    modsHubCta: "MOD・コミュニティWikiハブ (JA/EN/FR/ES)",
    wikisBadgeLabel: "Wiki",
    modsBadgeLabel: "MODハブ",
    emptyResults: "条件に一致するタイトルが見つかりませんでした。",
    resetFiltersLabel: "フィルターをリセット",
  },
  es: {
    title: "Juegos de Inazuma Eleven",
    subtitle:
      "Cronología completa de la saga: desde el primer Inazuma Eleven (2008) hasta Victory Road, Cross y los proyectos en desarrollo (RE, Bold Revolution).",
    upcomingSpotlightHeading: "Juegos anunciados y en desarrollo",
    upcomingSpotlightSubtitle:
      "Proyectos revelados oficialmente por LEVEL-5 en inazuma.jp",
    allErasLabel: "Todas las eras",
    allStatusesLabel: "Todos los estados",
    searchPlaceholder: "Buscar juego, consola o protagonista...",
    protagonistLabel: "Protagonista",
    releaseLabel: "Lanzamiento",
    platformsLabel: "Plataformas",
    playersCountSuffix: "jugadores indexados",
    noPlayersDirectNote: "Plantilla transversal / spin-off",
    officialSiteLabel: "Sitio oficial",
    viewDetailsLabel: "Ver ficha del juego",
    storeLabel: "Tienda oficial",
    totalGamesLabel: "Juegos catalogados",
    activeDevLabel: "En desarrollo",
    playableCharactersLabel: "Jugadores en base",
    modsHubCta: "Explorar mods y wikis (JA/EN/FR/ES)",
    wikisBadgeLabel: "wikis",
    modsBadgeLabel: "hubs de mods",
    emptyResults: "Ningún título coincide con los filtros seleccionados.",
    resetFiltersLabel: "Restablecer filtros",
  },
};

export interface GameDetailCopy {
  wikiHome: string;
  gamesIndex: string;
  overviewHeading: string;
  highlightsHeading: string;
  charactersHeading: string;
  charactersAllCta: string;
  linksHeading: string;
  officialPortalLabel: string;
  storeLabel: string;
  communityWikisHeading: string;
  modHubsHeading: string;
  modsDirectoryCta: string;
  metadataDeveloper: string;
  metadataRelease: string;
  metadataProtagonist: string;
  metadataPlatforms: string;
  relatedHeading: string;
  backToCatalog: string;
  breadcrumbSep: string;
  metaSep: string;
}

export const GAME_DETAIL_COPY_MAP: Readonly<
  Record<InazumaLocale, GameDetailCopy>
> = {
  fr: {
    wikiHome: "Encyclopédie",
    gamesIndex: "Jeux",
    overviewHeading: "Présentation de l'épisode",
    highlightsHeading: "Points clés",
    charactersHeading: "Joueurs représentatifs en base",
    charactersAllCta: "Voir tous les joueurs de cette série",
    linksHeading: "Accès rapides Azalée",
    officialPortalLabel: "Portail officiel LEVEL-5",
    storeLabel: "Web Store officiel",
    communityWikisHeading: "Wikis communautaires (JA / EN / FR / ES)",
    modHubsHeading: "Hubs de mods communautaires",
    modsDirectoryCta: "Ouvrir l'annuaire Mods & Wikis",
    metadataDeveloper: "Développement",
    metadataRelease: "Sortie / État",
    metadataProtagonist: "Héros principal",
    metadataPlatforms: "Support(s)",
    relatedHeading: "Autres épisodes de la saga",
    backToCatalog: "Retour au catalogue des jeux",
    breadcrumbSep: "/",
    metaSep: "·",
  },
  en: {
    wikiHome: "Encyclopedia",
    gamesIndex: "Games",
    overviewHeading: "Chapter Overview",
    highlightsHeading: "Highlights",
    charactersHeading: "Featured Indexed Players",
    charactersAllCta: "Browse all players in this series",
    linksHeading: "Azalee Quick Links",
    officialPortalLabel: "Official LEVEL-5 Portal",
    storeLabel: "Official Web Store",
    communityWikisHeading: "Community Wikis (JA / EN / FR / ES)",
    modHubsHeading: "Community Mod Hubs",
    modsDirectoryCta: "Open Mods & Wikis Directory",
    metadataDeveloper: "Developer",
    metadataRelease: "Release / Status",
    metadataProtagonist: "Protagonist",
    metadataPlatforms: "Platform(s)",
    relatedHeading: "More Saga Titles",
    backToCatalog: "Back to Games Catalog",
    breadcrumbSep: "/",
    metaSep: "·",
  },
  ja: {
    wikiHome: "百科事典",
    gamesIndex: "作品一覧",
    overviewHeading: "作品概要",
    highlightsHeading: "主な特徴",
    charactersHeading: "収録キャラクター",
    charactersAllCta: "このシリーズの選手をすべて表示",
    linksHeading: "Azalee クイックアクセス",
    officialPortalLabel: "LEVEL-5 公式サイト",
    storeLabel: "公式Webストア",
    communityWikisHeading: "コミュニティWiki（日・英・仏・西）",
    modHubsHeading: "MOD配布ハブ",
    modsDirectoryCta: "MOD・Wikiディレクトリを開く",
    metadataDeveloper: "開発元",
    metadataRelease: "発売・配信",
    metadataProtagonist: "主人公",
    metadataPlatforms: "対応プラットフォーム",
    relatedHeading: "その他のシリーズ作品",
    backToCatalog: "作品一覧へ戻る",
    breadcrumbSep: "/",
    metaSep: "・",
  },
  es: {
    wikiHome: "Enciclopedia",
    gamesIndex: "Juegos",
    overviewHeading: "Resumen del título",
    highlightsHeading: "Puntos clave",
    charactersHeading: "Jugadores destacados en la base",
    charactersAllCta: "Ver todos los jugadores de esta serie",
    linksHeading: "Accesos rápidos Azalée",
    officialPortalLabel: "Portal oficial LEVEL-5",
    storeLabel: "Tienda web oficial",
    communityWikisHeading: "Wikis comunitarias (JA / EN / FR / ES)",
    modHubsHeading: "Hubs de mods comunitarios",
    modsDirectoryCta: "Abrir directorio de Mods y Wikis",
    metadataDeveloper: "Desarrollo",
    metadataRelease: "Lanzamiento / Estado",
    metadataProtagonist: "Protagonista",
    metadataPlatforms: "Plataforma(s)",
    relatedHeading: "Otras entregas de la saga",
    backToCatalog: "Volver al catálogo de juegos",
    breadcrumbSep: "/",
    metaSep: "·",
  },
};

export interface SagaHubCopy {
  eyebrow: string;
  bannerTitle: string;
  bannerSubtitle: string;
  gamesBadgeCount: string;
  sagaSectionTitle: string;
  sagaSectionSubtitle: string;
  sagaSectionCta: string;
  gamesCardTitle: string;
  gamesCardDescription: string;
  carouselDesktopTitle: string;
  carouselMobileTitle: string;
  crossBadgeLive: string;
  modsBadgeLabel: string;
  patchBannerLabel: string;
}

export const SAGA_HUB_COPY_MAP: Readonly<Record<InazumaLocale, SagaHubCopy>> = {
  fr: {
    eyebrow: "Encyclopédie multi-jeux Azalée",
    bannerTitle: "Encyclopédie Inazuma Eleven : toute la saga",
    bannerSubtitle:
      "Du premier Inazuma Eleven sur Nintendo DS jusqu'à Victory Road (Ver. 7.2.2), Cross, INAZUMA ELEVEN RE et Bold Revolution : personnages, techniques, équipes et notes de mise à jour.",
    gamesBadgeCount: "18 jeux & projets",
    sagaSectionTitle: "Toute la saga & jeux en développement",
    sagaSectionSubtitle:
      "Parcourez les titres classiques, Victory Road, Cross et les prochains opus officiels",
    sagaSectionCta: "Voir tous les jeux",
    gamesCardTitle: "Jeux & Saga",
    gamesCardDescription: "Du 1er DS à RE & Bold Revolution",
    carouselDesktopTitle: "Wiki : encyclopédie de la saga Inazuma Eleven",
    carouselMobileTitle: "Wiki Inazuma Eleven",
    crossBadgeLive: "Disponible",
    modsBadgeLabel: "Mods & Wikis (JA/ES/EN/FR)",
    patchBannerLabel: "Dernier patch VR",
  },
  en: {
    eyebrow: "Azalee Multi-Game Encyclopedia",
    bannerTitle: "Inazuma Eleven Encyclopedia: Complete Saga",
    bannerSubtitle:
      "From the original Nintendo DS title to Victory Road, Cross, INAZUMA ELEVEN RE, and Bold Revolution: characters, skills, teams, mods, and community wikis.",
    gamesBadgeCount: "18 games & projects",
    sagaSectionTitle: "Complete Saga & Upcoming Releases",
    sagaSectionSubtitle:
      "Browse classic chapters, Victory Road, Cross, and official upcoming projects",
    sagaSectionCta: "View all games",
    gamesCardTitle: "Games & Saga",
    gamesCardDescription: "DS to RE & Bold Revolution",
    carouselDesktopTitle: "Wiki: Complete Inazuma Eleven Saga",
    carouselMobileTitle: "Inazuma Eleven Wiki",
    crossBadgeLive: "Live Now",
    modsBadgeLabel: "Mods & Wikis (JA/ES/EN/FR)",
    patchBannerLabel: "Latest VR Patch",
  },
  ja: {
    eyebrow: "Azalee 全シリーズ対応百科事典",
    bannerTitle: "イナズマイレブン総合百科事典",
    bannerSubtitle:
      "初代DS作品から『ヴィクトリーロード』『クロス』『RE』『続編』まで全作品の選手・必殺技・チーム・MOD・コミュニティWikiを収録。",
    gamesBadgeCount: "全18作品",
    sagaSectionTitle: "シリーズ全作品＆開発中タイトル",
    sagaSectionSubtitle:
      "歴代作品からヴィクトリーロード、クロス、REまで横断して閲覧",
    sagaSectionCta: "作品一覧を見る",
    gamesCardTitle: "作品＆シリーズ一覧",
    gamesCardDescription: "初代DSからRE・最新作まで",
    carouselDesktopTitle: "Wiki: イナズマイレブン全作品百科事典",
    carouselMobileTitle: "イナズマイレブン Wiki",
    crossBadgeLive: "配信中",
    modsBadgeLabel: "MOD・Wiki (日/西/英/仏)",
    patchBannerLabel: "最新VRパッチ",
  },
  es: {
    eyebrow: "Enciclopedia multijuego Azalée",
    bannerTitle: "Enciclopedia Inazuma Eleven: toda la saga",
    bannerSubtitle:
      "Desde el primer Inazuma Eleven de Nintendo DS hasta Victory Road, Cross, INAZUMA ELEVEN RE y Bold Revolution: jugadores, técnicas, equipos y mods.",
    gamesBadgeCount: "18 juegos y proyectos",
    sagaSectionTitle: "Toda la saga y próximos lanzamientos",
    sagaSectionSubtitle:
      "Explora las entregas clásicas, Victory Road, Cross y los próximos títulos oficiales",
    sagaSectionCta: "Ver todos los juegos",
    gamesCardTitle: "Juegos y Saga",
    gamesCardDescription: "De DS a RE y Bold Revolution",
    carouselDesktopTitle: "Wiki: enciclopedia de toda la saga Inazuma Eleven",
    carouselMobileTitle: "Wiki Inazuma Eleven",
    crossBadgeLive: "Disponible",
    modsBadgeLabel: "Mods y Wikis (JA/ES/EN/FR)",
    patchBannerLabel: "Último parche VR",
  },
};

export interface CrossPageCopy {
  releasedStatusBadge: string;
  heroServiceLabel: string;
  heroHeading: string;
  heroKeyVisualAlt: string;
  heroLogoAlt: string;
  noteLabel: string;
  bannerPrefix: string;
  bannerDistinctLabel: string;
  bannerSuffix: string;
  bannerReleasedSuffix: string;
  factualTranslationNote: string;
  storeCtaLabel: string;
  guideCtaLabel: string;
  googlePlayCta: string;
  appStoreCta: string;
  vrCollabSectionTitle: string;
  vrCollabPeriodPrefix: string;
  vrCollabCharactersLabel: string;
  vrCollabSourceLabel: string;
  guideSectionTitle: string;
  guideSourceLabel: string;
  gameSheetCta: string;
  modsCta: string;
}

export const CROSS_PAGE_COPY_MAP: Readonly<
  Record<InazumaLocale, CrossPageCopy>
> = {
  fr: {
    releasedStatusBadge: "Disponible maintenant (iOS / Android)",
    heroServiceLabel: "Service lancé le",
    heroHeading: "Inazuma Eleven Cross",
    heroKeyVisualAlt: "Inazuma Eleven Cross : visuel principal",
    heroLogoAlt: "Logo Inazuma Eleven Cross",
    noteLabel: "À noter :",
    bannerPrefix: "Inazuma Eleven Cross est un jeu mobile ",
    bannerDistinctLabel: "distinct",
    bannerSuffix:
      " d'Inazuma Eleven: Victory Road. Cette page synthétise le portail officiel et l'écosystème technique Azalée.",
    bannerReleasedSuffix: "Service ouvert depuis le",
    factualTranslationNote: "traduction factuelle",
    storeCtaLabel: "Web Store officiel Cross",
    guideCtaLabel: "Guide officiel inazuma-cross.jp",
    googlePlayCta: "Google Play",
    appStoreCta: "App Store",
    vrCollabSectionTitle: "Collaboration Cross × Victory Road",
    vrCollabPeriodPrefix: "Période : ",
    vrCollabCharactersLabel: "Personnages Victory Road à l'affiche",
    vrCollabSourceLabel: "Voir la page officielle victory-road-2026",
    guideSectionTitle: "Points clés du guide officiel",
    guideSourceLabel: "Voir le guide officiel inazuma-cross.jp/guide",
    gameSheetCta: "Fiche jeu dans le catalogue",
    modsCta: "Mods & wikis communautaires",
  },
  en: {
    releasedStatusBadge: "Available Now (iOS / Android)",
    heroServiceLabel: "Launched on",
    heroHeading: "Inazuma Eleven Cross",
    heroKeyVisualAlt: "Inazuma Eleven Cross: key visual",
    heroLogoAlt: "Inazuma Eleven Cross Logo",
    noteLabel: "Note:",
    bannerPrefix: "Inazuma Eleven Cross is a ",
    bannerDistinctLabel: "standalone",
    bannerSuffix:
      " mobile title separate from Inazuma Eleven: Victory Road, synced with the official portal and Azalee schemas.",
    bannerReleasedSuffix: "Live since",
    factualTranslationNote: "factual translation",
    storeCtaLabel: "Cross Official Web Store",
    guideCtaLabel: "Official inazuma-cross.jp Guide",
    googlePlayCta: "Google Play",
    appStoreCta: "App Store",
    vrCollabSectionTitle: "Cross × Victory Road Collaboration",
    vrCollabPeriodPrefix: "Window: ",
    vrCollabCharactersLabel: "Featured Victory Road Characters",
    vrCollabSourceLabel: "Open official victory-road-2026 page",
    guideSectionTitle: "Official Guide Highlights",
    guideSourceLabel: "Open official inazuma-cross.jp/guide",
    gameSheetCta: "View in Games Catalog",
    modsCta: "Mods & Community Wikis",
  },
  ja: {
    releasedStatusBadge: "好評配信中 (iOS / Android)",
    heroServiceLabel: "サービス開始日",
    heroHeading: "イナズマイレブン クロス",
    heroKeyVisualAlt: "イナズマイレブン クロス キービジュアル",
    heroLogoAlt: "イナズマイレブン クロス ロゴ",
    noteLabel: "ご注意:",
    bannerPrefix: "『イナズマイレブン クロス』は『ヴィクトリーロード』とは ",
    bannerDistinctLabel: "独立した",
    bannerSuffix:
      " スマートフォン向け新作アプリです。公式サイト情報とAzaleeデータ構造を掲載しています。",
    bannerReleasedSuffix: "配信開始",
    factualTranslationNote: "公式情報翻訳",
    storeCtaLabel: "Cross 公式Webストア",
    guideCtaLabel: "公式ガイド inazuma-cross.jp",
    googlePlayCta: "Google Play",
    appStoreCta: "App Store",
    vrCollabSectionTitle: "クロス × ヴィクトリーロード コラボ",
    vrCollabPeriodPrefix: "開催期間：",
    vrCollabCharactersLabel: "コラボ参戦『ヴィクトリーロード』選手",
    vrCollabSourceLabel: "公式コラボページを見る",
    guideSectionTitle: "公式ガイド要点",
    guideSourceLabel: "公式ガイドページを開く",
    gameSheetCta: "シリーズ作品カタログを見る",
    modsCta: "MOD・コミュニティWiki",
  },
  es: {
    releasedStatusBadge: "Disponible ahora (iOS / Android)",
    heroServiceLabel: "Lanzamiento:",
    heroHeading: "Inazuma Eleven Cross",
    heroKeyVisualAlt: "Inazuma Eleven Cross: arte principal",
    heroLogoAlt: "Logo de Inazuma Eleven Cross",
    noteLabel: "Nota:",
    bannerPrefix: "Inazuma Eleven Cross es un juego móvil ",
    bannerDistinctLabel: "independiente",
    bannerSuffix:
      " de Inazuma Eleven: Victory Road, conectado al portal oficial y esquemas Azalée.",
    bannerReleasedSuffix: "Disponible desde",
    factualTranslationNote: "traducción factual",
    storeCtaLabel: "Tienda Web oficial Cross",
    guideCtaLabel: "Guía oficial inazuma-cross.jp",
    googlePlayCta: "Google Play",
    appStoreCta: "App Store",
    vrCollabSectionTitle: "Colaboración Cross × Victory Road",
    vrCollabPeriodPrefix: "Periodo: ",
    vrCollabCharactersLabel: "Personajes destacados de Victory Road",
    vrCollabSourceLabel: "Ver página oficial victory-road-2026",
    guideSectionTitle: "Claves de la guía oficial",
    guideSourceLabel: "Ver guía oficial inazuma-cross.jp/guide",
    gameSheetCta: "Ficha en el catálogo",
    modsCta: "Mods y wikis comunitarias",
  },
};

export interface ModsPageCopy {
  title: string;
  subtitle: string;
  statsModsLabel: string;
  statsWikisLabel: string;
  statsHubsLabel: string;
  searchPlaceholder: string;
  allGamesLabel: string;
  allLangsLabel: string;
  modsTabLabel: string;
  wikisTabLabel: string;
  hubsTabLabel: string;
  byAuthorPrefix: string;
  downloadsLabel: string;
  openModCta: string;
  openWikiCta: string;
  openHubCta: string;
  gamesBackCta: string;
  snapshotUpdatedPrefix: string;
  emptyModsMessage: string;
  emptyWikisMessage: string;
}

export const MODS_PAGE_COPY_MAP: Readonly<Record<InazumaLocale, ModsPageCopy>> =
  {
    fr: {
      title: "Mods & Wikis Communautaires",
      subtitle:
        "Index en direct des mods GameBanana, inazuma-eleven.fr/mods et des wikis communautaires (Japonais, Espagnol, Anglais, Français) sur toute la saga.",
      statsModsLabel: "Mods indexés",
      statsWikisLabel: "Pages & sources wikis",
      statsHubsLabel: "Portails GameBanana",
      searchPlaceholder:
        "Filtrer un mod, un wiki, un créateur ou un épisode...",
      allGamesLabel: "Tous les jeux",
      allLangsLabel: "Toutes les langues",
      modsTabLabel: "Mods communautaires",
      wikisTabLabel: "Wikis & Guides (JA/ES/EN/FR)",
      hubsTabLabel: "Hubs GameBanana par jeu",
      byAuthorPrefix: "Par",
      downloadsLabel: "téléchargements",
      openModCta: "Voir le mod",
      openWikiCta: "Ouvrir le wiki",
      openHubCta: "Ouvrir GameBanana",
      gamesBackCta: "Catalogue des 18 jeux",
      snapshotUpdatedPrefix: "Dernier crawl communautaire :",
      emptyModsMessage: "Aucun mod ne correspond à votre filtre.",
      emptyWikisMessage: "Aucune source wiki ne correspond à votre filtre.",
    },
    en: {
      title: "Community Mods & Wikis",
      subtitle:
        "Live directory of GameBanana mods, inazuma-eleven.fr/mods, and community wikis (Japanese, Spanish, English, French) across all titles.",
      statsModsLabel: "Indexed Mods",
      statsWikisLabel: "Wiki Entries & Sources",
      statsHubsLabel: "GameBanana Portals",
      searchPlaceholder: "Filter a mod, wiki, author, or title...",
      allGamesLabel: "All Games",
      allLangsLabel: "All Languages",
      modsTabLabel: "Community Mods",
      wikisTabLabel: "Wikis & Guides (JA/ES/EN/FR)",
      hubsTabLabel: "GameBanana Hubs",
      byAuthorPrefix: "By",
      downloadsLabel: "downloads",
      openModCta: "Open mod",
      openWikiCta: "Open wiki",
      openHubCta: "Open GameBanana",
      gamesBackCta: "Browse all 18 games",
      snapshotUpdatedPrefix: "Latest community crawl:",
      emptyModsMessage: "No mods match your active filter.",
      emptyWikisMessage: "No wiki entries match your active filter.",
    },
    ja: {
      title: "MOD・コミュニティWikiハブ",
      subtitle:
        "GameBanana・inazuma-eleven.fr/mods および日本語・スペイン語・英語・フランス語の各コミュニティWikiを統合インデックス化。",
      statsModsLabel: "登録MOD数",
      statsWikisLabel: "Wiki収録ソース",
      statsHubsLabel: "GameBanana窓口",
      searchPlaceholder: "MOD名・Wiki・制作者・タイトル名で検索...",
      allGamesLabel: "全作品",
      allLangsLabel: "全言語",
      modsTabLabel: "コミュニティMOD",
      wikisTabLabel: "各国語Wiki・攻略 (JA/ES/EN/FR)",
      hubsTabLabel: "作品別GameBanana",
      byAuthorPrefix: "制作:",
      downloadsLabel: "DL",
      openModCta: "MODページを開く",
      openWikiCta: "Wikiを開く",
      openHubCta: "GameBananaへ",
      gamesBackCta: "全18作品カタログ",
      snapshotUpdatedPrefix: "最終クロール:",
      emptyModsMessage: "一致するMODはありません。",
      emptyWikisMessage: "一致するWikiはありません。",
    },
    es: {
      title: "Mods y Wikis Comunitarias",
      subtitle:
        "Directorio en vivo de mods de GameBanana, inazuma-eleven.fr/mods y wikis comunitarias (japonés, español, inglés y francés) de toda la saga.",
      statsModsLabel: "Mods indexados",
      statsWikisLabel: "Páginas y wikis",
      statsHubsLabel: "Portales GameBanana",
      searchPlaceholder: "Filtrar mod, wiki, autor o entrega...",
      allGamesLabel: "Todos los juegos",
      allLangsLabel: "Todos los idiomas",
      modsTabLabel: "Mods comunitarios",
      wikisTabLabel: "Wikis y Guías (JA/ES/EN/FR)",
      hubsTabLabel: "Hubs GameBanana por juego",
      byAuthorPrefix: "Por",
      downloadsLabel: "descargas",
      openModCta: "Ver mod",
      openWikiCta: "Abrir wiki",
      openHubCta: "Abrir GameBanana",
      gamesBackCta: "Catálogo de 18 juegos",
      snapshotUpdatedPrefix: "Último rastreo comunitario:",
      emptyModsMessage: "Ningún mod coincide con tu filtro.",
      emptyWikisMessage: "Ninguna fuente wiki coincide con tu filtro.",
    },
  };

export function getGamesPageCopy(locale?: string | null): GamesPageCopy {
  return GAMES_PAGE_COPY_MAP[normalizeGameLocale(locale)];
}

export function getGameDetailCopy(locale?: string | null): GameDetailCopy {
  return GAME_DETAIL_COPY_MAP[normalizeGameLocale(locale)];
}

export function getSagaHubCopy(locale?: string | null): SagaHubCopy {
  return SAGA_HUB_COPY_MAP[normalizeGameLocale(locale)];
}

export function getCrossPageCopy(locale?: string | null): CrossPageCopy {
  return CROSS_PAGE_COPY_MAP[normalizeGameLocale(locale)];
}

export function getModsPageCopy(locale?: string | null): ModsPageCopy {
  return MODS_PAGE_COPY_MAP[normalizeGameLocale(locale)];
}

export const GAMES_PAGE_COPY = GAMES_PAGE_COPY_MAP.fr;
export const GAME_DETAIL_COPY = GAME_DETAIL_COPY_MAP.fr;
export const SAGA_HUB_COPY = SAGA_HUB_COPY_MAP.fr;
export const CROSS_PAGE_COPY = CROSS_PAGE_COPY_MAP.fr;
export const MODS_PAGE_COPY = MODS_PAGE_COPY_MAP.fr;
