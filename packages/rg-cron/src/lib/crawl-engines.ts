import { existsSync } from "node:fs";
import { CString, dlopen, FFIType, ptr, type Pointer } from "bun:ffi";
import type { CrawlEngineKind, CrawlPageStatus } from "@rosegriffon/db/schema";
import {
  assertPublicCrawlUrl,
  isObscuraCdpAvailable,
  obscuraFetch,
} from "./obscura";
import { chargerDocument } from "./webview";

const DEFAULT_BROWSER_AGENT_UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

export interface AntiBotDetectionResult {
  challenged: boolean;
  provider: string | null;
}

export interface CrawlFetchOutcome {
  url: string;
  normalizedUrl: string;
  engine: CrawlEngineKind;
  sourceKind: string;
  status: CrawlPageStatus;
  httpStatus: number | null;
  title: string | null;
  language: string | null;
  excerpt: string | null;
  bodyText: string | null;
  discoveredLinks: string[];
  discoveredAssets: string[];
  metadata: Record<string, unknown>;
}

export function detectAntiBotChallenge(
  httpStatus: number | null,
  body: string,
  title?: string | null,
): AntiBotDetectionResult {
  const sample = `${title ?? ""}\n${body.slice(0, 4096)}`.toLowerCase();
  if (
    sample.includes("just a moment...") ||
    sample.includes("cf-challenge") ||
    sample.includes("/cdn-cgi/challenge-platform") ||
    (httpStatus === 403 && sample.includes("cloudflare"))
  ) {
    return { challenged: true, provider: "cloudflare" };
  }
  if (sample.includes("cf-turnstile")) {
    return { challenged: true, provider: "turnstile" };
  }
  if (sample.includes("g-recaptcha") || sample.includes("unusual traffic")) {
    return { challenged: true, provider: "recaptcha" };
  }
  if (sample.includes("hcaptcha.com")) {
    return { challenged: true, provider: "hcaptcha" };
  }
  if (
    httpStatus === 403 &&
    (sample.includes("verify you are human") ||
      sample.includes("enable cookies"))
  ) {
    return { challenged: true, provider: "bot-gate" };
  }
  return { challenged: false, provider: null };
}

export interface MediaWikiTargetInfo {
  apiEndpoint: string;
  pageTitle: string;
  canonicalWikiUrl: string;
  language: string;
}

export function resolveMediaWikiTarget(
  rawUrl: string,
): MediaWikiTargetInfo | null {
  let parsed: URL;
  try {
    parsed = new URL(rawUrl);
  } catch {
    return null;
  }
  const host = parsed.hostname.toLowerCase();
  if (!host.endsWith(".fandom.com") && !host.endsWith("wikipedia.org")) {
    return null;
  }

  // Matches /wiki/Title or /fr/wiki/Title
  const match = parsed.pathname.match(/^\/(?:([a-z]{2,5})\/)?wiki\/(.+)$/i);
  if (!match) return null;

  const langPrefix = match[1]?.toLowerCase() ?? null;
  const pageTitle = decodeURIComponent(match[2]!).replace(/_/g, " ");
  const basePath = langPrefix ? `/${langPrefix}` : "";
  return {
    apiEndpoint: `${parsed.origin}${basePath}/api.php`,
    pageTitle,
    canonicalWikiUrl: `${parsed.origin}${basePath}/wiki/${encodeURIComponent(pageTitle.replace(/ /g, "_"))}`,
    language: langPrefix ?? (host.startsWith("fr.") ? "fr" : "en"),
  };
}

