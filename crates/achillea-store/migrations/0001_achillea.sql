-- Achillea data model (PostgreSQL 18). Rules live in achillea-core; this schema only stores state.
CREATE SCHEMA IF NOT EXISTS achillea;
SET search_path = achillea, public;

CREATE TABLE IF NOT EXISTS player (
  id         BIGSERIAL PRIMARY KEY,
  discord_id TEXT NOT NULL UNIQUE,
  username   TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS season (
  id          SERIAL PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE,
  k_factor    INTEGER NOT NULL DEFAULT 32 CHECK (k_factor > 0),
  base_rating INTEGER NOT NULL DEFAULT 0 CHECK (base_rating >= 0),
  starts_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  ends_at     TIMESTAMPTZ
);

-- Tiers are data, not code: the thresholds of a season are rows (idx 0 = lowest).
CREATE TABLE IF NOT EXISTS rank_tier (
  season_id INTEGER NOT NULL REFERENCES season(id) ON DELETE CASCADE,
  idx       SMALLINT NOT NULL CHECK (idx >= 0),
  key       TEXT NOT NULL,
  min_elo   INTEGER NOT NULL,
  max_elo   INTEGER NOT NULL CHECK (max_elo >= min_elo),
  PRIMARY KEY (season_id, idx)
);

CREATE TABLE IF NOT EXISTS rating (
  season_id INTEGER NOT NULL REFERENCES season(id) ON DELETE CASCADE,
  player_id BIGINT NOT NULL REFERENCES player(id) ON DELETE CASCADE,
  ap        INTEGER NOT NULL CHECK (ap >= 0),
  wins      INTEGER NOT NULL DEFAULT 0,
  losses    INTEGER NOT NULL DEFAULT 0,
  draws     INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (season_id, player_id)
);
CREATE INDEX IF NOT EXISTS rating_leaderboard ON rating (season_id, ap DESC);

CREATE TABLE IF NOT EXISTS ranked_match (
  id             BIGSERIAL PRIMARY KEY,
  season_id      INTEGER NOT NULL REFERENCES season(id),
  mode           SMALLINT NOT NULL CHECK (mode IN (2, 4)),
  home_player_id BIGINT NOT NULL REFERENCES player(id),
  away_player_id BIGINT NOT NULL REFERENCES player(id),
  home_score     SMALLINT NOT NULL CHECK (home_score BETWEEN 0 AND 99),
  away_score     SMALLINT NOT NULL CHECK (away_score BETWEEN 0 AND 99),
  home_ap_delta  INTEGER NOT NULL,
  away_ap_delta  INTEGER NOT NULL,
  played_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (home_player_id <> away_player_id)
);

CREATE TABLE IF NOT EXISTS clan (
  id         BIGSERIAL PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE,
  tag        TEXT NOT NULL UNIQUE,
  fame       BIGINT NOT NULL DEFAULT 0,
  recruiting BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A player belongs to at most one clan.
CREATE TABLE IF NOT EXISTS clan_member (
  clan_id          BIGINT NOT NULL REFERENCES clan(id) ON DELETE CASCADE,
  player_id        BIGINT NOT NULL UNIQUE REFERENCES player(id) ON DELETE CASCADE,
  role             SMALLINT NOT NULL CHECK (role BETWEEN 0 AND 3),
  distinctions     TEXT[] NOT NULL DEFAULT '{}',
  fame_contributed BIGINT NOT NULL DEFAULT 0,
  joined_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (clan_id, player_id)
);
-- Exactly one captain per clan.
CREATE UNIQUE INDEX IF NOT EXISTS clan_one_captain ON clan_member (clan_id) WHERE role = 3;

CREATE TABLE IF NOT EXISTS tournament (
  id           BIGSERIAL PRIMARY KEY,
  season_id    INTEGER REFERENCES season(id),
  name         TEXT NOT NULL,
  stage_type   TEXT NOT NULL CHECK (stage_type IN ('single_elimination')),
  match_format TEXT NOT NULL DEFAULT 'BO1',
  bracket_size INTEGER NOT NULL CHECK (bracket_size >= 2),
  state        TEXT NOT NULL DEFAULT 'registration' CHECK (state IN ('registration', 'running', 'finished')),
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS tournament_entry (
  tournament_id BIGINT NOT NULL REFERENCES tournament(id) ON DELETE CASCADE,
  player_id     BIGINT NOT NULL REFERENCES player(id),
  seed          INTEGER,
  placement     INTEGER,
  PRIMARY KEY (tournament_id, player_id)
);

CREATE TABLE IF NOT EXISTS tournament_match (
  id            BIGSERIAL PRIMARY KEY,
  tournament_id BIGINT NOT NULL REFERENCES tournament(id) ON DELETE CASCADE,
  round         INTEGER NOT NULL,
  slot          INTEGER NOT NULL,
  home_player_id BIGINT REFERENCES player(id),
  away_player_id BIGINT REFERENCES player(id),
  home_score    SMALLINT,
  away_score    SMALLINT,
  winner_id     BIGINT REFERENCES player(id),
  UNIQUE (tournament_id, round, slot)
);

CREATE TABLE IF NOT EXISTS league (
  id            BIGSERIAL PRIMARY KEY,
  name          TEXT NOT NULL UNIQUE,
  points_scheme TEXT NOT NULL CHECK (points_scheme IN ('asytral', 'football')),
  double_legged BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS league_team (
  league_id BIGINT NOT NULL REFERENCES league(id) ON DELETE CASCADE,
  team_id   TEXT NOT NULL,
  name      TEXT NOT NULL,
  clan_id   BIGINT REFERENCES clan(id),
  PRIMARY KEY (league_id, team_id)
);

CREATE TABLE IF NOT EXISTS league_match (
  id         BIGSERIAL PRIMARY KEY,
  league_id  BIGINT NOT NULL REFERENCES league(id) ON DELETE CASCADE,
  matchday   INTEGER NOT NULL,
  home_id    TEXT NOT NULL,
  away_id    TEXT NOT NULL,
  home_score INTEGER,
  away_score INTEGER,
  overtime   BOOLEAN NOT NULL DEFAULT FALSE,
  FOREIGN KEY (league_id, home_id) REFERENCES league_team(league_id, team_id),
  FOREIGN KEY (league_id, away_id) REFERENCES league_team(league_id, team_id),
  CHECK (home_id <> away_id)
);

-- Ultimate Team. Cards are rows describing a game character: vfs_path/chara_hash point at the
-- game data imported by nie-cfgsql (vfs_file / rdbn_<id>), so the pool is generated, not typed.
CREATE TABLE IF NOT EXISTS ut_rarity (
  rarity SMALLINT PRIMARY KEY,
  name   TEXT NOT NULL,
  weight INTEGER NOT NULL CHECK (weight > 0)
);

CREATE TABLE IF NOT EXISTS ut_card (
  id         BIGSERIAL PRIMARY KEY,
  name       TEXT NOT NULL,
  rarity     SMALLINT NOT NULL REFERENCES ut_rarity(rarity),
  vfs_path   TEXT,
  chara_hash BIGINT,
  UNIQUE (vfs_path, chara_hash)
);

CREATE TABLE IF NOT EXISTS ut_collection (
  player_id BIGINT NOT NULL REFERENCES player(id) ON DELETE CASCADE,
  card_id   BIGINT NOT NULL REFERENCES ut_card(id),
  quantity  INTEGER NOT NULL CHECK (quantity > 0),
  PRIMARY KEY (player_id, card_id)
);

CREATE TABLE IF NOT EXISTS ut_pack_open (
  id        BIGSERIAL PRIMARY KEY,
  player_id BIGINT NOT NULL REFERENCES player(id),
  seed      BIGINT NOT NULL,
  card_ids  BIGINT[] NOT NULL,
  opened_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
