/** `azalee games` — catalogue multi-langue complet de toute la saga Inazuma Eleven. */

import type { Command } from "commander";

import { queryGameDetail, queryGamesCatalog } from "../../games";
import {
  colors,
  errorMessage,
  reportError,
  restoreLogs,
  suppressLogs,
} from "../context";
import type { GamesOptions } from "../types";

export function registerGamesCommand(program: Command): void {
  program
    .command("games [query]")
    .description(
      "Affiche tous les jeux Inazuma Eleven (IE1 à Victory Road, Cross, RE, Bold Revolution)",
    )
    .option("-l, --locale <locale>", "Langue (fr, en, ja, es)", "fr")
    .option("-e, --era <era>", "Filtrer par époque")
    .option("-s, --status <status>", "Filtrer par statut")
    .option("-j, --json", "Format de sortie en JSON brute")
    .action(async (query: string | undefined, options: GamesOptions) => {
      suppressLogs(!!options.json);
      try {
        const trimmed = query?.trim() ?? "";
        if (trimmed) {
          const detail = queryGameDetail(trimmed, options.locale);
          if (detail) {
            restoreLogs(!!options.json);
            if (options.json) {
              console.log(JSON.stringify(detail, null, 2));
            } else {
              const g = detail.game;
              console.log(
                `${colors.cyan}${colors.bold}${g.title}${colors.reset} (${g.slug})`,
              );
              console.log(
                `${detail.era?.label ?? g.era} · ${detail.statusMeta.label} · ${g.releaseYear}`,
              );
              console.log(g.summary);
              console.log(
                `${colors.yellow}Plateformes:${colors.reset} ${g.platforms.join(", ")}`,
              );
              console.log(
                `${colors.yellow}Wikis:${colors.reset} ${g.communityWikis.length} | ${colors.yellow}Hubs mods:${colors.reset} ${g.modHubs.length}`,
              );
            }
            return;
          }
        }

        const catalog = queryGamesCatalog({
          locale: options.locale,
          era: options.era,
          status: options.status,
          q: trimmed || undefined,
        });

        restoreLogs(!!options.json);
        if (options.json) {
          console.log(JSON.stringify(catalog, null, 2));
          return;
        }

        console.log(
          `${colors.cyan}${colors.bold}Inazuma Eleven (${catalog.locale})${colors.reset} — ${catalog.total} jeux`,
        );
        for (const g of catalog.games) {
          console.log(
            `  ${String(g.order).padStart(2, "0")}. ${g.title} [${g.status}] (${g.releaseYear}) -> ${g.slug}`,
          );
        }
      } catch (err) {
        reportError(
          options.json,
          errorMessage(err),
          `${colors.red}Erreur games: ${errorMessage(err)}${colors.reset}`,
        );
      }
    });
}
