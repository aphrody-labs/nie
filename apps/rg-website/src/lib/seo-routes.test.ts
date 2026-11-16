import { describe, expect, test } from "bun:test";

import { pagesPubliques, routeDepuisFichier } from "./admin/routes";
import {
  estIndexable,
  HORS_PLAN_DE_SITE,
  routesPlanRacine,
  routesPlanWiki,
  WIKI_HORS_PLAN,
} from "./seo-routes";

const racine = `${import.meta.dir}/../app`;
const routes = [...new Bun.Glob("**/page.{tsx,ts,mdx}").scanSync({ cwd: racine })]
  .map((f) => routeDepuisFichier(f.replaceAll("\\", "/")))
  .filter((r) => r !== null);
const statiques = pagesPubliques(routes);

describe("plans de site", () => {
  test("chaque exclusion correspond à une route réelle", () => {
    const toutes = routes.map((r) => r.route);
    for (const p of [...HORS_PLAN_DE_SITE, ...WIKI_HORS_PLAN]) {
      if (["/api", "/login", "/forgot-password", "/reset-password"].includes(p)) continue;
      expect(toutes.some((r) => r === p || r.startsWith(`${p}/`)), p).toBe(true);
    }
  });

  test("racine : accueil présent, wiki, boutique et pages privées absents", () => {
    const plan = routesPlanRacine(statiques);
    expect(plan).toContain("/");
    expect(plan).toContain("/achillea");
    expect(plan.some((r) => r.startsWith("/wiki") || r.startsWith("/shop"))).toBe(false);
    expect(plan.some((r) => /^\/(admin|dashboard|profil|search)(\/|$)/.test(r))).toBe(false);
  });

  test("wiki : accueil et listes présents, outils absents", () => {
    const plan = routesPlanWiki(statiques);
    expect(plan).toContain("");
    expect(plan).toContain("/chara");
    expect(plan.some((r) => r.startsWith("/tools"))).toBe(false);
  });

  test("les pages noindex ne sont jamais indexables", () => {
    for (const r of ["/search", "/patreon-migration", "/maintenance", "/shop/panier"]) {
      expect(estIndexable(r)).toBe(false);
    }
  });
});
