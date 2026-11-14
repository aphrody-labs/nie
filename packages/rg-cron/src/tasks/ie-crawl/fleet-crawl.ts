import { timingSafeEqual } from "node:crypto";
import { hostname } from "node:os";
import {
  fetchZukanCharacterIntel,
  resolveDefaultCharacterTarget,
} from "@rosegriffon/inagle";
import type { CrawlEngineKind } from "@rosegriffon/db/schema";
import { CharacterCrawlStore } from "../../lib/character-store";
import {
  crawlUrlWithEngines,
  detectLocalBrowserCapabilities,
  discoverCharacterSearchUrls,
  filterRecursiveChildLinks,
  type CrawlFetchOutcome,
} from "../../lib/crawl-engines";
import {
  CrawleeFrontierQueue,
  type FrontierRow,
} from "../../lib/crawlee-queue";
import { downloadAndMirrorAsset } from "../../lib/r2-assets";

export interface RegisteredFleetWorker {
  workerId: string;
  host: string;
  meshIp: string | null;
  browsers: CrawlEngineKind[];
  concurrency: number;
  claimedCount: number;
  submittedCount: number;
  succeededCount: number;
  failedCount: number;
  challengedCount: number;
  registeredAt: string;
  lastSeenAt: string;
}

export interface FleetCoordinatorOptions {
  targetSlug?: string;
  maxDepth?: number;
  maxPages?: number;
  maxAssetsPerPage?: number;
  port?: number;
  host?: string;
  authToken?: string;
  sqlitePath?: string;
  r2StagingDir?: string;
  skipDiscovery?: boolean;
  skipAssets?: boolean;
  fetchImpl?: typeof fetch;
}

export interface FleetCoordinatorHandle {
  url: string;
  targetSlug: string;
  stop: () => Promise<void>;
  getStatus: () => Record<string, unknown>;
}

function constantTimeMatch(received: string, expected: string): boolean {
  const a = Buffer.from(received);
  const b = Buffer.from(expected);
  return a.length === b.length && timingSafeEqual(a, b);
}

function resolveCoordinatorToken(explicit?: string): string {
  return (
    explicit?.trim() ||
    Bun.env.RG_FLEET_TOKEN?.trim() ||
    Bun.env.CRON_SECRET?.trim() ||
    ""
  );
}

