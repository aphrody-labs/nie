/**
 * Chemins de la racine du dépôt, des checkouts voisins et des dossiers d'état.
 * Toute surcharge passe par une variable RG_* (voir `.env.example`).
 */
import { existsSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const env = (...names: string[]): string | undefined => {
  for (const name of names) {
    const value = process.env[name]?.trim();
    if (value) return value;
  }
  return undefined;
};

/** Racine du dépôt : `RG_REPO_ROOT`, sinon premier parent contenant `turbo.json` et `package.json`. */
export function repoRoot(from: string = process.cwd()): string {
  const explicit = env("RG_REPO_ROOT");
  if (explicit) return resolve(explicit);
  let current = resolve(from);
  while (true) {
    if (existsSync(join(current, "turbo.json")) && existsSync(join(current, "package.json"))) {
      return current;
    }
    const parent = dirname(current);
    if (parent === current) throw new Error(`Racine du dépôt introuvable au-dessus de ${from}`);
    current = parent;
  }
}

export type Checkout = "rg" | "iecode";

/** Racine d'un checkout : `rg` = ce dépôt ; `iecode` = `RG_IECODE_ROOT`, sinon le dossier voisin `iecode`. */
export function checkoutRoot(checkout: Checkout): string {
  if (checkout === "rg") return repoRoot();
  const explicit = env("RG_IECODE_ROOT", "IECODE_ROOT", "NIE_ROOT");
  return explicit ? resolve(explicit) : resolve(dirname(repoRoot()), "iecode");
}

export function workspacePath(...parts: string[]): string {
  return resolve(repoRoot(), ...parts);
}

/** Données du jeu : `DATA_PATH`, `DATA_ROOT`, `NIE_DATA_ROOT`, sinon `<iecode>/data`. */
export function iecodeDataPath(...parts: string[]): string {
  const base = env("DATA_PATH", "DATA_ROOT", "NIE_DATA_ROOT") ?? join(checkoutRoot("iecode"), "data");
  return resolve(base, ...parts);
}

export function homePath(...parts: string[]): string {
  return resolve(homedir(), ...parts);
}

export function tempPath(...parts: string[]): string {
  return resolve(env("RG_TMP_DIR") ?? tmpdir(), ...parts);
}

/** Dossier d'état : `RG_HOME`, sinon `~/.rosegriffon`. */
export function statePath(...parts: string[]): string {
  return resolve(env("RG_HOME") ?? homePath(".rosegriffon"), ...parts);
}
