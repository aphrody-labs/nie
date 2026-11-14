import type { Metadata } from "next";
import dynamicImport from "next/dynamic";

import { HomeAssetShowcase } from "@/components/home/HomeAssetShowcase";
import { HomeDiscordSection } from "@/components/home/HomeDiscordSection";
import { HomeHeroShowcase } from "@/components/home/HomeHeroShowcase";
import { HomeUniversesBento } from "@/components/home/HomeUniversesBento";
import { NextEventBanner } from "@/components/home/NextEventBanner";
import { RealtimeSettingsListener } from "@/components/shared/RealtimeSettingsListener";
import {
  OrganizationSchema,
  WebsiteSchema,
} from "@/components/shared/StructuredData";
import { getCalendarEvents } from "@/lib/api/events";
import { getPublicSetting } from "@/lib/api/settings";
import { SERVEURS_DISCORD } from "@/lib/discord";
import { chargerVitrineAccueil } from "@/lib/home-showcase-server";
import { SITE_URL } from "@/lib/site-url";

const ProjectsSection = dynamicImport(
  () => import("@/components/shared/ProjectsSection"),
);

// ISR : HTML mis en cache, régénéré au plus toutes les 5 min en arrière-plan (et à la publication).
// `force-static` est requis : le layout racine lit cookies()/headers() pour la langue, ce qui rendait
// la page dynamique malgré `revalidate`.
export const dynamic = "force-static";
export const revalidate = 300;

export const metadata: Metadata = {
  alternates: { canonical: SITE_URL },
  description:
    "Rose Griffon, association et hub communautaire Inazuma Eleven en France : wiki Victory Road, ladder et tournois e-sport, actualités et communauté.",
  keywords: [
    "Rose Griffon",
    "Inazuma Eleven",
    "Inazuma Eleven Victory Road",
    "Wiki Inazuma Eleven",
    "Azalée",
    "Achilléa",
    "wiki Victory Road",
    "techniques Inazuma Eleven",
    "joueurs Inazuma Eleven",
    "compétitif",
    "tournois Inazuma",
    "ladder ELO",
    "communauté Discord",
  ],
  openGraph: {
    description:
      "Association française Inazuma Eleven : wiki Victory Road, scène compétitive, événements et communauté depuis 2019.",
    images: [
      {
        alt: "Rose Griffon - Association Inazuma Eleven",
        height: 630,
        url: "/opengraph-image.png",
        width: 1200,
      },
    ],
    locale: "fr_FR",
    siteName: "Rose Griffon",
    title: "Rose Griffon - Association Inazuma Eleven France",
    type: "website",
    url: SITE_URL,
  },
  robots: {
    follow: true,
    googleBot: {
      follow: true,
      index: true,
      "max-image-preview": "large",
      "max-snippet": -1,
      "max-video-preview": -1,
    },
    index: true,
  },
  title: "Rose Griffon - Association Inazuma Eleven France",
  twitter: {
    card: "summary_large_image",
    creator: "@rose_griffon",
    description:
      "Association française Inazuma Eleven : wiki Victory Road, compétition, streaming et communauté passionnée",
    site: "@rose_griffon",
    title: "Rose Griffon - Association Inazuma Eleven France",
  },
};

/**
 * Charge l'ensemble des données nécessaires à la page d'accueil unifiée :
 * paramètres publics, actualités, données du wiki et communauté.
 */
async function chargerDonneesAccueil() {
  const [introTitle, introText, introImage, events] = await Promise.all([
    getPublicSetting("home_intro_title", "Qu'est-ce que Rose Griffon ?"),
    getPublicSetting(
      "home_intro_text",
      "Rose Griffon est une association française régie par la loi de 1901, ayant pour vocation de rassembler les principaux acteurs de la communauté Inazuma Eleven ainsi que l'ensemble des passionnés sous une même bannière. Elle valorise la scène artistique comme son univers compétitif.",
    ),
    getPublicSetting("home_intro_image", "/RG_screens_twitch_ethan.webp"),
    getCalendarEvents(),
  ]);

  const maintenant = Date.now();
  const prochain =
    events
      .map((evenement) => ({
        ...evenement,
        debut: evenement.start?.dateTime || evenement.start?.date,
        fin: evenement.end?.dateTime || evenement.end?.date,
      }))
      .filter((evenement) => {
        const cible = evenement.fin ?? evenement.debut;
        return cible ? new Date(cible).getTime() >= maintenant : false;
      })
      .toSorted(
        (a, b) =>
          new Date(a.debut || 0).getTime() - new Date(b.debut || 0).getTime(),
      )[0] ?? null;

  return {
    prochain,
    introImage,
    introText,
    introTitle,
  };
}

export default async function Home() {
  const [data, showcase] = await Promise.all([
    chargerDonneesAccueil(),
    chargerVitrineAccueil(),
  ]);

  return (
    <>
      <OrganizationSchema />
      <WebsiteSchema />
      <RealtimeSettingsListener />

      <div className="flex flex-col min-h-screen">
        <div className="flex flex-col gap-10 pb-14 pt-2 min-[600px]:gap-12 min-[840px]:pb-20 min-[840px]:pt-4">
          {data.prochain && (
            <section className="w-full" aria-label="Prochain événement">
              <NextEventBanner event={data.prochain} />
            </section>
          )}

          <HomeHeroShowcase rgtvFollowers={null} showcase={showcase} />

          <HomeUniversesBento rgtvFollowers={null} showcase={showcase} />

          <HomeAssetShowcase />

          <ProjectsSection />

          <HomeDiscordSection serveurs={SERVEURS_DISCORD} />
        </div>
      </div>
    </>
  );
}
