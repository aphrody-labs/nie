// SPDX-License-Identifier: Apache-2.0
import { describe, expect, test } from "bun:test";
import {
  ROSE_GRIFFON_ORIGINS,
  ROSE_GRIFFON_ROUTES,
  ROSE_GRIFFON_URLS,
  ROSE_GRIFFON_AUTH_COOKIES,
  type FetchLike,
  roseGriffonUrl,
  createRoseGriffonAuthCookies,
  roseGriffonAuthHeaders,
  injectRoseGriffonAuth,
  checkRoseGriffonApiHealth,
} from "./rosegriffon-site.ts";

const mockFetch: FetchLike = async () => {
  return new Response(JSON.stringify({ status: "healthy", version: "1.0.0" }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
};

const mockFailingFetch: FetchLike = async () => {
  throw new Error("Connection refused");
};

describe("Rose Griffon site helpers", () => {
  test("URL presets and roseGriffonUrl normalizer enforce canonical SEO shape", () => {
    expect(ROSE_GRIFFON_ORIGINS.site).toBe("https://rosegriffon.fr");
    expect(ROSE_GRIFFON_ROUTES.news).toBe("/news");
    expect(ROSE_GRIFFON_URLS.news).toBe("https://rosegriffon.fr/news");

    // Strip trailing slashes on sub-paths
    expect(roseGriffonUrl("/news/")).toBe("https://rosegriffon.fr/news");
    expect(roseGriffonUrl("a-propos")).toBe("https://rosegriffon.fr/a-propos");
    expect(roseGriffonUrl("/")).toBe("https://rosegriffon.fr/");
    expect(roseGriffonUrl("/api/v1/status", ROSE_GRIFFON_ORIGINS.api)).toBe(
      "https://api.rosegriffon.fr/api/v1/status",
    );
  });

  test("createRoseGriffonAuthCookies creates valid cookie parameters", () => {
    const cookies = createRoseGriffonAuthCookies({
      session: "custom-sess-123",
      csrf: "custom-csrf-456",
      token: "custom-jwt-789",
    });

    expect(cookies).toHaveLength(3);
    expect(
      cookies.find((c) => c.name === ROSE_GRIFFON_AUTH_COOKIES.session)?.value,
    ).toBe("custom-sess-123");
    expect(
      cookies.find((c) => c.name === ROSE_GRIFFON_AUTH_COOKIES.csrf)?.value,
    ).toBe("custom-csrf-456");
    expect(
      cookies.find((c) => c.name === ROSE_GRIFFON_AUTH_COOKIES.token)?.value,
    ).toBe("custom-jwt-789");
    expect(cookies.every((c) => c.domain === "rosegriffon.fr")).toBe(true);
  });

  test("roseGriffonAuthHeaders creates bearer and csrf headers", () => {
    const headers = roseGriffonAuthHeaders("tok-abc", "csrf-xyz");
    expect(headers.Authorization).toBe("Bearer tok-abc");
    expect(headers["x-csrf-token"]).toBe("csrf-xyz");
    expect(headers.Cookie).toContain("rg_session=tok-abc");
    expect(headers.Cookie).toContain("rg_csrf=csrf-xyz");
  });

  test("injectRoseGriffonAuth injects cookies through a CDP session", async () => {
    const sent: Array<{ method: string; params: unknown }> = [];
    await injectRoseGriffonAuth(
      {
        _cdp: {
          _send: async (method: string, params: unknown) => {
            sent.push({ method, params });
            return {};
          },
        },
      },
      { session: "injected-session", csrf: "injected-csrf" },
    );

    expect(sent).toHaveLength(1);
    expect(sent[0]?.method).toBe("Network.setCookies");
    const { cookies } = sent[0]!.params as {
      cookies: Array<{ name: string; value: string }>;
    };
    expect(cookies.find((k) => k.name === "rg_session")?.value).toBe(
      "injected-session",
    );
    expect(cookies.find((k) => k.name === "rg_csrf")?.value).toBe(
      "injected-csrf",
    );
  });

  test("checkRoseGriffonApiHealth reports latency and status", async () => {
    const health = await checkRoseGriffonApiHealth({
      fetch: mockFetch,
      apiOrigin: "https://api.rosegriffon.fr",
      endpoint: "/health",
    });

    expect(health.ok).toBe(true);
    expect(health.status).toBe(200);
    expect(health.latencyMs).toBeGreaterThanOrEqual(0);
    expect(health.data).toEqual({ status: "healthy", version: "1.0.0" });
  });

  test("checkRoseGriffonApiHealth handles network errors gracefully", async () => {
    const health = await checkRoseGriffonApiHealth({
      fetch: mockFailingFetch,
      apiOrigin: "https://api.rosegriffon.fr",
    });

    expect(health.ok).toBe(false);
    expect(health.status).toBe(0);
    expect(health.error).toContain("Connection refused");
  });
});
