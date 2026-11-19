/** `azalee mods` — annuaire des mods GameBanana et wikis communautaires JA/ES/EN/FR. */

import type { Command } from "commander";

import {
  queryCommunityModsEndpoint,
  queryCommunityWikisEndpoint,
} from "../../server/ecosystem";
import {
  colors,
  errorMessage,
  reportError,
  restoreLogs,
  suppressLogs,
} from "../context";
import type { ModsOptions } from "../types";

export function registerModsCommand(program: Command): void {
  program
    .command("mods [query]")
    .description(
      "Liste les mods GameBanana et les wikis communautaires (JA, ES, EN, FR)",
    )
    .option("-g, --game <slug>", "Filtrer par slug de jeu")
    .option("-l, --lang <lang>", "Filtrer par langue (fr, en, ja, es)")
    .option("-j, --json", "Format de sortie en JSON brute")
    .action(async (query: string | undefined, options: ModsOptions) => {
      suppressLogs(!!options.json);
      try {
        const modsPayload = queryCommunityModsEndpoint({
          gameSlug: options.game,
          q: query,
          locale: options.lang,
          limit: 50,
        });
        const wikisPayload = queryCommunityWikisEndpoint({
          lang: options.lang,
          gameSlug: options.game,
          q: query,
          locale: options.lang,
          limit: 50,
        });

        restoreLogs(!!options.json);
        if (options.json) {
          console.log(
            JSON.stringify(
              {
                mods: modsPayload,
                wikis: wikisPayload,
              },
              null,
              2,
            ),
          );
          return;
        }

        console.log(
          `${colors.cyan}${colors.bold}Mods & Wikis Inazuma Eleven${colors.reset} — ${modsPayload.totalMods} mods, ${modsPayload.totalHubs} hubs, ${wikisPayload.totalSources} wikis`,
        );
        for (const hub of modsPayload.hubs.slice(0, 12)) {
          console.log(`  [HUB] ${hub.gameTitle} · ${hub.name} -> ${hub.url}`);
        }
        for (const wiki of wikisPayload.sources.slice(0, 12)) {
          console.log(
            `  [WIKI:${wiki.lang.toUpperCase()}] ${wiki.name} (${wiki.gameSlug}) -> ${wiki.url}`,
          );
        }
      } catch (err) {
        reportError(
          options.json,
          errorMessage(err),
          `${colors.red}Erreur mods: ${errorMessage(err)}${colors.reset}`,
        );
      }
    });
}
