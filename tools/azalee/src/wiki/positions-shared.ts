/**
 * Postes d'un joueur, du code court des données du jeu (`data.position`) au
 * libellé français stocké dans `inagle_characters.position`.
 *
 * La colonne `position` n'est renseignée que pour une partie des lignes : environ
 * 6 100 sur 10 000 portent la valeur `Inconnu` alors que le vrai poste est présent
 * dans `data->>'position'` (`GK` / `DF` / `MF` / `FW`). Tout ce qui lit ou filtre
 * le poste doit donc passer par ce module, jamais par la colonne seule.
 */
export const POSITION_LABEL_BY_CODE = {
  DF: "Défenseur",
  FW: "Attaquant",
  GK: "Gardien",
  MF: "Milieu",
} as const;

export type PositionCode = keyof typeof POSITION_LABEL_BY_CODE;

const POSITION_LABELS: ReadonlySet<string> = new Set(
  Object.values(POSITION_LABEL_BY_CODE),
);

/** Code court (`GK`…) correspondant à un libellé français, sinon `undefined`. */
export function positionCodeOf(
  label: string | null | undefined,
): PositionCode | undefined {
  for (const [code, name] of Object.entries(POSITION_LABEL_BY_CODE)) {
    if (name === label) return code as PositionCode;
  }
  return undefined;
}

/**
 * Libellé français du poste d'une ligne : la colonne `position` quand elle porte
 * un vrai poste, sinon le libellé déduit du code des données du jeu. Sans aucune
 * des deux sources, renvoie la colonne telle quelle (`Inconnu`, `Coach`…).
 */
export function resolvePositionLabel(
  position: string | null | undefined,
  dataPosition: string | null | undefined,
): string | null {
  if (position && POSITION_LABELS.has(position)) return position;
  const fromCode =
    POSITION_LABEL_BY_CODE[
      String(dataPosition ?? "").toUpperCase() as PositionCode
    ];
  return fromCode ?? position ?? null;
}
