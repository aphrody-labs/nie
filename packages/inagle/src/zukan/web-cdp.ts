// SPDX-License-Identifier: MIT
/**
 * Headless browser page used by the zukan crawlers (`zukan/order.ts`,
 * `zukan/scripts/scrape-skills-pages.ts`).
 *
 * The default transport is patchright (optional dependency). Hosts may also inject
 * another transport with `setWebCdpPageFactory` (rg-cron plugs in Bun.WebView). Without a transport,
 * `connect()` fails explicitly instead of returning an empty page, so a crawl
 * never reports success on content it did not load.
 */

export interface NavigationResponse {
  ok?: boolean;
  status?: number;
  [key: string]: unknown;
}

export type WaitUntil = "load" | "domcontentloaded" | "commit" | "networkidle";

/** Minimal page contract implemented by every transport. */
export interface CdpPageLike {
  goto(url: string, waitUntil?: WaitUntil): Promise<NavigationResponse>;
  evaluate<T>(script: string): Promise<T>;
  content(): Promise<string>;
  close(): Promise<void>;
}

export type CdpPageFactory = (endpoint?: string) => Promise<CdpPageLike>;

let pageFactory: CdpPageFactory | null = null;

/** Replace the CDP transport (tests, other hosts); `null` restores the default. */
export function setWebCdpPageFactory(factory: CdpPageFactory | null): void {
  pageFactory = factory;
}

const runtimeImport = new Function("specifier", "return import(specifier)") as <
  T,
>(
  specifier: string,
) => Promise<T>;

async function createPatchrightPage(endpoint?: string): Promise<CdpPageLike> {
  const { chromium } =
    await runtimeImport<typeof import("patchright")>("patchright");
  const { existsSync } = await import("node:fs");
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
        ];
  const executablePath = candidates.find((p) => existsSync(p));
  const browser = endpoint
    ? await chromium.connectOverCDP(endpoint)
    : await chromium.launch({
        headless: true,
        ...(executablePath ? { executablePath } : {}),
        args: ["--no-sandbox", "--disable-dev-shm-usage"],
      });
  const context = await browser.newContext();
  const page = await context.newPage();
  return {
    async goto(url, waitUntil) {
      const response = await page.goto(url, {
        waitUntil: waitUntil ?? "domcontentloaded",
        timeout: 35_000,
      });
      const status = response?.status() ?? 200;
      return { status, ok: status >= 200 && status < 400 };
    },
    async evaluate<T>(script: string) {
      return (await page.evaluate((code) => {
        // biome-ignore lint/security/noGlobalEval: matches CDP Runtime.evaluate contract
        return (0, eval)(code);
      }, script)) as T;
    },
    content: () => page.content(),
    async close() {
      await context.close().catch(() => undefined);
      await browser.close().catch(() => undefined);
    },
  };
}

function defaultFactory(): CdpPageFactory {
  return (endpoint) => createPatchrightPage(endpoint);
}

export class WebCdpPage implements CdpPageLike {
  readonly #page: CdpPageLike;

  private constructor(page: CdpPageLike) {
    this.#page = page;
  }

  /** Connect to the configured CDP endpoint (optional). */
  static async connect(endpoint?: string): Promise<WebCdpPage> {
    const factory = pageFactory ?? defaultFactory();
    return new WebCdpPage(await factory(endpoint));
  }

  goto(url: string, waitUntil?: WaitUntil): Promise<NavigationResponse> {
    return this.#page.goto(url, waitUntil);
  }

  evaluate<T = unknown>(script: string): Promise<T> {
    return this.#page.evaluate<T>(script);
  }

  content(): Promise<string> {
    return this.#page.content();
  }

  close(): Promise<void> {
    return this.#page.close();
  }
}

export async function waitForSelector(
  page: WebCdpPage,
  selector: string,
  timeoutMs = 15_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  const expression = `Boolean(document.querySelector(${JSON.stringify(selector)}))`;
  while (Date.now() < deadline) {
    if (await page.evaluate<boolean>(expression)) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(
    `Selector ${JSON.stringify(selector)} was not found within ${timeoutMs} ms`,
  );
}
