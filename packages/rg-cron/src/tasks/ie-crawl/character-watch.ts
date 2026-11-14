import {
  fetchZukanCharacterIntel,
  resolveDefaultCharacterTarget,
  type CharacterIntelTarget,
  type InagleCharacterIntelResult,
} from "@rosegriffon/inagle";
import { CharacterCrawlStore } from "../../lib/character-store";
import {
  crawlUrlWithEngines,
  discoverCharacterSearchUrls,
  getLightpandaCapabilities,
} from "../../lib/crawl-engines";
import {
  CrawleeFrontierQueue,
  isCrawleeCoreAvailable,
} from "../../lib/crawlee-queue";
import { downloadAndMirrorAsset } from "../../lib/r2-assets";

export interface CharacterCrawlOptions {
  argument?: string;
  slug?: string;
  watch?: boolean;
  intervalMs?: number;
  maxCycles?: number;
  maxPagesPerCycle?: number;
  maxAssetsPerCycle?: number;
  requeueTtlMs?: number;
  sqlitePath?: string;
  r2StagingDir?: string;
  preferBrowser?: boolean;
  skipDiscovery?: boolean;
  skipAssets?: boolean;
  fetchImpl?: typeof fetch;
}

export interface CharacterCrawlReport {
  slug: string;
  canonicalName: string;
  cyclesCompleted: number;
  pagesFetched: number;
  pagesSucceeded: number;
  pagesChallenged: number;
  pagesFailed: number;
  assetsMirrored: number;
  zukanCatalogMatches: number;
  zukanLiveEntries: number;
  enginesUsed: string[];
  crawleeCoreDetected: boolean;
  lightpandaFfiDetected: boolean;
  frontierStats: Record<string, number>;
  startedAt: string;
  finishedAt: string;
}

export async function runCharacterCrawlCycle(
  target: CharacterIntelTarget,
  frontier: CrawleeFrontierQueue,
  store: CharacterCrawlStore,
  options: CharacterCrawlOptions = {},
): Promise<
  Omit<CharacterCrawlReport, "cyclesCompleted" | "startedAt" | "finishedAt">
> {
  const fetchImpl = options.fetchImpl ?? fetch;
  const maxPages = options.maxPagesPerCycle ?? 24;
  const maxAssets = options.maxAssetsPerCycle ?? 16;
  const requeueTtlMs = options.requeueTtlMs ?? 60 * 60 * 1000;

  await store.upsertTarget(target);
  frontier.requeueExpired(target.slug, requeueTtlMs);

  // 1. Seed initial target URLs into durable Crawlee frontier queue.
  frontier.enqueueBatch(
    target.seedUrls.map((url) => ({
      targetSlug: target.slug,
      url,
      sourceKind: url.includes("zukan.") ? "zukan" : "wiki",
      priority: 100,
    })),
  );

  // 2. Real-time Inagle + Zukan character resolution.
  let inagleIntel: InagleCharacterIntelResult = {
    slug: target.slug,
    canonicalName: target.canonicalName,
    fetchedAt: new Date().toISOString(),
    catalogMatches: [],
    liveEntries: [],
    discoveredAssetUrls: [],
    discoveredPageUrls: [],
    errors: [],
  };
  try {
    inagleIntel = await fetchZukanCharacterIntel(target, { fetchImpl });
    frontier.enqueueBatch(
      inagleIntel.discoveredPageUrls.map((url) => ({
        targetSlug: target.slug,
        url,
        sourceKind: "zukan",
        priority: 90,
      })),
    );
  } catch (err) {
    console.warn("[character:crawl] inagle intel warning:", err);
  }

  // 3. Google CSE + Google News RSS + MediaWiki OpenSearch discovery.
  if (!options.skipDiscovery) {
    try {
      const searchHits = await discoverCharacterSearchUrls(
        target.searchQueries,
        {
          fetchImpl,
        },
      );
      frontier.enqueueBatch(
        searchHits.map((hit) => ({
          targetSlug: target.slug,
          url: hit.url,
          engineHint: hit.engine,
          sourceKind: hit.sourceKind,
          priority: 70,
        })),
      );
    } catch (err) {
      console.warn("[character:crawl] discovery warning:", err);
    }
  }

  // 4. Drain durable Crawlee frontier using Obscura / MediaWiki API / Lightpanda / agent-browser.
  const enginesUsed = new Set<string>(
    inagleIntel.liveEntries.length > 0 ? ["zukan-inagle"] : [],
  );
  const discoveredAssets = new Set<string>(inagleIntel.discoveredAssetUrls);

  let pagesFetched = 0;
  let pagesSucceeded = 0;
  let pagesChallenged = 0;
  let pagesFailed = 0;

  for (let i = 0; i < maxPages; i++) {
    const job = frontier.claimNext(target.slug, 250);
    if (!job) break;

    pagesFetched++;
    try {
      const outcome = await crawlUrlWithEngines(job.url, {
        sourceKind: job.source_kind,
        preferBrowser: options.preferBrowser,
        fetchImpl,
      });
      enginesUsed.add(outcome.engine);

      const saved = await store.savePage(target.slug, outcome);
      if (outcome.status === "ok") {
        pagesSucceeded++;
        frontier.markDone(job.id, saved.contentHash);
        for (const assetUrl of outcome.discoveredAssets) {
          discoveredAssets.add(assetUrl);
        }
      } else if (outcome.status === "challenged") {
        pagesChallenged++;
        frontier.markChallenged(
          job.id,
          String(outcome.metadata.challengeProvider ?? "anti-bot"),
        );
      } else {
        pagesFailed++;
        frontier.markFailed(
          job.id,
          String(
            outcome.metadata.error ?? `HTTP ${outcome.httpStatus ?? "error"}`,
          ),
        );
      }
    } catch (err) {
      pagesFailed++;
      frontier.markFailed(
        job.id,
        err instanceof Error ? err.message : String(err),
      );
    }
  }

  // 5. Mirror discovered character assets to Cloudflare R2 / rclone / local R2 staging.
  let assetsMirrored = 0;
  if (!options.skipAssets) {
    for (const assetUrl of [...discoveredAssets].slice(0, maxAssets)) {
      try {
        const uploaded = await downloadAndMirrorAsset(target.slug, assetUrl, {
          fetchImpl,
          stagingDir: options.r2StagingDir,
        });
        if (!uploaded) continue;
        await store.saveAsset(target.slug, null, uploaded);
        assetsMirrored++;
      } catch {
        // Non-fatal individual asset error; continue mirroring remaining assets.
      }
    }
  }

  const frontierStats = frontier.stats(target.slug);
  const lightpandaFfiDetected = Boolean(getLightpandaCapabilities());
  const crawleeCoreDetected = await isCrawleeCoreAvailable();

  const cycleSummary = {
    slug: target.slug,
    canonicalName: target.canonicalName,
    pagesFetched,
    pagesSucceeded,
    pagesChallenged,
    pagesFailed,
    assetsMirrored,
    zukanCatalogMatches: inagleIntel.catalogMatches.length,
    zukanLiveEntries: inagleIntel.liveEntries.length,
    enginesUsed: [...enginesUsed],
    crawleeCoreDetected,
    lightpandaFfiDetected,
    frontierStats,
  };

  await store.upsertTarget(target, cycleSummary);
  await store.publishCycleSummary(target.slug, cycleSummary);

  return cycleSummary;
}

