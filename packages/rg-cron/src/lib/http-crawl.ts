/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { existsSync } from "node:fs";
import { mkdir, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { impitFetchBinary, impitFetchText } from "./impit-client";
import { chargerDocument } from "./webview";

export function resolveRepoRoot(startDir = import.meta.dir): string | null {
  const explicit = Bun.env.RG_REPO_ROOT ?? Bun.env.BUN_RG_ROOT;
  if (explicit && existsSync(join(explicit, "apps/rg-website"))) {
    return resolve(explicit);
  }
  let current = resolve(startDir);
  for (let i = 0; i < 8; i++) {
    if (existsSync(join(current, "apps/rg-website"))) {
      return current;
    }
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return null;
}

const BLOCK_SIGNAUX = [
  "cf-browser-verification",
  "Just a moment...",
  "Attention Required! | Cloudflare",
  "__cf_chl_opt",
];

export interface HttpCrawlResult {
  status: number;
  ok: boolean;
  html: string;
  transport: "impit" | "fetch" | "patchright";
}

function estBloqueParWaf(status: number, html: string): boolean {
  if (status === 403 || status === 429 || status === 503) return true;
  if (!html) return false;
  return BLOCK_SIGNAUX.some((signal) => html.includes(signal));
}

/**
 * Fetches an HTML document using Chrome TLS impersonation (`impit` via BoringSSL),
 * falling back to stealth `patchright` Chromium when JS rendering or WAF traversal is required.
 */
export async function fetchHtmlResilient(
  url: string,
  options: { timeoutMs?: number; allowWebViewFallback?: boolean } = {},
): Promise<HttpCrawlResult> {
  const timeoutMs = options.timeoutMs ?? 25_000;
  const allowBrowserFallback = options.allowWebViewFallback ?? true;

  let httpRes: {
    status: number;
    ok: boolean;
    html: string;
    transport: "impit" | "fetch";
  } | null = null;

  try {
    const res = await impitFetchText(url, { timeoutMs });
    httpRes = {
      status: res.status,
      ok: res.ok,
      html: res.text,
      transport: res.transport,
    };
    if (
      (httpRes.ok || httpRes.status === 404) &&
      !estBloqueParWaf(httpRes.status, httpRes.html)
    ) {
      return httpRes;
    }
  } catch {
    // Fall through to stealth browser fallback when enabled.
  }

  if (allowBrowserFallback) {
    try {
      const rendered = await chargerDocument(url, Math.max(timeoutMs, 35_000));
      if (rendered.html && !estBloqueParWaf(rendered.status, rendered.html)) {
        return {
          status: rendered.status || 200,
          ok: true,
          html: rendered.html,
          transport: "patchright",
        };
      }
    } catch {
      // Return best HTTP attempt if stealth browser cannot render page.
    }
  }

  return httpRes ?? { status: 0, ok: false, html: "", transport: "impit" };
}

/**
 * Downloads a binary asset via `impit` (Chrome TLS fingerprint) when absent or forced.
 */
export async function downloadAssetIfMissing(
  url: string,
  destPath: string,
  options: { force?: boolean; timeoutMs?: number } = {},
): Promise<"skipped" | "downloaded" | "failed"> {
  if (!options.force) {
    try {
      const existing = await stat(destPath);
      if (existing.size > 0) return "skipped";
    } catch {
      // Missing destination file, proceed with download.
    }
  }

  await mkdir(dirname(destPath), { recursive: true });
  const timeoutMs = options.timeoutMs ?? 30_000;

  try {
    const res = await impitFetchBinary(url, { timeoutMs });
    if (res.ok && res.bytes.byteLength > 0) {
      await Bun.write(destPath, res.bytes);
      return "downloaded";
    }
  } catch {
    // Handled below as failed download.
  }

  return "failed";
}
