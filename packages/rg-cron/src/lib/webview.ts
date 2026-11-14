/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * 2026 headless stealth browser transport backed by `patchright` (`1.64.3`),
 * avoiding CDP `Runtime.enable` leaks and managing a clean singleton Browser
 * with refcounted cleanup. Preserves compatibility with `PageWebView`
 * (`WebCdpPage`), `ouvrirVue`, `fermerVue`, `naviguer`, `chargerDocument`, and
 * `serviceWebNatif` across cron tasks and `@rosegriffon/inagle`.
 */

import { existsSync } from "node:fs";
import type { Browser, BrowserContext, CDPSession, Page } from "patchright";

export interface NavigationResponse {
  status: number;
  ok: boolean;
  url?: string;
  [key: string]: unknown;
}

export interface DocumentCharge extends NavigationResponse {
  html: string;
  title: string;
}

export interface WebViewCookie {
  name: string;
  value: string;
  domain?: string;
  path?: string;
  url?: string;
  expires?: number;
  httpOnly?: boolean;
  secure?: boolean;
  sameSite?: "Strict" | "Lax" | "None";
}

type PatchrightModule = typeof import("patchright");

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

let browserPromise: Promise<Browser> | null = null;
let activeViews = 0;

async function getSharedPatchrightBrowser(): Promise<Browser> {
  if (!browserPromise) {
    browserPromise = (async () => {
      const mod = await runtimeDynamicImport<PatchrightModule>("patchright");
      const executablePath = detectBrowserExecutable();
      const browser = await mod.chromium.launch({
        headless: true,
        ...(executablePath ? { executablePath } : {}),
        args: [
          "--no-sandbox",
          "--disable-dev-shm-usage",
          "--disable-blink-features=AutomationControlled",
        ],
      });
      browser.on("disconnected", () => {
        browserPromise = null;
      });
      return browser;
    })();
  }
  return browserPromise;
}

export async function closeSharedStealthBrowser(): Promise<void> {
  const current = browserPromise;
  browserPromise = null;
  activeViews = 0;
  if (current) {
    const browser = await current.catch(() => null);
    await browser?.close().catch(() => undefined);
  }
}

export interface StealthVueHandle {
  navigate(url: string): Promise<NavigationResponse>;
  evaluate<T = unknown>(expression: string): Promise<T>;
  content(): Promise<string>;
  getTitle(): Promise<string>;
  cdp<T = unknown>(
    method: string,
    params?: Record<string, unknown>,
  ): Promise<T>;
  addEventListener(
    event: string,
    listener: (event: MessageEvent<Record<string, unknown>>) => void,
  ): void;
  resize(width: number, height: number): Promise<void>;
  screenshot(options?: {
    format?: "png" | "jpeg";
    quality?: number;
    encoding?: "buffer";
  }): Promise<Uint8Array>;
  addCookies(cookies: Array<Record<string, unknown>>): Promise<void>;
  get url(): string;
  close(): Promise<void>;
  /** @internal */
  __ready(): Promise<{
    context: BrowserContext;
    page: Page;
    session: CDPSession | null;
  }>;
}

class PatchrightVueHandle implements StealthVueHandle {
  private readonly readyPromise: Promise<{
    context: BrowserContext;
    page: Page;
    session: CDPSession | null;
  }>;
  private context: BrowserContext | null = null;
  private page: Page | null = null;
  private session: CDPSession | null = null;
  private currentUrl = "about:blank";
  private closed = false;
  private readonly listeners = new Map<
    string,
    Set<(event: MessageEvent<Record<string, unknown>>) => void>
  >();

  constructor(options?: { largeur?: number; hauteur?: number }) {
    activeViews++;
    const width = options?.largeur ?? 1440;
    const height = options?.hauteur ?? 900;
    this.readyPromise = (async () => {
      const browser = await getSharedPatchrightBrowser();
      const context = await browser.newContext({
        viewport: { width, height },
        locale: "fr-FR",
      });
      const page = await context.newPage();
      this.context = context;
      this.page = page;
      return { context, page, session: null };
    })();
  }

  __ready() {
    return this.readyPromise;
  }

  private async ensurePage(): Promise<Page> {
    if (this.closed) throw new Error("Patchright view is closed");
    const { page } = await this.readyPromise;
    return page;
  }

  private async ensureSession(): Promise<CDPSession> {
    if (this.session) return this.session;
    const { context, page } = await this.readyPromise;
    const cdpSession = await context.newCDPSession(page);
    this.session = cdpSession;
    for (const [eventName, set] of this.listeners.entries()) {
      this.attachSessionListener(cdpSession, eventName, set);
    }
    return cdpSession;
  }

  private attachSessionListener(
    cdpSession: CDPSession,
    eventName: string,
    callbacks: Set<(event: MessageEvent<Record<string, unknown>>) => void>,
  ): void {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    cdpSession.on(eventName as any, (payload: Record<string, unknown>) => {
      const wrapped = { data: payload } as MessageEvent<
        Record<string, unknown>
      >;
      for (const fn of callbacks) fn(wrapped);
    });
  }

  get url(): string {
    return this.page?.url() ?? this.currentUrl;
  }

  async navigate(url: string): Promise<NavigationResponse> {
    const page = await this.ensurePage();
    if (url === "about:blank") {
      await page.goto("about:blank", { waitUntil: "domcontentloaded" });
      this.currentUrl = "about:blank";
      return { status: 200, ok: true, url: "about:blank" };
    }
    const response = await page.goto(url, {
      waitUntil: "domcontentloaded",
      timeout: 35_000,
    });
    this.currentUrl = page.url() || url;
    const status = response?.status() ?? 200;
    return {
      status,
      ok: status >= 200 && status < 400,
      url: this.currentUrl,
    };
  }

