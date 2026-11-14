// SPDX-License-Identifier: Apache-2.0
import { IntlMessageFormat } from "intl-messageformat";
import { buildFallbackChain, canonicalizeLocaleTag } from "./registry";

const french = {
  "account.patreon.title": "Compte Patreon",
  "account.patreon.status.linked": "Connecté",
  "account.patreon.status.unlinked": "Non lié",
  "account.patreon.action.disconnect": "Déconnecter",
  "account.patreon.action.link": "Lier mon Patreon",
  "account.patreon.confirmDisconnect.title": "Déconnecter Patreon ?",
  "common.action.cancel": "Annuler",
};

export type UiMessageKey = keyof typeof french;
type Messages = Readonly<Record<UiMessageKey, string>>;

/** UI messages, separate from the host's game-data dictionaries and their hash keys. */
export const UI_MESSAGES: Readonly<
  Record<"fr" | "en" | "ja" | "es", Messages>
> = {
  fr: french,
  en: {
    "account.patreon.title": "Patreon account",
    "account.patreon.status.linked": "Connected",
    "account.patreon.status.unlinked": "Not linked",
    "account.patreon.action.disconnect": "Disconnect",
    "account.patreon.action.link": "Link my Patreon",
    "account.patreon.confirmDisconnect.title": "Disconnect Patreon?",
    "common.action.cancel": "Cancel",
  },
  ja: {
    "account.patreon.title": "Patreonアカウント",
    "account.patreon.status.linked": "連携済み",
    "account.patreon.status.unlinked": "未連携",
    "account.patreon.action.disconnect": "連携を解除",
    "account.patreon.action.link": "Patreonと連携",
    "account.patreon.confirmDisconnect.title":
      "Patreonとの連携を解除しますか？",
    "common.action.cancel": "キャンセル",
  },
  es: {
    "account.patreon.title": "Cuenta de Patreon",
    "account.patreon.status.linked": "Conectado",
    "account.patreon.status.unlinked": "No vinculado",
    "account.patreon.action.disconnect": "Desconectar",
    "account.patreon.action.link": "Vincular mi Patreon",
    "account.patreon.confirmDisconnect.title": "¿Desconectar Patreon?",
    "common.action.cancel": "Cancelar",
  },
};

const catalogues: ReadonlyMap<string, Messages> = new Map(
  Object.entries(UI_MESSAGES),
);

export type UiMessageValues = Record<
  string,
  string | number | boolean | Date | null | undefined
>;

const formatterCache = new Map<string, IntlMessageFormat>();

function getFormatter(locale: string, pattern: string): IntlMessageFormat {
  const canonical = canonicalizeLocaleTag(locale) || "fr";
  const cacheKey = `${canonical}\0${pattern}`;
  let formatter = formatterCache.get(cacheKey);
  if (!formatter) {
    formatter = new IntlMessageFormat(pattern, canonical);
    formatterCache.set(cacheKey, formatter);
  }
  return formatter;
}

/** Format an ICU MessageFormat pattern for `locale` using `intl-messageformat`. */
export function formatIcuMessage(
  locale: string | null | undefined,
  pattern: string,
  values?: UiMessageValues,
): string {
  if (!values || !pattern.includes("{")) return pattern;
  const tag = buildFallbackChain(locale)[0] ?? "fr";
  const formatted = getFormatter(tag, pattern).format(values);
  return Array.isArray(formatted) ? formatted.join("") : String(formatted);
}

/** Resolve semantic UI messages with the same BCP-47 negotiation and French fallback as the host. */
export function uiMessage(
  locale: string | null | undefined,
  key: UiMessageKey,
  values?: UiMessageValues,
): string {
  for (const tag of buildFallbackChain(locale)) {
    const message = catalogues.get(tag)?.[key];
    if (message !== undefined) {
      return formatIcuMessage(tag, message, values);
    }
  }
  throw new RangeError(`Unknown UI message: ${key}`);
}
