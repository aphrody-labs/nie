// SPDX-License-Identifier: Apache-2.0
// Rose Griffon Cloudflare Edge Worker: R2 CDN gateway + nie-wasm edge engine.

import wasmModule from "./pkg/nie_wasm_bg.wasm";
import {
  calculate_stats,
  cfgbin_parse_json,
  cfgbin_typed_json,
  cpk_parse_entries,
  crilayla_decompress,
  detect_format,
  g4md_parse_json,
  g4pk_parse_json,
  g4tx_info_json,
  initSync,
  utf_table_json,
  vfs_content_summary,
} from "./pkg/nie_wasm.js";

export interface EdgeR2Object {
  body: ReadableStream | Uint8Array | null;
  size: number;
  httpEtag: string;
  range?: { offset: number; length: number };
  writeHttpMetadata(headers: Headers): void;
  arrayBuffer?(): Promise<ArrayBuffer>;
}

export interface EdgeR2Bucket {
  get(
    key: string,
    options?: { range?: { offset?: number; length?: number; suffix?: number } },
  ): Promise<EdgeR2Object | null>;
}

export interface Env {
  CDN_BUCKET: EdgeR2Bucket;
  NIE_RAW_VFS_TOKEN?: string;
}

const MAX_WASM_BODY_BYTES = 16 * 1024 * 1024;
const CORS_HEADERS = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "GET, HEAD, POST, OPTIONS",
  "access-control-allow-headers": "authorization, content-type, range",
} as const;

let wasmReady = false;
let wasmBootPromise: Promise<void> | null = null;

async function ensureWasm(): Promise<void> {
  if (wasmReady) return;
  if (wasmModule instanceof WebAssembly.Module) {
    initSync({ module: wasmModule });
    wasmReady = true;
    return;
  }
  if (!wasmBootPromise) {
    wasmBootPromise = (async () => {
      const wasmPath =
        typeof wasmModule === "string"
          ? wasmModule
          : new URL("./pkg/nie_wasm_bg.wasm", import.meta.url);
      let bytes: ArrayBuffer;
      if (typeof Bun !== "undefined" && typeof wasmPath === "string") {
        bytes = await Bun.file(wasmPath).arrayBuffer();
      } else if (typeof Bun !== "undefined") {
        bytes = await Bun.file(wasmPath as URL).arrayBuffer();
      } else {
        const res = await fetch(wasmPath as URL);
        bytes = await res.arrayBuffer();
      }
      const mod = await WebAssembly.compile(bytes);
      initSync({ module: mod });
      wasmReady = true;
    })();
  }
  await wasmBootPromise;
}

function constantTimeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) {
    diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return diff === 0;
}

function checkBearerAuth(request: Request, token?: string): boolean {
  const trimmed = token?.trim();
  if (!trimmed) return true;
  const auth = request.headers.get("authorization") ?? "";
  if (!auth.startsWith("Bearer ")) return false;
  return constantTimeEqual(auth.slice(7).trim(), trimmed);
}

export function sanitizeR2Key(raw: string): string | null {
  let decoded: string;
  try {
    decoded = decodeURIComponent(raw);
  } catch {
    return null;
  }
  const clean = decoded.replace(/^\/+/, "");
  if (!clean || clean.includes("\0") || clean.includes("\\")) return null;
  const parts = clean.split("/");
  if (parts.some((seg) => seg === "" || seg === "." || seg === "..")) {
    return null;
  }
  return parts.join("/");
}

export function parseHttpRange(
  header: string | null,
): { offset?: number; length?: number; suffix?: number } | null {
  if (!header) return null;
  const m = /^bytes=(\d*)-(\d*)$/i.exec(header.trim());
  if (!m) return null;
  const [, startStr, endStr] = m;
  if (!startStr && !endStr) return null;
  if (!startStr && endStr) {
    const suffix = Number(endStr);
    return Number.isSafeInteger(suffix) && suffix > 0 ? { suffix } : null;
  }
  const start = Number(startStr);
  if (!Number.isSafeInteger(start) || start < 0) return null;
  if (!endStr) {
    return { offset: start };
  }
  const end = Number(endStr);
  if (!Number.isSafeInteger(end) || end < start) return null;
  return { offset: start, length: end - start + 1 };
}