export async function fetchMediaWikiApi(
  rawUrl: string,
  options: { fetchImpl?: typeof fetch; timeoutMs?: number } = {},
): Promise<CrawlFetchOutcome | null> {
  const target = resolveMediaWikiTarget(rawUrl);
  if (!target) return null;

  assertPublicCrawlUrl(target.apiEndpoint);
  const fetchImpl = options.fetchImpl ?? fetch;
  const timeoutMs = options.timeoutMs ?? 15_000;

  const queryParams = new URLSearchParams({
    action: "query",
    titles: target.pageTitle,
    prop: "extracts|pageimages|categories|images|revisions|info",
    inprop: "url",
    explaintext: "1",
    exsectionformat: "plain",
    pithumbsize: "1024",
    cllimit: "50",
    imlimit: "25",
    redirects: "1",
    format: "json",
  });

  const apiUrl = `${target.apiEndpoint}?${queryParams.toString()}`;
  const response = await fetchImpl(apiUrl, {
    headers: {
      "user-agent":
        "RoseGriffonBot/1.0 (https://rosegriffon.fr; contact@rosegriffon.fr)",
      accept: "application/json",
    },
    signal: AbortSignal.timeout(timeoutMs),
  });

  if (!response.ok) {
    const body = await response.text().catch(() => "");
    const challenge = detectAntiBotChallenge(response.status, body);
    return {
      url: rawUrl,
      normalizedUrl: target.canonicalWikiUrl,
      engine: "mediawiki-api",
      sourceKind: "wiki",
      status: challenge.challenged ? "challenged" : "error",
      httpStatus: response.status,
      title: target.pageTitle,
      language: target.language,
      excerpt: null,
      bodyText: null,
      discoveredLinks: [],
      discoveredAssets: [],
      metadata: {
        apiUrl,
        challengeProvider: challenge.provider,
      },
    };
  }

  const data = (await response.json()) as {
    query?: {
      pages?: Record<
        string,
        {
          pageid?: number;
          title?: string;
          fullurl?: string;
          extract?: string;
          thumbnail?: { source?: string };
          original?: { source?: string };
          categories?: Array<{ title?: string }>;
          images?: Array<{ title?: string }>;
        }
      >;
    };
  };

  const pageObj = Object.values(data.query?.pages ?? {})[0];
  if (!pageObj) {
    return null;
  }

  const discoveredAssets = new Set<string>();
  if (pageObj.thumbnail?.source) discoveredAssets.add(pageObj.thumbnail.source);
  if (pageObj.original?.source) discoveredAssets.add(pageObj.original.source);

  // Resolve up to 8 wiki image files to direct CDN URLs via MediaWiki imageinfo API.
  const fileTitles = (pageObj.images ?? [])
    .map((img) => img.title)
    .filter((t): t is string => Boolean(t))
    .filter((t) => /\.(png|jpe?g|webp|gif)$/i.test(t))
    .slice(0, 8);

  if (fileTitles.length > 0) {
    try {
      const imgParams = new URLSearchParams({
        action: "query",
        titles: fileTitles.join("|"),
        prop: "imageinfo",
        iiprop: "url|mime",
        format: "json",
      });
      const imgRes = await fetchImpl(
        `${target.apiEndpoint}?${imgParams.toString()}`,
        {
          headers: { accept: "application/json" },
          signal: AbortSignal.timeout(timeoutMs),
        },
      );
      if (imgRes.ok) {
        const imgJson = (await imgRes.json()) as {
          query?: {
            pages?: Record<
              string,
              { imageinfo?: Array<{ url?: string; mime?: string }> }
            >;
          };
        };
        for (const imgPage of Object.values(imgJson.query?.pages ?? {})) {
          const directUrl = imgPage.imageinfo?.[0]?.url;
          if (directUrl) discoveredAssets.add(directUrl);
        }
      }
    } catch {
      // Optional imageinfo enrichment failure is non-fatal.
    }
  }

  const bodyTextParts: string[] = [];
  if (pageObj.extract?.trim()) {
    bodyTextParts.push(pageObj.extract.trim());
  }

  let parsedTitle = pageObj.title ?? target.pageTitle;
  const discoveredLinks = new Set<string>(
    pageObj.fullurl ? [pageObj.fullurl] : [],
  );

  // Many MediaWiki installs (including Fandom) disable TextExtracts: fall back to `action=parse`.
  if (bodyTextParts.length === 0) {
    try {
      const parseParams = new URLSearchParams({
        action: "parse",
        page: parsedTitle,
        prop: "text|displaytitle|images",
        redirects: "1",
        format: "json",
      });
      const parseRes = await fetchImpl(
        `${target.apiEndpoint}?${parseParams.toString()}`,
        {
          headers: { accept: "application/json" },
          signal: AbortSignal.timeout(timeoutMs),
        },
      );
      if (parseRes.ok) {
        const parseJson = (await parseRes.json()) as {
          parse?: {
            title?: string;
            displaytitle?: string;
            text?: { "*"?: string };
            images?: string[];
          };
        };
        const htmlContent = parseJson.parse?.text?.["*"] ?? "";
        if (htmlContent) {
          const parsedSignals = extractHtmlSignals(
            htmlContent,
            pageObj.fullurl ?? target.canonicalWikiUrl,
          );
          if (parsedSignals.text) bodyTextParts.push(parsedSignals.text);
          for (const asset of parsedSignals.assets) {
            if (!asset.startsWith("data:")) discoveredAssets.add(asset);
          }
          for (const link of parsedSignals.links.slice(0, 20)) {
            discoveredLinks.add(link);
          }
        }
        if (parseJson.parse?.title) parsedTitle = parseJson.parse.title;
      }
    } catch {
      // Optional action=parse fallback error is non-fatal.
    }
  }

  const bodyText = bodyTextParts.join("\n").trim();
  const categories = (pageObj.categories ?? [])
    .map((c) => c.title)
    .filter((t): t is string => Boolean(t));

  return {
    url: rawUrl,
    normalizedUrl: pageObj.fullurl ?? target.canonicalWikiUrl,
    engine: "mediawiki-api",
    sourceKind: "wiki",
    status: "ok",
    httpStatus: response.status,
    title: parsedTitle,
    language: target.language,
    excerpt: bodyText.slice(0, 500) || null,
    bodyText: bodyText || null,
    discoveredLinks: [...discoveredLinks],
    discoveredAssets: [...discoveredAssets],
    metadata: {
      pageId: pageObj.pageid ?? null,
      apiEndpoint: target.apiEndpoint,
      categories,
      imageCount: discoveredAssets.size,
    },
  };
}

