/** Prédicat d'environnement sans dépendance à Bun (sûr côté navigateur). */
/** Préfixe d'une charge dotenvx jamais déchiffrée. */
const ENCRYPTED_ENV_PREFIX = "eyJ2Ijo";

/** Vrai pour une valeur utilisable telle quelle : non vide, ni "undefined"/"null" littéraux, ni blob dotenvx scellé. */
export function isUsableEnvValue(
  value: string | undefined | null,
): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value !== "undefined" &&
    value !== "null" &&
    !value.startsWith(ENCRYPTED_ENV_PREFIX)
  );
}
