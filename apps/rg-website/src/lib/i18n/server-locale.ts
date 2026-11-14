/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { cookies, headers } from "next/headers";
import { resolveLocale } from "@rosegriffon/i18n";
import type { InazumaLocale } from "@rosegriffon/inagle/games";

/** Resolve active UI/catalog locale from SSR cookie + Accept-Language headers. */
export async function getServerLocale(): Promise<InazumaLocale> {
  try {
    const cookieStore = await cookies();
    const headerStore = await headers();
    const resolved = resolveLocale({
      cookieLocale: cookieStore.get("rg-locale")?.value,
      acceptLanguage: headerStore.get("accept-language"),
    });
    const base = resolved.locale.toLowerCase().slice(0, 2);
    if (base === "en" || base === "ja" || base === "es" || base === "fr") {
      return base;
    }
  } catch {
    // Fallback outside request context (static generation / tests)
  }
  return "fr";
}
