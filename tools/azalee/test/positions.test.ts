/**
 * Poste d'un joueur : la colonne `inagle_characters.position` vaut `Inconnu` pour
 * ~60 % des lignes alors que le code du jeu est dans `data.position`. Ces tests
 * figent la résolution (pure et SQL) et, si une base est configurée, vérifient
 * que le filtre par poste et les pools d'équipe aléatoire ne sont plus vides.
 */

import { describe, expect, test } from "bun:test";
import {
  DummyDriver,
  Kysely,
  PostgresAdapter,
  PostgresIntrospector,
  PostgresQueryCompiler,
} from "kysely";

import { hasPostgresConfiguration } from "../src/config";
import { positionLabelSql } from "../src/wiki/positions";
import {
  POSITION_LABEL_BY_CODE,
  positionCodeOf,
  resolvePositionLabel,
} from "../src/wiki/positions-shared";

describe("resolvePositionLabel", () => {
  test("garde un libellé de poste valide de la colonne", () => {
    expect(resolvePositionLabel("Gardien", null)).toBe("Gardien");
    // La colonne prime sur le code quand elle est renseignée.
    expect(resolvePositionLabel("Milieu", "GK")).toBe("Milieu");
  });

  test("déduit le poste du code `data.position` quand la colonne vaut Inconnu", () => {
    expect(resolvePositionLabel("Inconnu", "GK")).toBe("Gardien");
    expect(resolvePositionLabel("Inconnu", "DF")).toBe("Défenseur");
    expect(resolvePositionLabel("Inconnu", "MF")).toBe("Milieu");
    expect(resolvePositionLabel("Inconnu", "FW")).toBe("Attaquant");
    expect(resolvePositionLabel(null, "fw")).toBe("Attaquant");
  });

  test("sans aucune source exploitable, renvoie la colonne telle quelle", () => {
    expect(resolvePositionLabel("Inconnu", null)).toBe("Inconnu");
    expect(resolvePositionLabel("Inconnu", "ZZ")).toBe("Inconnu");
    expect(resolvePositionLabel("Coach", null)).toBe("Coach");
    expect(resolvePositionLabel(null, null)).toBeNull();
  });
});

describe("positionCodeOf", () => {
  test("inverse la table code -> libellé", () => {
    for (const [code, label] of Object.entries(POSITION_LABEL_BY_CODE)) {
      expect(positionCodeOf(label)).toBe(
        code as keyof typeof POSITION_LABEL_BY_CODE,
      );
    }
    expect(positionCodeOf("Inconnu")).toBeUndefined();
    expect(positionCodeOf(null)).toBeUndefined();
  });
});

describe("positionLabelSql", () => {
  const db = new Kysely<Record<string, never>>({
    dialect: {
      createAdapter: () => new PostgresAdapter(),
      createDriver: () => new DummyDriver(),
      createIntrospector: (kysely) => new PostgresIntrospector(kysely),
      createQueryCompiler: () => new PostgresQueryCompiler(),
    },
  });

  test("traduit chaque code du jeu et retombe sur la colonne", () => {
    const compiled = positionLabelSql.compile(db);
    const text = compiled.sql.replace(/\s+/g, " ");
    expect(text).toContain(
      "when position in ('Gardien', 'Défenseur', 'Milieu', 'Attaquant') then position",
    );
    for (const [code, label] of Object.entries(POSITION_LABEL_BY_CODE)) {
      expect(text).toContain(
        `when data->>'position' = '${code}' then '${label}'`,
      );
    }
    expect(text).toContain("else position end");
    expect(compiled.parameters).toEqual([]);
  });
});

const suite = describe.skipIf(!hasPostgresConfiguration());

suite("filtre par poste sur la base réelle", () => {
  test("chaque poste renvoie des joueurs, y compris ceux dont la colonne vaut Inconnu", async () => {
    const { wikiService } = await import("../src/wiki/service");
    for (const [code, label] of Object.entries(POSITION_LABEL_BY_CODE)) {
      const res = await wikiService.getCharactersList({
        limit: 10,
        position: code,
      });
      expect(res.total).toBeGreaterThan(0);
      for (const chara of res.data as Array<Record<string, any>>) {
        expect(chara.variants[0].position).toBe(label);
      }
    }
  });

  test("les pools d'équipe aléatoire ne sont pas vides", async () => {
    const { wikiService } = await import("../src/wiki/service");
    const pools = await wikiService.getRandomTeamPools();
    expect(pools.gk.length).toBeGreaterThan(0);
    expect(pools.df.length).toBeGreaterThan(0);
    expect(pools.mf.length).toBeGreaterThan(0);
    expect(pools.fw.length).toBeGreaterThan(0);
  });
});
