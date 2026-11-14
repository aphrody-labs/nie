"use client";

import Image from "next/image";
import Link from "@/components/shared/Link";
import { useMemo } from "react";
import {
  ArrowLeft,
  ArrowRight,
  BookOpen,
  Calendar,
  CheckCircle2,
  ExternalLink,
  Gamepad2,
  Package,
  ShoppingBag,
  User,
  Users,
} from "lucide-react";
import {
  getLocalizedEra,
  getLocalizedGame,
  getLocalizedStatusLabel,
  INAZUMA_ERAS,
  normalizeGameLocale,
  type InazumaGame,
} from "@rosegriffon/inagle/games";
import { useLanguage } from "@/components/providers/language-provider";
import { getGameDetailCopy } from "@/lib/wiki/games-copy";

export interface FeaturedGameCharacter {
  id: string;
  slug: string | null;
  displayName: string;
  avatar: string;
  subtitle: string | null;
}

export interface GameDetailClientProps {
  game: InazumaGame;
  otherGames: readonly InazumaGame[];
  playersHref: string;
  featuredChars: readonly FeaturedGameCharacter[];
}

export function GameDetailClient({
  game,
  otherGames,
  playersHref,
  featuredChars,
}: GameDetailClientProps) {
  const { language } = useLanguage();
  const locale = normalizeGameLocale(language);
  const copy = useMemo(() => getGameDetailCopy(locale), [locale]);
  const loc = useMemo(() => getLocalizedGame(game, locale), [game, locale]);
  const statusMeta = useMemo(
    () => getLocalizedStatusLabel(game.status, locale),
    [game.status, locale],
  );
  const era = useMemo(() => {
    const raw = INAZUMA_ERAS.find((e) => e.id === game.era);
    return raw ? getLocalizedEra(raw, locale) : null;
  }, [game.era, locale]);

  const subtitleAlt = [game.titleJa, game.titleEn, game.titleEs]
    .filter((t, i, arr) => t && t !== loc.title && arr.indexOf(t) === i)
    .join(` ${copy.metaSep} `);

  return (
    <div className="space-y-8">
      <nav className="flex items-center gap-2 text-xs text-on-surface-variant">
        <Link href="/wiki" className="hover:text-primary transition-colors">
          {copy.wikiHome}
        </Link>
        <span>{copy.breadcrumbSep}</span>
        <Link
          href="/wiki/jeux"
          className="hover:text-primary transition-colors"
        >
          {copy.gamesIndex}
        </Link>
        <span>{copy.breadcrumbSep}</span>
        <span className="text-on-surface font-semibold truncate">
          {loc.title}
        </span>
      </nav>

      {/* Hero du jeu */}
      <section className="rounded-3xl border border-outline-variant/20 bg-surface-container overflow-hidden">
        <div className="grid grid-cols-1 lg:grid-cols-12">
          <div className="relative lg:col-span-5 aspect-[16/10] lg:aspect-auto min-h-60 bg-surface-container-highest">
            <Image
              src={game.cover || game.thumbnail}
              alt={loc.title}
              fill
              priority
              sizes="(max-width: 1024px) 100vw, 42vw"
              className="object-cover"
            />
          </div>

          <div className="lg:col-span-7 p-6 md:p-8 flex flex-col justify-between gap-6">
            <div className="space-y-3">
              <div className="flex flex-wrap items-center gap-2">
                <span className="px-3 py-1 rounded-full text-xs font-bold bg-primary-container text-on-primary-container">
                  {statusMeta.label}
                </span>
                {era && (
                  <span className="px-3 py-1 rounded-full text-xs font-semibold bg-secondary-container text-on-secondary-container">
                    {era.label}
                  </span>
                )}
              </div>

              <div>
                <h1 className="text-2xl md:text-3xl font-black text-on-surface tracking-tight">
                  {loc.title}
                </h1>
                {subtitleAlt && (
                  <p className="text-xs md:text-sm text-on-surface-variant mt-1">
                    {subtitleAlt}
                  </p>
                )}
              </div>

              <p className="text-sm md:text-base text-on-surface-variant leading-relaxed">
                {loc.summary}
              </p>
            </div>

            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 pt-4 border-t border-outline-variant/15 text-xs">
              <div>
                <div className="text-on-surface-variant flex items-center gap-1">
                  <Calendar className="size-3.5 text-primary" />
                  <span>{copy.metadataRelease}</span>
                </div>
                <div className="font-bold text-on-surface mt-0.5">
                  {game.releaseDateJa || game.releaseYear}
                </div>
              </div>
              <div>
                <div className="text-on-surface-variant flex items-center gap-1">
                  <User className="size-3.5 text-primary" />
                  <span>{copy.metadataProtagonist}</span>
                </div>
                <div className="font-bold text-on-surface mt-0.5 line-clamp-2">
                  {game.protagonist}
                </div>
              </div>
              <div>
                <div className="text-on-surface-variant flex items-center gap-1">
                  <Gamepad2 className="size-3.5 text-primary" />
                  <span>{copy.metadataPlatforms}</span>
                </div>
                <div className="font-bold text-on-surface mt-0.5 line-clamp-2">
                  {game.platforms.join(", ")}
                </div>
              </div>
              <div>
                <div className="text-on-surface-variant">
                  {copy.metadataDeveloper}
                </div>
                <div className="font-bold text-on-surface mt-0.5">
                  {game.developer}
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Points clés & liens */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <section className="lg:col-span-2 rounded-2xl border border-outline-variant/20 bg-surface-container p-6 space-y-4">
          <h2 className="text-lg font-black text-on-surface">
            {copy.highlightsHeading}
          </h2>
          <p className="text-xs font-semibold text-primary">
            {loc.statusDetail}
          </p>
          <ul className="space-y-2.5">
            {loc.highlights.map((point) => (
              <li
                key={point}
                className="flex items-start gap-2.5 text-sm text-on-surface-variant"
              >
                <CheckCircle2 className="size-4 text-primary shrink-0 mt-0.5" />
                <span>{point}</span>
              </li>
            ))}
          </ul>
        </section>

        <aside className="rounded-2xl border border-outline-variant/20 bg-surface-container p-6 space-y-4">
          <h2 className="text-lg font-black text-on-surface">
            {copy.linksHeading}
          </h2>
          <div className="flex flex-col gap-2.5">
            {loc.internalLinks.map((link) => (
              <Link
                key={link.href}
                href={link.href}
                className="flex items-center justify-between px-3.5 py-2.5 rounded-xl bg-surface-container-high hover:border-primary/40 border border-outline-variant/15 text-sm font-bold text-on-surface hover:text-primary transition"
              >
                <span>{link.label}</span>
                <ArrowRight className="size-4" />
              </Link>
            ))}

            <a
              href={game.officialUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center justify-between px-3.5 py-2.5 rounded-xl bg-primary text-on-primary text-sm font-bold hover:opacity-90 transition"
            >
              <span>{copy.officialPortalLabel}</span>
              <ExternalLink className="size-4" />
            </a>

            {game.storeUrl && (
              <a
                href={game.storeUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center justify-between px-3.5 py-2.5 rounded-xl bg-secondary-container text-on-secondary-container text-sm font-bold hover:opacity-90 transition"
              >
                <span>{copy.storeLabel}</span>
                <ShoppingBag className="size-4" />
              </a>
            )}
          </div>
        </aside>
      </div>

      {/* Wikis communautaires (JA / EN / FR / ES) & Hubs de mods */}
      {(game.communityWikis.length > 0 || game.modHubs.length > 0) && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {game.communityWikis.length > 0 && (
            <section className="rounded-2xl border border-outline-variant/20 bg-surface-container p-6 space-y-4">
              <h2 className="text-lg font-black text-on-surface flex items-center gap-2">
                <BookOpen className="size-5 text-primary" />
                <span>{copy.communityWikisHeading}</span>
              </h2>
              <div className="flex flex-col gap-2.5">
                {game.communityWikis.map((wiki) => (
                  <a
                    key={`${wiki.lang}-${wiki.url}`}
                    href={wiki.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="flex items-center justify-between px-3.5 py-2.5 rounded-xl bg-surface-container-high border border-outline-variant/15 text-sm font-bold text-on-surface hover:border-primary/40 hover:text-primary transition"
                  >
                    <span className="flex items-center gap-2.5 min-w-0">
                      <span className="px-2 py-0.5 rounded-md text-xs font-black uppercase bg-secondary-container text-on-secondary-container shrink-0">
                        {wiki.lang}
                      </span>
                      <span className="truncate">{wiki.name}</span>
                    </span>
                    <ExternalLink className="size-4 shrink-0" />
                  </a>
                ))}
              </div>
            </section>
          )}

          {game.modHubs.length > 0 && (
            <section className="rounded-2xl border border-outline-variant/20 bg-surface-container p-6 space-y-4">
              <div className="flex items-center justify-between gap-2">
                <h2 className="text-lg font-black text-on-surface flex items-center gap-2">
                  <Package className="size-5 text-primary" />
                  <span>{copy.modHubsHeading}</span>
                </h2>
                <Link
                  href="/wiki/mods"
                  className="text-xs font-bold text-primary hover:underline"
                >
                  {copy.modsDirectoryCta}
                </Link>
              </div>
              <div className="flex flex-col gap-2.5">
                {game.modHubs.map((hub) => (
                  <a
                    key={hub.url}
                    href={hub.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="flex items-center justify-between px-3.5 py-2.5 rounded-xl bg-surface-container-high border border-outline-variant/15 text-sm font-bold text-on-surface hover:border-primary/40 hover:text-primary transition"
                  >
                    <span className="truncate">{hub.name}</span>
                    <ExternalLink className="size-4 shrink-0" />
                  </a>
                ))}
              </div>
            </section>
          )}
        </div>
      )}

      {/* Personnages liés dans la base PostgreSQL */}
      {featuredChars.length > 0 && (
        <section className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <h2 className="text-lg md:text-xl font-black text-on-surface flex items-center gap-2">
              <Users className="size-5 text-primary" />
              <span>{copy.charactersHeading}</span>
            </h2>
            <Link
              href={playersHref}
              className="inline-flex items-center gap-1.5 text-xs md:text-sm font-bold text-primary hover:underline"
            >
              <span>{copy.charactersAllCta}</span>
              <ArrowRight className="size-4" />
            </Link>
          </div>

          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-3.5">
            {featuredChars.map((ch) => (
              <Link
                key={ch.id}
                href={`/wiki/chara/${ch.slug || ch.id}`}
                className="group rounded-2xl border border-outline-variant/15 bg-surface-container p-3 hover:border-primary/40 transition flex flex-col gap-2"
              >
                <div className="relative aspect-square rounded-xl bg-surface-container-high overflow-hidden">
                  <Image
                    src={ch.avatar}
                    alt={ch.displayName}
                    fill
                    sizes="(max-width: 640px) 50vw, 180px"
                    className="object-contain p-1 group-hover:scale-105 transition-transform"
                  />
                </div>
                <div>
                  <div className="text-xs font-bold text-on-surface line-clamp-1 group-hover:text-primary">
                    {ch.displayName}
                  </div>
                  {ch.subtitle && (
                    <div className="text-xs text-on-surface-variant line-clamp-1">
                      {ch.subtitle}
                    </div>
                  )}
                </div>
              </Link>
            ))}
          </div>
        </section>
      )}

      {/* Autres jeux */}
      <section className="space-y-4 pt-4 border-t border-outline-variant/15">
        <div className="flex items-center justify-between gap-4">
          <h2 className="text-lg font-black text-on-surface">
            {copy.relatedHeading}
          </h2>
          <Link
            href="/wiki/jeux"
            className="inline-flex items-center gap-1.5 text-xs font-bold text-primary hover:underline"
          >
            <ArrowLeft className="size-3.5" />
            <span>{copy.backToCatalog}</span>
          </Link>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          {otherGames.map((other) => {
            const otherLoc = getLocalizedGame(other, locale);
            return (
              <Link
                key={other.slug}
                href={`/wiki/jeux/${other.slug}`}
                className="group rounded-2xl border border-outline-variant/20 bg-surface-container overflow-hidden hover:border-primary/40 transition flex flex-col"
              >
                <div className="relative aspect-[16/9] bg-surface-container-highest">
                  <Image
                    src={other.thumbnail}
                    alt={otherLoc.title}
                    fill
                    sizes="(max-width: 640px) 100vw, 25vw"
                    className="object-cover group-hover:scale-105 transition-transform"
                  />
                </div>
                <div className="p-3.5">
                  <div className="text-xs font-bold text-primary">
                    {other.releaseYear}
                  </div>
                  <div className="text-sm font-black text-on-surface group-hover:text-primary line-clamp-1 mt-0.5">
                    {otherLoc.title}
                  </div>
                </div>
              </Link>
            );
          })}
        </div>
      </section>
    </div>
  );
}
