/**
 * Genre d'un joueur.
 *
 * Source de vérité : le code du jeu (`CHARA_BASE_INFO` var[11], conservé dans
 * `inagle_characters.data->>'gender'`) : 1 = masculin, 2 = féminin, autre
 * (0, 4, 5 : avatars, robots, créatures) = pas de genre. Vérifié sur les données :
 * 1 = Mark Evans, Nathan Swift, Shawn Froste ; 2 = Adelina Gearhart, Alexia Sand.
 *
 * La colonne texte `gender` ('M' / 'F') a été écrite à l'envers par l'ancien import
 * (`gender === 1 ? "F" : "M"`, corrigé dans `cli-push.ts` mais pas encore rejoué
 * en base, cf. scripts/plan-ievr-gender-prod.ts) : tant que ce n'est pas fait, la
 * colonne ne doit pas être lue seule. `resolveGenderLetter` donne priorité au code
 * du jeu, et retombe sur la colonne quand il est absent.
 */
export type GenderLetter = "M" | "F" | "X";

/** Code genre du jeu → lettre. Tout code hors 1 et 2 n'a pas de genre (`null`). */
export function genderLetterOfGameCode(
  code: number | string | null | undefined,
): "M" | "F" | null {
  const n = typeof code === "string" ? Number.parseInt(code, 10) : code;
  if (n === 1) return "M";
  if (n === 2) return "F";
  return null;
}

/**
 * Lettre de genre d'une ligne : le code du jeu s'il existe (même quand il ne
 * désigne aucun genre), sinon la colonne texte.
 */
export function resolveGenderLetter(
  column: string | null | undefined,
  dataGender: number | string | null | undefined,
): GenderLetter | null {
  if (dataGender !== null && dataGender !== undefined && dataGender !== "") {
    return genderLetterOfGameCode(dataGender);
  }
  return column === "M" || column === "F" || column === "X" ? column : null;
}
