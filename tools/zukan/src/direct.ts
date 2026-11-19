import ZUKAN_CODE_HASHES from "./zukan-code-hashes.json";
import ZUKAN_HASHES from "./zukan-hashes.json";

/** Official Zukan CloudFront origin serving portraits, posters and Hissatsu streams. */
export const ZUKAN_CDN_BASE = "https://dxi4wb638ujep.cloudfront.net/1";

const ID_HASHES = ZUKAN_HASHES as Record<string, string>;
const CODE_HASHES = ZUKAN_CODE_HASHES as Record<string, string>;

/** Formats a Zukan hash or relative path into a full CloudFront asset URL. */
export function zukanAssetUrl(
  hashOrUrl: string | undefined | null,
  extension = "png",
): string | null {
  if (!hashOrUrl) return null;
  const trimmed = hashOrUrl.trim();
  if (!trimmed) return null;
  if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
    return trimmed;
  }
  const clean = trimmed.replace(/^\/+/, "");
  if (!clean) return null;
  const hasExt = /\.[a-z0-9]{2,5}$/i.test(clean);
  return `${ZUKAN_CDN_BASE}/${hasExt ? clean : `${clean}.${extension}`}`;
}

/** Resolves an official Zukan portrait hash from an explicit hash, hex ID, or internal code. */
export function resolveZukanCharacterHash(input: {
  zukanHash?: string | null;
  id?: string | null;
  internalCode?: string | null;
}): string | undefined {
  if (input.zukanHash && input.zukanHash.trim()) {
    return input.zukanHash.trim();
  }
  if (input.id) {
    const idClean = input.id.trim();
    const byId = ID_HASHES[idClean] ?? CODE_HASHES[idClean];
    if (byId) return byId;
    const baseFromId = idClean.replace(/_\d{4}$/, "");
    if (CODE_HASHES[baseFromId]) return CODE_HASHES[baseFromId];
  }
  if (input.internalCode) {
    const codeClean = input.internalCode.trim();
    if (CODE_HASHES[codeClean]) return CODE_HASHES[codeClean];
    const baseCode = codeClean.replace(/_\d{4}$/, "");
    if (CODE_HASHES[baseCode]) return CODE_HASHES[baseCode];
  }
  return undefined;
}

/** Returns the direct Zukan CloudFront PNG URL for a character when known, or `null`. */
export function resolveZukanFaceUrl(input: {
  zukanHash?: string | null;
  id?: string | null;
  internalCode?: string | null;
}): string | null {
  const hash = resolveZukanCharacterHash(input);
  return hash ? zukanAssetUrl(hash, "png") : null;
}
