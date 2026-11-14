import { describe, expect, test } from "bun:test";
import worker, {
  parseHttpRange,
  sanitizeR2Key,
  type EdgeR2Bucket,
  type Env,
} from "./worker.ts";

function makeMemoryBucket(files: Record<string, Uint8Array>): EdgeR2Bucket {
  return {
    async get(key, options) {
      const data = files[key];
      if (!data) return null;
      const range = options?.range;
      let slice = data;
      let resolvedRange: { offset: number; length: number } | undefined;
      if (range) {
        if (typeof range.suffix === "number") {
          const offset = Math.max(0, data.byteLength - range.suffix);
          slice = data.subarray(offset);
          resolvedRange = { offset, length: slice.byteLength };
        } else {
          const offset = range.offset ?? 0;
          const end =
            typeof range.length === "number"
              ? Math.min(data.byteLength, offset + range.length)
              : data.byteLength;
          slice = data.subarray(offset, end);
          resolvedRange = { offset, length: slice.byteLength };
        }
      }
      return {
        body: slice,
        size: data.byteLength,
        httpEtag: `"${key}-${data.byteLength}"`,
        range: resolvedRange,
        writeHttpMetadata(headers) {
          headers.set("content-type", "application/octet-stream");
        },
        async arrayBuffer() {
          return slice.slice().buffer;
        },
      };
    },
  };
}

function makeEnv(files: Record<string, Uint8Array> = {}, token?: string): Env {
  return {
    CDN_BUCKET: makeMemoryBucket(files),
    NIE_RAW_VFS_TOKEN: token,
  };
}

describe("rosegriffon-edge Cloudflare Worker + aphrody-nie-wasm", () => {
  test("GET /health reports nieWasm capability", async () => {
    const res = await worker.fetch(
      new Request("https://edge.test/health"),
      makeEnv(),
    );
    expect(res.status).toBe(200);
    const body = (await res.json()) as { ok: boolean; nieWasm: boolean };
    expect(body.ok).toBe(true);
    expect(body.nieWasm).toBe(true);
  });

  test("GET /nie/health initializes real aphrody-nie-wasm engine", async () => {
    const res = await worker.fetch(
      new Request("https://edge.test/nie/health"),
      makeEnv(),
    );
    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      ok: boolean;
      engine: string;
      features: string[];
    };
    expect(body.ok).toBe(true);
    expect(body.engine).toBe("aphrody-nie-wasm");
    expect(body.features).toContain("detect_format");
    expect(body.features).toContain("r2_range_vfs");
  });

  test("POST /nie/formats/detect identifies G4TX and CPK magics via WASM", async () => {
    const g4tx = new Uint8Array([0x47, 0x34, 0x54, 0x58, 0, 0, 0, 0]);
    const res = await worker.fetch(
      new Request("https://edge.test/nie/formats/detect?path=ui/test.g4tx", {
        method: "POST",
        body: g4tx,
      }),
      makeEnv(),
    );
    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      ok: boolean;
      format: string;
      bytes: number;
    };
    expect(body.ok).toBe(true);
    expect(body.format).toBe("G4TX");
    expect(body.bytes).toBe(8);
  });

  test("GET /nie/core/stats computes level 99 stats via aphrody-nie-wasm", async () => {
    const res = await worker.fetch(
      new Request(
        "https://edge.test/nie/core/stats?mainPosition=4&rank=5&level=99",
      ),
      makeEnv(),
    );
    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      ok: boolean;
      data: { stats: { kc: number } };
    };
    expect(body.ok).toBe(true);
    expect(body.data.stats.kc).toBeGreaterThan(0);
  });

  test("GET /nie/r2/* serves HTTP byte ranges and enforces bearer token + traversal guard", async () => {
    const env = makeEnv(
      { "ievr/packs/sample.bin": new Uint8Array([10, 20, 30, 40, 50, 60]) },
      "edge-secret",
    );

    const unauth = await worker.fetch(
      new Request("https://edge.test/nie/r2/ievr/packs/sample.bin"),
      env,
    );
    expect(unauth.status).toBe(401);

    const traversal = await worker.fetch(
      new Request("https://edge.test/nie/r2/..%2Fsecret.txt", {
        headers: { authorization: "Bearer edge-secret" },
      }),
      env,
    );
    expect(traversal.status).toBe(400);

    const ranged = await worker.fetch(
      new Request("https://edge.test/nie/r2/ievr/packs/sample.bin", {
        headers: {
          authorization: "Bearer edge-secret",
          range: "bytes=1-3",
        },
      }),
      env,
    );
    expect(ranged.status).toBe(206);
    expect(ranged.headers.get("content-range")).toBe("bytes 1-3/6");
    expect(Array.from(new Uint8Array(await ranged.arrayBuffer()))).toEqual([
      20, 30, 40,
    ]);
  });

  test("sanitizeR2Key and parseHttpRange reject unsafe inputs", () => {
    expect(sanitizeR2Key("ievr/data/cpk_list.cfg.bin")).toBe(
      "ievr/data/cpk_list.cfg.bin",
    );
    expect(sanitizeR2Key("../etc/passwd")).toBeNull();
    expect(sanitizeR2Key("a\\b")).toBeNull();
    expect(parseHttpRange("bytes=0-15")).toEqual({ offset: 0, length: 16 });
    expect(parseHttpRange("bytes=-8")).toEqual({ suffix: 8 });
    expect(parseHttpRange("bytes=10-2")).toBeNull();
  });
});
