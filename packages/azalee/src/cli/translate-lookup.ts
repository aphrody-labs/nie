import {
  getInagleDatabaseURL,
  isInagleDatabaseConfigured,
} from "@rosegriffon/db/pg";
/**
 * Recherche d'entités et traduction assistée par glossaire, pour la commande
 * `azalee translate`.
 *
 * Une seule source de vérité est interrogée : les projections SQL `inagle_*`,
 * PostgreSQL en priorité et le miroir SQLite en repli hors ligne.
 * La traduction elle-même protège les noms propres du jeu derrière des
 * marqueurs avant de passer par le service de traduction, puis les restaure :
 * « Fire Tornado » reste « Tornade de feu », pas « Tornade d'incendie ».
 */

import { Database } from "bun:sqlite";
import translateApi from "google-translate-api-x";
import { isKana, toHiragana, toKatakana } from "wanakana";

import {
  translateJp,
  type TranslateJpResult,
  type TranslateTargetLang,
} from "../text/ievr-i18n";
import {
  containsJapanese,
  stripRubyAnnotations,
} from "../text/japanese-detect";
import { japaneseToRomaji } from "../text/japanese-romaji";
import { createPgClient, getSqlitePath } from "./context";

/** Entité trouvée dans la base ou le glossaire, telle qu'affichée. */
export interface TranslateMatch {
  id: string;
  type: string;
  typeLabel: string;
  name_fr: string | null;
  name_en: string | null;
  name_ja: string | null;
  name_roma: string | null;
  url: string;
}

/** Traduction brute via `google-translate-api-x` (sans fetch/curl maison). */
export async function translateWithGoogle(
  text: string,
  from: string,
  to: string,
): Promise<string> {
  if (!text.trim()) return "";
  try {
    const res = await translateApi(text, { from, to });
    if (res?.text) {
      return res.text;
    }
  } catch (e) {
    console.error("Google Translation API Error:", e);
  }
  return text;
}

/**
 * Cherche l'entité dans PostgreSQL puis, en repli silencieux, dans le miroir
 * SQLite. Les requêtes acceptent kana ↔ romaji via `wanakana`.
 */
