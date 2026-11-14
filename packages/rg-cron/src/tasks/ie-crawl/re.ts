/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  downloadAssetIfMissing,
  fetchHtmlResilient,
  resolveRepoRoot,
} from "../../lib/http-crawl";
import { crawlDataPath } from "../../lib/shared-path";
import { extraireRePortal } from "./portal-extract";

const RE_URLS = [
  { lang: "en", url: "https://www.inazuma.jp/re/en/" },
  { lang: "ja", url: "https://www.inazuma.jp/re/" },
] as const;

export async function crawlRe(): Promise<{
  success: boolean;
  error?: string;
}> {
  console.log("[Crawl RE] Recuperation du portail INAZUMA ELEVEN RE...");

  try {
    const extractedByLang: Record<
      string,
      ReturnType<typeof extraireRePortal>
    > = {};
    let teaserImageUrl: string | null = null;

    for (const src of RE_URLS) {
      const res = await fetchHtmlResilient(src.url);
      if (!res.ok || res.html.length < 400) continue;

      const extracted = extraireRePortal(res.html);
      extractedByLang[src.lang] = extracted;
      if (!teaserImageUrl && extracted.teaserImageUrl) {
        teaserImageUrl = extracted.teaserImageUrl;
      }

      try {
        const outDir = crawlDataPath("inazuma.jp/re", src.lang);
        await mkdir(outDir, { recursive: true });
        await writeFile(join(outDir, "index.html"), res.html, "utf8");
        await writeFile(
          join(outDir, "meta.json"),
          JSON.stringify(
            {
              crawledAt: new Date().toISOString(),
              url: src.url,
              ...extracted,
            },
            null,
            2,
          ),
          "utf8",
        );
      } catch {
        // Disque partage optionnel
      }
    }

    if (Object.keys(extractedByLang).length === 0) {
      return {
        success: false,
        error: "Portail INAZUMA ELEVEN RE inaccessible",
      };
    }

    const repoRoot = resolveRepoRoot();
    if (repoRoot) {
      if (teaserImageUrl) {
        await downloadAssetIfMissing(
          teaserImageUrl,
          join(repoRoot, "apps/rg-website/public/games/inazuma-re-pv.jpg"),
        );
      }
      await writeFile(
        join(repoRoot, "packages/inagle/data/re-live.json"),
        `${JSON.stringify(
          {
            crawledAt: new Date().toISOString(),
            en: extractedByLang.en ?? null,
            ja: extractedByLang.ja ?? null,
          },
          null,
          2,
        )}\n`,
        "utf8",
      );
    }

    console.log(
      `[Crawl RE] Synchronise (${Object.keys(extractedByLang).join("/")}).`,
    );
    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error("[Crawl RE] Erreur critique :", msg);
    return { success: false, error: msg };
  }
}
