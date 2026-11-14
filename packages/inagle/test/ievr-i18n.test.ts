import { describe, expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import {
  createIevrGlossaryIndex,
  extractFuriganaPairs,
  formatDescription,
  formatJapaneseName,
  hasUnresolvedTags,
  loadIevrI18n,
  lookupIevrTerm,
  matchIevrEntities,
  parseIevrRichText,
  segmentJapanese,
  translateJp,
} from "../src/text";

describe("format-description linear parser", () => {
  test("tokenizes tags, furigana and text cleanly without regex backtracking", () => {
    const tokens = parseIevrRichText(
      "[亜風炉/あふろ] [照美/てるみ] et <FST:GOENJI> affrontent <MNT:TEIKOKU>!",
    );
    expect(tokens).toEqual([
      { kind: "furigana", base: "亜風炉", reading: "あふろ" },
      { kind: "text", text: " " },
      { kind: "furigana", base: "照美", reading: "てるみ" },
      { kind: "text", text: " et " },
      { kind: "tag", tagType: "FST", tagName: "GOENJI" },
      { kind: "text", text: " affrontent " },
      { kind: "tag", tagType: "MNT", tagName: "TEIKOKU" },
      { kind: "text", text: "!" },
    ]);

    expect(extractFuriganaPairs("[円堂/えんどう] [守/まもる]")).toEqual([
      { base: "円堂", reading: "えんどう" },
      { base: "守", reading: "まもる" },
    ]);
    expect(formatJapaneseName("[豪炎寺/ごうえんじ]")).toBe("豪炎寺");
    expect(formatDescription("<FST:ARTHUR> vs <MNT:ALIEA>", "es")).toBe(
      "Rey Arturo vs Academia Alius",
    );
    expect(hasUnresolvedTags("Pas de balise ici [漢字/かんじ]")).toBe(false);
    expect(hasUnresolvedTags("Avec <FST:TENMA>")).toBe(true);
  });
});

describe("ievr-i18n VFS glossary and Japanese pipeline", () => {
  test("longest-match entity lookup and BudouX segmentation preserve game terms", async () => {
    const sampleBundle = {
      langs: ["ja", "en", "fr", "es"],
      hash_entries: {
        "0x4B2792B9": {
          category: "chara",
          ja: "亜風炉 照美",
          fr: "Byron Love",
          en: "Byron Love",
          es: "Byron Love",
          romaji: "afuro terumi",
        },
        "0x789CBEAD": {
          category: "skill",
          ja: "ゴッドノウズ",
          fr: "Savoir suprême",
          en: "God Knows",
          es: "Sabiduría divina",
          romaji: "goddonouzu",
        },
        "0xBB6E36F4": {
          category: "skill",
          ja: "ゴッドノウズ・インパクト",
          fr: "Savoir ultime",
          en: "God Alone Knows",
          es: "Omnisabiduría divina",
          romaji: "goddonouzu/inpakuto",
        },
      },
      ja_to_translations: {
        "亜風炉 照美": {
          category: "chara",
          hash_id: "0x4B2792B9",
          fr: "Byron Love",
          en: "Byron Love",
          es: "Byron Love",
          romaji: "afuro terumi",
        },
        ゴッドノウズ: {
          category: "skill",
          hash_id: "0x789CBEAD",
          fr: "Savoir suprême",
          en: "God Knows",
          es: "Sabiduría divina",
          romaji: "goddonouzu",
        },
        "ゴッドノウズ・インパクト": {
          category: "skill",
          hash_id: "0xBB6E36F4",
          fr: "Savoir ultime",
          en: "God Alone Knows",
          es: "Omnisabiduría divina",
          romaji: "goddonouzu/inpakuto",
        },
      },
    };

    const index = createIevrGlossaryIndex(sampleBundle, "es");
    const matches = matchIevrEntities("ゴッドノウズ・インパクト", index);
    expect(matches).toHaveLength(1);
    expect(matches[0].target).toBe("Omnisabiduría divina");

    const term = await lookupIevrTerm("0x789CBEAD", "fr", sampleBundle);
    expect(term?.target).toBe("Savoir suprême");

    const translated = await translateJp("ゴッドノウズ", "es", {
      bundle: sampleBundle,
      skipRemote: true,
    });
    expect(translated.translated).toBe("Sabiduría divina");
    expect(translated.matchedEntities[0].hashId).toBe("0x789CBEAD");
    expect(translated.romaji).toContain("godd");

    const segs = segmentJapanese("稲妻イレブンの必殺技を使う");
    expect(segs.length).toBeGreaterThan(1);
  });

  test("loads live packages/inagle/data/ievr-i18n.json when present", async () => {
    const bundlePath = resolve(import.meta.dir, "../data/ievr-i18n.json");
    if (!existsSync(bundlePath)) return;

    const bundle = await loadIevrI18n(bundlePath);
    expect(bundle).not.toBeNull();
    expect(bundle!.langs).toContain("es");

    const byron = await lookupIevrTerm("亜風炉 照美", "fr", bundle);
    expect(byron?.target).toBe("Byron Love");

    const godKnowsEs = await lookupIevrTerm("ゴッドノウズ", "es", bundle);
    expect(godKnowsEs?.target).toBe("Sabiduría divina");
  });
});
