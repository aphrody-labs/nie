import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { S3Client } from "bun";
import { assertPublicCrawlUrl } from "./obscura";

export interface R2UploadInput {
  targetSlug: string;
  sourceUrl: string;
  data: Uint8Array;
  contentType?: string;
  r2KeyPrefix?: string;
  stagingDir?: string;
}

export interface R2UploadResult {
  sourceUrl: string;
  sha256: string;
  contentType: string;
  byteSize: number;
  r2Bucket: string | null;
  r2Key: string;
  r2Url: string | null;
  storageMode: "r2" | "rclone" | "local-staging";
}

export function detectBinaryMimeType(
  data: Uint8Array,
  fallback = "application/octet-stream",
): { mime: string; ext: string } {
  if (data.length >= 12) {
    // RIFF....WEBP
    if (
      data[0] === 0x52 &&
      data[1] === 0x49 &&
      data[2] === 0x46 &&
      data[3] === 0x46 &&
      data[8] === 0x57 &&
      data[9] === 0x45 &&
      data[10] === 0x42 &&
      data[11] === 0x50
    ) {
      return { mime: "image/webp", ext: "webp" };
    }
  }
  if (
    data.length >= 8 &&
    data[0] === 0x89 &&
    data[1] === 0x50 &&
    data[2] === 0x4e &&
    data[3] === 0x47
  ) {
    return { mime: "image/png", ext: "png" };
  }
  if (
    data.length >= 3 &&
    data[0] === 0xff &&
    data[1] === 0xd8 &&
    data[2] === 0xff
  ) {
    return { mime: "image/jpeg", ext: "jpg" };
  }
  if (
    data.length >= 4 &&
    data[0] === 0x47 &&
    data[1] === 0x49 &&
    data[2] === 0x46 &&
    data[3] === 0x38
  ) {
    return { mime: "image/gif", ext: "gif" };
  }
  if (
    data.length >= 4 &&
    data[0] === 0x1a &&
    data[1] === 0x45 &&
    data[2] === 0xdf &&
    data[3] === 0xa3
  ) {
    return { mime: "video/webm", ext: "webm" };
  }
  const cleanFallback = fallback.split(";")[0]!.trim().toLowerCase();
  if (cleanFallback.startsWith("image/svg")) {
    return { mime: "image/svg+xml", ext: "svg" };
  }
  if (cleanFallback === "application/json") {
    return { mime: "application/json", ext: "json" };
  }
  if (cleanFallback.startsWith("text/")) {
    return { mime: cleanFallback, ext: "txt" };
  }
  return { mime: cleanFallback || "application/octet-stream", ext: "bin" };
}

export function isR2Configured(): boolean {
  return Boolean(
    Bun.env.R2_ACCOUNT_ID &&
    Bun.env.R2_BUCKET &&
    Bun.env.R2_ACCESS_KEY_ID &&
    Bun.env.R2_SECRET_ACCESS_KEY,
  );
}

function createR2Client(): S3Client | null {
  if (!isR2Configured()) return null;
  return new S3Client({
    accessKeyId: Bun.env.R2_ACCESS_KEY_ID!,
    secretAccessKey: Bun.env.R2_SECRET_ACCESS_KEY!,
    bucket: Bun.env.R2_BUCKET!,
    endpoint: `https://${Bun.env.R2_ACCOUNT_ID!}.r2.cloudflarestorage.com`,
  });
}

function resolvePublicR2Url(key: string): string | null {
  const origin = Bun.env.R2_PUBLIC_URL?.replace(/\/+$/, "");
  if (origin) return `${origin}/${key}`;
  const bucket = Bun.env.R2_BUCKET;
  const accountId = Bun.env.R2_ACCOUNT_ID;
  if (bucket && accountId) {
    return `https://${accountId}.r2.cloudflarestorage.com/${bucket}/${key}`;
  }
  return null;
}

