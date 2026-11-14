import { describe, expect, test } from "bun:test";
import { handleAzaleeRequest } from "../src/server/serve";

describe("azalee headless ecosystem endpoints", () => {
  test("GET /api/games et /api/games/:slug servent le catalogue multilingue", async () => {
    const res = await handleAzaleeRequest(
      new Request(
        "http://127.0.0.1:8807/api/games?locale=ja&status=in-development",
      ),
    );
    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      locale: string;
      total: number;
      games: Array<{ slug: string; title: string }>;
    };
    expect(body.locale).toBe("ja");
    expect(body.total).toBe(2);
    expect(body.games.map((g) => g.slug)).toEqual([
      "inazuma-eleven-re",
      "inazuma-eleven-bold-revolution",
    ]);

    const detailRes = await handleAzaleeRequest(
      new Request(
        "http://127.0.0.1:8807/api/games/inazuma-eleven-victory-road?locale=es",
      ),
    );
    expect(detailRes.status).toBe(200);
    const detail = (await detailRes.json()) as {
      locale: string;
      game: { slug: string; communityWikis: unknown[] };
    };
    expect(detail.locale).toBe("es");
    expect(detail.game.slug).toBe("inazuma-eleven-victory-road");
    expect(detail.game.communityWikis.length).toBeGreaterThanOrEqual(4);

    const missing = await handleAzaleeRequest(
      new Request("http://127.0.0.1:8807/api/games/unknown-game-slug"),
    );
    expect(missing.status).toBe(404);
  });

  test("GET /api/mods et /api/community/wikis filtrent par langue et jeu", async () => {
    const modsRes = await handleAzaleeRequest(
      new Request(
        "http://127.0.0.1:8807/api/mods?game=inazuma-eleven-victory-road",
      ),
    );
    expect(modsRes.status).toBe(200);
    const modsBody = (await modsRes.json()) as {
      hubs: Array<{ gameSlug: string }>;
    };
    expect(modsBody.hubs.length).toBeGreaterThanOrEqual(1);
    expect(
      modsBody.hubs.every((h) => h.gameSlug === "inazuma-eleven-victory-road"),
    ).toBe(true);

    const wikisRes = await handleAzaleeRequest(
      new Request("http://127.0.0.1:8807/api/community/wikis?lang=ja"),
    );
    expect(wikisRes.status).toBe(200);
    const wikisBody = (await wikisRes.json()) as {
      sources: Array<{ lang: string }>;
    };
    expect(wikisBody.sources.length).toBeGreaterThanOrEqual(5);
    expect(wikisBody.sources.every((s) => s.lang === "ja")).toBe(true);
  });

  test("GET /api/design/themes et /api/design/tokens exposent les 9 thèmes M3", async () => {
    const themesRes = await handleAzaleeRequest(
      new Request("http://127.0.0.1:8807/api/design/themes"),
    );
    expect(themesRes.status).toBe(200);
    const themesBody = (await themesRes.json()) as {
      defaultTheme: string;
      themes: Array<{ id: string }>;
    };
    expect(themesBody.defaultTheme).toBe("roy");
    expect(themesBody.themes.length).toBe(9);

    const tokensRes = await handleAzaleeRequest(
      new Request("http://127.0.0.1:8807/api/design/tokens"),
    );
    expect(tokensRes.status).toBe(200);
    const tokensBody = (await tokensRes.json()) as {
      contract: { locales: string[] };
    };
    expect(tokensBody.contract.locales).toEqual(["fr", "en", "ja", "es"]);
  });

  test("GET /api/locales/:lang et GET/POST /api/translate résolvent IEVR i18n", async () => {
    const localeRes = await handleAzaleeRequest(
      new Request("http://127.0.0.1:8807/api/locales/fr?scope=interface"),
    );
    expect(localeRes.status).toBe(200);
    const localeBody = (await localeRes.json()) as {
      lang: string;
      scope: string;
      keyCount: number;
      dictionary: Record<string, string>;
    };
    expect(localeBody.lang).toBe("fr");
    expect(localeBody.scope).toBe("interface");
    expect(localeBody.keyCount).toBeGreaterThan(100);
    expect(
      Object.keys(localeBody.dictionary).some((k) =>
        /^0x[0-9A-Fa-f]+$/.test(k),
      ),
    ).toBe(false);

    const missingLocale = await handleAzaleeRequest(
      new Request("http://127.0.0.1:8807/api/locales/xx"),
    );
    expect(missingLocale.status).toBe(404);

    const translateGet = await handleAzaleeRequest(
      new Request(
        "http://127.0.0.1:8807/api/translate?q=%E5%86%86%E5%A0%82%E5%AE%88&to=fr&skipRemote=true",
      ),
    );
    expect(translateGet.status).toBe(200);
    const translateBody = (await translateGet.json()) as {
      translated: string;
      targetLang: string;
    };
    expect(translateBody.targetLang).toBe("fr");
    expect(translateBody.translated.length).toBeGreaterThan(0);

    const translatePost = await handleAzaleeRequest(
      new Request("http://127.0.0.1:8807/api/translate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          texts: ["ゴッドハンド", "ファイアトルネード"],
          to: "fr",
          skipRemote: true,
        }),
      }),
    );
    expect(translatePost.status).toBe(200);
    const batch = (await translatePost.json()) as {
      count: number;
      results: Array<{ translated: string }>;
    };
    expect(batch.count).toBe(2);
    expect(batch.results).toHaveLength(2);
  });
});
