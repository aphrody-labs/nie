import type { MetadataRoute } from "next";

import { pagesPubliques } from "@/lib/admin/routes";
import { inventaireRoutes } from "@/lib/admin/routes-systeme";
import { getPgPool } from "@/lib/db/pg";
import { listerIdentifiantsSondages } from "@/lib/discord/polls";
import { routesPlanRacine } from "@/lib/seo-routes";
import { SITE_URL } from "@/lib/site-url";

// Régénéré au plus toutes les heures (ISR) : les articles et les sondages
// changent sans redéploiement.
export const revalidate = 3600;

type ArticleRow = {
  slug: string;
  updated_at: string | null;
  published_at: string | null;
};

/**
 * Articles publiés des deux corpus (`azalee`, `website`), servis sous
 * `/news/<slug>`. Lecture Postgres directe : une erreur retire les articles,
 * jamais le plan de site entier.
 */
async function articlesPublies(): Promise<MetadataRoute.Sitemap> {
  try {
    const { rows } = await getPgPool().query<ArticleRow>(
      "SELECT slug, updated_at, published_at FROM articles WHERE status = $1 AND app = ANY($2::text[]) ORDER BY published_at DESC NULLS LAST",
      ["published", ["azalee", "website"]],
    );
    return rows
      .filter((row) => row.slug)
      .map((row) => ({
        url: `${SITE_URL}/news/${row.slug}`,
        lastModified: new Date(
          row.updated_at || row.published_at || Date.now(),
        ),
      }));
  } catch (error) {
    console.error("sitemap: lecture des articles impossible", error);
    return [];
  }
}

async function sondages(): Promise<MetadataRoute.Sitemap> {
  try {
    const ids = await listerIdentifiantsSondages();
    return ids.map((id) => ({ url: `${SITE_URL}/sondages/${id}` }));
  } catch (error) {
    console.error("sitemap: lecture des sondages impossible", error);
    return [];
  }
}

/**
 * Pages statiques lues dans l'arborescence `app/` (ou le manifeste du build),
 * filtrées par `lib/seo-routes.ts`. Le wiki et la boutique ont leur propre plan.
 * Pas de `changefreq` ni de `priority` : Google les ignore.
 */
export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  const { pages } = await inventaireRoutes();
  const statiques = routesPlanRacine(pagesPubliques(pages)).map((path) => ({
    url: `${SITE_URL}${path === "/" ? "" : path}`,
  }));
  const [articles, polls] = await Promise.all([articlesPublies(), sondages()]);
  return [...statiques, ...articles, ...polls];
}
