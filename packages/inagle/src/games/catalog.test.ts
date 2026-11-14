/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, expect, test } from "bun:test";
import {
  getCommunityWikis,
  getDbSeriesKeysForGame,
  getGameBySlug,
  getGamesByStatus,
  getLocalizedGame,
  getModHubs,
  INAZUMA_ERAS,
  INAZUMA_GAMES,
  queryCommunityWikis,
  queryGameDetail,
  queryGamesCatalog,
  queryModHubs,
} from "./index";

describe("@rosegriffon/inagle/games", () => {
  test("couvre toute la saga du 1er épisode jusqu'aux titres en développement", () => {
    expect(INAZUMA_GAMES[0]?.slug).toBe("inazuma-eleven");
    expect(getGameBySlug("inazuma-eleven-victory-road")?.status).toBe(
      "released",
    );
    expect(getGameBySlug("inazuma-eleven-cross")?.releaseDateJa).toBe(
      "2026-06-09",
    );
    expect(getGameBySlug("inazuma-eleven-re")?.status).toBe("in-development");
    expect(getGameBySlug("inazuma-eleven-bold-revolution")?.status).toBe(
      "in-development",
    );

    const slugs = new Set(INAZUMA_GAMES.map((g) => g.slug));
    expect(slugs.size).toBe(INAZUMA_GAMES.length);
  });

  test("expose les jeux en développement et les clés séries DB", () => {
    const dev = getGamesByStatus("in-development");
    expect(dev.map((g) => g.slug)).toEqual([
      "inazuma-eleven-re",
      "inazuma-eleven-bold-revolution",
    ]);

    const ares = getGameBySlug("inazuma-eleven-ares-great-road")!;
    expect(getDbSeriesKeysForGame(ares)).toEqual(["Ares", "Orion"]);

    const eraIds = new Set(INAZUMA_ERAS.map((e) => e.id));
    for (const game of INAZUMA_GAMES) {
      expect(eraIds.has(game.era)).toBe(true);
    }
  });

  test("localise les jeux en 4 langues et expose les wikis/mods communautaires depuis games.toml", () => {
    const vr = getGameBySlug("inazuma-eleven-victory-road")!;
    const vrEs = getLocalizedGame(vr, "es");
    const vrJa = getLocalizedGame(vr, "ja");
    expect(vrEs.title).toContain("Victory Road");
    expect(vrEs.summary.length).toBeGreaterThan(20);
    expect(vrJa.title).toContain("ヴィクトリーロード");
    expect(getCommunityWikis("es").length).toBeGreaterThanOrEqual(5);
    expect(getCommunityWikis("ja").length).toBeGreaterThanOrEqual(5);
    expect(getModHubs().length).toBeGreaterThanOrEqual(10);

    const devJa = queryGamesCatalog({ locale: "ja", status: "in-development" });
    expect(devJa.locale).toBe("ja");
    expect(devJa.total).toBe(2);
    const reDetail = queryGameDetail("inazuma-eleven-re", "en");
    expect(reDetail?.game.slug).toBe("inazuma-eleven-re");
    expect(queryGameDetail("does-not-exist")).toBeNull();
    expect(queryCommunityWikis({ lang: "es", limit: 3 })).toHaveLength(3);
    expect(
      queryModHubs({ gameSlug: "inazuma-eleven-victory-road" }).length,
    ).toBeGreaterThanOrEqual(1);
  });
});
