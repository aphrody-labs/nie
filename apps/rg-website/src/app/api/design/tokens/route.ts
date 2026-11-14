import { NextResponse } from "next/server";
import { getDesignThemesManifest } from "@rosegriffon/azalee/server/ecosystem";
import { STACK_CONTRACT } from "@rosegriffon/config/contract";
import {
  FORMES,
  MARQUE,
  MARQUES_TIERCES,
  NOMS_THEMES,
  THEMES,
} from "@rosegriffon/ui/tokens";

export const revalidate = 3600;

export function GET() {
  const manifest = getDesignThemesManifest();
  return NextResponse.json(
    {
      schemaVersion: manifest.schemaVersion,
      manifest,
      themeNames: NOMS_THEMES,
      shapes: FORMES,
      brand: MARQUE,
      thirdPartyBrands: MARQUES_TIERCES,
      themes: THEMES,
      contract: {
        locales: STACK_CONTRACT.project.locales,
        ui: STACK_CONTRACT.ui,
      },
    },
    {
      headers: {
        "Cache-Control": "public, max-age=3600, stale-while-revalidate=86400",
      },
    },
  );
}
