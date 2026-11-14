import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { CrawlOramaIndex } from "../../lib/orama-index";
import {
  GAMEBANANA_INAZUMA_GAMES,
  normalizeGameBananaSubfeed,
  normalizeInazumaModsPayload,
  runCommunityModsCrawl,
} from "./community-mods";

const tempDirs: string[] = [];

afterEach(() => {
  while (tempDirs.length > 0) {
    rmSync(tempDirs.pop()!, { recursive: true, force: true });
  }
});

describe("community:mods normalizers and crawler", () => {
  test("includes all 14 verified GameBanana Inazuma Eleven game IDs", () => {
    expect(GAMEBANANA_INAZUMA_GAMES).toHaveLength(14);
    const ids = GAMEBANANA_INAZUMA_GAMES.map((g) => g.id);
    expect(ids).toContain(20069);
    expect(ids).toContain(20068);
    expect(ids).toContain(19155);
    expect(ids).toContain(24334);
    expect(ids).toContain(19165);
  });

  test("normalizes GameBanana Subfeed and inazuma-eleven.fr/mods payloads", () => {
    const gbMods = normalizeGameBananaSubfeed(GAMEBANANA_INAZUMA_GAMES[0]!, {
      _aRecords: [
        {
          _idRow: 551234,
          _sModelName: "Mod",
          _sName: "Byron Love Classic Kit",
          _sProfileUrl: "https://gamebanana.com/mods/551234",
          _nDownloadCount: 340,
          _nLikeCount: 42,
          _tsDateModified: 1730000000,
          _aSubmitter: { _sName: "ZeusModder" },
        },
      ],
    });
    expect(gbMods).toHaveLength(1);
    expect(gbMods[0]?.gameId).toBe(20069);
    expect(gbMods[0]?.author).toBe("ZeusModder");

    const frMods = normalizeInazumaModsPayload([
      {
        slug: "hd-textures-raimon",
        title: "HD Textures Raimon",
        author: "Azalee",
        category: "Textures",
        downloads: 120,
      },
    ]);
    expect(frMods).toHaveLength(1);
    expect(frMods[0]?.url).toBe(
      "https://inazuma-eleven.fr/mods/hd-textures-raimon",
    );
  });

  test("indexes community mods, Fandom EN/FR/ES, JA wikis and TraduSquare ES into Orama & snapshot", async () => {
    const dir = mkdtempSync(join(tmpdir(), "rg-community-mods-"));
    tempDirs.push(dir);
    const oramaPath = join(dir, "mods-orama.json");

    const res = await runCommunityModsCrawl({
      sqlitePath: join(dir, "mods.sqlite"),
      oramaPath,
      outputDir: dir,
      maxGameBananaGames: 2,
      mirrorAssets: false,
      fetchText: async (url: string) => {
        if (url.includes("inazuma-eleven.fr/mods/api/mods")) {
          return {
            ok: true,
            status: 200,
            text: JSON.stringify([
              {
                slug: "byron-custom-aura",
                title: "Byron Custom Aura",
                author: "RoseGriffon",
                category: "Mods",
              },
            ]),
          };
        }
        if (url.includes("gamebanana.com/apiv11/Game/20069")) {
          return {
            ok: true,
            status: 200,
            text: JSON.stringify({
              _aRecords: [
                {
                  _idRow: 9001,
                  _sModelName: "Skin",
                  _sName: "Afuro Terumi Victory Uniform",
                  _sProfileUrl: "https://gamebanana.com/mods/9001",
                },
              ],
            }),
          };
        }
        if (url.includes("action=opensearch")) {
          return {
            ok: true,
            status: 200,
            text: JSON.stringify([
              "Inazuma Eleven",
              ["Inazuma Eleven: Victory Road"],
              ["Official wiki article"],
              [
                url.includes("inazuma.fandom.com/es")
                  ? "https://inazuma.fandom.com/es/wiki/Inazuma_Eleven"
                  : "https://inazuma-eleven.fandom.com/wiki/Inazuma_Eleven",
              ],
            ]),
          };
        }
        return {
          ok: true,
          status: 200,
          text: JSON.stringify({ _aRecords: [] }),
        };
      },
      crawleeBatch: async (urls) =>
        urls.map((u) => ({
          url: u,
          finalUrl: u,
          status: 200,
          title: u.includes("tradusquare")
            ? "TraduSquare Inazuma Eleven GO Galaxy ES"
            : `Guide ${u}`,
          language: u.includes("tradusquare") ? "es" : "ja",
          text: u.includes("tradusquare")
            ? "Traducción al español de Inazuma Eleven GO Galaxy completa"
            : "イナズマイレブン攻略まとめ Wiki",
          links: [],
          assets: [],
          html: "<html></html>",
          engine: "crawlee-cheerio",
        })),
    });

    expect(res.success).toBe(true);
    expect(res.modsCount).toBe(2);
    expect(res.wikisCount).toBeGreaterThanOrEqual(10);
    expect(existsSync(res.snapshotPath)).toBe(true);
    expect(existsSync(oramaPath)).toBe(true);

    const snapshot = JSON.parse(readFileSync(res.snapshotPath, "utf8"));
    expect(snapshot.modsCount).toBe(2);

    const reopened = new CrawlOramaIndex(oramaPath);
    const hits = reopened.searchDocuments("Afuro Terumi");
    expect(hits.length).toBeGreaterThanOrEqual(1);
    const esHits = reopened.searchDocuments("Traducción", { language: "es" });
    expect(esHits.length).toBeGreaterThanOrEqual(1);
  });
});