export async function startFleetCoordinator(
  options: FleetCoordinatorOptions = {},
): Promise<FleetCoordinatorHandle> {
  const target = resolveDefaultCharacterTarget(options.targetSlug);
  const maxDepth = options.maxDepth ?? 2;
  const maxPages = options.maxPages ?? 200;
  const maxAssetsPerPage = options.maxAssetsPerPage ?? 6;
  const authToken = resolveCoordinatorToken(options.authToken);
  const fetchImpl = options.fetchImpl ?? fetch;

  const frontier = new CrawleeFrontierQueue(options.sqlitePath);
  const store = new CharacterCrawlStore(options.sqlitePath);
  await store.upsertTarget(target);

  const seedHosts = new Set<string>();
  for (const s of target.seedUrls) {
    try {
      seedHosts.add(new URL(s).hostname.toLowerCase());
    } catch {
      // ignore invalid URL
    }
  }

  // Seed canonical target URLs + Inagle Zukan + Google/Web discovery.
  frontier.enqueueBatch(
    target.seedUrls.map((url) => ({
      targetSlug: target.slug,
      url,
      sourceKind: url.includes("zukan.") ? "zukan" : "wiki",
      priority: 100,
      depth: 0,
    })),
  );

  const discoveredSeedAssets = new Set<string>();
  try {
    const inagle = await fetchZukanCharacterIntel(target, { fetchImpl });
    for (const a of inagle.discoveredAssetUrls) discoveredSeedAssets.add(a);
    frontier.enqueueBatch(
      inagle.discoveredPageUrls.map((url) => ({
        targetSlug: target.slug,
        url,
        sourceKind: "zukan",
        priority: 90,
        depth: 0,
      })),
    );
  } catch (err) {
    console.warn("[fleet-coordinator] inagle seed warning:", err);
  }

  if (!options.skipDiscovery) {
    try {
      const hits = await discoverCharacterSearchUrls(target.searchQueries, {
        fetchImpl,
      });
      frontier.enqueueBatch(
        hits.map((hit) => ({
          targetSlug: target.slug,
          url: hit.url,
          engineHint: hit.engine,
          sourceKind: hit.sourceKind,
          priority: 80,
          depth: 0,
        })),
      );
    } catch (err) {
      console.warn("[fleet-coordinator] discovery seed warning:", err);
    }
  }

  if (!options.skipAssets) {
    await Promise.all(
      [...discoveredSeedAssets].slice(0, 10).map(async (assetUrl) => {
        try {
          const uploaded = await downloadAndMirrorAsset(target.slug, assetUrl, {
            fetchImpl,
            stagingDir: options.r2StagingDir,
          });
          if (uploaded) await store.saveAsset(target.slug, null, uploaded);
        } catch {
          // non-fatal seed asset failure
        }
      }),
    );
  }

  const workers = new Map<string, RegisteredFleetWorker>();
  let totalSubmitted = 0;
  let totalAssetsMirrored = 0;
  const hostEngineCounts: Record<string, number> = {};
  let browserRoundRobin = 0;
  let closed = false;

  function checkAuth(req: Request): boolean {
    if (!authToken) return true;
    const header = req.headers.get("authorization") ?? "";
    return constantTimeMatch(header, `Bearer ${authToken}`);
  }

  function pickAssignedBrowser(
    available: readonly CrawlEngineKind[],
  ): CrawlEngineKind | "auto" {
    const candidates = available.filter(
      (b) =>
        b === "lightpanda" ||
        b === "obscura" ||
        b === "agent-browser" ||
        b === "crawlee-fetch",
    );
    if (candidates.length === 0) return "auto";
    const picked = candidates[browserRoundRobin % candidates.length]!;
    browserRoundRobin++;
    return picked;
  }

  const getStatus = (): Record<string, unknown> => ({
    targetSlug: target.slug,
    canonicalName: target.canonicalName,
    maxDepth,
    maxPages,
    totalSubmitted,
    totalAssetsMirrored,
    frontierStats: frontier.stats(target.slug),
    depthHistogram: frontier.depthHistogram(target.slug),
    hostEngineCounts,
    workers: [...workers.values()],
    pagesStored: store.listPages(target.slug).length,
    assetsStored: store.listAssets(target.slug).length,
  });

  const server = Bun.serve({
    hostname: options.host ?? Bun.env.RG_FLEET_HOST ?? "0.0.0.0",
    port: options.port ?? Number(Bun.env.RG_FLEET_PORT ?? 8812),
    async fetch(req) {
      const url = new URL(req.url);
      if (url.pathname === "/health" && req.method === "GET") {
        return Response.json({ ok: true, ...getStatus() });
      }

      if (!checkAuth(req)) {
        return Response.json({ error: "Unauthorized" }, { status: 401 });
      }

      if (url.pathname === "/v1/fleet-crawl/status" && req.method === "GET") {
        return Response.json(getStatus());
      }

      if (
        req.method === "POST" &&
        url.pathname === "/v1/fleet-crawl/register"
      ) {
        const body = (await req.json()) as {
          workerId: string;
          host: string;
          meshIp?: string | null;
          browsers?: CrawlEngineKind[];
          concurrency?: number;
        };
        const nowIso = new Date().toISOString();
        const existing = workers.get(body.workerId);
        const reg: RegisteredFleetWorker = {
          workerId: body.workerId,
          host: body.host,
          meshIp: body.meshIp ?? null,
          browsers: body.browsers ?? ["crawlee-fetch"],
          concurrency: body.concurrency ?? 1,
          claimedCount: existing?.claimedCount ?? 0,
          submittedCount: existing?.submittedCount ?? 0,
          succeededCount: existing?.succeededCount ?? 0,
          failedCount: existing?.failedCount ?? 0,
          challengedCount: existing?.challengedCount ?? 0,
          registeredAt: existing?.registeredAt ?? nowIso,
          lastSeenAt: nowIso,
        };
        workers.set(body.workerId, reg);
        return Response.json({ ok: true, worker: reg, target });
      }

      if (req.method === "POST" && url.pathname === "/v1/fleet-crawl/claim") {
        const body = (await req.json()) as {
          workerId: string;
          limit?: number;
          browsers?: CrawlEngineKind[];
        };
        if (totalSubmitted >= maxPages) {
          return Response.json({ jobs: [] });
        }

        const worker = workers.get(body.workerId);
        if (worker) worker.lastSeenAt = new Date().toISOString();

        const rows = frontier.claimBatch(target.slug, {
          limit: Math.min(8, Math.max(1, body.limit ?? 2)),
          maxDepth,
          workerId: body.workerId,
          hostCooldownMs: 350,
        });

        const browsers = body.browsers ?? worker?.browsers ?? [];
        const jobs = rows.map((row: FrontierRow) => ({
          id: row.id,
          url: row.url,
          depth: row.depth,
          parentUrl: row.parent_url,
          sourceKind: row.source_kind,
          enginePreference: pickAssignedBrowser(browsers),
        }));
        if (worker) worker.claimedCount += jobs.length;

        return Response.json({ jobs });
      }

      if (req.method === "POST" && url.pathname === "/v1/fleet-crawl/submit") {
        const body = (await req.json()) as {
          workerId: string;
          workerHost: string;
          jobId: number;
          depth: number;
          parentUrl?: string | null;
          outcome: CrawlFetchOutcome;
        };

        const worker = workers.get(body.workerId);
        if (worker) {
          worker.lastSeenAt = new Date().toISOString();
          worker.submittedCount++;
          if (body.outcome.status === "ok") worker.succeededCount++;
          else if (body.outcome.status === "challenged")
            worker.challengedCount++;
          else worker.failedCount++;
        }

        totalSubmitted++;
        const heKey = `${body.workerHost}:${body.outcome.engine}`;
        hostEngineCounts[heKey] = (hostEngineCounts[heKey] ?? 0) + 1;

        const saved = await store.savePage(target.slug, body.outcome, {
          depth: body.depth,
          parentUrl: body.parentUrl ?? null,
          workerHost: body.workerHost,
          workerId: body.workerId,
        });

        let recursiveAdded = 0;
        let assetsAdded = 0;

        if (body.outcome.status === "ok") {
          frontier.markDone(body.jobId, saved.contentHash);

          if (
            body.depth < maxDepth &&
            body.outcome.discoveredLinks.length > 0
          ) {
            const nextLinks = filterRecursiveChildLinks(
              body.outcome.discoveredLinks,
              {
                parentUrl: body.outcome.url,
                depth: body.depth,
                maxDepth,
                aliases: target.aliases,
                seedHosts,
                maxLinks: 10,
              },
            );
            recursiveAdded = frontier.enqueueBatch(
              nextLinks.map((childUrl) => ({
                targetSlug: target.slug,
                url: childUrl,
                sourceKind: childUrl.includes("zukan.") ? "zukan" : "recursive",
                priority: Math.max(20, 75 - (body.depth + 1) * 15),
                depth: body.depth + 1,
                parentUrl: body.outcome.url,
              })),
            );
          }

          if (!options.skipAssets && body.outcome.discoveredAssets.length > 0) {
            for (const assetUrl of body.outcome.discoveredAssets.slice(
              0,
              maxAssetsPerPage,
            )) {
              try {
                const uploaded = await downloadAndMirrorAsset(
                  target.slug,
                  assetUrl,
                  {
                    fetchImpl,
                    stagingDir: options.r2StagingDir,
                  },
                );
                if (uploaded) {
                  await store.saveAsset(
                    target.slug,
                    body.outcome.url,
                    uploaded,
                  );
                  assetsAdded++;
                  totalAssetsMirrored++;
                }
              } catch {
                // individual asset errors non-fatal
              }
            }
          }
        } else if (body.outcome.status === "challenged") {
          frontier.markChallenged(
            body.jobId,
            String(body.outcome.metadata.challengeProvider ?? "anti-bot"),
          );
        } else {
          frontier.markFailed(
            body.jobId,
            String(
              body.outcome.metadata.error ??
                `HTTP ${body.outcome.httpStatus ?? "error"}`,
            ),
          );
        }

        const summary = getStatus();
        await store.upsertTarget(target, summary);
        await store.publishCycleSummary(target.slug, summary);

        return Response.json({
          ok: true,
          contentHash: saved.contentHash,
          recursiveAdded,
          assetsAdded,
        });
      }

      if (req.method === "POST" && url.pathname === "/v1/fleet-crawl/seed") {
        const body = (await req.json()) as { urls?: string[]; depth?: number };
        const added = frontier.enqueueBatch(
          (body.urls ?? []).map((u) => ({
            targetSlug: target.slug,
            url: u,
            sourceKind: "manual",
            priority: 95,
            depth: body.depth ?? 0,
          })),
        );
        return Response.json({ ok: true, added });
      }

      return Response.json({ error: "Not found" }, { status: 404 });
    },
  });

  const resolvedUrl = `http://${server.hostname === "0.0.0.0" ? "127.0.0.1" : server.hostname}:${server.port}`;

  return {
    url: resolvedUrl,
    targetSlug: target.slug,
    getStatus,
    stop: async () => {
      if (closed) return;
      closed = true;
      server.stop(true);
      frontier.close();
      store.close();
    },
  };
}

