import { describe, expect, test } from "bun:test";

import { hasPostgresConfiguration } from "../src/config";

const suite = describe.skipIf(!hasPostgresConfiguration());

suite("table d'expérience sur la base réelle", () => {
  test("100 niveaux, croissants, du niveau 1 à 100", async () => {
    const { getExpTableRows } = await import("../src/wiki/exp-table");
    const rows = await getExpTableRows();
    expect(rows.length).toBe(100);
    expect(rows[0]).toEqual({ level: 1, needExp: 124 });
    expect(rows.at(-1)?.level).toBe(100);
    for (let i = 1; i < rows.length; i++) {
      expect(rows[i]!.level).toBe(rows[i - 1]!.level + 1);
      expect(rows[i]!.needExp).toBeGreaterThanOrEqual(rows[i - 1]!.needExp);
    }
  });
});