async function readBoundedBytes(
  request: Request,
): Promise<Uint8Array | Response> {
  const lenHeader = request.headers.get("content-length");
  if (lenHeader !== null) {
    const declared = Number(lenHeader);
    if (Number.isFinite(declared) && declared > MAX_WASM_BODY_BYTES) {
      return Response.json(
        {
          ok: false,
          error: `Payload exceeds ${MAX_WASM_BODY_BYTES} bytes limit`,
        },
        { status: 413, headers: CORS_HEADERS },
      );
    }
  }
  const buf = new Uint8Array(await request.arrayBuffer());
  if (buf.byteLength === 0) {
    return Response.json(
      { ok: false, error: "Empty request body" },
      { status: 400, headers: CORS_HEADERS },
    );
  }
  if (buf.byteLength > MAX_WASM_BODY_BYTES) {
    return Response.json(
      {
        ok: false,
        error: `Payload exceeds ${MAX_WASM_BODY_BYTES} bytes limit`,
      },
      { status: 413, headers: CORS_HEADERS },
    );
  }
  return buf;
}

async function readObjectBytes(object: EdgeR2Object): Promise<Uint8Array> {
  if (object.arrayBuffer) {
    return new Uint8Array(await object.arrayBuffer());
  }
  if (object.body instanceof Uint8Array) {
    return object.body;
  }
  if (!object.body) return new Uint8Array(0);
  return new Uint8Array(await new Response(object.body).arrayBuffer());
}

async function handleR2Get(
  request: Request,
  env: Env,
  rawKey: string,
): Promise<Response> {
  if (!checkBearerAuth(request, env.NIE_RAW_VFS_TOKEN)) {
    return Response.json(
      { ok: false, error: "Unauthorized" },
      { status: 401, headers: CORS_HEADERS },
    );
  }
  const key = sanitizeR2Key(rawKey);
  if (!key) {
    return Response.json(
      { ok: false, error: "Invalid object key" },
      { status: 400, headers: CORS_HEADERS },
    );
  }
  const parsedRange = parseHttpRange(request.headers.get("range"));
  const object = await env.CDN_BUCKET.get(
    key,
    parsedRange ? { range: parsedRange } : undefined,
  );
  if (!object) {
    return new Response("Not Found", {
      status: 404,
      headers: { ...CORS_HEADERS, "cache-control": "no-store" },
    });
  }

  const headers = new Headers(CORS_HEADERS);
  object.writeHttpMetadata(headers);
  headers.set("etag", object.httpEtag);
  headers.set("accept-ranges", "bytes");
  if (!headers.has("cache-control")) {
    headers.set(
      "cache-control",
      "public, max-age=3600, stale-while-revalidate=86400",
    );
  }

  let status = 200;
  if (parsedRange && object.range && object.size > 0) {
    const start = object.range.offset;
    const end = start + object.range.length - 1;
    headers.set("content-range", `bytes ${start}-${end}/${object.size}`);
    headers.set("content-length", String(object.range.length));
    status = 206;
  } else if (object.size >= 0) {
    headers.set("content-length", String(object.size));
  }

  return new Response(request.method === "HEAD" ? null : object.body, {
    status,
    headers,
  });
}

/**
 * Public static assets of cdn.rosegriffon.fr: served from R2 (key = path without the leading
 * slash), falling back to the prd origin when the object is missing. R2 is the source, prd the
 * backup.
 */
export async function handleStatic(
  request: Request,
  env: Env,
  path: string,
  origin: (request: Request) => Promise<Response> = (r) => fetch(r),
): Promise<Response> {
  const key = sanitizeR2Key(path);
  const object = key ? await env.CDN_BUCKET.get(key) : null;
  if (!object) {
    const fallback = await origin(request);
    const headers = new Headers(fallback.headers);
    headers.set("x-rg-edge", "origin");
    return new Response(fallback.body, { status: fallback.status, headers });
  }
  const headers = new Headers(CORS_HEADERS);
  object.writeHttpMetadata(headers);
  headers.set("etag", object.httpEtag);
  headers.set("content-length", String(object.size));
  headers.set("x-rg-edge", "r2");
  if (!headers.has("cache-control"))
    headers.set("cache-control", "public, max-age=86400, stale-while-revalidate=604800");
  return new Response(request.method === "HEAD" ? null : object.body, { status: 200, headers });
}

