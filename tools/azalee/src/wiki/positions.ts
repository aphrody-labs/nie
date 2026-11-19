import { sql } from "@rosegriffon/db/inagle-sql";

export * from "./positions-shared";

/**
 * Même résolution côté SQL, pour filtrer sur le poste réel. Expression sur la
 * table `inagle_characters` (colonnes `position` et `data` non qualifiées).
 */
export const positionLabelSql = sql<string>`(case
  when position in ('Gardien', 'Défenseur', 'Milieu', 'Attaquant') then position
  when data->>'position' = 'GK' then 'Gardien'
  when data->>'position' = 'DF' then 'Défenseur'
  when data->>'position' = 'MF' then 'Milieu'
  when data->>'position' = 'FW' then 'Attaquant'
  else position
end)`;