export interface FleetWorkerOptions {
  coordinatorUrl: string;
  host?: string;
  meshIp?: string | null;
  concurrency?: number;
  authToken?: string;
  watch?: boolean;
  maxBatches?: number;
  idleExitMs?: number;
  allowExternalCli?: boolean;
  fetchImpl?: typeof fetch;
}

export interface FleetWorkerResult {
  workerId: string;
  host: string;
  browsers: CrawlEngineKind[];
  concurrency: number;
  processed: number;
  succeeded: number;
  challenged: number;
  failed: number;
}

export async function runFleetWorker(
  options: FleetWorkerOptions,
): Promise<FleetWorkerResult> {
  const host = options.host ?? Bun.env.RG_FLEET_NODE ?? hostname();
  const workerId = `${host}-${process.pid}-${Math.random().toString(36).slice(2, 7)}`;
  const caps = detectLocalBrowserCapabilities();
  const concurrency = Math.max(
    1,
    options.concurrency ?? Number(Bun.env.RG_FLEET_CONCURRENCY ?? 3),
  );
  const authToken = resolveCoordinatorToken(options.authToken);
  const fetchImpl = options.fetchImpl ?? fetch;
  const maxBatches = options.maxBatches ?? Number.POSITIVE_INFINITY;
  const idleExitMs = options.idleExitMs ?? 8_000;
  const watch = options.watch ?? Bun.env.RG_FLEET_WATCH === "1";
  const baseUrl = options.coordinatorUrl.replace(/\/+$/, "");

  const headers: Record<string, string> = {
    "content-type": "application/json",
    ...(authToken ? { authorization: `Bearer ${authToken}` } : {}),
  };

  const regRes = await fetchImpl(`${baseUrl}/v1/fleet-crawl/register`, {
    method: "POST",
    headers,
    body: JSON.stringify({
      workerId,
      host,
      meshIp: options.meshIp ?? null,
      browsers: caps.engines,
      concurrency,
    }),
  });
  if (!regRes.ok) {
    throw new Error(`Fleet worker register failed: HTTP ${regRes.status}`);
  }

  let processed = 0;
  let succeeded = 0;
  let challenged = 0;
  let failed = 0;

  async function workerLoop(slot: number): Promise<void> {
    let lastWorkAt = Date.now();
    let batches = 0;
    const slotId = `${workerId}#${slot}`;

    while (batches < maxBatches) {
      const claimRes = await fetchImpl(`${baseUrl}/v1/fleet-crawl/claim`, {
        method: "POST",
        headers,
        body: JSON.stringify({
          workerId: slotId,
          limit: 2,
          browsers: caps.engines,
        }),
      });
      if (!claimRes.ok) {
        await Bun.sleep(watch ? 1500 : 500);
        if (!watch && Date.now() - lastWorkAt >= idleExitMs) break;
        continue;
      }
      const { jobs } = (await claimRes.json()) as {
        jobs: Array<{
          id: number;
          url: string;
          depth: number;
          parentUrl?: string | null;
          sourceKind: string;
          enginePreference?: "auto" | CrawlEngineKind;
        }>;
      };

      if (!jobs || jobs.length === 0) {
        if (!watch && Date.now() - lastWorkAt >= idleExitMs) break;
        await Bun.sleep(watch ? 1500 : 450);
        continue;
      }

      lastWorkAt = Date.now();
      batches++;

      for (const job of jobs) {
        const outcome = await crawlUrlWithEngines(job.url, {
          sourceKind: job.sourceKind,
          enginePreference: job.enginePreference ?? "auto",
          allowExternalCli: options.allowExternalCli,
          fetchImpl,
        });

        processed++;
        if (outcome.status === "ok") succeeded++;
        else if (outcome.status === "challenged") challenged++;
        else failed++;

        await fetchImpl(`${baseUrl}/v1/fleet-crawl/submit`, {
          method: "POST",
          headers,
          body: JSON.stringify({
            workerId: slotId,
            workerHost: host,
            jobId: job.id,
            depth: job.depth,
            parentUrl: job.parentUrl ?? null,
            outcome,
          }),
        });
      }
    }
  }

  await Promise.all(
    Array.from({ length: concurrency }, (_, idx) => workerLoop(idx + 1)),
  );

  return {
    workerId,
    host,
    browsers: caps.engines,
    concurrency,
    processed,
    succeeded,
    challenged,
    failed,
  };
}

