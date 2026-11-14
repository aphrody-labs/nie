import { afterEach, describe, expect, it } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  detectAntiBotChallenge,
  resolveMediaWikiTarget,
} from "../../lib/crawl-engines";
import { CrawleeFrontierQueue } from "../../lib/crawlee-queue";
import { detectBinaryMimeType, syncAssetToR2 } from "../../lib/r2-assets";
import { runCharacterCrawl } from "./character-watch";

const tempDirs: string[] = [];

function createTempDir(): string {
  const dir = mkdtempSync(join(tmpdir(), "rg-char-crawl-"));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const dir = tempDirs.pop()!;
    rmSync(dir, { recursive: true, force: true });
  }
});

describe("character:crawl building blocks", () => {
  it("resolves EN/FR Fandom wiki URLs to their MediaWiki API endpoints", () => {
    const en = resolveMediaWikiTarget(
      "https://inazuma-eleven.fandom.com/wiki/Afuro_Terumi",
    );
    expect(en).toEqual({
      apiEndpoint: "https://inazuma-eleven.fandom.com/api.php",
      pageTitle: "Afuro Terumi",
      canonicalWikiUrl: "https://inazuma-eleven.fandom.com/wiki/Afuro_Terumi",
      language: "en",
    });

    const fr = resolveMediaWikiTarget(
      "https://manga-encyclopedie.fandom.com/fr/wiki/Afuro_Terumi_(Inazuma_Eleven)",
    );
    expect(fr?.apiEndpoint).toBe(
      "https://manga-encyclopedie.fandom.com/fr/api.php",
    );
    expect(fr?.language).toBe("fr");
  });

  it("detects anti-bot challenges without bypassing them", () => {
    expect(
      detectAntiBotChallenge(
        403,
        "<html><title>Just a moment...</title><div>cloudflare</div></html>",
      ),
    ).toEqual({ challenged: true, provider: "cloudflare" });

    expect(
      detectAntiBotChallenge(200, "<html><body>Byron Love</body></html>"),
    ).toEqual({ challenged: false, provider: null });
  });

  it("enqueues, claims, deduplicates, and updates status in SQLite Crawlee frontier", () => {
    const dir = createTempDir();
    const queue = new CrawleeFrontierQueue(join(dir, "frontier.sqlite"));

    expect(
      queue.enqueue({
        targetSlug: "afuro-terumi",
        url: "https://example.org/page?b=2&a=1#frag",
      }),
    ).toBe(true);
    // Duplicate normalized query order / fragment must be ignored.
    expect(
      queue.enqueue({
        targetSlug: "afuro-terumi",
        url: "https://example.org/page?a=1&b=2",
      }),
    ).toBe(false);

    const claimed = queue.claimNext("afuro-terumi", 0);
    expect(claimed?.normalized_url).toBe("https://example.org/page?a=1&b=2");
    queue.markDone(claimed!.id, "deadbeef");

    expect(queue.stats("afuro-terumi").done).toBe(1);
    queue.close();
  });

  it("sniffs binary asset headers and stages SHA-256 artifacts cleanly for R2", async () => {
    const dir = createTempDir();
    const fakeWebp = new Uint8Array([
      0x52, 0x49, 0x46, 0x46, 0x10, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50,
    ]);
    expect(detectBinaryMimeType(fakeWebp)).toEqual({
      mime: "image/webp",
      ext: "webp",
    });

    const res = await syncAssetToR2({
      targetSlug: "afuro-terumi",
      sourceUrl:
        "https://dxi4wb638ujep.cloudfront.net/1/k/d/y/dykb3jbxeis.webp",
      data: fakeWebp,
      stagingDir: join(dir, "r2"),
    });
    expect(res.storageMode).toBe("local-staging");
    expect(res.contentType).toBe("image/webp");
    expect(res.r2Key).toMatch(/^characters\/afuro-terumi\/[0-9a-f]{16}\.webp$/);
  });
});

describe("runCharacterCrawl end-to-end", () => {
  it("runs a multi-cycle watch loop with MediaWiki API, Zukan, search discovery, SQLite, and R2 staging", async () => {
    const dir = createTempDir();
    const sqlitePath = join(dir, "crawl.sqlite");
    const r2StagingDir = join(dir, "r2");

    const pngHeader = new Uint8Array([
      0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x01,
    ]);

    const fakeFetch: typeof fetch = (async (input: string | URL | Request) => {
      const url =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.href
            : input.url;

      if (url.includes("/api.php") && url.includes("action=query")) {
        return new Response(
          JSON.stringify({
            query: {
              pages: {
                "123": {
                  pageid: 123,
                  title: "Afuro Terumi",
                  fullurl:
                    "https://inazuma-eleven.fandom.com/wiki/Afuro_Terumi",
                  extract:
                    "Afuro Terumi, also known as Byron Love, captain of Zeus.",
                  thumbnail: {
                    source:
                      "https://static.wikia.nocookie.net/inazuma-eleven/images/afuro.png",
                  },
                },
              },
            },
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }

      if (url.includes("news.google.com/rss")) {
        return new Response(
          `<rss><channel><item><title>Byron Love news</title><link>https://example.org/byron-news</link></item></channel></rss>`,
          { status: 200, headers: { "content-type": "application/xml" } },
        );
      }

      if (url.includes("zukan.inazuma.jp")) {
        return new Response(
          `<html><body><a href="/en/chara_param/?q=hN2dn56NnpyLmo2glpvdxaTdfJ-fjJCbhZ-enpvdxaKgg=="><img src="https://dxi4wb638ujep.cloudfront.net/1/k/d/y/dykb3jbxeis.webp"/><p class="name">Byron Love</p></a></body></html>`,
          { status: 200, headers: { "content-type": "text/html" } },
        );
      }

      if (url.endsWith(".png") || url.endsWith(".webp")) {
        return new Response(pngHeader, {
          status: 200,
          headers: { "content-type": "image/png" },
        });
      }

      if (url.includes("baike.baidu.com") || url.includes("example.org")) {
        return new Response(
          `<html lang="fr"><head><title>Byron Love Profile</title></head><body><p>Aphrodi profile content</p></body></html>`,
          { status: 200, headers: { "content-type": "text/html" } },
        );
      }

      return new Response("Just a moment... cloudflare", { status: 403 });
    }) as typeof fetch;

    const report = await runCharacterCrawl({
      argument: "afuro-terumi",
      watch: true,
      maxCycles: 2,
      intervalMs: 10,
      maxPagesPerCycle: 10,
      maxAssetsPerCycle: 3,
      sqlitePath,
      r2StagingDir,
      fetchImpl: fakeFetch,
    });

    expect(report.slug).toBe("afuro-terumi");
    expect(report.cyclesCompleted).toBe(2);
    expect(report.zukanCatalogMatches).toBeGreaterThanOrEqual(3);
    expect(report.zukanLiveEntries).toBeGreaterThanOrEqual(1);
    expect(report.pagesSucceeded).toBeGreaterThanOrEqual(1);
    expect(report.assetsMirrored).toBeGreaterThanOrEqual(1);
    expect(report.enginesUsed).toContain("zukan-inagle");
  });
});