  async evaluate<T = unknown>(expression: string): Promise<T> {
    const page = await this.ensurePage();
    return (await page.evaluate(expression)) as T;
  }

  async content(): Promise<string> {
    const page = await this.ensurePage();
    return page.content();
  }

  async getTitle(): Promise<string> {
    const page = await this.ensurePage();
    return page.title();
  }

  async cdp<T = unknown>(
    method: string,
    params?: Record<string, unknown>,
  ): Promise<T> {
    const cdpSession = await this.ensureSession();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return (await cdpSession.send(method as any, params as any)) as T;
  }

  addEventListener(
    event: string,
    listener: (event: MessageEvent<Record<string, unknown>>) => void,
  ): void {
    let set = this.listeners.get(event);
    if (!set) {
      set = new Set();
      this.listeners.set(event, set);
      if (this.session) {
        this.attachSessionListener(this.session, event, set);
      }
    }
    set.add(listener);
  }

  async resize(width: number, height: number): Promise<void> {
    const page = await this.ensurePage();
    await page.setViewportSize({ width, height });
  }

  async screenshot(options?: {
    format?: "png" | "jpeg";
    quality?: number;
    encoding?: "buffer";
  }): Promise<Uint8Array> {
    const page = await this.ensurePage();
    const type = options?.format === "jpeg" ? "jpeg" : "png";
    const buf = await page.screenshot({
      type,
      ...(type === "jpeg" && options?.quality
        ? { quality: options.quality }
        : {}),
    });
    return new Uint8Array(buf);
  }

  async addCookies(cookies: Array<Record<string, unknown>>): Promise<void> {
    const { context } = await this.readyPromise;
    const normalized = cookies.map((cookie) => ({
      name: String(cookie.name ?? ""),
      value: String(cookie.value ?? ""),
      ...(cookie.url ? { url: String(cookie.url) } : {}),
      ...(cookie.domain ? { domain: String(cookie.domain) } : {}),
      ...(cookie.path ? { path: String(cookie.path) } : { path: "/" }),
      ...(typeof cookie.expires === "number"
        ? { expires: cookie.expires }
        : {}),
      ...(typeof cookie.httpOnly === "boolean"
        ? { httpOnly: cookie.httpOnly }
        : {}),
      ...(typeof cookie.secure === "boolean" ? { secure: cookie.secure } : {}),
    }));
    await context.addCookies(normalized);
  }

  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    activeViews = Math.max(0, activeViews - 1);
    await this.readyPromise.catch(() => null);
    await this.session?.detach().catch(() => undefined);
    await this.context?.close().catch(() => undefined);
    this.session = null;
    this.page = null;
    this.context = null;
  }
}

export function ouvrirVue(options?: {
  largeur?: number;
  hauteur?: number;
}): StealthVueHandle {
  return new PatchrightVueHandle(options);
}

export function fermerVue(vue: StealthVueHandle): void {
  void vue.close();
}

export async function naviguer(
  vue: StealthVueHandle,
  url: string,
  delaiMs = 35_000,
): Promise<NavigationResponse> {
  const timer = new Promise<never>((_, reject) =>
    setTimeout(
      () => reject(new Error(`Timeout navigating to ${url}`)),
      delaiMs,
    ),
  );
  return Promise.race([vue.navigate(url), timer]);
}

export async function chargerDocument(
  url: string,
  delaiMs = 35_000,
): Promise<DocumentCharge> {
  const vue = ouvrirVue();
  try {
    const reponse = await naviguer(vue, url, delaiMs);
    const [html, title] = await Promise.all([vue.content(), vue.getTitle()]);
    return { ...reponse, html, title };
  } finally {
    await vue.close();
  }
}

export class PageWebView {
  readonly #vue: StealthVueHandle;

  private constructor(vue: StealthVueHandle) {
    this.#vue = vue;
  }

  static async connect(_endpoint?: string): Promise<PageWebView> {
    const vue = ouvrirVue();
    await vue.__ready();
    return new PageWebView(vue);
  }

  get url(): string {
    return this.#vue.url;
  }

  async goto(
    url: string,
    _waitUntil?: "domcontentloaded" | "load" | "networkidle",
  ): Promise<NavigationResponse> {
    return naviguer(this.#vue, url);
  }

  async evaluate<T = unknown>(expression: string): Promise<T> {
    return this.#vue.evaluate<T>(expression);
  }

  async content(): Promise<string> {
    return this.#vue.content();
  }

  async title(): Promise<string> {
    return this.#vue.getTitle();
  }

  async addCookies(cookies: Array<Record<string, unknown>>): Promise<void> {
    await this.#vue.addCookies(cookies);
  }

  async resize(width: number, height: number): Promise<void> {
    await this.#vue.resize(width, height);
  }

  async screenshot(options?: {
    format?: "png" | "jpeg";
    quality?: number;
    encoding?: "buffer";
  }): Promise<Uint8Array> {
    return this.#vue.screenshot(options);
  }

  async close(): Promise<void> {
    await this.#vue.close();
  }
}

export const serviceWebNatif = {
  async response(url: string, options: { maxBytes?: number } = {}) {
    const document = await chargerDocument(url);
    const size = Buffer.byteLength(document.html);
    return {
      status: document.status,
      truncated: options.maxBytes !== undefined && size > options.maxBytes,
      body: document.html,
    };
  },
  async bytes(url: string): Promise<Uint8Array> {
    const { impitFetchBinary } = await import("./impit-client");
    const res = await impitFetchBinary(url, { timeoutMs: 60_000 });
    if (!res.ok) throw new Error(`HTTP ${res.status} : ${url}`);
    return res.bytes;
  },
};
