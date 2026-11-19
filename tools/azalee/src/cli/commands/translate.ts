/** `azalee translate` — traduction FR assistée par le glossaire consolidé. */

import type { Command } from "commander";

import {
  colors,
  errorMessage,
  getOrReadInput,
  renderAsciiTable,
  reportError,
  restoreLogs,
  suppressLogs,
} from "../context";
import { containsJapanese } from "../../text/japanese-detect";
import {
  searchDatabaseTranslate,
  searchGlossaryTranslate,
  translateTextDetailed,
  type TranslateMatch,
} from "../translate-lookup";
import type { TranslateOptions } from "../types";

function parseTargetLang(raw?: string): "fr" | "en" | "es" {
  if (raw === "en" || raw === "es" || raw === "fr") return raw;
  return "fr";
}

export function registerTranslateCommand(program: Command): void {
  program
    .command("translate [text]")
    .description(
      "Traduit du texte (FR, EN, ES) en appliquant le dictionnaire IEVR et le glossaire consolidé",
    )
    .option("-t, --to <lang>", "Langue cible (fr, en, es)", "fr")
    .option("-j, --json", "Format de sortie en JSON brute")
    .action(async (text: string | undefined, options: TranslateOptions) => {
      suppressLogs(!!options.json);
      try {
        const inputText = await getOrReadInput(text);
        if (!inputText.trim()) {
          reportError(
            options.json,
            "Aucun texte à traduire",
            `${colors.red}Erreur : Aucun texte fourni.${colors.reset}`,
          );
          return;
        }

        const isJa = containsJapanese(inputText);
        if (!options.json) {
          console.log(
            `${colors.cyan}Langue source détectée : ${isJa ? "Japonais" : "Anglais/Français"}${colors.reset}`,
          );
        }

        // Base + glossaire en parallèle : deux sources indépendantes.
        const [dbResults, glossaryResults] = await Promise.all([
          searchDatabaseTranslate(inputText),
          searchGlossaryTranslate(inputText),
        ]);

        const seen = new Set<string>();
        const merged: TranslateMatch[] = [];

        if (dbResults) {
          for (const r of dbResults) {
            const key = `${r.type}-${r.id}`.toLowerCase();
            if (!seen.has(key)) {
              seen.add(key);
              merged.push(r);
            }
          }
        }

        if (glossaryResults) {
          for (const r of glossaryResults) {
            const key = `${r.type}-${r.id}`.toLowerCase();
            const nameKey = `${r.type}-${r.name_fr || r.name_en}`.toLowerCase();
            if (!seen.has(key) && !seen.has(nameKey)) {
              seen.add(key);
              seen.add(nameKey);
              merged.push(r);
            }
          }
        }

        // Pertinence : correspondance exacte > préfixe > sous-chaîne.
        const queryLower = inputText.toLowerCase();
        merged.sort((a, b) => {
          const scoreOf = (r: TranslateMatch) => {
            for (const name of [r.name_fr, r.name_en, r.name_ja, r.name_roma]) {
              if (!name) continue;
              const n = name.toLowerCase();
              if (n === queryLower) return 3;
              if (n.startsWith(queryLower)) return 2;
            }
            return 1;
          };
          return scoreOf(b) - scoreOf(a);
        });

        const targetLang = parseTargetLang(options.to);
        const detailed = await translateTextDetailed(inputText, targetLang);

        if (options.json) {
          restoreLogs(true);
          console.log(
            JSON.stringify(
              {
                original: inputText,
                detectedLanguage: isJa ? "ja" : "en",
                targetLang,
                translatedText: detailed.translated,
                reading: detailed.reading,
                romaji: detailed.romaji,
                segments: detailed.segments,
                matchedEntities: detailed.matchedEntities,
                matches: merged.slice(0, 15),
              },
              null,
              2,
            ),
          );
        } else {
          if (merged.length > 0) {
            console.log(
              `\n${colors.bold}${colors.blue}Entités trouvées dans la base & le glossaire :${colors.reset}`,
            );
            const rows = merged.slice(0, 15).map((r) => ({
              Type: r.typeLabel,
              "Nom FR": r.name_fr || "-",
              "Nom EN": r.name_en || "-",
              "Nom JA": r.name_ja || "-",
              Romaji: r.name_roma || "-",
            }));
            console.log(renderAsciiTable(rows));
          }

          if (detailed.reading || detailed.romaji) {
            console.log(
              `\n${colors.cyan}Lecture : ${detailed.reading || "-"} | Romaji : ${detailed.romaji || "-"}${colors.reset}`,
            );
          }

          console.log(
            `\n${colors.bold}${colors.green}Traduction (${targetLang}) :${colors.reset}`,
          );
          console.log(detailed.translated);
        }
        process.exit(0);
      } catch (e) {
        reportError(
          options.json,
          errorMessage(e),
          `${colors.red}Erreur de traduction : ${errorMessage(e)}${colors.reset}`,
        );
        process.exit(1);
      }
    });
}
