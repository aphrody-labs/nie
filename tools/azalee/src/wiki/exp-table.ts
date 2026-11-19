import { createInagleSql } from "@rosegriffon/db/inagle-sql";

/**
 * Table d'expérience des joueurs : `inagle_exp_table` (100 lignes, niveaux 1 à 100,
 * `need_exp` de 124 à 124 072), dans la base de jeu (inagle).
 *
 * Deux colonnes seulement (`level`, `need_exp`) : rien d'autre n'est exposé.
 */

export interface ExpTableRow {
  level: number | null;
  need_exp: number | null;
}

interface ExpDatabase {
  inagle_exp_table: ExpTableRow;
}

export interface ExpLevelRow {
  level: number;
  needExp: number;
}

/** Lignes valides de la table d'expérience, triées par niveau. Lève en cas d'erreur de lecture. */
export async function getExpTableRows(): Promise<ExpLevelRow[]> {
  const rows = await createInagleSql<ExpDatabase>()
    .selectFrom("inagle_exp_table")
    .select(["level", "need_exp"])
    .orderBy("level", "asc")
    .execute();
  const entries: ExpLevelRow[] = [];
  for (const row of rows) {
    if (typeof row.level === "number" && typeof row.need_exp === "number") {
      entries.push({ level: row.level, needExp: row.need_exp });
    }
  }
  return entries;
}
