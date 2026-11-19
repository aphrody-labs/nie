import { sql } from "@rosegriffon/db/inagle-sql";

export * from "./genders-shared";

/**
 * Même résolution côté SQL, pour filtrer sur le genre réel. Expression sur la
 * table `inagle_characters` (colonnes `gender` et `data` non qualifiées).
 */
export const genderLetterSql = sql<string | null>`(case
  when data->>'gender' is not null then
    case data->>'gender' when '1' then 'M' when '2' then 'F' else null end
  else gender
end)`;