export async function searchDatabaseTranslate(
  query: string,
): Promise<TranslateMatch[]> {
  const sanitized = query.replaceAll(/[%_,().*\\]/g, "").trim();
  if (!sanitized) return [];

  const q = `%${sanitized}%`;

  const hira = toHiragana(sanitized.toLowerCase());
  const kata = toKatakana(sanitized.toLowerCase());
  const isValidHira =
    hira.length > 0 && [...hira].every((c) => isKana(c) || c === "ー");
  const isValidKata =
    kata.length > 0 && [...kata].every((c) => isKana(c) || c === "ー");

  // PostgreSQL en premier (requêtes en parallèle + fermeture garantie du socket).
  const dbUrl = isInagleDatabaseConfigured()
    ? getInagleDatabaseURL()
    : undefined;
  if (dbUrl) {
    const client = createPgClient(dbUrl);
    try {
      await client.connect();

      let charSql = `
				SELECT slug as id, 'character' as type, 'Personnage' as "typeLabel",
				       name_fr, name_en, name_ja, name_roma, ('/chara/' || slug) as url
				FROM inagle_characters
				WHERE (name_fr ILIKE $1 OR name_en ILIKE $1 OR name_ja ILIKE $1 OR name_roma ILIKE $1)
				  AND internal_code NOT LIKE '%_5000'
				ORDER BY name_fr ASC LIMIT 10
			`;
      const charParams: unknown[] = [q];
      if (isValidHira && hira !== sanitized.toLowerCase()) {
        charSql = charSql.replace(
          "OR name_roma ILIKE $1",
          `OR name_roma ILIKE $1 OR name_ja ILIKE $2`,
        );
        charParams.push(`%${hira}%`);
      }
      if (isValidKata && kata !== sanitized.toLowerCase()) {
        const paramIndex = charParams.length + 1;
        charSql = charSql.replace(
          "OR name_ja ILIKE $2",
          `OR name_ja ILIKE $2 OR name_ja ILIKE $${paramIndex}`,
        );
        charParams.push(`%${kata}%`);
      }

      const [charRes, skillRes, teamRes, itemRes] = await Promise.all([
        client.query<TranslateMatch>(charSql, charParams),
        client.query<TranslateMatch>(
          `
				SELECT id::text, 'skill' as type, 'Technique' as "typeLabel",
				       name_fr, name_en, name_ja, NULL as name_roma, ('/skill/' || id) as url
				FROM inagle_skills
				WHERE name_fr ILIKE $1 OR name_en ILIKE $1 OR name_ja ILIKE $1
				ORDER BY name_fr ASC LIMIT 10
			`,
          [q],
        ),
        client.query<TranslateMatch>(
          `
				SELECT id::text, 'team' as type, 'Équipe' as "typeLabel",
				       name_fr, name_en, name_ja, NULL as name_roma, ('/team/' || id) as url
				FROM inagle_teams
				WHERE name_fr ILIKE $1 OR name_en ILIKE $1 OR name_ja ILIKE $1
				ORDER BY name_fr ASC LIMIT 10
			`,
          [q],
        ),
        client.query<TranslateMatch>(
          `
				SELECT id::text, 'item' as type, 'Objet' as "typeLabel",
				       name_fr, name_en, name_ja, NULL as name_roma, ('/item/' || id) as url
				FROM inagle_items
				WHERE category != 'special_tactics' AND (name_fr ILIKE $1 OR name_en ILIKE $1 OR name_ja ILIKE $1)
				ORDER BY name_fr ASC LIMIT 10
			`,
          [q],
        ),
      ]);

      return [
        ...charRes.rows,
        ...skillRes.rows,
        ...teamRes.rows,
        ...itemRes.rows,
      ];
    } catch {
      // Échec silencieux : on tente le miroir SQLite.
    } finally {
      await client.end().catch(() => {});
    }
  }

  // Repli miroir SQLite (fermeture garantie en finally).
  const dbPath = getSqlitePath();
  if (dbPath) {
    const db = new Database(dbPath, { readonly: true });
    try {
      const results: TranslateMatch[] = [];

      const queryRun = (sql: string, params: unknown[]): SqliteNameRow[] => {
        const stmt = db.prepare(sql);
        return stmt.all(...(params as never[])) as SqliteNameRow[];
      };

      // Personnages
      let charSql = `
				SELECT id, 'character' as type, 'Personnage' as typeLabel,
				       name_fr, name_en, name_ja, slug,
				       ('/chara/' || slug) as url
				FROM inagle_characters
				WHERE (name_fr LIKE ?1 OR name_en LIKE ?1 OR name_ja LIKE ?1)
				  AND internal_code NOT LIKE '%_5000'
				ORDER BY name_fr ASC LIMIT 10
			`;
      const charParams: unknown[] = [q];
      if (isValidHira && hira !== sanitized.toLowerCase()) {
        charSql = charSql.replace(
          "OR name_ja LIKE ?1",
          `OR name_ja LIKE ?1 OR name_ja LIKE ?2`,
        );
        charParams.push(`%${hira}%`);
      }
      if (isValidKata && kata !== sanitized.toLowerCase()) {
        const paramIndex = charParams.length + 1;
        charSql = charSql.replace(
          "OR name_ja LIKE ?2",
          `OR name_ja LIKE ?2 OR name_ja LIKE ?${paramIndex}`,
        );
        charParams.push(`%${kata}%`);
      }

      for (const c of queryRun(charSql, charParams)) results.push(toMatch(c));

      // Techniques
      const skillSql = `
				SELECT id, 'skill' as type, 'Technique' as typeLabel,
				       name_fr, name_en, name_ja, ('/skill/' || id) as url
				FROM inagle_skills
				WHERE name_fr LIKE ?1 OR name_en LIKE ?1 OR name_ja LIKE ?1
				ORDER BY name_fr ASC LIMIT 10
			`;
      for (const s of queryRun(skillSql, [q])) results.push(toMatch(s));

      // Équipes
      const teamSql = `
				SELECT id, 'team' as type, 'Équipe' as typeLabel,
				       name_fr, name_en, name_ja, ('/team/' || id) as url
				FROM inagle_teams
				WHERE name_fr LIKE ?1 OR name_en LIKE ?1 OR name_ja LIKE ?1
				ORDER BY name_fr ASC LIMIT 10
			`;
      for (const t of queryRun(teamSql, [q])) results.push(toMatch(t));

      // Objets
      const itemSql = `
				SELECT id, 'item' as type, 'Objet' as typeLabel,
				       name_fr, name_en, name_ja, ('/item/' || id) as url
				FROM inagle_items
				WHERE category != 'special_tactics' AND (name_fr LIKE ?1 OR name_en LIKE ?1 OR name_ja LIKE ?1)
				ORDER BY name_fr ASC LIMIT 10
			`;
      for (const i of queryRun(itemSql, [q])) results.push(toMatch(i));

      return results;
    } catch {
      // Miroir illisible : on renvoie ce qu'on a (rien).
    } finally {
      db.close();
    }
  }

  return [];
}

