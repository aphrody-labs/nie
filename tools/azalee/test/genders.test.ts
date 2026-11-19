/**
 * Genre d'un joueur : code du jeu (1 = masculin, 2 = féminin) prioritaire sur la
 * colonne texte `gender`, écrite à l'envers par l'ancien import.
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
import { genderLetterSql } from "../src/wiki/genders";
import {
  genderLetterOfGameCode,
  resolveGenderLetter,
} from "../src/wiki/genders-shared";

describe("genderLetterOfGameCode", () => {
  test("1 = masculin, 2 = féminin, le reste n'a pas de genre", () => {
    expect(genderLetterOfGameCode(1)).toBe("M");
    expect(genderLetterOfGameCode("1")).toBe("M");
    expect(genderLetterOfGameCode(2)).toBe("F");
    expect(genderLetterOfGameCode("2")).toBe("F");
    for (const code of [0, 4, 5, "x", null, undefined]) {
      expect(genderLetterOfGameCode(code as number)).toBeNull();
    }
  });
});

describe("resolveGenderLetter", () => {
  test("le code du jeu l'emporte sur la colonne inversée", () => {
    // Mark Evans : code 1 (masculin), colonne 'F' héritée de l'import inversé.
    expect(resolveGenderLetter("F", 1)).toBe("M");
    expect(resolveGenderLetter("M", 2)).toBe("F");
  });

  test("un code sans genre (robot, avatar) ne retombe pas sur la colonne", () => {
    expect(resolveGenderLetter("M", 5)).toBeNull();
    expect(resolveGenderLetter("M", 0)).toBeNull();
  });

  test("sans code du jeu, utilise la colonne", () => {
    expect(resolveGenderLetter("F", null)).toBe("F");
    expect(resolveGenderLetter("M", undefined)).toBe("M");
    expect(resolveGenderLetter("X", null)).toBe("X");
    expect(resolveGenderLetter(null, null)).toBeNull();
    expect(resolveGenderLetter("?", null)).toBeNull();
  });
});

describe("genderLetterSql", () => {
  const db = new Kysely<Record<string, never>>({
    dialect: {
      createAdapter: () => new PostgresAdapter(),
      createDriver: () => new DummyDriver(),
      createIntrospector: (kysely) => new PostgresIntrospector(kysely),
      createQueryCompiler: () => new PostgresQueryCompiler(),
    },
  });

  test("traduit le code du jeu et retombe sur la colonne", () => {
    const text = genderLetterSql.compile(db).sql.replace(/\s+/g, " ");
    expect(text).toContain("when data->>'gender' is not null then");
    expect(text).toContain("when '1' then 'M'");
    expect(text).toContain("when '2' then 'F'");
    expect(text).toContain("else gender end");
  });
});

const suite = describe.skipIf(!hasPostgresConfiguration());

suite("filtre par genre sur la base réelle", () => {
  test("gender=1 ne renvoie que des joueurs masculins, gender=2 que des féminins", async () => {
    const { wikiService } = await import("../src/wiki/service");
    const male = await wikiService.getCharactersList({
      gender: "1",
      limit: 30,
    });
    const female = await wikiService.getCharactersList({
      gender: "2",
      limit: 30,
    });
    expect(male.total).toBeGreaterThan(0);
    expect(female.total).toBeGreaterThan(0);
    for (const c of male.data as Array<Record<string, any>>) {
      expect(c.gender).toBe(0);
    }
    for (const c of female.data as Array<Record<string, any>>) {
      expect(c.gender).toBe(1);
    }
  });
});
