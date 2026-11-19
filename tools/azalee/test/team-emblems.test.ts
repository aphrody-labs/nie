/**
 * Emblèmes d'équipe : `emblemId` du jeu = CRC32 du nom de fichier d'emblème.
 */

import { describe, expect, test } from "bun:test";

import { hasPostgresConfiguration } from "../src/config";
import { emblemNameOfCrc, getEmblemImageUrlByCrc } from "../src/images/utils";

describe("emblemNameOfCrc", () => {
  test("inverse le CRC32 sur les noms d'emblèmes du CPK", () => {
    // Valeurs de inagle_emblems : crc32("em010001"), identifiants d'équipe réels.
    expect(emblemNameOfCrc("0xD5ABE1F0")).toBe("em010001");
    expect(emblemNameOfCrc("0x73576F57")).toBe("em040010");
    expect(emblemNameOfCrc("0x27425D40")).toBe("em020036");
  });

  test("tolère la casse et l'absence de préfixe, et refuse l'inconnu", () => {
    expect(emblemNameOfCrc("d5abe1f0")).toBe("em010001");
    expect(emblemNameOfCrc("0xDEADBEEF")).toBeUndefined();
    expect(emblemNameOfCrc("")).toBeUndefined();
    expect(emblemNameOfCrc(null)).toBeUndefined();
  });
});

describe("getEmblemImageUrlByCrc", () => {
  test("URL du fichier réel, vide si aucun emblème ne porte ce CRC", () => {
    expect(getEmblemImageUrlByCrc("0xD5ABE1F0")).toEndWith(
      "/200_icon/01_icon_emblem/em010001.png",
    );
    expect(getEmblemImageUrlByCrc("0xDEADBEEF")).toBe("");
  });
});

const suite = describe.skipIf(!hasPostgresConfiguration());

suite("emblèmes des équipes sur la base réelle", () => {
  test("la grande majorité des équipes a un emblème résolu", async () => {
    const { getTeamsList } = await import("../src/wiki/teams");
    const teams = await getTeamsList();
    expect(teams.length).toBeGreaterThan(100);
    const withEmblem = teams.filter((t) => t.emblemUrl);
    // 188 équipes sur 208 portent au moins un emblème dans les données du jeu
    // (les « Non-affilié » et équipes spéciales n'en ont pas).
    expect(withEmblem.length).toBeGreaterThan(150);
    for (const t of withEmblem) {
      expect(t.emblemUrl).toMatch(/01_icon_emblem\/em\w+\.png$/);
    }
  });
});