// ---------------------------------------------------------------------------
// Lightpanda native FFI (`browser_capi.dll`) + CLI/CDP bridge
// ---------------------------------------------------------------------------

const lightpandaSymbols = {
  lp_abi_version: { args: [], returns: FFIType.u32 },
  lp_free: { args: [FFIType.ptr], returns: FFIType.void },
  lp_capabilities_json: { args: [], returns: FFIType.ptr },
  lp_extract_title_json: { args: [FFIType.ptr], returns: FFIType.ptr },
  lp_normalize_url_json: { args: [FFIType.ptr], returns: FFIType.ptr },
} as const;

type LightpandaLib = ReturnType<typeof dlopen<typeof lightpandaSymbols>>;
let cachedLpLib: LightpandaLib | null | undefined;

function getLightpandaLib(): LightpandaLib | null {
  if (cachedLpLib !== undefined) return cachedLpLib;
  const candidate =
    Bun.env.LIGHTPANDA_DLL_PATH ||
    "C:\\src\\packages\\browser\\native\\zig-out\\bin\\browser_capi.dll";
  if (!existsSync(candidate)) {
    cachedLpLib = null;
    return null;
  }
  try {
    const lib = dlopen(candidate, lightpandaSymbols);
    if (lib.symbols.lp_abi_version() !== 1) {
      cachedLpLib = null;
      return null;
    }
    cachedLpLib = lib;
    return lib;
  } catch {
    cachedLpLib = null;
    return null;
  }
}

function readLpJson<T>(
  lib: LightpandaLib,
  p: Pointer | bigint | null,
): T | null {
  if (!p) return null;
  const ptrVal = p as Pointer;
  try {
    const text = new CString(ptrVal).toString();
    return JSON.parse(text) as T;
  } catch {
    return null;
  } finally {
    lib.symbols.lp_free(ptrVal);
  }
}

function lpCStringPtr(value: string): Pointer {
  return ptr(Buffer.from(`${value}\0`, "utf8"));
}

export function getLightpandaCapabilities(): Record<string, unknown> | null {
  const lib = getLightpandaLib();
  if (!lib) return null;
  return readLpJson<Record<string, unknown>>(
    lib,
    lib.symbols.lp_capabilities_json(),
  );
}