/** Ligne brute du miroir SQLite (colonnes de nommage communes). */
interface SqliteNameRow {
  id: string;
  type: string;
  typeLabel: string;
  name_fr: string | null;
  name_en: string | null;
  name_ja: string | null;
  url: string;
}

/** Normalise une ligne SQLite en `TranslateMatch` (romaji dérivé du japonais). */
function toMatch(row: SqliteNameRow): TranslateMatch {
  return {
    id: row.id,
    type: row.type,
    typeLabel: row.typeLabel,
    name_fr: row.name_fr,
    name_en: row.name_en,
    name_ja: row.name_ja,
    name_roma: row.name_ja ? japaneseToRomaji(row.name_ja) : null,
    url: row.url,
  };
}

interface GlossaryRow {
  id: string;
  type: string;
  typeLabel: string;
  name_fr: string | null;
  name_en: string | null;
  name_ja: string | null;
  url: string;
  sub_type: string | null;
}

interface GlossaryCache {
  rows: GlossaryRow[];
  extraEntities: Array<{
    ja: string;
    fr?: string;
    en?: string;
    category: string;
    hashId: string;
  }>;
  sortedEnToFr: Array<[string, string]>;
}

let glossaryCachePromise: Promise<GlossaryCache> | null = null;

/** Charge et pré-indexe toutes les variantes nommables depuis SQL (PG → SQLite). */
async function loadGlossaryCache(): Promise<GlossaryCache> {
  if (glossaryCachePromise) return glossaryCachePromise;
  glossaryCachePromise = (async (): Promise<GlossaryCache> => {
    const sql = `
			SELECT slug::text AS id, 'character' AS type, 'Personnage' AS "typeLabel",
			       name_fr, name_en, name_ja, ('/chara/' || slug) AS url, NULL::text AS sub_type
			FROM inagle_characters
			WHERE internal_code NOT LIKE '%_5000' AND slug IS NOT NULL
			UNION ALL
			SELECT id::text, 'skill', 'Technique', name_fr, name_en, name_ja,
			       ('/skill/' || id), NULL::text
			FROM inagle_skills
			UNION ALL
			SELECT id::text, 'aura', 'Aura', name_fr, name_en, name_ja,
			       ('/aura/' || id), sub_type
			FROM inagle_souls
			UNION ALL
			SELECT id::text, 'passive', 'Passif', name_fr, name_en, name_ja,
			       ('/skill/' || id), NULL::text
			FROM inagle_passives
			UNION ALL
			SELECT id::text, 'team', 'Équipe', name_fr, name_en, name_ja,
			       ('/team/' || id), NULL::text
			FROM inagle_teams
			UNION ALL
			SELECT id::text, 'item', 'Objet', name_fr, name_en, name_ja,
			       ('/item/' || id), NULL::text
			FROM inagle_items
			WHERE category <> 'special_tactics'
		`;
    let rows: GlossaryRow[] = [];
    const dbUrl = isInagleDatabaseConfigured()
      ? getInagleDatabaseURL()
      : undefined;
    let loadedFromPg = false;
    if (dbUrl) {
      const client = createPgClient(dbUrl);
      try {
        await client.connect();
        const result = await client.query<GlossaryRow>(sql);
        rows = result.rows;
        loadedFromPg = true;
      } catch {
        // Le miroir local prend le relais hors ligne.
      } finally {
        await client.end().catch(() => {});
      }
    }
    if (!loadedFromPg) {
      const dbPath = getSqlitePath();
      if (dbPath) {
        const db = new Database(dbPath, { readonly: true });
        try {
          rows = db
            .prepare(sql.replaceAll(/::text/g, ""))
            .all() as GlossaryRow[];
        } catch {
          rows = [];
        } finally {
          db.close();
        }
      }
    }

    const extraEntities: GlossaryCache["extraEntities"] = [];
    const enReplacements = new Map<string, string>();
    for (const row of rows) {
      const jaClean = row.name_ja?.trim();
      if (jaClean) {
        extraEntities.push({
          ja: stripRubyAnnotations(jaClean),
          fr: row.name_fr ?? undefined,
          en: row.name_en ?? undefined,
          category: row.type,
          hashId: row.id,
        });
      }
      const enVal = row.name_en?.trim();
      const frVal = row.name_fr?.trim();
      if (enVal && frVal && enVal.toLowerCase() !== frVal.toLowerCase()) {
        enReplacements.set(enVal, frVal);
      }
    }
    const sortedEnToFr = [...enReplacements.entries()].sort(
      (a, b) => b[0].length - a[0].length,
    );
    return { rows, extraEntities, sortedEnToFr };
  })().catch((err) => {
    glossaryCachePromise = null;
    throw err;
  });
  return glossaryCachePromise;
}

