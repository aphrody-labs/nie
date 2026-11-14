-- Veille et crawl continu de fiches personnages (Inazuma Eleven / Zukan / Google / Wikis).
-- Tables serveur uniquement (RLS activée, aucun accès anon/authenticated).

begin;

create table if not exists public.character_crawl_targets (
  slug text primary key,
  canonical_name text not null,
  localized_names jsonb not null default '{}'::jsonb,
  aliases text[] not null default '{}'::text[],
  zukan_ids text[] not null default '{}'::text[],
  seed_urls text[] not null default '{}'::text[],
  search_queries text[] not null default '{}'::text[],
  crawl_interval_sec integer not null default 900,
  enabled boolean not null default true,
  last_crawled_at timestamptz,
  last_summary jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists public.character_crawl_pages (
  id bigserial primary key,
  target_slug text not null references public.character_crawl_targets(slug) on delete cascade,
  url text not null,
  normalized_url text not null,
  engine text not null,
  source_kind text not null,
  status text not null,
  http_status integer,
  title text,
  language text,
  content_hash text,
  excerpt text,
  body_text text,
  metadata jsonb not null default '{}'::jsonb,
  discovered_links jsonb not null default '[]'::jsonb,
  r2_snapshot_key text,
  r2_snapshot_url text,
  fetched_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint character_crawl_pages_target_url_uniq unique (target_slug, normalized_url)
);

create table if not exists public.character_crawl_assets (
  id bigserial primary key,
  target_slug text not null references public.character_crawl_targets(slug) on delete cascade,
  page_url text,
  source_url text not null,
  sha256 text not null,
  content_type text not null,
  byte_size bigint not null,
  r2_bucket text,
  r2_key text not null,
  r2_url text,
  storage_mode text not null,
  metadata jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint character_crawl_assets_target_sha_uniq unique (target_slug, sha256)
);

create index if not exists idx_character_crawl_pages_target_fetched
  on public.character_crawl_pages (target_slug, fetched_at desc);

create index if not exists idx_character_crawl_pages_hash
  on public.character_crawl_pages (content_hash)
  where content_hash is not null;

create index if not exists idx_character_crawl_assets_target_created
  on public.character_crawl_assets (target_slug, created_at desc);

alter table public.character_crawl_targets enable row level security;
alter table public.character_crawl_pages enable row level security;
alter table public.character_crawl_assets enable row level security;

revoke all on table public.character_crawl_targets from anon, authenticated;
revoke all on table public.character_crawl_pages from anon, authenticated;
revoke all on table public.character_crawl_assets from anon, authenticated;

insert into public.character_crawl_targets (
  slug,
  canonical_name,
  localized_names,
  aliases,
  zukan_ids,
  seed_urls,
  search_queries,
  crawl_interval_sec,
  enabled
)
values (
  'afuro-terumi',
  'Afuro Terumi',
  '{"en":"Byron Love","fr":"Byron Love","ja":"亜風炉照美","nickname_en":"Aphrodi","nickname_ja":"アフロディ","reading":"亜風炉あふろ 照美てるみ"}'::jsonb,
  array[
    'Afuro Terumi',
    'Byron Love',
    'Aphrodi',
    'Aphrody',
    '亜風炉照美',
    '亜風炉 照美',
    'アフロディ',
    '아프로디'
  ],
  array['c01001900', 'c05026590', 'c07080010'],
  array[
    'https://inazuma-eleven.fandom.com/wiki/Afuro_Terumi',
    'https://inazuma-eleven.fandom.com/fr/wiki/Byron_Love',
    'https://baike.baidu.com/fr/item/Afuro%20Terumi/4316218',
    'https://manga-encyclopedie.fandom.com/fr/wiki/Afuro_Terumi_(Inazuma_Eleven)',
    'https://zukan.inazuma.jp/en/chara_list/?q=hN2RnpKaoIyano2cl93FpN2-j5eNkJuG392igg=='
  ],
  array[
    'byron love',
    '"Byron Love" inazuma eleven',
    '"Afuro Terumi" inazuma eleven',
    '"Aphrodi" inazuma eleven',
    '亜風炉照美 イナズマイレブン'
  ],
  900,
  true
)
on conflict (slug) do update set
  canonical_name = excluded.canonical_name,
  localized_names = excluded.localized_names,
  aliases = excluded.aliases,
  zukan_ids = excluded.zukan_ids,
  seed_urls = excluded.seed_urls,
  search_queries = excluded.search_queries,
  crawl_interval_sec = excluded.crawl_interval_sec,
  enabled = excluded.enabled,
  updated_at = now();

do $$
begin
  if exists (select 1 from pg_namespace where nspname = 'cron')
     and exists (select 1 from pg_namespace where nspname = 'rg_cron') then
    perform cron.schedule(
      'rg:character:crawl',
      '*/15 * * * *',
      $job$select rg_cron.appeler('character:crawl')$job$
    );
  end if;
end $$;

notify pgrst, 'reload schema';

commit;
