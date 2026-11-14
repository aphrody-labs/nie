import { rankImagePath } from "@rosegriffon/achillea-core";
import { Badge, Button, Card } from "@rosegriffon/ui";

import Link from "@/components/shared/Link";
import { CommonSpriteIcon } from "@/components/ui/CommonSpriteIcon";
import { BENTO_COPY, ELEMENT_ICON_BY_NAME } from "@/lib/home-showcase";
import type { HomeShowcaseData } from "@/lib/home-showcase-server";
import { ArrowRight } from "@/lib/icons-config";

interface HomeUniversesBentoProps {
  rgtvFollowers: number | null;
  showcase: HomeShowcaseData;
}

export function HomeUniversesBento({
  rgtvFollowers,
  showcase,
}: HomeUniversesBentoProps) {
  const { achillea, characters, nextStream, wikiStats } = showcase;

  return (
    <section
      aria-label={BENTO_COPY.ariaLabel}
      className="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 sm:px-6 lg:px-8"
    >
      <div className="flex flex-col gap-2">
        <h2 className="text-2xl font-black tracking-tight text-foreground sm:text-3xl">
          {BENTO_COPY.heading}
        </h2>
        <p className="max-w-2xl text-sm text-muted-foreground sm:text-base">
          {BENTO_COPY.subHeading}
        </p>
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-12">
        <Card className="rg-bento-azalee flex flex-col justify-between gap-5 overflow-hidden rounded-2xl border border-border/80 p-5 shadow-xs lg:col-span-5">
          <div className="flex flex-col gap-4">
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-center gap-3">
                <img
                  src={BENTO_COPY.azalee.logoSrc}
                  alt={BENTO_COPY.azalee.logoAlt}
                  width={44}
                  height={44}
                  className="size-11 shrink-0 rounded-xl border border-border/70 bg-background/85 p-1 object-contain"
                />
                <div>
                  <Badge variant="secondary" className="mb-1">
                    {BENTO_COPY.azalee.badge}
                  </Badge>
                  <h3 className="text-lg font-extrabold text-foreground">
                    {BENTO_COPY.azalee.title}
                  </h3>
                </div>
              </div>
              {wikiStats.skills > 0 && (
                <Badge variant="outline" className="font-mono tabular-nums">
                  {wikiStats.skills.toLocaleString("fr-FR")}
                </Badge>
              )}
            </div>

            <p className="text-sm leading-relaxed text-muted-foreground">
              {BENTO_COPY.azalee.subtitle}
            </p>

            <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-4 lg:grid-cols-2">
              {characters.map((chara) => {
                const elemIcon =
                  ELEMENT_ICON_BY_NAME[chara.element] ??
                  "/spirit_type/wind.webp";
                return (
                  <Link
                    key={chara.internalCode}
                    href={chara.href}
                    className="group flex items-center gap-2.5 rounded-xl border border-border/80 bg-background/85 p-2 backdrop-blur-sm transition-colors hover:border-primary/60 hover:bg-background"
                  >
                    <img
                      src={chara.portraitUrl}
                      alt={chara.name}
                      width={48}
                      height={48}
                      loading="lazy"
                      className="size-11 shrink-0 rounded-lg bg-muted object-cover transition-transform group-hover:scale-105"
                    />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-xs font-bold text-foreground">
                        {chara.name}
                      </p>
                      <div className="mt-0.5 flex items-center gap-1">
                        <img
                          src={elemIcon}
                          alt={chara.element}
                          width={14}
                          height={14}
                          className="size-3.5 shrink-0 object-contain"
                        />
                        <span className="truncate text-xs text-muted-foreground">
                          {chara.element}
                        </span>
                      </div>
                    </div>
                  </Link>
                );
              })}
            </div>
          </div>

          <Button asChild variant="outline" className="w-full rounded-xl">
            <Link href={BENTO_COPY.azalee.ctaHref}>
              <CommonSpriteIcon name="chara" scale={0.22} />
              <span>{BENTO_COPY.azalee.ctaLabel}</span>
              <ArrowRight className="ml-auto size-4" />
            </Link>
          </Button>
        </Card>

        <Card className="rg-bento-achillea flex flex-col justify-between gap-5 overflow-hidden rounded-2xl border border-border/80 p-5 shadow-xs lg:col-span-4">
          <div className="flex flex-col gap-4">
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-center gap-3">
                <img
                  src={BENTO_COPY.achillea.logoSrc}
                  alt={BENTO_COPY.achillea.logoAlt}
                  width={44}
                  height={44}
                  className="size-11 shrink-0 rounded-xl border border-border/70 bg-background/85 p-1 object-contain"
                />
                <div>
                  <Badge variant="secondary" className="mb-1">
                    {BENTO_COPY.achillea.badge}
                  </Badge>
                  <h3 className="text-lg font-extrabold text-foreground">
                    {BENTO_COPY.achillea.title}
                  </h3>
                </div>
              </div>
              <Link
                href={BENTO_COPY.achillea.astralHref}
                title={BENTO_COPY.achillea.astralLabel}
                className="flex size-10 shrink-0 items-center justify-center rounded-lg border border-border/80 bg-background/85 p-1 transition-colors hover:bg-background"
              >
                <img
                  src={BENTO_COPY.achillea.astralImage}
                  alt={BENTO_COPY.achillea.astralAlt}
                  width={28}
                  height={28}
                  className="size-7 object-contain"
                />
              </Link>
            </div>

            <p className="text-sm leading-relaxed text-muted-foreground">
              {BENTO_COPY.achillea.subtitle}
            </p>

            {achillea.leaderboard.length > 0 && (
              <div className="flex flex-col gap-2">
                {achillea.leaderboard.map((row) => (
                  <Link
                    key={row.userId}
                    href={`/achillea/users/${row.userId}`}
                    className="flex items-center gap-2.5 rounded-xl border border-border/80 bg-background/85 px-3 py-2 transition-colors hover:bg-background"
                  >
                    <span className="w-4 text-center font-mono text-xs font-bold tabular-nums text-muted-foreground">
                      {row.rang}
                    </span>
                    <img
                      src={rankImagePath(row.palier, "/achillea/ranks/")}
                      alt={row.palier ?? row.nom}
                      width={24}
                      height={24}
                      className="size-6 shrink-0 object-contain"
                    />
                    <span className="min-w-0 flex-1 truncate text-xs font-semibold text-foreground">
                      {row.nom}
                    </span>
                    <span className="font-mono text-xs font-bold tabular-nums text-primary">
                      {row.elo}
                    </span>
                  </Link>
                ))}
              </div>
            )}

            {achillea.clans.length > 0 && (
              <div className="flex flex-wrap gap-2">
                {achillea.clans.map((clan) => (
                  <Link
                    key={clan.id}
                    href="/achillea/clans"
                    className="inline-flex items-center gap-1.5 rounded-lg border border-border/70 bg-background/80 px-2.5 py-1 text-xs font-semibold text-foreground transition-colors hover:bg-background"
                  >
                    <img
                      src={
                        clan.clanAvatarUrl ||
                        BENTO_COPY.achillea.fallbackClanAvatar
                      }
                      alt={clan.name}
                      width={18}
                      height={18}
                      className="size-4 shrink-0 rounded-full object-cover"
                    />
                    <span className="truncate">
                      {clan.shortenedName || clan.name}
                    </span>
                  </Link>
                ))}
              </div>
            )}
          </div>

          <Button asChild variant="outline" className="w-full rounded-xl">
            <Link href={BENTO_COPY.achillea.ctaHref}>
              <img
                src="/achillea/ranks/legendaire.webp"
                alt=""
                width={18}
                height={18}
                aria-hidden="true"
                className="size-4.5 object-contain"
              />
              <span>{BENTO_COPY.achillea.ctaLabel}</span>
              <ArrowRight className="ml-auto size-4" />
            </Link>
          </Button>
        </Card>

        <Card className="rg-bento-rgtv flex flex-col justify-between gap-5 overflow-hidden rounded-2xl border border-border/80 p-5 shadow-xs lg:col-span-3">
          <div className="flex flex-col gap-3.5">
            <div className="flex items-center gap-3">
              <img
                src={BENTO_COPY.rgtv.logoSrc}
                alt={BENTO_COPY.rgtv.logoAlt}
                width={44}
                height={44}
                className="size-11 shrink-0 rounded-xl border border-border/70 object-cover"
              />
              <div className="min-w-0">
                <Badge variant="secondary" className="mb-1">
                  {BENTO_COPY.rgtv.badge}
                </Badge>
                <h3 className="truncate text-lg font-extrabold text-foreground">
                  {BENTO_COPY.rgtv.title}
                </h3>
              </div>
            </div>

            <div className="overflow-hidden rounded-xl border border-border/70 bg-muted aspect-[16/11]">
              <img
                src={BENTO_COPY.rgtv.artworkSrc}
                alt={BENTO_COPY.rgtv.artworkAlt}
                width={2407}
                height={2560}
                loading="lazy"
                className="size-full object-cover object-top"
              />
            </div>

            <div className="space-y-1">
              <p className="text-xs font-bold text-foreground">
                {nextStream?.startTime
                  ? `${nextStream.startTime.slice(0, 5)} · ${nextStream.streamerName}`
                  : rgtvFollowers != null && rgtvFollowers > 0
                    ? `${rgtvFollowers.toLocaleString("fr-FR")}`
                    : BENTO_COPY.rgtv.offlineBadge}
              </p>
              <p className="text-xs leading-relaxed text-muted-foreground">
                {nextStream?.category || BENTO_COPY.rgtv.subtitle}
              </p>
            </div>
          </div>

          <Button asChild variant="outline" className="w-full rounded-xl">
            <Link href={BENTO_COPY.rgtv.ctaHref}>
              <img
                src={BENTO_COPY.rgtv.logoSrc}
                alt=""
                width={18}
                height={18}
                aria-hidden="true"
                className="size-4.5 rounded-sm object-cover"
              />
              <span>{BENTO_COPY.rgtv.ctaLabel}</span>
              <ArrowRight className="ml-auto size-4" />
            </Link>
          </Button>
        </Card>
      </div>
    </section>
  );
}
