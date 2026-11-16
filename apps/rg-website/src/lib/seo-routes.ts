/**
 * Source unique de ce qui s'indexe : `robots.ts`, les trois plans de site et
 * la page d'administration des routes lisent ces listes. La liste des pages
 * vient de l'arborescence `app/` (`inventaireRoutes`), plus d'une liste tenue
 * à la main : une nouvelle page publique entre seule dans le plan de site.
 */

/** Interdits à tous les robots (`robots.txt`). `/_next/` n'y va jamais : il sert le CSS, le JS et les images. */
export const ROBOTS_INTERDITS = [
  "/api/",
  "/admin/",
  "/dashboard/",
  "/login",
  "/forgot-password",
  "/reset-password",
  "/auth/",
  "/profil/",
  "/shop/panier",
  "/shop/merci",
  "/shop/commandes",
  "/maintenance",
] as const;

/**
 * Pages accessibles mais hors plan de site : espaces membre, formulaires de
 * création, pages `noindex`, redirections et outils sans contenu indexable.
 * Un préfixe couvre aussi ses sous-pages.
 */
export const HORS_PLAN_DE_SITE = [
  ...ROBOTS_INTERDITS.map((p) => p.replace(/\/$/, "")),
  "/settings",
  "/2fa",
  "/search",
  "/play",
  "/patreon-migration",
  "/achillea/admins",
  "/achillea/clans/create",
  "/achillea/clans/dashboard",
  "/achillea/events/create",
  "/achillea/organizations",
  "/achillea/tournaments/create",
  "/achillea/forbidden",
  "/achillea/wallet",
  "/achillea/ranked/match",
  "/achillea/ranked/history",
] as const;

/** Préfixes servis par leur propre plan de site (`app/wiki/sitemap.ts`, `app/shop/sitemap.ts`). */
export const PLANS_DEDIES = ["/wiki", "/shop"] as const;

/**
 * Pages du wiki laissées hors de son plan de site : navigateurs d'assets et
 * outils, sans texte propre à indexer.
 */
export const WIKI_HORS_PLAN = [
  "/wiki/gallery",
  "/wiki/videos",
  "/wiki/textures",
  "/wiki/modeles",
  "/wiki/tools",
  "/wiki/save",
  "/wiki/demo",
  "/wiki/sons",
  "/wiki/vroid",
  "/wiki/keshin",
  "/wiki/mode",
  "/wiki/avatar",
] as const;

const sous = (route: string, prefixe: string) =>
  route === prefixe || route.startsWith(`${prefixe}/`);

/** Vrai si la route (sans segment dynamique) a sa place dans un plan de site. */
export function estIndexable(route: string): boolean {
  if (HORS_PLAN_DE_SITE.some((p) => sous(route, p))) return false;
  if (sous(route, "/wiki")) return !WIKI_HORS_PLAN.some((p) => sous(route, p));
  return true;
}

/** Pages statiques du plan de site racine : indexables et hors des plans dédiés. */
export function routesPlanRacine(pagesStatiques: readonly string[]): string[] {
  return pagesStatiques
    .filter((r) => estIndexable(r) && !PLANS_DEDIES.some((p) => sous(r, p)))
    .sort();
}

/** Pages statiques du wiki pour son plan de site, chemin relatif à `/wiki` (`""` pour l'accueil). */
export function routesPlanWiki(pagesStatiques: readonly string[]): string[] {
  return pagesStatiques
    .filter((r) => sous(r, "/wiki") && estIndexable(r))
    .map((r) => r.slice("/wiki".length))
    .sort();
}
