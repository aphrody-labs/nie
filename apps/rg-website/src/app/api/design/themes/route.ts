import { NextResponse } from "next/server";
import { getDesignThemesManifest } from "@rosegriffon/azalee/server/ecosystem";
import { THEMES, type NomTheme } from "@rosegriffon/ui/tokens";

export const revalidate = 3600;

export function GET() {
  const manifest = getDesignThemesManifest();
  const themes = manifest.themes.map((entry) => {
    const tokenKey = entry.selectors
      .find((sel) => sel.startsWith(".theme-"))
      ?.slice(1) as NomTheme | undefined;
    return {
      ...entry,
      tokenKey: tokenKey ?? null,
      tokens: tokenKey ? THEMES[tokenKey] : null,
    };
  });

  return NextResponse.json(
    {
      ...manifest,
      themes,
    },
    {
      headers: {
        "Cache-Control": "public, max-age=3600, stale-while-revalidate=86400",
      },
    },
  );
}
