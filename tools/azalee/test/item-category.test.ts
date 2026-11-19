import { describe, expect, test } from "bun:test";

import { normalizeItemCategory } from "../src/wiki/item-category";

describe("normalizeItemCategory", () => {
  test("ramène les graphies de la base en snake_case", () => {
    expect(normalizeItemCategory("Shoes")).toBe("shoes");
    expect(normalizeItemCategory("NamePlate")).toBe("name_plate");
    expect(normalizeItemCategory("KizunaLink")).toBe("kizuna_link");
    expect(normalizeItemCategory("CraftObj")).toBe("craft_obj");
    expect(normalizeItemCategory("SpecialTactics")).toBe("special_tactics");
    expect(normalizeItemCategory("SuperTactics")).toBe("super_tactics");
  });

  test("laisse intactes les clés déjà normalisées et gère le vide", () => {
    expect(normalizeItemCategory("special_tactics")).toBe("special_tactics");
    expect(normalizeItemCategory("important")).toBe("important");
    expect(normalizeItemCategory("")).toBe("");
    expect(normalizeItemCategory(null)).toBe("");
    expect(normalizeItemCategory(undefined)).toBe("");
  });
});
