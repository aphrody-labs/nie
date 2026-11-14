import type { Metadata } from "next";

import { absoluteUrl } from "@/lib/site-url";

export const metadata: Metadata = {
  title: {
    default: "Azalée — Encyclopédie Inazuma Eleven | Rose Griffon",
    // Pas de suffixe : chaque page du wiki porte déjà sa marque (« … - Azalée »,
    // « … | Azalée »). L'ancien gabarit « %s | Wiki Azalée | Rose Griffon » la
    // répétait et poussait 169 titres au-delà de 65 caractères (crawl du 5/11/2026).
    template: "%s",
  },
  description:
    "Encyclopédie complète de toute la saga Inazuma Eleven : du premier épisode jusqu'à Victory Road, Cross, RE et Bold Revolution.",
  icons: {
    icon: "/RG_Azalee-logo.webp",
    shortcut: "/RG_Azalee-logo.webp",
    apple: "/RG_Azalee-logo.webp",
  },
  openGraph: {
    title: "Azalée — Encyclopédie Inazuma Eleven",
    description:
      "Encyclopédie complète de toute la saga Inazuma Eleven : du premier épisode jusqu'à Victory Road, Cross, RE et Bold Revolution.",
    url: absoluteUrl("/wiki"),
    siteName: "Azalée Wiki",
    locale: "fr_FR",
    type: "website",
    images: [
      {
        url: "/RG_Azalee-logo.webp",
        width: 1200,
        height: 630,
        alt: "Logo Azalée Wiki",
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: "Azalée — Encyclopédie Inazuma Eleven",
    description:
      "Encyclopédie complète de toute la saga Inazuma Eleven : du premier épisode jusqu'à Victory Road, Cross, RE et Bold Revolution.",
    images: ["/RG_Azalee-logo.webp"],
  },
};

export default function WikiLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <div className="wiki-scope min-h-full">{children}</div>;
}
