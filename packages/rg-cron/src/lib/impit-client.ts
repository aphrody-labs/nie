/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Shared 2026 Chrome TLS-impersonating HTTP client powered by `impit` (`0.14.9`)
 * and `@crawlee/impit-client` (`3.18.2`).
 *
 * Uses a runtime dynamic loader so single-file cross-compiles (`bun build --compile`)
 * continue to succeed even if platform-specific native N-API prebuilds are external.
 */

import type {
  Browser,
  Impit,
  ImpitOptions,
  ImpitResponse,
  RequestInit as ImpitRequestInit,
} from "impit";

type ImpitModule = typeof import("impit");
type CrawleeImpitModule = typeof import("@crawlee/impit-client");

const runtimeDynamicImport = new Function(
  "specifier",
  "return import(specifier)",
) as <T>(specifier: string) => Promise<T>;

let impitModulePromise: Promise<ImpitModule | null> | null = null;
let defaultImpitPromise: Promise<Impit | null> | null = null;

async function loadImpitModule(): Promise<ImpitModule | null> {
  if (!impitModulePromise) {
    impitModulePromise = runtimeDynamicImport<ImpitModule>("impit").catch(
      () => null,
    );
  }
  return impitModulePromise;
}

export async function isImpitAvailable(): Promise<boolean> {
  return (await loadImpitModule()) !== null;
}

export async function createImpitClient(
  options: ImpitOptions = {},
): Promise<Impit | null> {
  const mod = await loadImpitModule();
  if (!mod) return null;
  return new mod.Impit({
    browser: "chrome",
    vanillaFallback: true,
    followRedirects: true,
    ...options,
  });
}

export async function getSharedImpitClient(): Promise<Impit | null> {
  if (!defaultImpitPromise) {
    defaultImpitPromise = createImpitClient({
      browser: "chrome",
      vanillaFallback: true,
      followRedirects: true,
      timeout: 30_000,
    });
  }
  return defaultImpitPromise;
}

export async function createCrawleeImpitClient(
  options: Omit<ImpitOptions, "proxyUrl"> & {
    maxRedirects?: number;
    cacheClients?: boolean;
  } = {},
): Promise<InstanceType<CrawleeImpitModule["ImpitHttpClient"]> | null> {
  try {
    const mod = await runtimeDynamicImport<CrawleeImpitModule>(
      "@crawlee/impit-client",
    );
    return new mod.ImpitHttpClient({
      browser: "chrome",
      vanillaFallback: true,
      followRedirects: true,
      cacheClients: true,
      ...options,
    });
  } catch {
    return null;
  }
}

export interface ImpitFetchResult {
  status: number;
  ok: boolean;
  url: string;
  headers: Headers;
  transport: "impit" | "fetch";
  response: ImpitResponse | Response;
}

/**
 * Performs an HTTP request with Chrome BoringSSL TLS + HTTP/2 impersonation (`impit`),
 * falling back cleanly to native `fetch` if native prebuilds are unavailable or throw
 * an unsupported TLS quirk.
 */
export async function impitFetch(
  url: string,
  options: {
    method?: ImpitRequestInit["method"];
    headers?: Record<string, string>;
    body?: ImpitRequestInit["body"];
    timeoutMs?: number;
    browser?: Browser;
    redirect?: "follow" | "manual" | "error";
    forceHttp3?: boolean;
    proxyUrl?: string;
  } = {},
): Promise<ImpitFetchResult> {
  const timeoutMs = options.timeoutMs ?? 25_000;
  const client =
    options.browser || options.proxyUrl
      ? await createImpitClient({
          browser: options.browser ?? "chrome",
          proxyUrl: options.proxyUrl,
          timeout: timeoutMs,
        })
      : await getSharedImpitClient();

  if (client) {
    try {
      const response = await client.fetch(url, {
        method: options.method ?? "GET",
        headers: options.headers,
        body: options.body,
        timeout: timeoutMs,
        redirect: options.redirect,
        forceHttp3: options.forceHttp3,
        signal: AbortSignal.timeout(timeoutMs + 2000),
      });
      return {
        status: response.status,
        ok: response.ok,
        url: response.url || url,
        headers: response.headers,
        transport: "impit",
        response,
      };
    } catch {
      // Fall through to native fetch on protocol/DNS/transport fallback.
    }
  }

  const fallbackResponse = await fetch(url, {
    method: options.method ?? "GET",
    headers: options.headers,
    ...(options.body !== undefined
      ? { body: options.body as NonNullable<RequestInit["body"]> }
      : {}),
    redirect: options.redirect ?? "follow",
    signal: AbortSignal.timeout(timeoutMs),
  });
  return {
    status: fallbackResponse.status,
    ok: fallbackResponse.ok,
    url: fallbackResponse.url || url,
    headers: fallbackResponse.headers,
    transport: "fetch",
    response: fallbackResponse,
  };
}

export async function impitFetchText(
  url: string,
  options: {
    headers?: Record<string, string>;
    timeoutMs?: number;
    browser?: Browser;
    proxyUrl?: string;
  } = {},
): Promise<{
  status: number;
  ok: boolean;
  url: string;
  text: string;
  headers: Headers;
  transport: "impit" | "fetch";
}> {
  const res = await impitFetch(url, options);
  const text = await res.response.text();
  return {
    status: res.status,
    ok: res.ok,
    url: res.url,
    text,
    headers: res.headers,
    transport: res.transport,
  };
}

export async function impitFetchBinary(
  url: string,
  options: {
    headers?: Record<string, string>;
    timeoutMs?: number;
    browser?: Browser;
  } = {},
): Promise<{
  status: number;
  ok: boolean;
  url: string;
  contentType: string;
  bytes: Uint8Array;
  transport: "impit" | "fetch";
}> {
  const res = await impitFetch(url, options);
  const contentType =
    res.headers.get("content-type")?.split(";")[0]?.trim() ||
    "application/octet-stream";
  const bytes =
    "bytes" in res.response && typeof res.response.bytes === "function"
      ? await res.response.bytes()
      : new Uint8Array(await res.response.arrayBuffer());
  return {
    status: res.status,
    ok: res.ok,
    url: res.url,
    contentType,
    bytes,
    transport: res.transport,
  };
}
