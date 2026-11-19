/**
 * Repli des `zukan_hash` (identifiant de portrait du zukan officiel, servi par
 * `dxi4wb638ujep.cloudfront.net/1/<hash>.png`).
 *
 * `zukan_hash` n'est pas une donnée du jeu : il vient du crawl de zukan.inazuma.jp.
 * Le push Inagle (`db:sync`) réécrit `inagle_characters` depuis les fichiers du jeu
 * et le remet à NULL : mesuré le 2026-11-05 sur `rg-inagle-pg`, 0 ligne sur 6 111
 * en porte un, contre 5 948 dans l'instantané Supabase `rg` copié le 2026-10-17.
 * Sans lui, chaque portrait du wiki retombe sur la texture `dx11` du CDN, que le VPS
 * ne sait pas décoder (502).
 *
 * `@rosegriffon/inagle/zukan/hashes` = `id` → `zukan_hash` extrait de cet instantané (5 948 lignes,
 * vérifiées sur CloudFront). La base reste prioritaire ; le repli est journalisé
 * une fois par processus pour que le trou de données reste visible.
 */
import ZUKAN_HASHES from "@rosegriffon/inagle/zukan/hashes";
import { resolveZukanCharacterHash } from "@rosegriffon/inagle/zukan/direct";

const HASHES = ZUKAN_HASHES as Record<string, string>;

/** `zukan_hash` en base, sinon celui de l'instantané par `id` ou `internal_code`, sinon `undefined`. */
export function zukanHashOf(row: {
  id?: string | null;
  internal_code?: string | null;
  zukan_hash?: string | null;
}): string | undefined {
  return resolveZukanCharacterHash({
    zukanHash: row.zukan_hash,
    id: row.id,
    internalCode: row.internal_code,
  });
}

/** Ids qui ont un portrait zukan dans l'instantané (pour les filtres SQL `zukan_hash is not null`). */
export function zukanFallbackIds(): string[] {
  return Object.keys(HASHES);
}
