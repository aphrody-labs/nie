export const dynamic = "force-static";
export const revalidate = 3600;

import type { Metadata } from "next";
import { TeamsListClient } from "@/components/wiki/TeamsListClient";
import { getTeamsList } from "@/lib/wiki/teams";
import { wiki } from "@rosegriffon/app-shell";

export const metadata: Metadata = {
  alternates: {
    canonical: wiki("/equipe"),
  },
  description:
    "Toutes les équipes de la saga Inazuma Eleven : emblèmes, kits, apparitions et effectifs. Raimon, Royal Academy, Nagumohara et toutes les formations.",
  title: "Équipes | Wiki Inazuma Eleven - Azalée",
};

export default async function TeamsPage() {
  const teams = await getTeamsList();

  return (
    <div className="space-y-6">
      <header className="space-y-1">
        <h1 className="text-fluid-headline-md font-extrabold text-on-surface">
          Équipes
        </h1>
        <p className="text-sm text-on-surface-variant">
          {teams.length.toLocaleString("fr-FR")} équipes recensées — emblèmes,
          uniformes et effectifs liés.
        </p>
      </header>

      <TeamsListClient teams={teams} />
    </div>
  );
}
