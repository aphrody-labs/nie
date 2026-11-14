/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

export type InazumaLocale = "fr" | "en" | "ja" | "es";

export type InazumaLocalizedText = Readonly<Record<InazumaLocale, string>>;
export type InazumaLocalizedList = Readonly<
  Record<InazumaLocale, readonly string[]>
>;

export type InazumaGameEra =
  | "original"
  | "go"
  | "spinoff"
  | "mobile"
  | "ares-orion"
  | "modern"
  | "upcoming";

export type InazumaGameStatus =
  "released" | "in-development" | "discontinued" | "cancelled-evolved";

export interface InazumaEraInfo {
  id: InazumaGameEra;
  labelFr: string;
  labelEn: string;
  labelJa: string;
  labelEs: string;
  labels: InazumaLocalizedText;
  descriptionFr: string;
  descriptionEn: string;
  descriptionJa: string;
  descriptionEs: string;
  descriptions: InazumaLocalizedText;
}

export interface InazumaGameInternalLink {
  label: string;
  labels?: Partial<InazumaLocalizedText>;
  href: string;
}

export interface InazumaCommunityWiki {
  lang: InazumaLocale;
  name: string;
  url: string;
  apiUrl?: string;
  kind: string;
}

export interface InazumaModHub {
  platform: string;
  name: string;
  url: string;
  gamebananaId?: number;
  apiUrl?: string;
}

export interface InazumaGameRelease {
  platform: string;
  year?: number;
  regions: readonly string[];
  format: string;
  verified: boolean;
}

export interface InazumaGameEcosystemLink {
  kind: string;
  name: string;
  url: string;
}

export interface InazumaGame {
  id: string;
  slug: string;
  order: number;
  series: string;
  titleFr: string;
  titleEn: string;
  titleJa: string;
  titleEs: string;
  titles: InazumaLocalizedText;
  era: InazumaGameEra;
  status: InazumaGameStatus;
  statusDetailFr: string;
  statusDetail: InazumaLocalizedText;
  releaseYear: string;
  releaseDateJa?: string;
  releaseDateEu?: string;
  platforms: readonly string[];
  developer: string;
  protagonist: string;
  thumbnail: string;
  cover?: string;
  officialUrl: string;
  storeUrl?: string;
  /** Série correspondante dans `inagle_characters.series` pour compter et filtrer les joueurs. */
  seriesFilterKey: string | null;
  relatedSeriesKeys?: readonly string[];
  aliases?: readonly string[];
  summaryFr: string;
  summaries: InazumaLocalizedText;
  highlightsFr: readonly string[];
  highlights: InazumaLocalizedList;
  features: Readonly<Record<string, string>>;
  releases: readonly InazumaGameRelease[];
  ecosystem: readonly InazumaGameEcosystemLink[];
  internalLinks: readonly InazumaGameInternalLink[];
  communityWikis: readonly InazumaCommunityWiki[];
  modHubs: readonly InazumaModHub[];
}
