"use client";

import { Badge, Button, Card } from "@rosegriffon/ui";
import { useEffect, useState } from "react";

import type { ServeurDiscord } from "@/lib/discord";
import {
  DISCORD_HOME_COPY,
  sanitiserNomServeurDiscord,
} from "@/lib/home-showcase";
import { ExternalLink } from "@/lib/icons-config";

interface WidgetMember {
  avatar_url?: string;
  id: string;
  status?: string;
  username: string;
}

interface WidgetPayload {
  instant_invite?: string;
  members?: WidgetMember[];
  name?: string;
  presence_count?: number;
}

function DiscordServerCard({ serveur }: { serveur: ServeurDiscord }) {
  const [widget, setWidget] = useState<WidgetPayload | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    fetch(`https://discord.com/api/guilds/${serveur.guildId}/widget.json`, {
      signal: controller.signal,
    })
      .then((res) => (res.ok ? (res.json() as Promise<WidgetPayload>) : null))
      .then((data) => {
        if (data) setWidget(data);
      })
      .catch(() => {});
    return () => controller.abort();
  }, [serveur.guildId]);

  const cleanName = sanitiserNomServeurDiscord(widget?.name || serveur.nom);
  const emblemSrc =
    serveur.cle === "rose-griffon"
      ? "/RG_Logo_V2.5.webp"
      : "/achillea/achillea.png";
  const onlineCount = widget?.presence_count ?? null;
  const members = (widget?.members ?? []).slice(0, 6);
  const inviteUrl = widget?.instant_invite || serveur.invitation;

  return (
    <Card className="flex flex-col justify-between gap-4 rounded-2xl border border-border/80 bg-card p-5 shadow-xs">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <img
            src={emblemSrc}
            alt={cleanName}
            width={44}
            height={44}
            loading="lazy"
            className="size-11 shrink-0 rounded-xl border border-border/70 bg-muted object-cover"
          />
          <div className="min-w-0">
            <h3 className="truncate text-base font-extrabold text-foreground">
              {cleanName}
            </h3>
            <p className="truncate text-xs text-muted-foreground">
              {serveur.description}
            </p>
          </div>
        </div>

        <Button asChild size="sm" className="shrink-0 rounded-lg">
          <a
            href={inviteUrl}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={`${DISCORD_HOME_COPY.openDiscordAriaPrefix} ${cleanName}`}
          >
            <span>{DISCORD_HOME_COPY.joinLabel}</span>
            <ExternalLink className="ml-1.5 size-3.5" />
          </a>
        </Button>
      </div>

      <div className="flex items-center justify-between gap-2 border-t border-border/60 pt-3">
        {onlineCount != null ? (
          <Badge
            variant="secondary"
            className="font-mono text-xs tabular-nums"
          >
            {onlineCount.toLocaleString("fr-FR")}{" "}
            {DISCORD_HOME_COPY.memberCountSuffix}
          </Badge>
        ) : (
          <span className="text-xs text-muted-foreground">
            {DISCORD_HOME_COPY.emptyNote}
          </span>
        )}
      </div>

      {members.length > 0 && (
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
          {members.map((m) => (
            <div
              key={m.id}
              className="flex items-center gap-2 rounded-lg border border-border/70 bg-background/70 px-2.5 py-1.5"
            >
              <img
                src={m.avatar_url || "/default-avatar.webp"}
                alt={m.username}
                width={20}
                height={20}
                loading="lazy"
                className="size-5 shrink-0 rounded-full bg-muted object-cover"
              />
              <span className="truncate text-xs font-medium text-foreground">
                {m.username}
              </span>
            </div>
          ))}
        </div>
      )}
    </Card>
  );
}

interface HomeDiscordSectionProps {
  serveurs: readonly ServeurDiscord[];
}

export function HomeDiscordSection({ serveurs }: HomeDiscordSectionProps) {
  if (serveurs.length === 0) return null;

  return (
    <section
      aria-label={DISCORD_HOME_COPY.ariaLabel}
      className="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 sm:px-6 lg:px-8"
    >
      <div className="flex flex-col gap-1.5 text-center">
        <h2 className="text-2xl font-black tracking-tight text-foreground sm:text-3xl">
          {DISCORD_HOME_COPY.heading}
        </h2>
        <p className="mx-auto max-w-2xl text-sm text-muted-foreground sm:text-base">
          {DISCORD_HOME_COPY.subHeading}
        </p>
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2 lg:gap-8">
        {serveurs.map((srv) => (
          <DiscordServerCard key={srv.cle} serveur={srv} />
        ))}
      </div>
    </section>
  );
}