export function extractTitleWithLightpanda(html: string): string | null {
  const lib = getLightpandaLib();
  if (!lib) return null;
  const res = readLpJson<{ ok?: boolean; title?: string | null }>(
    lib,
    lib.symbols.lp_extract_title_json(lpCStringPtr(html)),
  );
  return res?.ok ? (res.title ?? null) : null;
}

export function normalizeUrlWithLightpanda(rawUrl: string): string | null {
  const lib = getLightpandaLib();
  if (!lib) return null;
  const res = readLpJson<{ ok?: boolean; normalized?: string | null }>(
    lib,
    lib.symbols.lp_normalize_url_json(lpCStringPtr(rawUrl)),
  );
  return res?.ok ? (res.normalized ?? null) : null;
}

// ---------------------------------------------------------------------------
// HTML extraction (Bun native HTMLRewriter, zero extra dependencies)
// ---------------------------------------------------------------------------

export function extractHtmlSignals(
  html: string,
  baseUrl: string,
): {
  title: string | null;
  language: string | null;
  text: string;
  links: string[];
  assets: string[];
} {
  let title = extractTitleWithLightpanda(html);
  if (!title) {
    const titleMatch = html.match(/<title[^>]*>([\s\S]*?)<\/title>/i);
    title = titleMatch?.[1]?.replace(/\s+/g, " ").trim() ?? null;
  }

  const langMatch = html.match(/<html[^>]+lang=["']([^"']+)["']/i);
  const language = langMatch?.[1]?.trim() ?? null;

  const links = new Set<string>();
  const assets = new Set<string>();
  const rewriter = new HTMLRewriter()
    .on("a[href]", {
      element(el) {
        const href = el.getAttribute("href");
        if (!href || href.startsWith("#") || href.startsWith("javascript:"))
          return;
        try {
          const resolved = new URL(href, baseUrl);
          if (resolved.protocol === "http:" || resolved.protocol === "https:") {
            links.add(resolved.href);
          }
        } catch {
          // ignore malformed href
        }
      },
    })
    .on("img[src], img[data-src], meta[property='og:image']", {
      element(el) {
        const raw =
          el.getAttribute("data-src") ||
          el.getAttribute("src") ||
          el.getAttribute("content");
        if (!raw || raw.startsWith("data:")) return;
        try {
          const resolved = new URL(raw, baseUrl);
          if (resolved.protocol === "http:" || resolved.protocol === "https:") {
            assets.add(resolved.href);
          }
        } catch {
          // ignore malformed asset URL
        }
      },
    });

  // Trigger HTMLRewriter handlers synchronously/fast over full HTML.
  rewriter.transform(html);

  const plainText = html
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<style[\s\S]*?<\/style>/gi, " ")
    .replace(/<noscript[\s\S]*?<\/noscript>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/\s+/g, " ")
    .trim();

  return {
    title,
    language,
    text: plainText,
    links: [...links].slice(0, 60),
    assets: [...assets].slice(0, 40),
  };
}

// ---------------------------------------------------------------------------
// Multi-engine orchestrator: MediaWiki API -> Crawlee Fetch -> Obscura -> Lightpanda -> agent-browser
// ---------------------------------------------------------------------------

