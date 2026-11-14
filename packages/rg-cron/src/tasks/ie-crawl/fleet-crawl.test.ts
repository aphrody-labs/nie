import { afterEach, describe, expect, it } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { filterRecursiveChildLinks } from "../../lib/crawl-engines";
import { runFleetWorker, startFleetCoordinator } from "./fleet-crawl";

const tempDirs: string[] = [];

function createTempDir(): string {
  const dir = mkdtempSync(join(tmpdir(), "rg-fleet-crawl-"));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const dir = tempDirs.pop()!;
    rmSync(dir, { recursive: true, force: true });
  }
});

describe("filterRecursiveChildLinks", () => {
  it("filters relevant public child links within maxDepth and drops static/edit noise", () => {
    const links = filterRecursiveChildLinks(
      [
        "https://inazuma-eleven.fandom.com/wiki/Zeus_Jr._High",
        "https://inazuma-eleven.fandom.com/wiki/Special:RecentChanges",
        "https://inazuma-eleven.fandom.com/wiki/Afuro_Terumi?action=edit",
        "https://cdn.example.org/app.css",
        "http://127.0.0.1:8080/internal",
        "https://zukan.inazuma.jp/en/chara_param/?q=abc",
      ],
      {
        parentUrl: "https://inazuma-eleven.fandom.com/wiki/Afuro_Terumi",
        depth: 0,
        maxDepth: 2,
        aliases: ["Afuro Terumi", "Byron Love", "Aphrodi"],
      },
    );

    expect(links).toEqual([
      "https://inazuma-eleven.fandom.com/wiki/Zeus_Jr._High",
      "https://zukan.inazuma.jp/en/chara_param/?q=abc",
    ]);

    expect(
      filterRecursiveChildLinks(links, {
        parentUrl: "https://example.org",
        depth: 2,
        maxDepth: 2,
        aliases: ["Byron Love"],
      }),
    ).toEqual([]);
  });
});

describe("multi-VPS recursive fleet coordinator + multi-worker swarm", () => {
  it("enforces bearer auth and executes recursive depth 0 -> depth 1 crawl across 3 simulated VPS nodes", async () => {
    const dir = createTempDir();
    const sqlitePath = join(dir, "fleet.sqlite");
    const r2StagingDir = join(dir, "r2");
    const authToken = "test-fleet-secret";

    const pngBytes = new Uint8Array([
      0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x01,
    ]);

    const realFetch = globalThis.fetch;
    const mockExternalFetch: typeof fetch = (async (
      input: string | URL | Request,
      init?: RequestInit,
    ) => {
      const url =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.href
            : input.url;

      // Forward coordinator HTTP API calls to live Bun.serve coordinator!
      if (url.startsWith("http://127.0.0.1:")) {
        return realFetch(input, init);
      }

      if (url.includes("/api.php") && url.includes("action=query")) {
        return new Response(
          JSON.stringify({
            query: {
              pages: {
                "100": {
                  pageid: 100,
                  title: "Afuro Terumi",
                  fullurl:
                    "https://inazuma-eleven.fandom.com/wiki/Afuro_Terumi",
                  extract: "Afuro Terumi depth-0 page",
                  thumbnail: {
                    source:
                      "https://static.wikia.nocookie.net/inazuma-eleven/images/byron.png",
                  },
                },
              },
            },
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }

      if (url.includes("baike.baidu.com")) {
        return new Response(
          `<html><head><title>Afuro Terumi Baidu</title></head><body><a href="https://baike.baidu.com/item/Zeus_Inazuma_Eleven">Zeus</a><p>Baidu body</p></body></html>`,
          { status: 200, headers: { "content-type": "text/html" } },
        );
      }

      if (url.includes("Zeus_Inazuma_Eleven")) {
        return new Response(
          `<html><head><title>Zeus Junior High</title></head><body><p>Recursive depth-1 page</p></body></html>`,
          { status: 200, headers: { "content-type": "text/html" } },
        );
      }

      if (url.includes("zukan.inazuma.jp")) {
        return new Response(
          `<html><body><a href="/en/chara_param/?q=hN2dn56NnpyLmo2glpvdxaTdfJ-fjJCbhZ-enpvdxaKgg=="><img src="https://dxi4wb638ujep.cloudfront.net/1/k/d/y/dykb3jbxeis.webp"/><p class="name">Byron Love</p></a></body></html>`,
          { status: 200, headers: { "content-type": "text/html" } },
        );
      }

      if (url.endsWith(".png") || url.endsWith(".webp")) {
        return new Response(pngBytes, {
          status: 200,
          headers: { "content-type": "image/png" },
        });
      }

      return new Response(
        "<html><title>Fallback</title><body>Inazuma Eleven</body></html>",
        {
          status: 200,
          headers: { "content-type": "text/html" },
        },
      );
    }) as typeof fetch;

    const coord = await startFleetCoordinator({
      targetSlug: "afuro-terumi",
      host: "127.0.0.1",
      port: 0,
      maxDepth: 2,
      maxPages: 30,
      authToken,
      sqlitePath,
      r2StagingDir,
      skipDiscovery: true,
      fetchImpl: mockExternalFetch,
    });

    try {
      const unauthRes = await realFetch(`${coord.url}/v1/fleet-crawl/status`);
      expect(unauthRes.status).toBe(401);

      const [wVps, wCdn, wPrd] = await Promise.all([
        runFleetWorker({
          coordinatorUrl: coord.url,
          host: "vps",
          concurrency: 2,
          authToken,
          idleExitMs: 500,
          allowExternalCli: false,
          fetchImpl: mockExternalFetch,
        }),
        runFleetWorker({
          coordinatorUrl: coord.url,
          host: "cdn",
          concurrency: 2,
          authToken,
          idleExitMs: 500,
          allowExternalCli: false,
          fetchImpl: mockExternalFetch,
        }),
        runFleetWorker({
          coordinatorUrl: coord.url,
          host: "prd",
          concurrency: 2,
          authToken,
          idleExitMs: 500,
          allowExternalCli: false,
          fetchImpl: mockExternalFetch,
        }),
      ]);

      const totalProcessed = wVps.processed + wCdn.processed + wPrd.processed;
      expect(totalProcessed).toBeGreaterThanOrEqual(5);

      const status = coord.getStatus() as {
        pagesStored: number;
        assetsStored: number;
        depthHistogram: Record<string, number>;
        workers: Array<{ host: string }>;
      };
      expect(status.pagesStored).toBeGreaterThanOrEqual(5);
      expect(status.assetsStored).toBeGreaterThanOrEqual(1);
      expect(status.workers).toHaveLength(3);
      expect(status.depthHistogram.depth_0).toBeGreaterThanOrEqual(1);
      expect(status.depthHistogram.depth_1).toBeGreaterThanOrEqual(1);
    } finally {
      await coord.stop();
    }
  });
});
