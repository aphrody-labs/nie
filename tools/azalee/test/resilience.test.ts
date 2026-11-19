import { describe, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

describe("azalee build/offline resilience & Windows SQLite cleanup", () => {
  test("wiki readers degrade gracefully when Postgres endpoints refuse connections", async () => {
    const wikiDir = join(import.meta.dir, "..", "src", "wiki");
    const script = `
      process.env.INAGLE_DATABASE_URL = "postgresql://u:p@127.0.0.1:1/inagle";
      process.env.RG_DATABASE_URL = "postgresql://u:p@127.0.0.1:1/rg";
      process.env.DATABASE_URL = "postgresql://u:p@127.0.0.1:1/rg";
      const { wikiService } = await import(${JSON.stringify(pathToFileURL(join(wikiDir, "service.ts")).href)});
      const { getDropsData } = await import(${JSON.stringify(pathToFileURL(join(wikiDir, "drops.ts")).href)});
      const { getTeamsList } = await import(${JSON.stringify(pathToFileURL(join(wikiDir, "teams.ts")).href)});
      const { getCoachesList } = await import(${JSON.stringify(pathToFileURL(join(wikiDir, "coaches.ts")).href)});
      const [slugs, drops, teams, coaches, tweets, passives, galleryCounts] = await Promise.all([
        wikiService.getAllBaseSlugParams(),
        getDropsData(),
        getTeamsList(),
        getCoachesList(),
        wikiService.getTweets(),
        wikiService.getTeamPassives(),
        wikiService.getGalleryCategoryCounts(),
      ]);
      console.log(JSON.stringify({
        slugs: Array.isArray(slugs) ? slugs.length : -1,
        drops: drops.counts.concreteDrops,
        teams: teams.length,
        coaches: coaches.length,
        tweets: tweets.length,
        passives: passives.length,
        galleryAll: galleryCounts.all,
      }));
    `;

    const child = Bun.spawn({
      cmd: [process.execPath, "-e", script],
      cwd: join(import.meta.dir, ".."),
      stdout: "pipe",
      stderr: "pipe",
    });
    const [stdout, _stderr, exitCode] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);

    expect(exitCode).toBe(0);
    const parsed = JSON.parse(stdout.trim());
    expect(parsed).toEqual({
      slugs: 0,
      drops: 0,
      teams: 0,
      coaches: 0,
      tweets: 0,
      passives: 0,
      galleryAll: 0,
    });
  });

  test("createSqliteClient executes select & exact count cleanly and unlocks file on exit", async () => {
    const tmpDir = mkdtempSync(join(tmpdir(), "azalee-sqlite-client-"));
    try {
      const dbPath = join(tmpDir, "mirror.sqlite");
      const db = new Database(dbPath, { create: true });
      try {
        db.exec(`
          CREATE TABLE inagle_characters (
            id TEXT PRIMARY KEY,
            name_fr TEXT,
            zukan_order TEXT,
            is_primary INTEGER
          );
        `);
        const insert = db.prepare(
          "INSERT INTO inagle_characters (id, name_fr, zukan_order, is_primary) VALUES (?, ?, ?, ?)",
        );
        try {
          insert.run("0x01", "Mark Evans", "0", 1);
          insert.run("0x02", "Axel Blaze", "\\N", 1);
        } finally {
          insert.finalize();
        }
      } finally {
        db.close();
      }

      const configUrl = pathToFileURL(
        join(import.meta.dir, "..", "src", "config.ts"),
      ).href;
      const clientUrl = pathToFileURL(
        join(import.meta.dir, "..", "src", "db", "sqlite-client.ts"),
      ).href;
      const script = `
        const { configureAzalee } = await import(${JSON.stringify(configUrl)});
        const { createSqliteClient } = await import(${JSON.stringify(clientUrl)});
        configureAzalee({ mirrorPath: ${JSON.stringify(dbPath)} });
        const client = createSqliteClient();
        const res = await client
          .from("inagle_characters")
          .select("id,name_fr,zukan_order", { count: "exact" })
          .eq("is_primary", 1);
        console.log(JSON.stringify(res));
      `;

      const child = Bun.spawn({
        cmd: [process.execPath, "-e", script],
        cwd: join(import.meta.dir, ".."),
        stdout: "pipe",
        stderr: "pipe",
      });
      const [stdout, stderr, exitCode] = await Promise.all([
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
        child.exited,
      ]);

      expect(stderr).toBe("");
      expect(exitCode).toBe(0);
      const res = JSON.parse(stdout.trim());
      expect(res.count).toBe(2);
      expect(res.data).toEqual([
        { id: "0x01", name_fr: "Mark Evans", zukan_order: 0 },
        { id: "0x02", name_fr: "Axel Blaze", zukan_order: null },
      ]);
    } finally {
      rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});
