/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { mkdir, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import {
  downloadAssetIfMissing,
  fetchHtmlResilient,
  resolveRepoRoot,
} from "../../lib/http-crawl";
import { crawlDataPath } from "../../lib/shared-path";
import {
  extraireSeriesPortal,
  type ExtractedSeriesGame,
} from "./portal-extract";

const SERIES_PORTALS = [
  { key: "series-home", url: "https://www.inazuma.jp/" },
  { key: "18th", url: "https://www.inazuma.jp/18th/" },
  {
    key: "bold-revolution-fr",
    url: "https://www.inazuma.jp/victory-road/fr/topics/260910_02/",
  },
  {
    key: "bold-revolution-en",
    url: "https://www.inazuma.jp/victory-road/en/topics/260910_02/",
  },
  { key: "sd", url: "https://www.inazuma.jp/sd/" },
  { key: "everyday-plus", url: "https://www.inazuma.jp/everyday-plus/" },
] as const;

export async function crawlSeries(): Promise<{
  success: boolean;
  error?: string;
}> {
  console.log("[Crawl Series] Indexation du portail officiel inazuma.jp...");

  try {
    const homeRes = await fetchHtmlResilient(SERIES_PORTALS[0].url);
    if (!homeRes.ok || homeRes.html.length < 400) {
      return {
        success: false,
        error: `Echec chargement inazuma.jp (status ${homeRes.status})`,
      };
    }

    const games: ExtractedSeriesGame[] = extraireSeriesPortal(homeRes.html);
    const repoRoot = resolveRepoRoot();
    const mirroredGames: Array<
      ExtractedSeriesGame & { localThumbnail: string | null }
    > = [];

    for (const game of games) {
      let localThumbnail: string | null = null;
      if (repoRoot && game.thumbnailUrl) {
        const cleanName = basename(new URL(game.thumbnailUrl).pathname).replace(
          /[^a-zA-Z0-9._-]/g,
          "",
        );
        if (cleanName) {
          const dest = join(
            repoRoot,
            "apps/rg-website/public/games",
            cleanName,
          );
          const status = await downloadAssetIfMissing(game.thumbnailUrl, dest);
          if (status !== "failed") {
            localThumbnail = `/games/${cleanName}`;
          }
        }
      }
      mirroredGames.push({ ...game, localThumbnail });
    }

    const portalStatus: Record<string, { ok: boolean; status: number }> = {
      "series-home": { ok: homeRes.ok, status: homeRes.status },
    };
    for (const portal of SERIES_PORTALS.slice(1)) {
      const r = await fetchHtmlResilient(portal.url);
      portalStatus[portal.key] = { ok: r.ok, status: r.status };
      if (r.ok) {
        try {
          const dir = crawlDataPath("inazuma.jp/series", portal.key);
          await mkdir(dir, { recursive: true });
          await writeFile(join(dir, "index.html"), r.html, "utf8");
        } catch {
          // Optionnel hors VPS
        }
      }
    }

    const payload = {
      crawledAt: new Date().toISOString(),
      source: SERIES_PORTALS[0].url,
      gamesCount: mirroredGames.length,
      games: mirroredGames,
      portalStatus,
    };

    if (repoRoot) {
      await downloadAssetIfMissing(
        "https://www.inazuma.jp/victory-road/en/topics/260910_02/img/img_main.jpg",
        join(repoRoot, "apps/rg-website/public/games/img_bold-revolution.jpg"),
      );
      await downloadAssetIfMissing(
        "https://www.inazuma.jp/sd/image/sd/image01.jpg",
        join(repoRoot, "apps/rg-website/public/games/img_sd.jpg"),
      );
      await writeFile(
        join(repoRoot, "packages/inagle/data/series-live.json"),
        `${JSON.stringify(payload, null, 2)}\n`,
        "utf8",
      );
    }

    console.log(
      `[Crawl Series] ${mirroredGames.length} jeux officiels indexés sur inazuma.jp.`,
    );
    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error("[Crawl Series] Erreur critique :", msg);
    return { success: false, error: msg };
  }
}
