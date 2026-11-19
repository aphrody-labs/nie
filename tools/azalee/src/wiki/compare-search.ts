import { createInagleSql, sql } from "@rosegriffon/db/inagle-sql";
import { positionLabelSql } from "./positions";
import { POSITION_LABEL_BY_CODE, type PositionCode } from "./positions-shared";

/**
 * Recherche de joueurs du comparateur (uniquement ceux qui ont des stats).
 *
 * Les filtres de l'interface arrivent en codes (`Fire`, `GK`, `0`/`2`/`10`/`20`)
 * alors que la base stocke des libellés français (`Feu`, `Gardien`, `Normal`…) :
 * l'ancienne requête comparait les codes bruts aux libellés et ne filtrait donc
 * presque rien. Le poste passe par la même résolution que le reste du wiki
 * (`positionLabelSql`, la colonne valant `Inconnu` pour ~60 % des lignes).
 */

const ELEMENT_LABEL_BY_CODE: Record<string, string> = {
  Fire: "Feu",
  Forest: "Forêt",
  Mountain: "Montagne",
  Wind: "Vent",
};

const RARITY_LABEL_BY_CODE: Record<string, string> = {
  "0": "Normal",
  "1": "Normal",
  "10": "Héros",
  "2": "Expérimenté",
  "20": "BASARA",
  BASARA: "BASARA",
  Normal: "Normal",
  Expérimenté: "Expérimenté",
  Héros: "Héros",
};

export interface CompareSearchFilters {
  element?: string;
  position?: string;
  rarity?: string;
}

export interface ResolvedCompareFilters {
  element?: string;
  position?: string;
  rarity?: string;
}

/** Code ou libellé d'interface -> libellé français de la base (`undefined` si inconnu). */
export function resolveCompareFilters(
  filters: CompareSearchFilters | undefined,
): ResolvedCompareFilters {
  const out: ResolvedCompareFilters = {};
  const element = filters?.element;
  if (element) {
    out.element =
      ELEMENT_LABEL_BY_CODE[element] ??
      (Object.values(ELEMENT_LABEL_BY_CODE).includes(element)
        ? element
        : undefined);
  }
  const position = filters?.position;
  if (position) {
    out.position =
      POSITION_LABEL_BY_CODE[position as PositionCode] ??
      (Object.values(POSITION_LABEL_BY_CODE).includes(
        position as (typeof POSITION_LABEL_BY_CODE)[PositionCode],
      )
        ? position
        : undefined);
  }
  const rarity = filters?.rarity;
  if (rarity) out.rarity = RARITY_LABEL_BY_CODE[rarity];
  return out;
}

export interface CompareSearchRow {
  id: string;
  slug: string | null;
  name_fr: string | null;
  name_en: string | null;
  name_ja: string | null;
  position: string | null;
  /** Code du jeu (`GK`/`DF`/`MF`/`FW`), `data->>'position'`. */
  data_position: string | null;
  element: string | null;
  rarity_label: string | null;
  series: string | null;
  internal_code: string | null;
  zukan_hash: string | null;
  stat_frappe: number | null;
  stat_controle: number | null;
  stat_technique: number | null;
  stat_pression: number | null;
  stat_physique: number | null;
  stat_agilite: number | null;
  stat_intelligence: number | null;
}

interface CompareDatabase {
  inagle_characters: CompareSearchRow;
}

export async function searchCharactersWithStats(options: {
  /** Motifs `ilike` déjà préparés (`%mario%`) : un nom correspondant à l'un d'eux. */
  namePatterns?: string[];
  /** Motifs `ilike` appliqués au seul nom japonais (variante kana). */
  japanesePatterns?: string[];
  filters?: CompareSearchFilters;
  limit?: number;
}): Promise<CompareSearchRow[]> {
  const db = createInagleSql<CompareDatabase>();
  const predicates = [sql<boolean>`stat_frappe is not null`];

  const textClauses = [
    ...(options.namePatterns ?? []).map(
      (p) =>
        sql<boolean>`(name_fr ilike ${p} or name_en ilike ${p} or name_ja ilike ${p})`,
    ),
    ...(options.japanesePatterns ?? []).map(
      (p) => sql<boolean>`name_ja ilike ${p}`,
    ),
  ];
  if (textClauses.length > 0) {
    predicates.push(sql<boolean>`(${sql.join(textClauses, sql` or `)})`);
  }

  const resolved = resolveCompareFilters(options.filters);
  if (options.filters?.element) {
    predicates.push(
      resolved.element
        ? sql<boolean>`element = ${resolved.element}`
        : sql<boolean>`false`,
    );
  }
  if (options.filters?.position) {
    predicates.push(
      resolved.position
        ? sql<boolean>`${positionLabelSql} = ${resolved.position}`
        : sql<boolean>`false`,
    );
  }
  if (options.filters?.rarity) {
    predicates.push(
      resolved.rarity
        ? sql<boolean>`rarity_label = ${resolved.rarity}`
        : sql<boolean>`false`,
    );
  }

  return db
    .selectFrom("inagle_characters")
    .select([
      "id",
      "slug",
      "name_fr",
      "name_en",
      "name_ja",
      "position",
      sql<string | null>`data->>'position'`.as("data_position"),
      "element",
      "rarity_label",
      "series",
      "internal_code",
      "zukan_hash",
      "stat_frappe",
      "stat_controle",
      "stat_technique",
      "stat_pression",
      "stat_physique",
      "stat_agilite",
      "stat_intelligence",
    ])
    .where(sql<boolean>`${sql.join(predicates, sql` and `)}`)
    .orderBy("stat_frappe", (ob) => ob.desc().nullsLast())
    .limit(options.limit ?? 30)
    .execute();
}
