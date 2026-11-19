import { afterEach, describe, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  ZUKAN_CDN_BASE,
  resolveZukanCharacterHash,
  resolveZukanFaceUrl,
  zukanAssetUrl,
} from "./direct.js";
import {
  cfgbinDbClose,
  cfgbinDbFindLatest,
  cfgbinDbList,
  cfgbinDbLoad,
} from "../core/cfgbin-db.js";

describe("Zukan direct URL resolution", () => {
  test("resolves featured characters by internal_code and strips variant suffixes", () => {
    expect(resolveZukanFaceUrl({ internalCode: "c01000010" })).toBe(
      `${ZUKAN_CDN_BASE}/k/d/w/dwho-wi8ruk.png`,
    );
    expect(resolveZukanFaceUrl({ internalCode: "c05028130_5000" })).toBe(
      `${ZUKAN_CDN_BASE}/k/b/v/bvrfyjqtgu8.png`,
    );
    expect(resolveZukanFaceUrl({ internalCode: "c99999999" })).toBeNull();
  });

  test("prefers explicit DB zukan_hash and formats raw hashes or URLs", () => {
    expect(
      resolveZukanCharacterHash({
        zukanHash: "k/x/y/custom_override",
        internalCode: "c01000010",
      }),
    ).toBe("k/x/y/custom_override");
    expect(zukanAssetUrl("k/x/y/custom_override", "png")).toBe(
      `${ZUKAN_CDN_BASE}/k/x/y/custom_override.png`,
    );
    expect(
      zukanAssetUrl("https://dxi4wb638ujep.cloudfront.net/1/test.png"),
    ).toBe("https://dxi4wb638ujep.cloudfront.net/1/test.png");
  });
});

describe("cfgbin-db Windows statement finalization", () => {
  let tmpDir: string | null = null;
  const previousPath = process.env.IEVR_CFGBIN_DB;

  afterEach(() => {
    cfgbinDbClose();
    if (previousPath === undefined) delete process.env.IEVR_CFGBIN_DB;
    else process.env.IEVR_CFGBIN_DB = previousPath;
    if (tmpDir) {
      rmSync(tmpDir, { recursive: true, force: true });
      tmpDir = null;
    }
  });

  test("finalizes prepared statements in cfgbinDbClose so SQLite files unlock on Windows", () => {
    tmpDir = mkdtempSync(join(tmpdir(), "inagle-cfgbin-close-"));
    const dbPath = join(tmpDir, "cfgbin.sqlite");
    const db = new Database(dbPath, { create: true });
    try {
      db.exec(`
        CREATE TABLE cfgbin (
          path TEXT PRIMARY KEY,
          cpk TEXT NOT NULL,
          size INTEGER NOT NULL,
          parsed INTEGER NOT NULL,
          format TEXT,
          error TEXT,
          json TEXT
        );
      `);
      const insert = db.prepare(
        "INSERT INTO cfgbin (path, cpk, size, parsed, format, error, json) VALUES (?, ?, ?, ?, ?, ?, ?)",
      );
      try {
        insert.run(
          "data/common/gamedata/character/chara_param_1.00.cfg.bin",
          "cpk_0",
          128,
          1,
          "T2B",
          null,
          JSON.stringify({ ok: true }),
        );
      } finally {
        insert.finalize();
      }
    } finally {
      db.close();
    }

    process.env.IEVR_CFGBIN_DB = dbPath;
    expect(cfgbinDbFindLatest("character", "chara_param_")).toBe(
      "chara_param_1.00.cfg.bin.json",
    );
    expect(cfgbinDbLoad("character", "chara_param_1.00.cfg.bin")).toEqual({
      ok: true,
    });
    expect(cfgbinDbList("data/common/gamedata/character/%")).toEqual([
      "data/common/gamedata/character/chara_param_1.00.cfg.bin",
    ]);

    // Must finalize _findStmt/_loadStmt/_listStmt before closing the DB so Windows
    // permits deleting the SQLite file in afterEach() without SQLITE_BUSY/EBUSY.
    cfgbinDbClose();
    rmSync(tmpDir, { recursive: true, force: true });
    tmpDir = null;
  });
});
