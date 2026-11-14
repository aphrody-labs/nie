"use client";

import { useTheme } from "next-themes";
import { useSyncExternalStore } from "react";

import { RgVectorIcon } from "@/components/icons/RgVectorIcons";
import Link from "@/components/shared/Link";
import { CommonSpriteIcon } from "@/components/ui/CommonSpriteIcon";
import {
  type RgThemeId,
  SIDEBAR_FOOTER_COPY,
  SIDEBAR_MASCOTS,
} from "@/lib/sidebar-showcase";

const noopSubscribe = () => () => {};
const getClientMounted = () => true;
const getServerMounted = () => false;

function resolveThemeId(value: string | undefined): RgThemeId {
  if (value && value in SIDEBAR_MASCOTS) return value as RgThemeId;
  return "roy";
}

/**
 * Compagnon de thème et raccourcis directs (IEVR Web + Studio 3D) en pied de sidebar.
 */
export function RgSidebarFooter() {
  const { theme } = useTheme();
  const mounted = useSyncExternalStore(
    noopSubscribe,
    getClientMounted,
    getServerMounted,
  );

  const mascot = SIDEBAR_MASCOTS[resolveThemeId(mounted ? theme : "roy")];

  return (
    <div className="border-t border-sidebar-border px-2.5 py-2.5">
      <div className="group-data-[collapsible=icon]:hidden flex flex-col gap-2">
        <div className="flex items-center gap-2.5 rounded-lg border border-sidebar-border bg-sidebar-accent/40 p-2">
          <img
            src={mascot.image}
            alt={mascot.alt}
            width={36}
            height={36}
            className="size-9 shrink-0 rounded-md bg-sidebar-primary/30 object-cover"
          />
          <div className="min-w-0 flex-1 leading-tight">
            <p className="truncate text-xs font-bold text-sidebar-foreground">
              {mascot.name}
            </p>
            <p className="truncate text-xs text-sidebar-foreground/75">
              {mascot.role}
            </p>
          </div>
          <Link
            href={SIDEBAR_FOOTER_COPY.texturesHref}
            title={SIDEBAR_FOOTER_COPY.texturesTitle}
            aria-label={SIDEBAR_FOOTER_COPY.texturesLabel}
            className="flex size-7 shrink-0 items-center justify-center rounded-md border border-sidebar-border bg-sidebar-accent/60 text-sidebar-foreground transition-colors hover:bg-sidebar-accent"
          >
            <RgVectorIcon name="palette" className="size-4" />
          </Link>
        </div>

        <nav
          aria-label={SIDEBAR_FOOTER_COPY.navAriaLabel}
          className="grid grid-cols-2 gap-1.5"
        >
          <Link
            href={SIDEBAR_FOOTER_COPY.playHref}
            title={SIDEBAR_FOOTER_COPY.playTitle}
            className="flex items-center justify-center gap-1.5 rounded-md border border-sidebar-border bg-sidebar-accent/55 px-2 py-1.5 text-xs font-semibold text-sidebar-foreground transition-colors hover:bg-sidebar-accent"
          >
            <CommonSpriteIcon name="justice" scale={0.18} />
            <span className="truncate">{SIDEBAR_FOOTER_COPY.playLabel}</span>
          </Link>
          <Link
            href={SIDEBAR_FOOTER_COPY.studioHref}
            title={SIDEBAR_FOOTER_COPY.studioTitle}
            className="flex items-center justify-center gap-1.5 rounded-md border border-sidebar-border bg-sidebar-accent/55 px-2 py-1.5 text-xs font-semibold text-sidebar-foreground transition-colors hover:bg-sidebar-accent"
          >
            <RgVectorIcon name="cube-3d" className="size-3.5" />
            <span className="truncate">{SIDEBAR_FOOTER_COPY.studioLabel}</span>
          </Link>
        </nav>
      </div>

      <div className="hidden group-data-[collapsible=icon]:flex flex-col items-center gap-2">
        <Link
          href={SIDEBAR_FOOTER_COPY.playHref}
          title={SIDEBAR_FOOTER_COPY.playTitle}
          aria-label={SIDEBAR_FOOTER_COPY.playLabel}
          className="flex size-9 items-center justify-center rounded-md border border-sidebar-border bg-sidebar-accent/55 text-sidebar-foreground transition-colors hover:bg-sidebar-accent"
        >
          <CommonSpriteIcon name="justice" scale={0.22} />
        </Link>
        <Link
          href={SIDEBAR_FOOTER_COPY.studioHref}
          title={SIDEBAR_FOOTER_COPY.studioTitle}
          aria-label={SIDEBAR_FOOTER_COPY.studioLabel}
          className="flex size-9 items-center justify-center rounded-md border border-sidebar-border bg-sidebar-accent/55 text-sidebar-foreground transition-colors hover:bg-sidebar-accent"
        >
          <RgVectorIcon name="cube-3d" className="size-4" />
        </Link>
      </div>
    </div>
  );
}
