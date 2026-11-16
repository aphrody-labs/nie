-- Editorial tables for the portal. Game data stays in the nie-cfgsql import (vfs_*, t2b_*, rdbn_*).
-- Access: one authorisation check per write in the axum data layer (admin role from rg-auth); reads are public.
create table if not exists portal_game (
  slug text primary key,
  official_url text not null,
  nie_supported boolean not null default false,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);
create table if not exists portal_news (
  id uuid primary key default gen_random_uuid(),
  game_slug text references portal_game(slug) on delete set null,
  published_at timestamptz not null,
  author_id uuid not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);
create index if not exists portal_news_game_published on portal_news (game_slug, published_at desc);
create table if not exists portal_news_i18n (
  news_id uuid not null references portal_news(id) on delete cascade,
  lang text not null check (lang ~ '^[a-z]{2}$'),
  title text not null,
  body_md text not null,
  primary key (news_id, lang)
);
create table if not exists portal_patchnote (
  id uuid primary key default gen_random_uuid(),
  game_slug text not null references portal_game(slug) on delete cascade,
  platform text not null check (platform in ('steam','ps','xbox','switch')),
  version text not null,
  released_at timestamptz not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (game_slug, platform, version)
);
create table if not exists portal_patchnote_i18n (
  patchnote_id uuid not null references portal_patchnote(id) on delete cascade,
  lang text not null check (lang ~ '^[a-z]{2}$'),
  body_md text not null,
  primary key (patchnote_id, lang)
);
