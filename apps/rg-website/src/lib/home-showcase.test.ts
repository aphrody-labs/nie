import { describe, expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { join } from "node:path";

import {
  ASSET_SHOWCASE_COPY,
  BENTO_COPY,
  ELEMENT_ASSETS,
  HERO_SHOWCASE_DATA,
  RANK_TIER_ASSETS,
  RARITY_ASSETS,
  ROLE_ASSETS,
  sanitiserNomServeurDiscord,
} from "./home-showcase";
import { PROJETS_ACCUEIL } from "./projets";
import { SIDEBAR_MASCOTS } from "./sidebar-showcase";

const PUBLIC_DIR = join(import.meta.dir, "../../public");

function assertPublicAsset(assetPath: string) {
  if (!assetPath.startsWith("/")) return;
  const diskPath = join(PUBLIC_DIR, assetPath.slice(1));
  expect(
    existsSync(diskPath),
    `Asset manquant dans public/: ${assetPath}`,
  ).toBe(true);
}

describe("home-showcase & sidebar-showcase assets", () => {
  test("sanitiserNomServeurDiscord retire les emojis sans abîmer le libellé", () => {
    expect(
      sanitiserNomServeurDiscord("Rose Griffon 🌹 - Inazuma Eleven FR"),
    ).toBe("Rose Griffon - Inazuma Eleven FR");
    expect(
      sanitiserNomServeurDiscord("Achillea 🌸 - Compétitif Inazuma Eleven FR"),
    ).toBe("Achillea - Compétitif Inazuma Eleven FR");
  });

  test("tous les assets locaux référencés existent sur disque", () => {
    const localPaths = [
      HERO_SHOWCASE_DATA.artwork.src,
      HERO_SHOWCASE_DATA.backdrop.src,
      HERO_SHOWCASE_DATA.badge.logo,
      ...HERO_SHOWCASE_DATA.charactersChip.map((c) => c.image),
      ...HERO_SHOWCASE_DATA.ctas.map((c) => c.icon),
      ...ELEMENT_ASSETS.map((e) => e.image),
      ...ROLE_ASSETS.map((r) => r.image),
      ...RARITY_ASSETS.map((r) => r.image),
      ...RANK_TIER_ASSETS.map((t) => t.image),
      BENTO_COPY.azalee.logoSrc,
      BENTO_COPY.achillea.logoSrc,
      BENTO_COPY.achillea.astralImage,
      BENTO_COPY.achillea.fallbackClanAvatar,
      BENTO_COPY.rgtv.logoSrc,
      BENTO_COPY.rgtv.artworkSrc,
      ASSET_SHOWCASE_COPY.viewer3d.posterBgSrc,
      ...Object.values(SIDEBAR_MASCOTS).map((m) => m.image),
      ...PROJETS_ACCUEIL.map((p) => p.image).filter(
        (img): img is string => typeof img === "string",
      ),
    ];

    for (const p of localPaths) {
      assertPublicAsset(p);
    }
  });
});
