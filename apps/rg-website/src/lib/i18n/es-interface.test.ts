import { describe, expect, test } from "bun:test";
import esInterface from "./es-interface.json";
import frInterface from "./fr-interface.json";
import esLocale from "../../../public/locales/es.json";

describe("es-interface dictionary", () => {
  test("covers every key in fr-interface.json with non-empty Spanish strings", () => {
    const frKeys = Object.keys(frInterface);
    const esRecord = esInterface as Record<string, string>;
    const publicEs = esLocale as Record<string, string>;

    for (const key of frKeys) {
      expect(typeof esRecord[key]).toBe("string");
      expect(esRecord[key].trim().length).toBeGreaterThan(0);
      expect(publicEs[key]).toBe(esRecord[key]);
      expect(esRecord[key]).not.toContain("\u2014");
    }

    expect(Object.keys(esRecord).length).toBe(frKeys.length);
  });

  test("preserves ICU placeholders between fr and es", () => {
    const frRecord = frInterface as Record<string, string>;
    const esRecord = esInterface as Record<string, string>;

    for (const [key, frValue] of Object.entries(frRecord)) {
      const frPlaceholders = frValue.match(/\{[a-zA-Z0-9_]+\}/g) ?? [];
      const esPlaceholders = esRecord[key].match(/\{[a-zA-Z0-9_]+\}/g) ?? [];
      expect(esPlaceholders).toEqual(frPlaceholders);
    }
  });
});
