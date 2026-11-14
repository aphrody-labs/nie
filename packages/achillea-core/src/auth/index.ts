/**
 * Portable session vocabulary shared by Achilléa adapters.
 *
 * Signing lives in the server auth package. Verification uses Web Crypto and
 * contains no Node APIs, so browser, edge, bot, API, ranked, and realtime
 * consumers share the same purpose-bound contract.
 */
import { STACK_CONTRACT } from "@rosegriffon/config/contract";

export const ACHILLEA_AUTH_ISSUER: string = STACK_CONTRACT.auth.issuer;
export const ACHILLEA_SESSION_AUDIENCE: string =
  STACK_CONTRACT.auth.audiences.session;
export const ACHILLEA_REALTIME_AUDIENCE: string =
  STACK_CONTRACT.auth.audiences.realtime;
export const ACHILLEA_SESSION_COOKIE_NAME: string =
  STACK_CONTRACT.auth.cookieName;
export const ACHILLEA_SESSION_TTL_SECONDS: number =
  STACK_CONTRACT.auth.ttlSeconds.session;
export const ACHILLEA_REALTIME_TTL_SECONDS: number =
  STACK_CONTRACT.auth.ttlSeconds.realtime;

export interface AchilleaSessionIdentity {
  id: string;
  username?: string | null;
  avatar?: string | null;
}

export interface AchilleaSessionClaims extends AchilleaSessionIdentity {
  expiresAt: number;
}

const TEXT_ENCODER = new TextEncoder();
const TEXT_DECODER = new TextDecoder();
const EXPECTED_HEADER_B64 = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
const HMAC_KEY_CACHE = new Map<string, Promise<CryptoKey>>();

interface BunCryptoHasherInstance {
  update(data: string): { digest(): Uint8Array };
}

type BunGlobalLike = {
  CryptoHasher?: new (
    algorithm: string,
    key: string,
  ) => BunCryptoHasherInstance;
};

function timingSafeEqualBytes(a: Uint8Array, b: Uint8Array): boolean {
  let diff = a.length ^ b.length;
  const len = Math.min(a.length, b.length);
  for (let i = 0; i < len; i += 1) {
    diff |= a[i]! ^ b[i]!;
  }
  return diff === 0;
}

async function verifyHmacSha256(
  secret: string,
  signingInput: string,
  encodedSignature: string,
): Promise<boolean> {
  const providedSig = decodeBase64Url(encodedSignature);
  const bunHasher = (globalThis as { Bun?: BunGlobalLike }).Bun?.CryptoHasher;
  if (typeof bunHasher === "function") {
    const expectedMac = new bunHasher("sha256", secret)
      .update(signingInput)
      .digest();
    return timingSafeEqualBytes(expectedMac, providedSig);
  }
  const key = await getHmacVerifyKey(secret);
  return crypto.subtle.verify(
    "HMAC",
    key,
    providedSig,
    TEXT_ENCODER.encode(signingInput),
  );
}

function getHmacVerifyKey(secret: string): Promise<CryptoKey> {
  let cached = HMAC_KEY_CACHE.get(secret);
  if (!cached) {
    if (HMAC_KEY_CACHE.size >= 32) {
      const oldest = HMAC_KEY_CACHE.keys().next().value;
      if (oldest !== undefined) HMAC_KEY_CACHE.delete(oldest);
    }
    cached = crypto.subtle.importKey(
      "raw",
      TEXT_ENCODER.encode(secret),
      { name: "HMAC", hash: "SHA-256" },
      false,
      ["verify"],
    );
    HMAC_KEY_CACHE.set(secret, cached);
  }
  return cached;
}