export async function runFleetCrawl(
  options: {
    argument?: string;
  } = {},
): Promise<Record<string, unknown>> {
  const arg = (options.argument ?? "").trim().toLowerCase();

  if (arg === "coordinator") {
    const coord = await startFleetCoordinator();
    console.info(`[fleet:crawl] coordinator listening on ${coord.url}`);
    await new Promise<void>((resolve) => {
      const stop = () => {
        void coord.stop().then(resolve);
      };
      process.once("SIGINT", stop);
      process.once("SIGTERM", stop);
    });
    return coord.getStatus();
  }

  if (arg === "worker") {
    const coordUrl = Bun.env.RG_FLEET_COORDINATOR_URL?.trim();
    if (!coordUrl) {
      throw new Error("RG_FLEET_COORDINATOR_URL is required in worker mode.");
    }
    const res = await runFleetWorker({
      coordinatorUrl: coordUrl,
      idleExitMs: Number(Bun.env.RG_FLEET_IDLE_EXIT_MS ?? 8000),
    });
    console.info(
      `[fleet:crawl] worker=${res.workerId} host=${res.host} processed=${res.processed} ok=${res.succeeded} challenged=${res.challenged} failed=${res.failed}`,
    );
    return res as unknown as Record<string, unknown>;
  }

  // Standalone multi-worker recursive local swarm when no remote cluster URL is passed.
  const coord = await startFleetCoordinator({
    targetSlug: options.argument || "afuro-terumi",
    port: 0,
    host: "127.0.0.1",
  });
  try {
    const worker = await runFleetWorker({
      coordinatorUrl: coord.url,
      concurrency: Number(Bun.env.RG_FLEET_CONCURRENCY ?? 3),
      idleExitMs: 2500,
    });
    const status = coord.getStatus();
    console.info(
      `[fleet:crawl] swarm done pages=${String(status.pagesStored)} assets=${String(status.assetsStored)} hostEngineCounts=${JSON.stringify(status.hostEngineCounts)}`,
    );
    return { status, worker };
  } finally {
    await coord.stop();
  }
}
