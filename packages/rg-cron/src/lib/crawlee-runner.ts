/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Real Crawlee (`crawlee@3.18.2`) crawler orchestration powered by
 * `CheerioCrawler` + `@crawlee/impit-client` (`ImpitHttpClient`) for fast TLS-impersonated
 * static extraction and `PlaywrightCrawler` + `patchright` (`1.64.3`) for stealth SPA rendering.
 */

import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { CrawlEngineKind } from "@rosegriffon/db/schema";
import { createCrawleeImpitClient } from "./impit-client";
import { assertPublicCrawlUrl } from "./obscura";

export interface CrawleeBatchItemResult {
  url: string;
  finalUrl: string;
  status: number;
  title: string | null;
  language: string | null;
  text: string;
  links: string[];
  assets: string[];
  html: string;
  engine: Extract<CrawlEngineKind, "crawlee-cheerio" | "crawlee-playwright">;
  error?: string;
}

const runtimeDynamicImport = new Function(
  "specifier",
  "return import(specifier)",
) as <T>(specifier: string) => Promise<T>;

function detectBrowserExecutable(): string | undefined {
  const fromEnv =
    Bun.env.RG_BROWSER_EXECUTABLE_PATH ??
    Bun.env.BUN_CHROME_PATH ??
    Bun.env.CHROME_PATH;
  if (fromEnv && existsSync(fromEnv)) return fromEnv;

  const candidates =
    process.platform === "win32"
      ? [
          "C:/Program Files/Google/Chrome/Application/chrome.exe",
          "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
        ]
      : [
          "/usr/bin/google-chrome",
          "/usr/bin/chromium",
          "/usr/bin/chromium-browser",
          "/opt/google/chrome/chrome",
        ];
  return candidates.find((p) => existsSync(p));
}

function collectDomAssetsAndLinks(
  $: {
    (selector: string): {
      each: (fn: (idx: number, el: unknown) => void) => unknown;
      attr: (name: string) => string | undefined;
    };
  },
  baseUrl: string,
) {
  const links = new Set<string>();
  const assets = new Set<string>();

  $("a[href]").each((_idx, el) => {
    const href = $(el as unknown as string).attr("href");
    if (!href || href.startsWith("#") || href.startsWith("javascript:")) return;
    try {
      const resolved = new URL(href, baseUrl);
      if (resolved.protocol === "http:" || resolved.protocol === "https:") {
        links.add(resolved.href);
      }
    } catch {
      // ignore invalid URL
    }
  });

  $("img[src], img[data-src], meta[property='og:image']").each((_idx, el) => {
    const node = $(el as unknown as string);
    const raw =
      node.attr("data-src") || node.attr("src") || node.attr("content");
    if (!raw || raw.startsWith("data:")) return;
    try {
      const resolved = new URL(raw, baseUrl);
      if (resolved.protocol === "http:" || resolved.protocol === "https:") {
        assets.add(resolved.href);
      }
    } catch {
      // ignore invalid asset URL
    }
  });

  return {
    links: [...links].slice(0, 60),
    assets: [...assets].slice(0, 40),
  };
}

/**
 * Runs a real Crawlee `CheerioCrawler` batch backed by `ImpitHttpClient` (Chrome TLS).
 */
export async function runCrawleeCheerioBatch(
  urls: readonly string[],
  options: {
    maxConcurrency?: number;
    maxRequestsPerCrawl?: number;
    timeoutSecs?: number;
    maxRetries?: number;
  } = {},
): Promise<CrawleeBatchItemResult[]> {
  const safeUrls = urls
    .map((u) => {
      try {
        return assertPublicCrawlUrl(u).href;
      } catch {
        return null;
      }
    })
    .filter((u): u is string => Boolean(u));

  if (safeUrls.length === 0) return [];

  const crawleeMod =
    await runtimeDynamicImport<typeof import("crawlee")>("crawlee");
  const cheerioMod =
    await runtimeDynamicImport<typeof import("cheerio")>("cheerio");
  const storageDir = mkdtempSync(join(tmpdir(), "rg-crawlee-cheerio-"));
  const results: CrawleeBatchItemResult[] = [];

  try {
    const config = new crawleeMod.Configuration({
      persistStorage: false,
      storageClientOptions: { localDataDirectory: storageDir },
    });
    const impitClient = await createCrawleeImpitClient();
    const crawler = new crawleeMod.CheerioCrawler(
      {
        ...(impitClient ? { httpClient: impitClient } : {}),
        maxConcurrency: options.maxConcurrency ?? 4,
        maxRequestsPerCrawl:
          options.maxRequestsPerCrawl ?? Math.max(10, safeUrls.length),
        maxRequestRetries: options.maxRetries ?? 1,
        requestHandlerTimeoutSecs: options.timeoutSecs ?? 25,
        async requestHandler({ request, response, body, json, $ }) {
          const rawText =
            typeof body === "string"
              ? body
              : body
                ? body.toString("utf8")
                : json
                  ? JSON.stringify(json)
                  : "";
          const finalUrl = request.loadedUrl || request.url;
          const contentType = String(
            response?.headers?.["content-type"] ?? "",
          ).toLowerCase();

          if (!$ || contentType.includes("application/json")) {
            results.push({
              url: request.url,
              finalUrl,
              status: response?.statusCode ?? 200,
              title: finalUrl,
              language: null,
              text: rawText.slice(0, 64_000),
              links: [],
              assets: [],
              html: rawText,
              engine: "crawlee-cheerio",
            });
            return;
          }

          const $doc = cheerioMod.load(rawText);
          const title =
            $doc("title").first().text().replace(/\s+/g, " ").trim() ||
            $doc("meta[property='og:title']").attr("content")?.trim() ||
            null;
          const language = $doc("html").attr("lang")?.trim() || null;

          const { links, assets } = collectDomAssetsAndLinks($, finalUrl);
          $doc("script, style, noscript, template").remove();
          const text = $doc("body").text().replace(/\s+/g, " ").trim();

          results.push({
            url: request.url,
            finalUrl,
            status: response?.statusCode ?? 200,
            title,
            language,
            text,
            links,
            assets,
            html: rawText,
            engine: "crawlee-cheerio",
          });
        },
        failedRequestHandler({ request }, error) {
          results.push({
            url: request.url,
            finalUrl: request.loadedUrl || request.url,
            status: 0,
            title: null,
            language: null,
            text: "",
            links: [],
            assets: [],
            html: "",
            engine: "crawlee-cheerio",
            error: error instanceof Error ? error.message : String(error),
          });
        },
      },
      config,
    );

    await crawler.run(safeUrls);
    return results;
  } finally {
    rmSync(storageDir, { recursive: true, force: true });
  }
}