export async function syncAssetToR2(
  input: R2UploadInput,
): Promise<R2UploadResult> {
  const hasher = new Bun.CryptoHasher("sha256");
  hasher.update(input.data);
  const sha256 = hasher.digest("hex");

  const { mime, ext } = detectBinaryMimeType(input.data, input.contentType);
  const prefix = (
    input.r2KeyPrefix ?? `characters/${input.targetSlug}`
  ).replace(/^\/+|\/+$/g, "");
  const r2Key = `${prefix}/${sha256.slice(0, 16)}.${ext}`;
  const r2Bucket = Bun.env.R2_BUCKET ?? null;

  const s3 = createR2Client();
  if (s3) {
    await s3.write(r2Key, input.data, { type: mime });
    return {
      sourceUrl: input.sourceUrl,
      sha256,
      contentType: mime,
      byteSize: input.data.byteLength,
      r2Bucket,
      r2Key,
      r2Url: resolvePublicR2Url(r2Key),
      storageMode: "r2",
    };
  }

  const stagingRoot =
    input.stagingDir ?? Bun.env.RG_CRAWL_R2_STAGING_DIR ?? "var/r2-staging";
  const localPath = join(stagingRoot, r2Key);
  mkdirSync(dirname(localPath), { recursive: true });
  writeFileSync(localPath, input.data);

  const rcloneRemote = Bun.env.R2_RCLONE_REMOTE?.trim();
  const rcloneBin = Bun.which("rclone");
  if (rcloneRemote && r2Bucket && rcloneBin) {
    const proc = Bun.spawn(
      [rcloneBin, "copyto", localPath, `${rcloneRemote}:${r2Bucket}/${r2Key}`],
      { stdout: "ignore", stderr: "pipe" },
    );
    const exitCode = await proc.exited;
    if (exitCode === 0) {
      return {
        sourceUrl: input.sourceUrl,
        sha256,
        contentType: mime,
        byteSize: input.data.byteLength,
        r2Bucket,
        r2Key,
        r2Url: resolvePublicR2Url(r2Key),
        storageMode: "rclone",
      };
    }
  }

  return {
    sourceUrl: input.sourceUrl,
    sha256,
    contentType: mime,
    byteSize: input.data.byteLength,
    r2Bucket,
    r2Key,
    r2Url: resolvePublicR2Url(r2Key),
    storageMode: "local-staging",
  };
}

export async function downloadAndMirrorAsset(
  targetSlug: string,
  assetUrl: string,
  options: {
    fetchImpl?: typeof fetch;
    timeoutMs?: number;
    maxBytes?: number;
    stagingDir?: string;
  } = {},
): Promise<R2UploadResult | null> {
  assertPublicCrawlUrl(assetUrl);
  const fetchImpl = options.fetchImpl ?? fetch;
  const timeoutMs = options.timeoutMs ?? 15_000;
  const maxBytes = options.maxBytes ?? 15 * 1024 * 1024;

  const response = await fetchImpl(assetUrl, {
    headers: {
      "user-agent":
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36",
      accept: "image/*,video/webm,*/*;q=0.8",
    },
    signal: AbortSignal.timeout(timeoutMs),
  });
  if (!response.ok) return null;

  const headerType = response.headers.get("content-type") ?? "";
  if (headerType.toLowerCase().includes("text/html")) {
    return null;
  }

  const buffer = new Uint8Array(await response.arrayBuffer());
  if (buffer.byteLength === 0 || buffer.byteLength > maxBytes) {
    return null;
  }

  // Reject disguised HTML error/challenge pages served with non-HTML headers.
  const head = new TextDecoder().decode(buffer.subarray(0, 128)).trimStart();
  if (
    head.startsWith("<!DOCTYPE html") ||
    head.startsWith("<html") ||
    head.includes("Just a moment...")
  ) {
    return null;
  }

  return syncAssetToR2({
    targetSlug,
    sourceUrl: assetUrl,
    data: buffer,
    contentType: headerType || undefined,
    stagingDir: options.stagingDir,
  });
}
