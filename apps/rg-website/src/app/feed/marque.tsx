import {
  InstagramIcon as Instagram,
  TikTokIcon,
  XIcon as Twitter,
  YouTubeIcon as Youtube,
} from "@rosegriffon/ui/brand-icons";
import { MARQUES } from "@/lib/feed/comptes";
import type { Reseau } from "@/lib/feed/types";

const ICONES = {
  x: Twitter,
  instagram: Instagram,
  tiktok: TikTokIcon,
  youtube: Youtube,
} as const;

/**
 * Pastille du réseau : la couleur de marque est la seule couleur brute de la page.
 * `decoratif` quand le nom du réseau est déjà écrit à côté (puces de filtre) : sans lui, le
 * nom en `sr-only` doublait le libellé visible (« Instagram Instagram », « X X »).
 */
export function BadgeReseau({
  reseau,
  taille = 20,
  decoratif = false,
}: {
  reseau: Reseau;
  taille?: number;
  decoratif?: boolean;
}) {
  const Icone = ICONES[reseau];
  const marque = MARQUES[reseau];
  return (
    <span
      title={decoratif ? undefined : marque.nom}
      aria-hidden={decoratif || undefined}
      className="inline-flex shrink-0 items-center justify-center rounded-full text-white ring-1 ring-outline-variant"
      style={{ backgroundColor: marque.couleur, width: taille, height: taille }}
    >
      <Icone className="size-[60%]" aria-hidden />
      {!decoratif && <span className="sr-only">{marque.nom}</span>}
    </span>
  );
}
