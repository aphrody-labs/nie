// bun test crates/rg-ffi/bindings/rg_ffi.test.ts   (RG_FFI_LIB = path of librg_ffi.so/.dylib/.dll)
import { expect, test } from "bun:test";

import { expectedScore, progressiveEloDelta } from "../../../packages/achillea-core/src/elo/index.ts";
import { openRgFfi } from "./rg_ffi";

const path = process.env.RG_FFI_LIB;

test.skipIf(!path)("the Rust library answers like the TypeScript rules", () => {
  const ffi = openRgFfi(path!);
  for (const [p, o] of [[1000, 1000], [0, 400], [1500, 900]] as const) {
    expect(ffi.eloExpectedScore(p, o)).toBeCloseTo(expectedScore(p, o), 12);
    for (const a of [0, 0.5, 1])
      for (const tier of [0, 4, 10])
        expect(ffi.eloProgressiveDelta(p, o, a, tier, 32)).toBe(progressiveEloDelta(p, o, a, tier, 32));
  }
  expect(ffi.tournamentPlacementPrizeAp(32, 1)).toBe(50);
  expect(ffi.bracketSingleEliminationRounds(6)).toBe(3);
  expect(ffi.bracketByeCount(6)).toBe(2);
  expect(ffi.clanCanActOnMember(3, 2)).toBe(true);
  expect(ffi.clanCanActOnMember(2, 2)).toBe(false);
  expect(ffi.sitemapEncodePath("data/a b/c&d.png")).toBe("data/a%20b/c%26d.png");
  expect(ffi.cfgbinToSql(new TextEncoder().encode("not a cfg.bin"), 0)).toBeNull();
  ffi.close();
});

test.skipIf(!path)("a real cfg.bin becomes SQL through the C ABI", async () => {
  const sample = process.env.RG_FFI_CFGBIN;
  if (!sample) return;
  const ffi = openRgFfi(path!);
  const bytes = new Uint8Array(await Bun.file(sample).arrayBuffer());
  const sql = ffi.cfgbinToSql(bytes, 1);
  expect(sql).not.toBeNull();
  expect(sql!).toContain("CREATE TABLE");
  expect(sql!).toContain("INSERT INTO");
  ffi.close();
});
