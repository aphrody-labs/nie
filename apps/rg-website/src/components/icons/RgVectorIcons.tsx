import type { SVGProps } from "react";

import { cn } from "@/lib/utils";

export type RgVectorIconId =
  | "home"
  | "calendar-cup"
  | "ballot"
  | "pulse-feed"
  | "workshop"
  | "patron-rose"
  | "patch-diff"
  | "level-chart"
  | "crest-plus"
  | "shield-board"
  | "match-history"
  | "bracket-plus"
  | "guild"
  | "playbook"
  | "cube-3d"
  | "news-press"
  | "palette";

interface SvgIconProps extends SVGProps<SVGSVGElement> {
  name: RgVectorIconId;
}

const SHARED = {
  fill: "none",
  stroke: "currentColor",
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  strokeWidth: 1.8,
  viewBox: "0 0 24 24",
};

/**
 * Jeu d'icônes vectorielles métier Rose Griffon (aucun emoji, jetons thématiques uniquement).
 */
export function RgVectorIcon({ className, name, ...rest }: SvgIconProps) {
  const cls = cn("size-5 shrink-0", className);

  switch (name) {
    case "home":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path d="M12 2.5 19.5 5.8V11.8c0 4.8-3.2 8.2-7.5 9.7-4.3-1.5-7.5-4.9-7.5-9.7V5.8L12 2.5Z" />
          <path d="M9.2 12.2 12 8.8l2.8 3.4-2.8 3.4-2.8-3.4Z" fill="currentColor" fillOpacity="0.22" />
          <path d="M12 8.8v6.8" />
        </svg>
      );
    case "calendar-cup":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <rect x="3.5" y="5" width="17" height="15.5" rx="3" />
          <path d="M8 3.2v3.2M16 3.2v3.2M3.5 9.5h17" />
          <path d="M9.5 13h5l-1 2.8a1.5 1.5 0 0 1-3 0L9.5 13Z" fill="currentColor" fillOpacity="0.22" />
          <path d="M12 16v2.2" />
        </svg>
      );
    case "ballot":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <rect x="3.5" y="4" width="17" height="16" rx="3" />
          <path d="M7.5 14.5v-3.5M12 14.5v-6M16.5 14.5v-2" strokeWidth="2.2" />
          <path d="M7 17.2h10" strokeOpacity="0.55" />
        </svg>
      );
    case "pulse-feed":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <circle cx="12" cy="12" r="8.5" />
          <path d="M7.5 13.5 10.2 10l2.4 4.2 2.2-3 1.8 2.3" />
          <circle cx="17.5" cy="6.5" r="2" fill="currentColor" stroke="none" />
        </svg>
      );
    case "workshop":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path d="M12 3.2 20 7.7v8.6L12 20.8 4 16.3V7.7L12 3.2Z" />
          <path d="M12 20.8V12l8-4.3M12 12 4 7.7" />
          <circle cx="12" cy="7.7" r="1.4" fill="currentColor" />
        </svg>
      );
    case "patron-rose":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path
            d="M12 4.2c1.7-1.5 4.7-1.2 6 .8 1.5 2.4.5 5.4-1.7 7.6L12 19.6l-4.3-7c-2.2-2.2-3.2-5.2-1.7-7.6 1.3-2 4.3-2.3 6-.8Z"
            fill="currentColor"
            fillOpacity="0.18"
          />
          <path d="M12 8.5c1.1 0 2 .9 2 2s-.9 2-2 2-2-.9-2-2" />
          <path d="M12 19.6v1.9" />
        </svg>
      );
    case "patch-diff":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <rect x="4" y="3.5" width="16" height="17" rx="2.5" />
          <path d="M8.5 8.5h7M8.5 12h4.5M8.5 15.5h6" />
          <path d="M15.5 14v3M14 15.5h3" />
        </svg>
      );
    case "level-chart":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path d="M4 18.5h16" />
          <path d="M6.5 15.5v-3.5M10.5 15.5V9.5M14.5 15.5V11M18.5 15.5V6.5" strokeWidth="2.2" />
          <path d="M6 8.5 11.5 5l3 2 4-3" />
        </svg>
      );
    case "crest-plus":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path d="M12 3 19 6v5.5c0 4.4-2.9 7.6-7 9-4.1-1.4-7-4.6-7-9V6l7-3Z" />
          <path d="M12 8.5v6M9 11.5h6" />
        </svg>
      );
    case "shield-board":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <rect x="3.5" y="4" width="17" height="16" rx="2.5" />
          <path d="M3.5 9.5h17M10.5 9.5V20" />
          <circle cx="7" cy="14.5" r="1.4" fill="currentColor" stroke="none" />
        </svg>
      );
    case "match-history":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path d="M4.5 12a7.5 7.5 0 1 0 2.2-5.3L4.5 9" />
          <path d="M4.5 5v4h4M12 8.2v4.1l2.7 1.6" />
        </svg>
      );
    case "bracket-plus":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path d="M4 6h4v4H4M4 14h4v4H4M8 8h4v8H8M12 12h3.5" />
          <path d="M18.5 9.5v5M16 12h5" />
        </svg>
      );
    case "guild":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path d="M4 20v-9.5L12 5l8 5.5V20H4Z" />
          <path d="M9.5 20v-5h5v5" />
          <circle cx="12" cy="9.8" r="1.2" fill="currentColor" stroke="none" />
        </svg>
      );
    case "playbook":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path d="M5 4.5h9.5a2.5 2.5 0 0 1 2.5 2.5v12.5H7.5A2.5 2.5 0 0 1 5 17V4.5Z" />
          <path d="M17 7h2v12.5H7.5" />
          <path d="M8.5 9.5h5M8.5 13h3.5" />
        </svg>
      );
    case "cube-3d":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path d="M12 3 20 7.5v9L12 21l-8-4.5v-9L12 3Z" />
          <path d="M12 12v9M12 12l8-4.5M12 12 4 7.5" />
        </svg>
      );
    case "news-press":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <rect x="4" y="4" width="16" height="16" rx="2.5" />
          <rect x="7" y="7.5" width="5" height="4.5" rx="0.8" fill="currentColor" fillOpacity="0.22" />
          <path d="M14 8.5h3M14 11.5h3M7 15.2h10" />
        </svg>
      );
    case "palette":
      return (
        <svg {...SHARED} {...rest} className={cls}>
          <path d="M12 3.5a8.5 8.5 0 1 0 0 17c1.4 0 2.3-.9 2.3-2 0-.6-.3-1.1-.6-1.5-.4-.5-.1-1.2.5-1.2h2.2c2.2 0 4.1-1.7 4.1-3.9A8.5 8.5 0 0 0 12 3.5Z" />
          <circle cx="8" cy="10.5" r="1.1" fill="currentColor" stroke="none" />
          <circle cx="11.5" cy="7.5" r="1.1" fill="currentColor" stroke="none" />
          <circle cx="15.5" cy="10" r="1.1" fill="currentColor" stroke="none" />
        </svg>
      );
  }
}