export async function runCharacterCrawl(
  options: CharacterCrawlOptions = {},
): Promise<CharacterCrawlReport> {
  const startedAt = new Date().toISOString();
  const arg = options.argument?.trim();
  const isWatch =
    options.watch === true ||
    arg?.toLowerCase() === "watch" ||
    Bun.env.RG_CRAWL_WATCH === "1";

  const target = resolveDefaultCharacterTarget(
    options.slug ??
      (arg && arg.toLowerCase() !== "watch" ? arg : "afuro-terumi"),
  );

  const frontier = new CrawleeFrontierQueue(options.sqlitePath);
  const store = new CharacterCrawlStore(options.sqlitePath);

  let stopRequested = false;
  const onStop = () => {
    stopRequested = true;
  };
  if (isWatch) {
    process.on("SIGINT", onStop);
    process.on("SIGTERM", onStop);
  }

  const maxCycles =
    options.maxCycles ?? (isWatch ? Number.POSITIVE_INFINITY : 1);
  const intervalMs =
    options.intervalMs ??
    (Number(Bun.env.RG_CRAWL_INTERVAL_MS || "60000") || 60_000);

  let cyclesCompleted = 0;
  let totalPagesFetched = 0;
  let totalPagesSucceeded = 0;
  let totalPagesChallenged = 0;
  let totalPagesFailed = 0;
  let totalAssetsMirrored = 0;
  const allEnginesUsed = new Set<string>();
  let lastCycle: Omit<
    CharacterCrawlReport,
    "cyclesCompleted" | "startedAt" | "finishedAt"
  > | null = null;

  try {
    while (!stopRequested && cyclesCompleted < maxCycles) {
      lastCycle = await runCharacterCrawlCycle(
        target,
        frontier,
        store,
        options,
      );
      cyclesCompleted++;
      totalPagesFetched += lastCycle.pagesFetched;
      totalPagesSucceeded += lastCycle.pagesSucceeded;
      totalPagesChallenged += lastCycle.pagesChallenged;
      totalPagesFailed += lastCycle.pagesFailed;
      totalAssetsMirrored += lastCycle.assetsMirrored;
      for (const engine of lastCycle.enginesUsed) {
        allEnginesUsed.add(engine);
      }

      console.info(
        `[character:crawl] cycle=${cyclesCompleted} slug=${target.slug} ok=${lastCycle.pagesSucceeded} challenged=${lastCycle.pagesChallenged} failed=${lastCycle.pagesFailed} assets=${lastCycle.assetsMirrored} engines=${lastCycle.enginesUsed.join(",") || "none"}`,
      );

      if (!isWatch || stopRequested || cyclesCompleted >= maxCycles) {
        break;
      }
      await Bun.sleep(intervalMs);
    }
  } finally {
    if (isWatch) {
      process.off("SIGINT", onStop);
      process.off("SIGTERM", onStop);
    }
    if (!lastCycle) {
      lastCycle = {
        slug: target.slug,
        canonicalName: target.canonicalName,
        pagesFetched: 0,
        pagesSucceeded: 0,
        pagesChallenged: 0,
        pagesFailed: 0,
        assetsMirrored: 0,
        zukanCatalogMatches: 0,
        zukanLiveEntries: 0,
        enginesUsed: [],
        crawleeCoreDetected: false,
        lightpandaFfiDetected: false,
        frontierStats: frontier.stats(target.slug),
      };
    }
    frontier.close();
    store.close();
  }

  return {
    ...lastCycle,
    pagesFetched: totalPagesFetched || lastCycle.pagesFetched,
    pagesSucceeded: totalPagesSucceeded || lastCycle.pagesSucceeded,
    pagesChallenged: totalPagesChallenged || lastCycle.pagesChallenged,
    pagesFailed: totalPagesFailed || lastCycle.pagesFailed,
    assetsMirrored: totalAssetsMirrored || lastCycle.assetsMirrored,
    enginesUsed:
      allEnginesUsed.size > 0 ? [...allEnginesUsed] : lastCycle.enginesUsed,
    cyclesCompleted,
    startedAt,
    finishedAt: new Date().toISOString(),
  };
}
