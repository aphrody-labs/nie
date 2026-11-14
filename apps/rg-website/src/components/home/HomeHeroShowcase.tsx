import { Button } from "@rosegriffon/ui";

import Link from "@/components/shared/Link";
import { CommonSpriteIcon } from "@/components/ui/CommonSpriteIcon";
import { ArrowRight } from "@/lib/icons-config";
import { HERO_SHOWCASE_DATA } from "@/lib/home-showcase";
import type { HomeShowcaseData } from "@/lib/home-showcase-server";

interface HomeHeroShowcaseProps {
  rgtvFollowers: number | null;
  showcase: HomeShowcaseData;
}

export function HomeHeroShowcase({
  rgtvFollowers,
  showcase,
}: HomeHeroShowcaseProps) {
  const { achillea, nextStream, wikiStats } = showcase;

  return (
    <section
      aria-label={HERO_SHOWCASE_DATA.ariaLabel}
      className="mx-auto w-full max-w-7xl px-4 sm:px-6 lg:px-8"
    >
      <div className="rg-hero-surface rounded-3xl border border-border/80 shadow-md">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 opacity-20 mix-blend-soft-light"
        >
          <img
            src={HERO_SHOWCASE_DATA.backdrop.src}
            alt=""
            width={2560}
            height={1190}
            className="size-full object-cover object-center"
          />
        </div>

        <div className="relative z-10 grid items-center gap-8 p-6 sm:p-8 lg:grid-cols-12 lg:gap-10 lg:p-10">
          <div className="flex flex-col gap-5 lg:col-span-6">
            <div className="inline-flex w-fit items-center gap-2.5 rounded-full border border-border bg-background/85 px-3.5 py-1.5 backdrop-blur-md">
              <img
                src={HERO_SHOWCASE_DATA.badge.logo}
                alt={HERO_SHOWCASE_DATA.badge.alt}
                width={24}
                height={24}
                className="size-5 rounded-sm object-contain"
              />
              <span className="text-xs font-bold uppercase tracking-wider text-foreground">
                {HERO_SHOWCASE_DATA.badge.text}
              </span>
            </div>

            <div className="space-y-3">
              <h1 className="text-4xl font-black tracking-tight text-foreground sm:text-5xl lg:text-6xl">
                {HERO_SHOWCASE_DATA.title}
              </h1>
              <p className="max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
                {HERO_SHOWCASE_DATA.description}
              </p>
            </div>

            <div className="flex flex-wrap items-center gap-3 pt-1">
              {HERO_SHOWCASE_DATA.ctas.map((cta) => {
                const variant =
                  cta.variant === "primary"
                    ? "default"
                    : cta.variant === "outline"
                      ? "outline"
                      : "secondary";
                return (
                  <Button
                    key={cta.href}
                    asChild
                    size="lg"
                    variant={variant}
                    className="rounded-xl font-semibold shadow-xs"
                  >
                    <Link href={cta.href}>
                      <img
                        src={cta.icon}
                        alt={cta.iconAlt}
                        width={20}
                        height={20}
                        className="size-5 rounded-sm object-contain"
                      />
                      <span>{cta.label}</span>
                      <ArrowRight className="size-4 opacity-75" />
                    </Link>
                  </Button>
                );
              })}
            </div>

            <nav
              aria-label={HERO_SHOWCASE_DATA.metricsAriaLabel}
              className="grid grid-cols-1 gap-2.5 pt-2 sm:grid-cols-3"
            >
              <Link
                href="/wiki"
                className="flex items-center gap-2.5 rounded-xl border border-border/80 bg-background/75 p-3 backdrop-blur-md transition-colors hover:bg-background"
              >
                <CommonSpriteIcon name="chara" scale={0.34} />
                <div className="min-w-0 leading-tight">
                  <p className="font-mono text-sm font-bold tabular-nums text-foreground">
                    {wikiStats.characters > 0
                      ? wikiStats.characters.toLocaleString("fr-FR")
                      : HERO_SHOWCASE_DATA.metricsLabels.wikiFallbackTitle}
                  </p>
                  <p className="truncate text-xs text-muted-foreground">
                    {HERO_SHOWCASE_DATA.metricsLabels.wiki}
                  </p>
                </div>
              </Link>

              <Link
                href="/achillea"
                className="flex items-center gap-2.5 rounded-xl border border-border/80 bg-background/75 p-3 backdrop-blur-md transition-colors hover:bg-background"
              >
                <img
                  src="/achillea/ranks/supernova.webp"
                  alt=""
                  width={28}
                  height={28}
                  aria-hidden="true"
                  className="size-7 shrink-0 object-contain"
                />
                <div className="min-w-0 leading-tight">
                  <p className="font-mono text-sm font-bold tabular-nums text-foreground">
                    {achillea.stats.joueurs > 0
                      ? achillea.stats.joueurs.toLocaleString("fr-FR")
                      : HERO_SHOWCASE_DATA.metricsLabels.achilleaFallbackTitle}
                  </p>
                  <p className="truncate text-xs text-muted-foreground">
                    {achillea.stats.clans > 0
                      ? `${achillea.stats.clans} ${HERO_SHOWCASE_DATA.metricsLabels.clans}`
                      : HERO_SHOWCASE_DATA.metricsLabels.achillea}
                  </p>
                </div>
              </Link>

              <Link
                href="/rose-griffon-tv"
                className="flex items-center gap-2.5 rounded-xl border border-border/80 bg-background/75 p-3 backdrop-blur-md transition-colors hover:bg-background"
              >
                <img
                  src="/rgtv/rgtv.webp"
                  alt=""
                  width={28}
                  height={28}
                  aria-hidden="true"
                  className="size-7 shrink-0 rounded-md object-cover"
                />
                <div className="min-w-0 leading-tight">
                  <p className="font-mono text-sm font-bold tabular-nums text-foreground">
                    {rgtvFollowers != null && rgtvFollowers > 0
                      ? rgtvFollowers.toLocaleString("fr-FR")
                      : HERO_SHOWCASE_DATA.metricsLabels.rgtvTitle}
                  </p>
                  <p className="truncate text-xs text-muted-foreground">
                    {nextStream?.startTime
                      ? `${nextStream.startTime.slice(0, 5)} · ${nextStream.streamerName}`
                      : rgtvFollowers != null && rgtvFollowers > 0
                        ? HERO_SHOWCASE_DATA.metricsLabels.rgtv
                        : HERO_SHOWCASE_DATA.metricsLabels.rgtvFallback}
                  </p>
                </div>
              </Link>
            </nav>
          </div>

          <div className="flex flex-col gap-3 lg:col-span-6">
            <div className="relative overflow-hidden rounded-2xl border border-border/80 bg-muted shadow-lg aspect-[2560/1811]">
              <img
                src={HERO_SHOWCASE_DATA.artwork.src}
                alt={HERO_SHOWCASE_DATA.artwork.alt}
                width={2560}
                height={1811}
                loading="eager"
                fetchPriority="high"
                decoding="sync"
                className="size-full object-cover object-center"
              />
            </div>

            <div className="grid grid-cols-2 gap-3">
              {HERO_SHOWCASE_DATA.charactersChip.map((chip) => (
                <Link
                  key={chip.name}
                  href={chip.href}
                  className="flex items-center gap-3 rounded-xl border border-border/80 bg-background/80 p-2.5 backdrop-blur-md transition-colors hover:bg-background"
                >
                  <img
                    src={chip.image}
                    alt={chip.alt}
                    width={40}
                    height={40}
                    className="size-10 shrink-0 rounded-lg bg-muted object-cover"
                  />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-bold text-foreground">
                      {chip.name}
                    </p>
                    <p className="truncate text-xs text-muted-foreground">
                      {chip.role}
                    </p>
                  </div>
                  <ArrowRight className="size-3.5 shrink-0 text-muted-foreground" />
                </Link>
              ))}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
