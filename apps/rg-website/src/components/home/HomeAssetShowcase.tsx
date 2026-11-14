"use client";

import { Badge, Button, Card } from "@rosegriffon/ui";
import { useState } from "react";

import { RgVectorIcon } from "@/components/icons/RgVectorIcons";
import Link from "@/components/shared/Link";
import { CommonSpriteIcon } from "@/components/ui/CommonSpriteIcon";
import { GameSpriteIcon } from "@/components/ui/GameSpriteIcon";
import InlineModelViewer from "@/components/wiki/InlineModelViewer";
import {
  ASSET_SHOWCASE_COPY,
  CATEGORY_ICON_SHORTCUTS,
  COMMON_SPRITE_SHORTCUTS,
  ELEMENT_ASSETS,
  RANK_TIER_ASSETS,
  RARITY_ASSETS,
  ROLE_ASSETS,
} from "@/lib/home-showcase";
import { ArrowRight } from "@/lib/icons-config";

type AssetTabId = (typeof ASSET_SHOWCASE_COPY.tabs)[number]["id"];

export function HomeAssetShowcase() {
  const [activeTab, setActiveTab] = useState<AssetTabId>("atlas");
  const [loadModel, setLoadModel] = useState(false);

  return (
    <section
      aria-label={ASSET_SHOWCASE_COPY.ariaLabel}
      className="mx-auto w-full max-w-7xl px-4 sm:px-6 lg:px-8"
    >
      <Card className="rg-asset-stage rounded-3xl border border-border/80 p-6 shadow-sm sm:p-8">
        <div className="grid grid-cols-1 gap-8 lg:grid-cols-12">
          <div className="flex flex-col gap-5 lg:col-span-7">
            <div className="flex flex-col gap-2">
              <Badge variant="secondary" className="w-fit">
                {ASSET_SHOWCASE_COPY.badge}
              </Badge>
              <h2 className="text-2xl font-black tracking-tight text-foreground sm:text-3xl">
                {ASSET_SHOWCASE_COPY.heading}
              </h2>
              <p className="text-sm leading-relaxed text-muted-foreground sm:text-base">
                {ASSET_SHOWCASE_COPY.subHeading}
              </p>
            </div>

            <div
              role="tablist"
              aria-label={ASSET_SHOWCASE_COPY.tabsAriaLabel}
              className="inline-flex w-fit flex-wrap gap-1.5 rounded-xl border border-border bg-background/85 p-1 backdrop-blur-sm"
            >
              {ASSET_SHOWCASE_COPY.tabs.map((tab) => {
                const selected = activeTab === tab.id;
                return (
                  <button
                    key={tab.id}
                    type="button"
                    role="tab"
                    aria-selected={selected}
                    onClick={() => setActiveTab(tab.id)}
                    className={
                      selected
                        ? "rounded-lg bg-primary px-3 py-1.5 text-xs font-bold text-primary-foreground shadow-xs transition-colors"
                        : "rounded-lg px-3 py-1.5 text-xs font-semibold text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                    }
                  >
                    {tab.label}
                  </button>
                );
              })}
            </div>

            {activeTab === "atlas" && (
              <div className="flex flex-col gap-4">
                <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-4">
                  {COMMON_SPRITE_SHORTCUTS.map((item) => (
                    <Link
                      key={item.key}
                      href={item.href}
                      className="flex items-center gap-2.5 rounded-xl border border-border/80 bg-background/90 p-2.5 transition-colors hover:border-primary/60 hover:bg-background"
                    >
                      {item.darkBadge ? (
                        <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                          <CommonSpriteIcon name={item.key} scale={0.3} />
                        </div>
                      ) : (
                        <CommonSpriteIcon name={item.key} scale={0.34} />
                      )}
                      <div className="min-w-0">
                        <p className="truncate text-xs font-bold text-foreground">
                          {item.label}
                        </p>
                        <p className="truncate text-xs text-muted-foreground">
                          {item.subtitle}
                        </p>
                      </div>
                    </Link>
                  ))}
                </div>

                <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-4">
                  {ROLE_ASSETS.map((role) => (
                    <Link
                      key={role.code}
                      href={role.href}
                      className="flex items-center justify-between gap-2 rounded-xl border border-border/80 bg-background/90 px-3 py-2 transition-colors hover:border-primary/60 hover:bg-background"
                    >
                      <img
                        src={role.image}
                        alt=""
                        aria-hidden="true"
                        width={28}
                        height={28}
                        className="size-7 shrink-0 object-contain"
                      />
                      <span className="truncate text-xs font-semibold text-foreground">
                        {role.label}
                      </span>
                    </Link>
                  ))}
                </div>
              </div>
            )}

            {activeTab === "elements" && (
              <div className="flex flex-col gap-4">
                <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                  {ELEMENT_ASSETS.map((elem) => (
                    <Link
                      key={elem.id}
                      href={elem.href}
                      className="flex items-center gap-3 rounded-xl border border-border/80 bg-background/90 p-3 transition-colors hover:border-primary/60 hover:bg-background"
                    >
                      <img
                        src={elem.image}
                        alt={elem.label}
                        width={36}
                        height={36}
                        className="size-9 shrink-0 object-contain"
                      />
                      <span className="text-sm font-bold text-foreground">
                        {elem.label}
                      </span>
                    </Link>
                  ))}
                </div>

                <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3">
                  {RARITY_ASSETS.map((rar) => (
                    <div
                      key={rar.id}
                      className="flex items-center justify-center rounded-xl border border-border/70 bg-background/85 p-2"
                    >
                      <img
                        src={rar.image}
                        alt={rar.label}
                        width={180}
                        height={28}
                        className="h-6 w-auto object-contain"
                      />
                    </div>
                  ))}
                </div>
              </div>
            )}

            {activeTab === "tiers" && (
              <div className="flex flex-col gap-4">
                <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3">
                  {CATEGORY_ICON_SHORTCUTS.map((cat) => (
                    <Link
                      key={cat.key}
                      href={cat.href}
                      className="flex items-center gap-2.5 rounded-xl border border-border/80 bg-background/90 p-2.5 transition-colors hover:border-primary/60 hover:bg-background"
                    >
                      <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                        <GameSpriteIcon name={cat.key} size={20} />
                      </div>
                      <div className="min-w-0">
                        <p className="truncate text-xs font-bold text-foreground">
                          {cat.label}
                        </p>
                        <p className="truncate text-xs text-muted-foreground">
                          {cat.subtitle}
                        </p>
                      </div>
                    </Link>
                  ))}
                </div>

                <div className="grid grid-cols-3 gap-2.5 sm:grid-cols-5">
                  {RANK_TIER_ASSETS.map((tier) => (
                    <Link
                      key={tier.label}
                      href={tier.href}
                      className="flex flex-col items-center gap-1 rounded-xl border border-border/70 bg-background/85 p-2 text-center transition-colors hover:border-primary/60"
                    >
                      <img
                        src={tier.image}
                        alt={tier.label}
                        width={40}
                        height={40}
                        className="size-9 object-contain"
                      />
                      <span className="truncate text-xs font-semibold text-foreground">
                        {tier.label}
                      </span>
                    </Link>
                  ))}
                </div>
              </div>
            )}

            <div className="flex flex-wrap gap-2.5 pt-1">
              <Button asChild size="sm" variant="outline" className="rounded-lg">
                <Link href={ASSET_SHOWCASE_COPY.texturesCta.href}>
                  <span>{ASSET_SHOWCASE_COPY.texturesCta.label}</span>
                  <ArrowRight className="ml-1.5 size-3.5" />
                </Link>
              </Button>
              <Button asChild size="sm" variant="outline" className="rounded-lg">
                <Link href={ASSET_SHOWCASE_COPY.viewer3d.modelsHref}>
                  <span>{ASSET_SHOWCASE_COPY.viewer3d.modelsLabel}</span>
                  <ArrowRight className="ml-1.5 size-3.5" />
                </Link>
              </Button>
            </div>
          </div>

          <div className="flex flex-col justify-between gap-4 rounded-2xl border border-border/80 bg-background/90 p-4 backdrop-blur-sm lg:col-span-5">
            <div className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                <RgVectorIcon name="cube-3d" className="size-4 text-primary" />
                <span className="text-sm font-extrabold text-foreground">
                  {ASSET_SHOWCASE_COPY.viewer3d.title}
                </span>
              </div>
              <Badge variant="secondary">
                {ASSET_SHOWCASE_COPY.viewer3d.badge}
              </Badge>
            </div>

            <div className="rg-3d-poster relative flex aspect-[4/3] w-full items-center justify-center overflow-hidden rounded-xl border border-border/70">
              {loadModel ? (
                <InlineModelViewer
                  glbUrl={ASSET_SHOWCASE_COPY.viewer3d.glbUrl}
                  name={ASSET_SHOWCASE_COPY.viewer3d.defaultName}
                />
              ) : (
                <div className="relative flex size-full flex-col items-center justify-between p-4 text-center">
                  <img
                    src={ASSET_SHOWCASE_COPY.viewer3d.posterBgSrc}
                    alt={ASSET_SHOWCASE_COPY.viewer3d.posterAlt}
                    width={960}
                    height={720}
                    loading="lazy"
                    className="pointer-events-none absolute inset-0 size-full object-cover object-top opacity-35"
                  />
                  <div className="relative z-10 mt-auto flex flex-col items-center gap-3 rounded-2xl border border-border/70 bg-background/85 px-4 py-3 backdrop-blur-md">
                    <img
                      src={ASSET_SHOWCASE_COPY.viewer3d.posterAvatarSrc}
                      alt={ASSET_SHOWCASE_COPY.viewer3d.posterAvatarAlt}
                      width={56}
                      height={56}
                      loading="lazy"
                      className="size-14 rounded-xl border border-border object-cover"
                    />
                    <Button
                      type="button"
                      size="sm"
                      onClick={() => setLoadModel(true)}
                      className="rounded-lg font-semibold"
                    >
                      <RgVectorIcon name="cube-3d" className="mr-1.5 size-4" />
                      <span>{ASSET_SHOWCASE_COPY.viewer3d.loadButtonLabel}</span>
                    </Button>
                  </div>
                </div>
              )}
            </div>

            <div className="flex flex-wrap items-center justify-between gap-2">
              <p className="text-xs text-muted-foreground">
                {ASSET_SHOWCASE_COPY.viewer3d.subtitle}
              </p>
              <Button asChild size="sm" variant="secondary" className="rounded-lg">
                <Link href={ASSET_SHOWCASE_COPY.viewer3d.demoHref}>
                  <span>{ASSET_SHOWCASE_COPY.viewer3d.demoLabel}</span>
                  <ArrowRight className="ml-1.5 size-3.5" />
                </Link>
              </Button>
            </div>
          </div>
        </div>
      </Card>
    </section>
  );
}
