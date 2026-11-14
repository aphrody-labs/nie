"use client";

import type { NavItem } from "@rosegriffon/app-shell";
import type { ReactNode } from "react";

import {
  RgVectorIcon,
  type RgVectorIconId,
} from "@/components/icons/RgVectorIcons";
import { NavIcone } from "@/components/layout/NavIcone";
import { CommonSpriteIcon } from "@/components/ui/CommonSpriteIcon";
import { GameSpriteIcon } from "@/components/ui/GameSpriteIcon";
import type { GameIconKey } from "@/config/game-icons";
import type { SpriteCommonKey } from "@/config/sprites-common";
import { cn } from "@/lib/utils";

type RichRouteAsset =
  | { kind: "image"; src: string }
  | { kind: "game"; key: GameIconKey }
  | { kind: "common"; key: SpriteCommonKey }
  | { kind: "svg"; key: RgVectorIconId };

const ROUTE_ASSET_MAP: Record<string, RichRouteAsset> = {
  "/": { key: "home", kind: "svg" },
  "/achillea": { kind: "image", src: "/achillea.webp" },
  "/achillea/admins/dashboard": { key: "shield-board", kind: "svg" },
  "/achillea/admins/ranked-disputes": { key: "breche", kind: "common" },
  "/achillea/admins/sanctions": { key: "contre", kind: "common" },
  "/achillea/casts": { kind: "image", src: "/rgtv/rgtv.webp" },
  "/achillea/clans": { key: "emblem", kind: "game" },
  "/achillea/clans/create": { key: "crest-plus", kind: "svg" },
  "/achillea/clans/dashboard": { key: "shield-board", kind: "svg" },
  "/achillea/guide": { key: "playbook", kind: "svg" },
  "/achillea/licence": { key: "chara", kind: "common" },
  "/achillea/organizations/create": { key: "guild", kind: "svg" },
  "/achillea/organizations/validate": { key: "medal", kind: "game" },
  "/achillea/ranked": { kind: "image", src: "/achillea/ranks/legendaire.webp" },
  "/achillea/ranked/history": { key: "match-history", kind: "svg" },
  "/achillea/ranked/leaderboard": { kind: "image", src: "/achillea/ranks/supernova.webp" },
  "/achillea/ranked/tiers": { kind: "image", src: "/achillea/ranks/divin.webp" },
  "/achillea/tournaments": { kind: "image", src: "/achillea/ranks/or.webp" },
  "/achillea/tournaments/circuit/ranking": { key: "medal", kind: "game" },
  "/achillea/tournaments/create": { key: "bracket-plus", kind: "svg" },
  "/achillea/wallet": { key: "moneybag", kind: "game" },
  "/evenements": { key: "calendar-cup", kind: "svg" },
  "/feed": { key: "pulse-feed", kind: "svg" },
  "/news": { key: "news-press", kind: "svg" },
  "/nous-soutenir": { key: "patron-rose", kind: "svg" },
  "/projets": { key: "workshop", kind: "svg" },
  "/shop": { key: "uniform", kind: "game" },
  "/sondages": { key: "ballot", kind: "svg" },
  "/wiki": { kind: "image", src: "/azale.webp" },
  "/wiki/niveau": { key: "level-chart", kind: "svg" },
  "/wiki/patch-notes": { key: "patch-diff", kind: "svg" },
  "/wiki/quete": { key: "star-token", kind: "game" },
};

/**
 * Enrichit chaque entrée de la sidebar avec un vrai asset cohérent (sprite
 * sheet du jeu, emblème de palier Achilléa, icône vectorielle dédiée ou émote).
 */
export function SidebarRichIcon(item: NavItem): ReactNode {
  if (item.imageUrl || item.emote || item.gameIcon || item.commonSprite) {
    return NavIcone(item);
  }

  const mapped = ROUTE_ASSET_MAP[item.path];
  if (!mapped) return null;

  switch (mapped.kind) {
    case "image":
      return (
        <img
          src={mapped.src}
          alt=""
          width={24}
          height={24}
          aria-hidden="true"
          className="size-5 shrink-0 rounded-sm object-contain"
        />
      );
    case "game":
      return (
        <GameSpriteIcon
          name={mapped.key}
          size={20}
          className="shrink-0"
        />
      );
    case "common":
      return (
        <CommonSpriteIcon
          name={mapped.key}
          scale={0.25}
          className={cn("shrink-0")}
        />
      );
    case "svg":
      return <RgVectorIcon name={mapped.key} />;
  }
}
