/**
 * Helpers d'environnement et fabrique `Bun.SQL` / `Bun.RedisClient` de @rosegriffon/db.
 * Aucune lecture d'environnement au chargement du module ; les valeurs ne sont jamais journalisées.
 */
import { RedisClient, type RedisOptions, SQL } from "bun";

export { isUsableEnvValue } from "./env-value";

/** Taille de pool (`parseInt`) bornée ; `fallback` si la valeur est absente ou invalide. */
export function resolvePoolSize(
  raw: string | undefined,
  options: { fallback: number; ceiling: number },
): number {
  const value = Number.parseInt(raw ?? "", 10);
  if (!Number.isFinite(value) || value <= 0) return options.fallback;
  return Math.min(value, options.ceiling);
}

const SECRET_PARAMS =
  /^(password|pass|sslpassword|sslkey|token|secret|api[_-]?key)$/i;

/** URL dont le mot de passe et les paramètres secrets sont remplacés par `***`. */
export function redactUrl(url: string): string {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return url.replace(/:[^:@/]+@/, ":***@");
  }
  if (parsed.password) parsed.password = "***";
  for (const key of Array.from(parsed.searchParams.keys())) {
    if (SECRET_PARAMS.test(key)) parsed.searchParams.set(key, "***");
  }
  return parsed.toString();
}

/** Vrai quand l'URL vise un pooler transactionnel (PgBouncer 6432, Supavisor 6543, Neon `-pooler`). */
export function isPoolerUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    if (parsed.hostname.includes("-pooler.")) return true;
    if (parsed.port === "6432" || parsed.port === "6543") return true;
    const flag = parsed.searchParams.get("pgbouncer");
    return flag === "true" || flag === "1";
  } catch {
    return false;
  }
}

export interface CreateSqlOptions {
  max: number;
  idleTimeout: number;
  maxLifetime: number;
  connectionTimeout: number;
}

/** Pool `Bun.SQL` paresseux ; TLS laissé à l'URL, requêtes préparées coupées derrière un pooler. */
export function createSql(url: string, options: CreateSqlOptions): SQL {
  return new SQL({
    url,
    ...options,
    ...(isPoolerUrl(url) ? { prepare: false } : {}),
  });
}

/** Client `Bun.RedisClient` ; la connexion s'ouvre à la première commande. */
export function createRedisClient(
  url: string,
  options: RedisOptions = {},
): RedisClient {
  return new RedisClient(url, options);
}