async function handleNieRoute(
  request: Request,
  env: Env,
  url: URL,
): Promise<Response> {
  const path = url.pathname;

  if (path === "/nie/health" && request.method === "GET") {
    await ensureWasm();
    return Response.json(
      {
        ok: true,
        service: "rosegriffon-edge",
        runtime: "cloudflare-workers",
        engine: "nie-wasm",
        features: [
          "detect_format",
          "vfs_content_summary",
          "g4tx_info_json",
          "cfgbin_parse_json",
          "cfgbin_typed_json",
          "g4pk_parse_json",
          "g4md_parse_json",
          "cpk_parse_entries",
          "crilayla_decompress",
          "utf_table_json",
          "calculate_stats",
          "r2_range_vfs",
        ],
      },
      { headers: { ...CORS_HEADERS, "cache-control": "no-store" } },
    );
  }

  if (path === "/nie/formats/detect" && request.method === "POST") {
    const bytesOrErr = await readBoundedBytes(request);
    if (bytesOrErr instanceof Response) return bytesOrErr;
    await ensureWasm();
    const hintPath = url.searchParams.get("path") ?? "payload.bin";
    const format = detect_format(bytesOrErr);
    let summary: unknown = null;
    try {
      summary = JSON.parse(vfs_content_summary(hintPath, bytesOrErr));
    } catch {
      summary = null;
    }
    return Response.json(
      { ok: true, format, bytes: bytesOrErr.byteLength, summary },
      { headers: { ...CORS_HEADERS, "cache-control": "no-store" } },
    );
  }

  if (path === "/nie/formats/parse" && request.method === "POST") {
    const bytesOrErr = await readBoundedBytes(request);
    if (bytesOrErr instanceof Response) return bytesOrErr;
    await ensureWasm();

    const detected = detect_format(bytesOrErr);
    const requested = (
      url.searchParams.get("format") ?? detected
    ).toLowerCase();
    const filename = url.searchParams.get("filename") ?? "payload.bin";

    try {
      if (requested === "crilayla") {
        const out = crilayla_decompress(bytesOrErr);
        return new Response(out, {
          status: 200,
          headers: {
            ...CORS_HEADERS,
            "content-type": "application/octet-stream",
            "x-nie-format": detected,
            "cache-control": "no-store",
          },
        });
      }

      let rawJson: string;
      if (requested === "g4tx") {
        rawJson = g4tx_info_json(bytesOrErr);
      } else if (requested === "g4pk") {
        rawJson = g4pk_parse_json(bytesOrErr);
      } else if (requested === "g4md") {
        rawJson = g4md_parse_json(bytesOrErr);
      } else if (requested === "cfgbin" || requested === "cfg.bin") {
        rawJson = url.searchParams.has("filename")
          ? cfgbin_typed_json(bytesOrErr, filename)
          : cfgbin_parse_json(bytesOrErr);
      } else if (requested === "cpk") {
        rawJson = cpk_parse_entries(bytesOrErr, filename);
      } else if (requested === "@utf" || requested === "utf") {
        rawJson = utf_table_json(bytesOrErr);
      } else {
        rawJson = vfs_content_summary(filename, bytesOrErr);
      }

      return Response.json(
        {
          ok: true,
          detected,
          format: requested,
          data: JSON.parse(rawJson),
        },
        { headers: { ...CORS_HEADERS, "cache-control": "no-store" } },
      );
    } catch (err) {
      return Response.json(
        {
          ok: false,
          detected,
          format: requested,
          error: err instanceof Error ? err.message : String(err),
        },
        { status: 422, headers: CORS_HEADERS },
      );
    }
  }

  if (path === "/nie/vfs/summary" && request.method === "GET") {
    if (!checkBearerAuth(request, env.NIE_RAW_VFS_TOKEN)) {
      return Response.json(
        { ok: false, error: "Unauthorized" },
        { status: 401, headers: CORS_HEADERS },
      );
    }
    const key = sanitizeR2Key(url.searchParams.get("key") ?? "");
    if (!key) {
      return Response.json(
        { ok: false, error: "Missing or invalid R2 key" },
        { status: 400, headers: CORS_HEADERS },
      );
    }
    const object = await env.CDN_BUCKET.get(key);
    if (!object) {
      return Response.json(
        { ok: false, error: "Object not found" },
        { status: 404, headers: CORS_HEADERS },
      );
    }
    if (object.size > MAX_WASM_BODY_BYTES) {
      return Response.json(
        {
          ok: false,
          error: `Object exceeds ${MAX_WASM_BODY_BYTES} bytes limit`,
        },
        { status: 413, headers: CORS_HEADERS },
      );
    }
    await ensureWasm();
    const bytes = await readObjectBytes(object);
    return Response.json(
      {
        ok: true,
        key,
        size: object.size,
        format: detect_format(bytes),
        summary: JSON.parse(vfs_content_summary(key, bytes)),
      },
      { headers: { ...CORS_HEADERS, "cache-control": "public, max-age=300" } },
    );
  }

  if (
    path === "/nie/core/stats" &&
    (request.method === "GET" || request.method === "POST")
  ) {
    await ensureWasm();
    const numParam = (name: string, fallback: number): number => {
      const raw = url.searchParams.get(name);
      if (raw === null) return fallback;
      const n = Number(raw);
      return Number.isFinite(n) ? n : fallback;
    };
    let mainPos = numParam("mainPosition", 4);
    let subPos = numParam("subPosition", 0);
    let growth = numParam("growthPattern", 0);
    let rank = numParam("rank", 5);
    let playStyle = numParam("playStyle", 0);
    let level = numParam("level", 99);

    if (request.method === "POST") {
      try {
        const body = (await request.json()) as Record<string, unknown>;
        if (typeof body.mainPosition === "number") mainPos = body.mainPosition;
        if (typeof body.subPosition === "number") subPos = body.subPosition;
        if (typeof body.growthPattern === "number") growth = body.growthPattern;
        if (typeof body.rank === "number") rank = body.rank;
        if (typeof body.playStyle === "number") playStyle = body.playStyle;
        if (typeof body.level === "number") level = body.level;
      } catch {
        return Response.json(
          { ok: false, error: "Invalid JSON body" },
          { status: 400, headers: CORS_HEADERS },
        );
      }
    }

    const stats = JSON.parse(
      calculate_stats(mainPos, subPos, growth, rank, playStyle, level),
    );
    return Response.json(
      { ok: true, data: stats },
      { headers: { ...CORS_HEADERS, "cache-control": "public, max-age=3600" } },
    );
  }

  if (
    path.startsWith("/nie/r2/") &&
    (request.method === "GET" || request.method === "HEAD")
  ) {
    return handleR2Get(request, env, path.slice("/nie/r2/".length));
  }

  return Response.json(
    { ok: false, error: "Unknown NIE edge endpoint" },
    { status: 404, headers: CORS_HEADERS },
  );
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: CORS_HEADERS });
    }

    if (url.pathname.startsWith("/static/") && (request.method === "GET" || request.method === "HEAD")) {
      return handleStatic(request, env, url.pathname);
    }

    if (url.pathname === "/health" || url.pathname === "/") {
      return Response.json(
        {
          ok: true,
          service: "rosegriffon-edge",
          runtime: "cloudflare-workers",
          bucket: "rosegriffon-cdn",
          nieWasm: true,
        },
        {
          headers: { ...CORS_HEADERS, "cache-control": "no-store" },
        },
      );
    }

    if (url.pathname.startsWith("/nie/")) {
      return handleNieRoute(request, env, url);
    }

    if (url.pathname === "/fetch") {
      const target = url.searchParams.get("url");
      if (!target) {
        return new Response("Missing url", { status: 400 });
      }
      let parsed: URL;
      try {
        parsed = new URL(target);
      } catch {
        return new Response("Invalid url", { status: 400 });
      }
      if (
        (parsed.protocol !== "https:" && parsed.protocol !== "http:") ||
        /^(localhost|127\.|10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|0\.0\.0\.0|\[?::1\]?|.*\.local$)/i.test(
          parsed.hostname,
        )
      ) {
        return new Response("Forbidden target", { status: 403 });
      }
      const upstream = await fetch(parsed.toString(), {
        headers: {
          "user-agent":
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
          accept:
            "text/html,application/xhtml+xml,application/json;q=0.9,*/*;q=0.8",
        },
      });
      return new Response(upstream.body, {
        status: upstream.status,
        headers: {
          ...CORS_HEADERS,
          "content-type":
            upstream.headers.get("content-type") ?? "text/plain; charset=utf-8",
          "cache-control": "public, max-age=300",
        },
      });
    }

    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("Method Not Allowed", { status: 405 });
    }

    return handleR2Get(request, env, url.pathname);
  },
};