export async function crawlUrlWithEngines(
  rawUrl: string,
  options: {
    sourceKind?: string;
    preferBrowser?: boolean;
    allowExternalCli?: boolean;
    fetchImpl?: typeof fetch;
    timeoutMs?: number;
  } = {},
): Promise<CrawlFetchOutcome> {
  const safeUrl = assertPublicCrawlUrl(rawUrl);
  const timeoutMs = options.timeoutMs ?? 18_000;
  const allowExternalCli = options.allowExternalCli ?? !options.fetchImpl;
  const fetchImpl = options.fetchImpl ?? fetch;
  const sourceKind = options.sourceKind ?? "web";

  // 1. MediaWiki API fast path for Fandom / Wikipedia URLs (avoids Cloudflare HTML challenges cleanly).
  if (resolveMediaWikiTarget(safeUrl.href)) {
    const wikiOutcome = await fetchMediaWikiApi(safeUrl.href, {
      fetchImpl,
      timeoutMs,
    });
    if (wikiOutcome && wikiOutcome.status === "ok") {
      return wikiOutcome;
    }
  }

  const lpNormalized = normalizeUrlWithLightpanda(safeUrl.href) ?? safeUrl.href;

  // 2. Obscura CLI/daemon fetch when available.
  const obscuraPath = allowExternalCli ? Bun.which("obscura") : null;
  if (obscuraPath && !options.preferBrowser) {
    try {
      const obscuraHtml = await obscuraFetch(safeUrl.href, {
        dump: "html",
        timeoutMs,
      });
      if (obscuraHtml.ok && obscuraHtml.content) {
        const challenge = detectAntiBotChallenge(
          obscuraHtml.status,
          obscuraHtml.content,
        );
        if (!challenge.challenged) {
          const signals = extractHtmlSignals(obscuraHtml.content, safeUrl.href);
          return {
            url: safeUrl.href,
            normalizedUrl: lpNormalized,
            engine: "obscura",
            sourceKind,
            status: "ok",
            httpStatus: obscuraHtml.status,
            title: signals.title,
            language: signals.language,
            excerpt: signals.text.slice(0, 500) || null,
            bodyText: signals.text || null,
            discoveredLinks: signals.links,
            discoveredAssets: signals.assets,
            metadata: {
              obscuraCdpReady: await isObscuraCdpAvailable(),
              lightpandaFfi: Boolean(getLightpandaCapabilities()),
            },
          };
        }
      }
    } catch {
      // Fall through to HTTP fetch / Lightpanda / agent-browser.
    }
  }

  // 3. Lightpanda CLI / FFI assisted HTTP fetch.
  const lightpandaBin = allowExternalCli ? Bun.which("lightpanda") : null;
  if (lightpandaBin && options.preferBrowser) {
    try {
      const proc = Bun.spawn([lightpandaBin, "fetch", safeUrl.href], {
        stdout: "pipe",
        stderr: "pipe",
      });
      const exitCode = await proc.exited;
      const html = await new Response(proc.stdout).text();
      if (exitCode === 0 && html) {
        const challenge = detectAntiBotChallenge(200, html);
        if (!challenge.challenged) {
          const signals = extractHtmlSignals(html, safeUrl.href);
          return {
            url: safeUrl.href,
            normalizedUrl: lpNormalized,
            engine: "lightpanda",
            sourceKind,
            status: "ok",
            httpStatus: 200,
            title: signals.title,
            language: signals.language,
            excerpt: signals.text.slice(0, 500) || null,
            bodyText: signals.text || null,
            discoveredLinks: signals.links,
            discoveredAssets: signals.assets,
            metadata: { lightpandaCli: true },
          };
        }
      }
    } catch {
      // Fall through cleanly.
    }
  }

  // 4. Direct Crawlee HTTP fetch + Lightpanda FFI signal extraction.
  try {
    const res = await fetchImpl(safeUrl.href, {
      headers: {
        "user-agent": DEFAULT_BROWSER_AGENT_UA,
        accept:
          "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "accept-language": "fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7,ja;q=0.6",
      },
      signal: AbortSignal.timeout(timeoutMs),
    });
    const body = await res.text();
    const challenge = detectAntiBotChallenge(res.status, body);
    if (challenge.challenged) {
      return {
        url: safeUrl.href,
        normalizedUrl: lpNormalized,
        engine: "crawlee-fetch",
        sourceKind,
        status: "challenged",
        httpStatus: res.status,
        title: null,
        language: null,
        excerpt: null,
        bodyText: null,
        discoveredLinks: [],
        discoveredAssets: [],
        metadata: { challengeProvider: challenge.provider },
      };
    }

    if (res.ok && body) {
      const signals = extractHtmlSignals(body, safeUrl.href);
      const lpCaps = getLightpandaCapabilities();
      return {
        url: safeUrl.href,
        normalizedUrl: lpNormalized,
        engine: lpCaps ? "lightpanda" : "crawlee-fetch",
        sourceKind,
        status: "ok",
        httpStatus: res.status,
        title: signals.title,
        language: signals.language,
        excerpt: signals.text.slice(0, 500) || null,
        bodyText: signals.text || null,
        discoveredLinks: signals.links,
        discoveredAssets: signals.assets,
        metadata: {
          lightpandaFfi: Boolean(lpCaps),
        },
      };
    }
  } catch (error) {
    if (!options.preferBrowser) {
      return {
        url: safeUrl.href,
        normalizedUrl: lpNormalized,
        engine: "crawlee-fetch",
        sourceKind,
        status: "error",
        httpStatus: null,
        title: null,
        language: null,
        excerpt: null,
        bodyText: null,
        discoveredLinks: [],
        discoveredAssets: [],
        metadata: {
          error: error instanceof Error ? error.message : String(error),
        },
      };
    }
  }

  // 5. Optional agent-browser / Bun.WebView fallback when explicitly requested.
  if (options.preferBrowser) {
    try {
      const doc = await chargerDocument(safeUrl.href, timeoutMs);
      const challenge = detectAntiBotChallenge(doc.status, doc.html, doc.title);
      if (challenge.challenged) {
        return {
          url: safeUrl.href,
          normalizedUrl: lpNormalized,
          engine: "agent-browser",
          sourceKind,
          status: "challenged",
          httpStatus: doc.status,
          title: doc.title,
          language: null,
          excerpt: null,
          bodyText: null,
          discoveredLinks: [],
          discoveredAssets: [],
          metadata: { challengeProvider: challenge.provider },
        };
      }
      const signals = extractHtmlSignals(doc.html, doc.url);
      return {
        url: safeUrl.href,
        normalizedUrl: lpNormalized,
        engine: "agent-browser",
        sourceKind,
        status: "ok",
        httpStatus: doc.status,
        title: signals.title ?? doc.title,
        language: signals.language,
        excerpt: signals.text.slice(0, 500) || null,
        bodyText: signals.text || null,
        discoveredLinks: signals.links,
        discoveredAssets: signals.assets,
        metadata: { finalUrl: doc.url },
      };
    } catch (error) {
      return {
        url: safeUrl.href,
        normalizedUrl: lpNormalized,
        engine: "agent-browser",
        sourceKind,
        status: "error",
        httpStatus: null,
        title: null,
        language: null,
        excerpt: null,
        bodyText: null,
        discoveredLinks: [],
        discoveredAssets: [],
        metadata: {
          error: error instanceof Error ? error.message : String(error),
        },
      };
    }
  }

  return {
    url: safeUrl.href,
    normalizedUrl: lpNormalized,
    engine: "crawlee-fetch",
    sourceKind,
    status: "error",
    httpStatus: null,
    title: null,
    language: null,
    excerpt: null,
    bodyText: null,
    discoveredLinks: [],
    discoveredAssets: [],
    metadata: {},
  };
}

