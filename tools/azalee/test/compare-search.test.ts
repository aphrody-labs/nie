/**
 * Recherche du comparateur : filtres d'interface (codes) -> libellés de la base,
 * poste résolu comme le reste du wiki.
 */

import { describe, expect, test } from "bun:test";

import { hasPostgresConfiguration } from "../src/config";
import {
  resolveCompareFilters,
  searchCharactersWithStats,
} from "../src/wiki/compare-search";

describe("resolveCompareFilters", () => {
  test("traduit les codes d'interface en libellés de la base", () => {
    expect(
      resolveCompareFilters({ element: "Fire", position: "GK", rarity: "10" }),
    ).toEqual({ element: "Feu", position: "Gardien", rarity: "Héros" });
    expect(
      resolveCompareFilters({ element: "Wind", position: "MF", rarity: "0" }),
    ).toEqual({
      element: "Vent",
      position: "Milieu",
      rarity: "Normal",
    });
    expect(resolveCompareFilters({ rarity: "2" }).rarity).toBe("Expérimenté");
    expect(resolveCompareFilters({ rarity: "20" }).rarity).toBe("BASARA");
    expect(resolveCompareFilters({ rarity: "BASARA" }).rarity).toBe("BASARA");
  });

  test("accepte déjà un libellé français et rejette l'inconnu", () => {
    expect(resolveCompareFilters({ element: "Feu" }).element).toBe("Feu");
    expect(resolveCompareFilters({ position: "Attaquant" }).position).toBe(
      "Attaquant",
    );
    expect(resolveCompareFilters({ element: "Void" }).element).toBeUndefined();
    expect(resolveCompareFilters({ position: "ZZ" }).position).toBeUndefined();
    expect(resolveCompareFilters(undefined)).toEqual({});
  });
});

const suite = describe.skipIf(!hasPostgresConfiguration());

suite("searchCharactersWithStats sur la base réelle", () => {
  test("chaque filtre restreint réellement le résultat", async () => {
    const all = await searchCharactersWithStats({ limit: 30 });
    expect(all.length).toBeGreaterThan(0);

    const fire = await searchCharactersWithStats({
      filters: { element: "Fire" },
    });
    expect(fire.length).toBeGreaterThan(0);
    for (const row of fire) expect(row.element).toBe("Feu");

    // Toute rareté demandée ne renvoie que cette rareté (la base réelle n'a des
    // stats que pour les « Normal » : « Héros » peut légitimement être vide).
    const normal = await searchCharactersWithStats({
      filters: { rarity: "0" },
    });
    expect(normal.length).toBeGreaterThan(0);
    for (const row of normal) expect(row.rarity_label).toBe("Normal");
    const heroes = await searchCharactersWithStats({
      filters: { rarity: "10" },
    });
    for (const row of heroes) expect(row.rarity_label).toBe("Héros");

    const gk = await searchCharactersWithStats({ filters: { position: "GK" } });
    expect(gk.length).toBeGreaterThan(0);
    for (const row of gk) {
      // Colonne renseignée, ou colonne `Inconnu` + code du jeu GK.
      expect(row.position === "Gardien" || row.data_position === "GK").toBe(
        true,
      );
    }
  });

  test("combine texte et filtre, et un filtre inconnu ne renvoie rien", async () => {
    const rows = await searchCharactersWithStats({
      filters: { element: "Fire" },
      namePatterns: ["%axel%"],
    });
    expect(rows.length).toBeGreaterThan(0);
    for (const row of rows) {
      expect(`${row.name_fr} ${row.name_en}`.toLowerCase()).toContain("axel");
    }
    expect(
      await searchCharactersWithStats({ filters: { element: "Void" } }),
    ).toEqual([]);
  });
});