function decodeBase64Url(value: string): Uint8Array<ArrayBuffer> {
  if (typeof Uint8Array.fromBase64 === "function") {
    return Uint8Array.fromBase64(value, {
      alphabet: "base64url",
    }) as Uint8Array<ArrayBuffer>;
  }
  const normalized = value
    .replaceAll("-", "+")
    .replaceAll("_", "/")
    .padEnd(Math.ceil(value.length / 4) * 4, "=");
  const binary = atob(normalized);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/**
 * Verify a purpose-bound Achilléa token without Node/JWT dependencies.
 * Legacy unsigned/untagged tokens are rejected by every consumer.
 */
async function verifyAchilleaToken(
  token: string,
  secret: string,
  tokenUse: "session" | "realtime",
  audienceName: string,
  now: number,
): Promise<AchilleaSessionClaims | null> {
  if (!token || !secret) return null;
  const dot1 = token.indexOf(".");
  if (dot1 <= 0) return null;
  const dot2 = token.indexOf(".", dot1 + 1);
  if (dot2 <= dot1 + 1 || token.indexOf(".", dot2 + 1) !== -1) return null;
  const encodedHeader = token.slice(0, dot1);
  const encodedPayload = token.slice(dot1 + 1, dot2);
  const encodedSignature = token.slice(dot2 + 1);
  if (!encodedSignature) return null;
  try {
    if (encodedHeader !== EXPECTED_HEADER_B64) {
      const header = JSON.parse(
        TEXT_DECODER.decode(decodeBase64Url(encodedHeader)),
      ) as {
        alg?: string;
        typ?: string;
      };
      if (header.alg !== "HS256") return null;
    }
    const payload = JSON.parse(
      TEXT_DECODER.decode(decodeBase64Url(encodedPayload)),
    ) as {
      id?: unknown;
      sub?: unknown;
      username?: unknown;
      avatar?: unknown;
      tokenUse?: unknown;
      iss?: unknown;
      aud?: unknown;
      exp?: unknown;
    };
    if (payload.tokenUse !== tokenUse || payload.iss !== ACHILLEA_AUTH_ISSUER)
      return null;
    const audience = Array.isArray(payload.aud) ? payload.aud : [payload.aud];
    if (!audience.includes(audienceName)) return null;
    if (
      typeof payload.exp !== "number" ||
      !Number.isFinite(payload.exp) ||
      payload.exp <= now
    )
      return null;
    const id =
      typeof payload.id === "string" && payload.id ? payload.id : payload.sub;
    if (
      typeof id !== "string" ||
      !id ||
      (payload.id !== undefined && payload.id !== payload.sub)
    )
      return null;

    const valid = await verifyHmacSha256(
      secret,
      `${encodedHeader}.${encodedPayload}`,
      encodedSignature,
    );
    if (!valid) return null;
    return {
      id,
      username:
        typeof payload.username === "string" ? payload.username : undefined,
      avatar: typeof payload.avatar === "string" ? payload.avatar : undefined,
      expiresAt: payload.exp,
    };
  } catch {
    return null;
  }
}

/** Verify a web-session token; realtime tokens cannot authenticate a session. */
export function verifyAchilleaSessionToken(
  token: string,
  secret: string,
  now = Math.floor(Date.now() / 1000),
): Promise<AchilleaSessionClaims | null> {
  return verifyAchilleaToken(
    token,
    secret,
    "session",
    ACHILLEA_SESSION_AUDIENCE,
    now,
  );
}

/** Verify a realtime token; session cookies must use the session verifier. */
export function verifyAchilleaRealtimeToken(
  token: string,
  secret: string,
  now = Math.floor(Date.now() / 1000),
): Promise<AchilleaSessionClaims | null> {
  return verifyAchilleaToken(
    token,
    secret,
    "realtime",
    ACHILLEA_REALTIME_AUDIENCE,
    now,
  );
}

/** Constant-time comparison for internal bearer secrets (portable Web runtime). */
export function verifyAdminBearerToken(
  token: unknown,
  expected: string | undefined,
): boolean {
  if (typeof token !== "string" || !expected) return false;
  const left = new TextEncoder().encode(token);
  const right = new TextEncoder().encode(expected);
  let diff = left.length ^ right.length;
  const length = Math.max(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    diff |= (left[index] ?? 0) ^ (right[index] ?? 0);
  }
  return diff === 0;
}