/**
 * Runs a real Crawlee `PlaywrightCrawler` batch using stealth `patchright` Chromium.
 */
export async function runCrawleePatchrightBatch(
  urls: readonly string[],
  options: {
    maxConcurrency?: number;
    maxRequestsPerCrawl?: number;
    timeoutSecs?: number;
    waitAfterLoadMs?: number;
  } = {},
): Promise<CrawleeBatchItemResult[]> {
  const safeUrls = urls
    .map((u) => {
      try {
        return assertPublicCrawlUrl(u).href;
      } catch {
        return null;
      }
    })
    .filter((u): u is string => Boolean(u));

  if (safeUrls.length === 0) return [];

  const [crawleeMod, patchrightMod, cheerioMod] = await Promise.all([
    runtimeDynamicImport<typeof import("crawlee")>("crawlee"),
    runtimeDynamicImport<typeof import("patchright")>("patchright"),
    runtimeDynamicImport<typeof import("cheerio")>("cheerio"),
  ]);

  const storageDir = mkdtempSync(join(tmpdir(), "rg-crawlee-pw-"));
  const results: CrawleeBatchItemResult[] = [];
  const executablePath = detectBrowserExecutable();

  try {
    const config = new crawleeMod.Configuration({
      persistStorage: false,
      storageClientOptions: { localDataDirectory: storageDir },
    });
    const crawler = new crawleeMod.PlaywrightCrawler(
      {
        maxConcurrency: options.maxConcurrency ?? 2,
        maxRequestsPerCrawl:
          options.maxRequestsPerCrawl ?? Math.max(10, safeUrls.length),
        maxRequestRetries: 1,
        requestHandlerTimeoutSecs: options.timeoutSecs ?? 40,
        launchContext: {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          launcher: patchrightMod.chromium as any,
          launchOptions: {
            headless: true,
            ...(executablePath ? { executablePath } : {}),
            args: [
              "--no-sandbox",
              "--disable-dev-shm-usage",
              "--disable-blink-features=AutomationControlled",
            ],
          },
        },
        async requestHandler({ request, page, response }) {
          if (options.waitAfterLoadMs && options.waitAfterLoadMs > 0) {
            await page.waitForTimeout(options.waitAfterLoadMs);
          }
          const html = await page.content();
          const finalUrl = page.url() || request.url;
          const $doc = cheerioMod.load(html);
          const title =
            (await page.title().catch(() => "")) ||
            $doc("title").first().text().replace(/\s+/g, " ").trim() ||
            null;
          const language = $doc("html").attr("lang")?.trim() || null;

          const links = new Set<string>();
          const assets = new Set<string>();
          $doc("a[href]").each((_idx, el) => {
            const href = $doc(el).attr("href");
            if (!href || href.startsWith("#") || href.startsWith("javascript:"))
              return;
            try {
              const resolved = new URL(href, finalUrl);
              if (
                resolved.protocol === "http:" ||
                resolved.protocol === "https:"
              ) {
                links.add(resolved.href);
              }
            } catch {
              // ignore invalid URL
            }
          });
          $doc("img[src], img[data-src], meta[property='og:image']").each(
            (_idx, el) => {
              const node = $doc(el);
              const raw =
                node.attr("data-src") ||
                node.attr("src") ||
                node.attr("content");
              if (!raw || raw.startsWith("data:")) return;
              try {
                const resolved = new URL(raw, finalUrl);
                if (
                  resolved.protocol === "http:" ||
                  resolved.protocol === "https:"
                ) {
                  assets.add(resolved.href);
                }
              } catch {
                // ignore invalid asset URL
              }
            },
          );

          $doc("script, style, noscript, template").remove();
          const text = $doc("body").text().replace(/\s+/g, " ").trim();

          results.push({
            url: request.url,
            finalUrl,
            status: response?.status() ?? 200,
            title,
            language,
            text,
            links: [...links].slice(0, 60),
            assets: [...assets].slice(0, 40),
            html,
            engine: "crawlee-playwright",
          });
        },
        failedRequestHandler({ request }, error) {
          results.push({
            url: request.url,
            finalUrl: request.loadedUrl || request.url,
            status: 0,
            title: null,
            language: null,
            text: "",
            links: [],
            assets: [],
            html: "",
            engine: "crawlee-playwright",
            error: error instanceof Error ? error.message : String(error),
          });
        },
      },
      config,
    );

    await crawler.run(safeUrls);
    return results;
  } finally {
    rmSync(storageDir, { recursive: true, force: true });
  }
}