/** Cherche la requête dans toutes les projections SQL nommables. */
export async function searchGlossaryTranslate(
  query: string,
): Promise<TranslateMatch[]> {
  const sanitized = query
    .replaceAll(/[%_,().*\\]/g, "")
    .trim()
    .toLowerCase();
  if (!sanitized) return [];
  const hiraCandidate = toHiragana(sanitized).toLowerCase();
  const kataCandidate = toKatakana(sanitized).toLowerCase();
  const isValidHira = [...hiraCandidate].every((c) => isKana(c) || c === "ー");
  const isValidKata = [...kataCandidate].every((c) => isKana(c) || c === "ー");
  const { rows } = await loadGlossaryCache();
  const matches: TranslateMatch[] = [];
  for (const row of rows) {
    const names = [row.name_fr, row.name_en, row.name_ja]
      .filter((n): n is string => Boolean(n))
      .map((name) => name.toLowerCase());
    let matched = names.some(
      (name) =>
        name.includes(sanitized) ||
        (isValidHira && name.includes(hiraCandidate)) ||
        (isValidKata && name.includes(kataCandidate)),
    );
    let cachedRoma: string | null | undefined;
    if (!matched && row.name_ja) {
      cachedRoma = japaneseToRomaji(row.name_ja);
      matched = cachedRoma
        ? cachedRoma.toLowerCase().includes(sanitized)
        : false;
    }
    if (!matched) continue;
    const isSoul = row.sub_type?.toLowerCase().includes("soul") ?? false;
    matches.push({
      id: row.id,
      type: isSoul ? "soul" : row.type,
      typeLabel: isSoul ? "Totem" : row.typeLabel,
      name_fr: row.name_fr,
      name_en: row.name_en,
      name_ja: row.name_ja,
      name_roma:
        cachedRoma !== undefined
          ? cachedRoma
          : row.name_ja
            ? japaneseToRomaji(row.name_ja)
            : null,
      url: isSoul ? `/aura/totems/${row.id}` : row.url,
    });
  }
  return matches;
}

/**
 * Traduit un texte vers `targetLang` (`fr` | `en` | `es`) en combinant le
 * dictionnaire IEVR VFS, BudouX, Kuromoji/WanaKana, le glossaire SQL
 * consolidé et `google-translate-api-x`.
 */
export async function translateTextDetailed(
  inputText: string,
  targetLang: TranslateTargetLang = "fr",
): Promise<TranslateJpResult> {
  const { extraEntities, sortedEnToFr } = await loadGlossaryCache();

  if (containsJapanese(inputText)) {
    return await translateJp(inputText, targetLang, { extraEntities });
  }

  // Fallback for English/Latin input with glossary placeholder protection.
  let processed = inputText;
  const restored = new Map<string, string>();
  let idx = 0;
  if (targetLang === "fr") {
    for (const [term, val] of sortedEnToFr) {
      if (!processed.toLowerCase().includes(term.toLowerCase())) continue;
      const token = `__IEVR_${idx}__`;
      const next = processed.replaceAll(term, ` ${token} `);
      if (next !== processed) {
        processed = next;
        restored.set(token, val);
        idx += 1;
      }
    }
  }

  let translated =
    targetLang === "en"
      ? processed
      : await translateWithGoogle(processed, "en", targetLang);
  for (const [token, val] of restored.entries()) {
    translated = translated.replaceAll(token, val);
  }
  translated = translated.replace(/\s+/g, " ").trim();

  return {
    source: inputText,
    sourceLang: "en",
    targetLang,
    translated,
    reading: "",
    romaji: "",
    matchedEntities: [],
    segments: inputText.trim().split(/\s+/).filter(Boolean),
    usedRemote: targetLang !== "en",
  };
}

export async function translateTextHelper(
  inputText: string,
  targetLang: TranslateTargetLang = "fr",
): Promise<string> {
  if (!inputText.trim()) return "";
  const detailed = await translateTextDetailed(inputText, targetLang);
  return detailed.translated;
}
