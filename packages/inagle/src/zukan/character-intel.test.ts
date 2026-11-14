import { describe, expect, it } from "bun:test";
import {
  buildZukanFaceUrl,
  decodeZukanQuery,
  encodeZukanQuery,
  extractCharacterIdFromZukanHref,
  parseZukanCharaListHtml,
  resolveCatalogCharacterMatches,
  resolveDefaultCharacterTarget,
} from "./character-intel.js";

describe("inagle character-intel", () => {
  it("encodes and decodes Zukan bitwise-NOT base64url query payloads", () => {
    const encoded = encodeZukanQuery({ character_id: ["c01000010"] });
    expect(encoded).toBe("hN2cl56NnpyLmo2glpvdxaTdnM_Oz8_Pz87P3aKC");
    expect(decodeZukanQuery<{ character_id: string[] }>(encoded)).toEqual({
      character_id: ["c01000010"],
    });

    const userSeedQ = "hN2RnpKaoIyano2cl93FpN2-j5eNkJuG392igg==";
    const decodedSeed = decodeZukanQuery(userSeedQ);
    expect(decodedSeed).toBeTruthy();
  });

  it("resolves Byron Love / Afuro Terumi / Aphrodi across catalog.json without false positives", () => {
    const target = resolveDefaultCharacterTarget("afuro-terumi");
    expect(target.canonicalName).toBe("Afuro Terumi");
    expect(target.zukanIds).toEqual(["c01001900", "c05026590", "c07080010"]);
    expect(target.seedUrls.length).toBeGreaterThanOrEqual(5);

    const matches = resolveCatalogCharacterMatches(target.aliases);
    expect(matches.length).toBeGreaterThanOrEqual(3);
    for (const m of matches) {
      expect(m.name).toBe("Byron Love");
      expect(m.nickname).toBe("Aphrody");
      expect(m.faceUrl).toStartWith(
        "https://dxi4wb638ujep.cloudfront.net/1/k/",
      );
    }
    expect(matches.some((m) => m.name === "Ruth Byron")).toBe(false);
  });

  it("builds official CloudFront webp portrait URLs from zukan hashes", () => {
    expect(buildZukanFaceUrl("k/d/y/dykb3jbxeis")).toBe(
      "https://dxi4wb638ujep.cloudfront.net/1/k/d/y/dykb3jbxeis.webp",
    );
  });

  it("parses Zukan chara_list HTML and decodes chara_param character IDs", () => {
    const qParam = encodeZukanQuery({ filter_chara_id_str: "c01001900" });
    const html = `
      <ul>
        <li>
          <a href="/en/chara_param/?q=${qParam}">
            <img data-src="https://dxi4wb638ujep.cloudfront.net/1/k/d/y/dykb3jbxeis.webp" alt="Byron Love" />
            <p class="name">Byron Love</p>
          </a>
          <span class="nickname">Aphrody</span>
        </li>
      </ul>
    `;
    expect(
      extractCharacterIdFromZukanHref(`/en/chara_param/?q=${qParam}`),
    ).toBe("c01001900");

    const parsed = parseZukanCharaListHtml(html);
    expect(parsed).toHaveLength(1);
    expect(parsed[0]!.characterId).toBe("c01001900");
    expect(parsed[0]!.name).toBe("Byron Love");
    expect(parsed[0]!.nickname).toBe("Aphrody");
    expect(parsed[0]!.imageUrl).toBe(
      "https://dxi4wb638ujep.cloudfront.net/1/k/d/y/dykb3jbxeis.webp",
    );
  });
});
