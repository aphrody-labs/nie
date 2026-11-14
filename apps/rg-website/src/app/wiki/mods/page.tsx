import type { Metadata } from "next";
import { wiki } from "@rosegriffon/app-shell";
import { WikiPageHeader } from "@/components/wiki/WikiPageHeader";
import { wikiTitle, withDefaultOgImage } from "@/lib/seo";
import { absoluteUrl } from "@/lib/site-url";
import { getServerLocale } from "@/lib/i18n/server-locale";
import { loadCommunityModsSnapshot } from "@/lib/wiki/community-mods-server";
import { getModsPageCopy, MODS_PAGE_COPY } from "@/lib/wiki/games-copy";
import { ModsDirectoryClient } from "./ModsDirectoryClient";

export const revalidate = 1800;

export const metadata: Metadata = {
  alternates: { canonical: absoluteUrl(wiki("/mods")) },
  description: MODS_PAGE_COPY.subtitle,
  openGraph: withDefaultOgImage({
    description: MODS_PAGE_COPY.subtitle,
    locale: "fr_FR",
    siteName: "Azalée Wiki",
    title: wikiTitle(MODS_PAGE_COPY.title),
    type: "website",
    url: absoluteUrl(wiki("/mods")),
  }),
  title: wikiTitle(MODS_PAGE_COPY.title),
};

export default async function WikiModsPage() {
  const locale = await getServerLocale();
  const copy = getModsPageCopy(locale);
  const snapshot = loadCommunityModsSnapshot(locale);
  const headerCount = `${snapshot.mods.length} ${copy.statsModsLabel.toLowerCase()} · ${snapshot.wikis.length + snapshot.curatedWikis.length} ${copy.statsWikisLabel.toLowerCase()}`;

  return (
    <div className="space-y-6">
      <WikiPageHeader
        icon="extension"
        title={copy.title}
        count={headerCount}
        description={copy.subtitle}
      />
      <ModsDirectoryClient snapshot={snapshot} />
    </div>
  );
}