// ---------------------------------------------------------------------------
// Google + Web open search discovery (Google CSE API, Google News RSS, OpenSearch)
// ---------------------------------------------------------------------------

export interface DiscoveredSearchItem {
  url: string;
  title: string;
  snippet: string;
  engine: CrawlEngineKind;
  sourceKind: string;
}

export async function discoverCharacterSearchUrls(
  queries: readonly string[],
  options: { fetchImpl?: typeof fetch; timeoutMs?: number } = {},
): Promise<DiscoveredSearchItem[]> {
  const fetchImpl = options.fetchImpl ?? fetch;
  const timeoutMs = options.timeoutMs ?? 12_000;
  const discovered: DiscoveredSearchItem[] = [];
  const seenUrls = new Set<string>();

  const pushUnique = (item: DiscoveredSearchItem) => {
    try {
      const safe = assertPublicCrawlUrl(item.url);
      if (seenUrls.has(safe.href)) return;
      seenUrls.add(safe.href);
      discovered.push({ ...item, url: safe.href });
    } catch {
      // Ignore unsafe/malformed discovery URLs.
    }
  };

  // 1. Google Custom Search JSON API (when configured via GOOGLE_API_KEY + GOOGLE_CSE_ID).
  const googleApiKey = Bun.env.GOOGLE_API_KEY?.trim();
  const googleCseId = Bun.env.GOOGLE_CSE_ID?.trim();
  if (googleApiKey && googleCseId) {
    for (const q of queries.slice(0, 3)) {
      try {
        const apiUrl = new URL("https://www.googleapis.com/customsearch/v1");
        apiUrl.searchParams.set("key", googleApiKey);
        apiUrl.searchParams.set("cx", googleCseId);
        apiUrl.searchParams.set("q", q);
        apiUrl.searchParams.set("num", "10");
        const res = await fetchImpl(apiUrl.href, {
          signal: AbortSignal.timeout(timeoutMs),
        });
        if (!res.ok) continue;
        const json = (await res.json()) as {
          items?: Array<{ link?: string; title?: string; snippet?: string }>;
        };
        for (const item of json.items ?? []) {
          if (!item.link) continue;
          pushUnique({
            url: item.link,
            title: item.title ?? "",
            snippet: item.snippet ?? "",
            engine: "google-cse",
            sourceKind: "search",
          });
        }
      } catch {
        // Non-fatal search failure.
      }
    }
  }

  // 2. Google News RSS (public RSS feed, zero anti-bot gate).
  for (const q of queries.slice(0, 2)) {
    try {
      const rssUrl = `https://news.google.com/rss/search?q=${encodeURIComponent(q)}&hl=fr&gl=FR&ceid=FR:fr`;
      const res = await fetchImpl(rssUrl, {
        headers: { accept: "application/rss+xml,application/xml,text/xml" },
        signal: AbortSignal.timeout(timeoutMs),
      });
      if (!res.ok) continue;
      const xml = await res.text();
      const itemRegex = /<item>([\s\S]*?)<\/item>/gi;
      let match: RegExpExecArray | null;
      let count = 0;
      while ((match = itemRegex.exec(xml)) !== null && count < 8) {
        const block = match[1] ?? "";
        const link = block.match(/<link>([\s\S]*?)<\/link>/i)?.[1]?.trim();
        const title = block
          .match(/<title>([\s\S]*?)<\/title>/i)?.[1]
          ?.replace(/<!\[CDATA\[|\]\]>/g, "")
          .trim();
        if (link) {
          pushUnique({
            url: link,
            title: title ?? "Google News result",
            snippet: title ?? "",
            engine: "google-news-rss",
            sourceKind: "news",
          });
          count++;
        }
      }
    } catch {
      // Non-fatal RSS failure.
    }
  }

  // 3. Fandom MediaWiki OpenSearch across EN / FR / Manga-Encyclopedie wikis.
  const wikiApis = [
    "https://inazuma-eleven.fandom.com/api.php",
    "https://inazuma-eleven.fandom.com/fr/api.php",
    "https://manga-encyclopedie.fandom.com/fr/api.php",
  ];
  const primarySearch = queries[0] ?? "Byron Love";
  for (const endpoint of wikiApis) {
    try {
      const url = new URL(endpoint);
      url.searchParams.set("action", "opensearch");
      url.searchParams.set("search", primarySearch);
      url.searchParams.set("limit", "5");
      url.searchParams.set("format", "json");
      const res = await fetchImpl(url.href, {
        headers: { accept: "application/json" },
        signal: AbortSignal.timeout(timeoutMs),
      });
      if (!res.ok) continue;
      const payload = (await res.json()) as [
        string?,
        string[]?,
        string[]?,
        string[]?,
      ];
      const titles = payload[1] ?? [];
      const snippets = payload[2] ?? [];
      const urls = payload[3] ?? [];
      for (let i = 0; i < urls.length; i++) {
        const targetUrl = urls[i];
        if (!targetUrl) continue;
        pushUnique({
          url: targetUrl,
          title: titles[i] ?? "",
          snippet: snippets[i] ?? "",
          engine: "mediawiki-api",
          sourceKind: "wiki",
        });
      }
    } catch {
      // Non-fatal OpenSearch failure.
    }
  }

  return discovered;
}
