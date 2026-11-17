//! Achillea state on PostgreSQL. All rules (Elo, brackets, league table, clan powers) come from
//! `achillea-core`; this crate persists their inputs and results inside transactions.

use achillea_core::brackets::{next_power_of_two, single_elimination_rounds, standard_seed_order};
use achillea_core::clans::{
    ClanMemberRole, can_act_on_member, can_manage_recruitment, member_distinction_priority,
    role_weight,
};
use achillea_core::elo::{apply_elo_update, placement_prize_ap, progressive_elo_delta};
use achillea_core::league::{
    ASYTRAL_POINTS, FOOTBALL_POINTS, LeagueMatchResult, LeagueStanding, compute_league_table,
    generate_single_round_robin,
};
use achillea_core::ranked::clamp_score;
use tokio_postgres::{Client, Transaction};

/// Migration script.
pub const MIGRATION: &str = include_str!("../migrations/0001_achillea.sql");

/// Errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Database error.
    #[error("database: {0}")]
    Db(#[from] tokio_postgres::Error),
    /// A rule refused the operation.
    #[error("{0}")]
    Rule(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

fn rule<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Rule(msg.into()))
}

/// Creates the schema and tables.
///
/// # Errors
/// Database errors.
pub async fn migrate(c: &Client) -> Result<()> {
    c.batch_execute(MIGRATION).await?;
    Ok(())
}

/// Finds or creates a player by Discord id; returns its id.
///
/// # Errors
/// Database errors.
pub async fn upsert_player(c: &Client, discord_id: &str, username: &str) -> Result<i64> {
    let r = c
        .query_one(
            "INSERT INTO achillea.player (discord_id, username) VALUES ($1, $2) ON CONFLICT (discord_id) DO UPDATE SET username = EXCLUDED.username RETURNING id",
            &[&discord_id, &username],
        )
        .await?;
    Ok(r.get(0))
}

/// Creates a season.
///
/// # Errors
/// Database errors.
pub async fn create_season(c: &Client, name: &str, k_factor: i32, base_rating: i32) -> Result<i32> {
    Ok(c.query_one("INSERT INTO achillea.season (name, k_factor, base_rating) VALUES ($1, $2, $3) RETURNING id", &[&name, &k_factor, &base_rating]).await?.get(0))
}

/// Replaces the tiers of a season (index = position in `tiers`, lowest first).
///
/// # Errors
/// Database errors.
pub async fn set_tiers(c: &mut Client, season: i32, tiers: &[(&str, i32, i32)]) -> Result<()> {
    let tx = c.transaction().await?;
    tx.execute(
        "DELETE FROM achillea.rank_tier WHERE season_id = $1",
        &[&season],
    )
    .await?;
    for (i, (key, min, max)) in tiers.iter().enumerate() {
        let idx = i16::try_from(i).unwrap_or(i16::MAX);
        tx.execute("INSERT INTO achillea.rank_tier (season_id, idx, key, min_elo, max_elo) VALUES ($1, $2, $3, $4, $5)", &[&season, &idx, key, min, max]).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn ensure_rating(tx: &Transaction<'_>, season: i32, player: i64) -> Result<()> {
    tx.execute(
        "INSERT INTO achillea.rating (season_id, player_id, ap) SELECT $1, $2, base_rating FROM achillea.season WHERE id = $1 ON CONFLICT DO NOTHING",
        &[&season, &player],
    )
    .await?;
    Ok(())
}

async fn tier_index(tx: &Transaction<'_>, season: i32, ap: i32) -> Result<f64> {
    let r = tx.query_opt("SELECT idx FROM achillea.rank_tier WHERE season_id = $1 AND $2 BETWEEN min_elo AND max_elo ORDER BY idx LIMIT 1", &[&season, &ap]).await?;
    Ok(r.map_or(0.0, |r| f64::from(r.get::<_, i16>(0))))
}

/// Result of a ranked match.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchOutcome {
    /// Match id.
    pub match_id: i64,
    /// Home AP change actually applied.
    pub home_delta: i32,
    /// Away AP change actually applied.
    pub away_delta: i32,
    /// Home AP after the match.
    pub home_ap: i32,
    /// Away AP after the match.
    pub away_ap: i32,
}

/// Records a ranked match and updates both ratings atomically (progressive AP by tier).
///
/// # Errors
/// Rule error for equal players or a bad mode; database errors.
pub async fn record_ranked_match(
    c: &mut Client,
    season: i32,
    mode: i16,
    home: i64,
    away: i64,
    home_score: f64,
    away_score: f64,
) -> Result<MatchOutcome> {
    if home == away {
        return rule("a player cannot play against themselves");
    }
    if mode != 2 && mode != 4 {
        return rule("mode must be 2 (1v1) or 4 (2v2)");
    }
    let (hs, as_) = (clamp_score(home_score), clamp_score(away_score));
    let tx = c.transaction().await?;
    ensure_rating(&tx, season, home).await?;
    ensure_rating(&tx, season, away).await?;
    let rows = tx
        .query("SELECT player_id, ap FROM achillea.rating WHERE season_id = $1 AND player_id IN ($2, $3) ORDER BY player_id FOR UPDATE", &[&season, &home, &away])
        .await?;
    let ap_of = |id: i64| {
        rows.iter()
            .find(|r| r.get::<_, i64>(0) == id)
            .map(|r| r.get::<_, i32>(1))
            .unwrap_or(0)
    };
    let (h_ap, a_ap) = (ap_of(home), ap_of(away));
    let k: i32 = tx
        .query_one(
            "SELECT k_factor FROM achillea.season WHERE id = $1",
            &[&season],
        )
        .await?
        .get(0);
    let actual = if hs > as_ {
        1.0
    } else if hs == as_ {
        0.5
    } else {
        0.0
    };
    let (h_tier, a_tier) = (
        tier_index(&tx, season, h_ap).await?,
        tier_index(&tx, season, a_ap).await?,
    );
    let h_raw = progressive_elo_delta(
        f64::from(h_ap),
        f64::from(a_ap),
        actual,
        h_tier,
        f64::from(k),
    );
    let a_raw = progressive_elo_delta(
        f64::from(a_ap),
        f64::from(h_ap),
        1.0 - actual,
        a_tier,
        f64::from(k),
    );
    let (h_new, h_applied) = apply_elo_update(f64::from(h_ap), h_raw);
    let (a_new, a_applied) = apply_elo_update(f64::from(a_ap), a_raw);
    let (h_new, a_new, h_applied, a_applied) = (
        h_new as i32,
        a_new as i32,
        h_applied as i32,
        a_applied as i32,
    );
    let (hw, hl, hd) = (
        i32::from(actual == 1.0),
        i32::from(actual == 0.0),
        i32::from(actual == 0.5),
    );
    tx.execute("UPDATE achillea.rating SET ap = $3, wins = wins + $4, losses = losses + $5, draws = draws + $6 WHERE season_id = $1 AND player_id = $2", &[&season, &home, &h_new, &hw, &hl, &hd]).await?;
    tx.execute("UPDATE achillea.rating SET ap = $3, wins = wins + $4, losses = losses + $5, draws = draws + $6 WHERE season_id = $1 AND player_id = $2", &[&season, &away, &a_new, &hl, &hw, &hd]).await?;
    let (hs16, as16) = (hs as i16, as_ as i16);
    let id: i64 = tx
        .query_one(
            "INSERT INTO achillea.ranked_match (season_id, mode, home_player_id, away_player_id, home_score, away_score, home_ap_delta, away_ap_delta) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id",
            &[&season, &mode, &home, &away, &hs16, &as16, &h_applied, &a_applied],
        )
        .await?
        .get(0);
    tx.commit().await?;
    Ok(MatchOutcome {
        match_id: id,
        home_delta: h_applied,
        away_delta: a_applied,
        home_ap: h_new,
        away_ap: a_new,
    })
}

/// One leaderboard line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderboardRow {
    /// 1-based rank (dense on AP).
    pub rank: i64,
    /// Display name.
    pub username: String,
    /// AP.
    pub ap: i32,
    /// Wins.
    pub wins: i32,
    /// Losses.
    pub losses: i32,
}

/// Top of the season ladder.
///
/// # Errors
/// Database errors.
pub async fn leaderboard(c: &Client, season: i32, limit: i64) -> Result<Vec<LeaderboardRow>> {
    Ok(c.query(
        "SELECT dense_rank() OVER (ORDER BY r.ap DESC), p.username, r.ap, r.wins, r.losses FROM achillea.rating r JOIN achillea.player p ON p.id = r.player_id WHERE r.season_id = $1 ORDER BY r.ap DESC, p.id LIMIT $2",
        &[&season, &limit],
    )
    .await?
    .iter()
    .map(|r| LeaderboardRow { rank: r.get(0), username: r.get(1), ap: r.get(2), wins: r.get(3), losses: r.get(4) })
    .collect())
}

// ---------------------------------------------------------------- clans

/// Creates a clan with `captain` as its captain.
///
/// # Errors
/// Database errors (name/tag taken, player already in a clan).
pub async fn create_clan(c: &mut Client, name: &str, tag: &str, captain: i64) -> Result<i64> {
    let tx = c.transaction().await?;
    let id: i64 = tx
        .query_one(
            "INSERT INTO achillea.clan (name, tag) VALUES ($1, $2) RETURNING id",
            &[&name, &tag],
        )
        .await?
        .get(0);
    tx.execute(
        "INSERT INTO achillea.clan_member (clan_id, player_id, role) VALUES ($1, $2, 3)",
        &[&id, &captain],
    )
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// Joins a recruiting clan as a trial member.
///
/// # Errors
/// Rule error when the clan is not recruiting; database errors.
pub async fn join_clan(c: &Client, clan: i64, player: i64) -> Result<()> {
    let recruiting: bool = c
        .query_one(
            "SELECT recruiting FROM achillea.clan WHERE id = $1",
            &[&clan],
        )
        .await?
        .get(0);
    if !recruiting {
        return rule("the clan is not recruiting");
    }
    c.execute(
        "INSERT INTO achillea.clan_member (clan_id, player_id, role) VALUES ($1, $2, 0)",
        &[&clan, &player],
    )
    .await?;
    Ok(())
}

async fn role_of(tx: &Transaction<'_>, clan: i64, player: i64) -> Result<ClanMemberRole> {
    let r = tx
        .query_opt(
            "SELECT role FROM achillea.clan_member WHERE clan_id = $1 AND player_id = $2",
            &[&clan, &player],
        )
        .await?;
    let Some(r) = r else {
        return rule("not a member of this clan");
    };
    ClanMemberRole::from_i32(i32::from(r.get::<_, i16>(0)))
        .map_or_else(|| rule("invalid stored role"), Ok)
}

/// Sets a member's role. The actor must manage recruitment, act on a strictly lower role and
/// grant a role strictly below their own.
///
/// # Errors
/// Rule errors; database errors.
pub async fn set_member_role(
    c: &mut Client,
    clan: i64,
    actor: i64,
    target: i64,
    new_role: i32,
) -> Result<()> {
    let Some(new) = ClanMemberRole::from_i32(new_role) else {
        return rule("unknown role");
    };
    let tx = c.transaction().await?;
    let (a, t) = (
        role_of(&tx, clan, actor).await?,
        role_of(&tx, clan, target).await?,
    );
    if !can_manage_recruitment(a) || !can_act_on_member(a, t) || role_weight(new) >= role_weight(a)
    {
        return rule("the actor's role does not allow this change");
    }
    tx.execute(
        "UPDATE achillea.clan_member SET role = $3 WHERE clan_id = $1 AND player_id = $2",
        &[&clan, &target, &(new as i16)],
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Members in canonical order: role, distinction, fame, name.
///
/// # Errors
/// Database errors.
pub async fn clan_members(c: &Client, clan: i64) -> Result<Vec<(String, i16, i64)>> {
    let rows = c
        .query("SELECT p.username, m.role, m.fame_contributed, m.distinctions FROM achillea.clan_member m JOIN achillea.player p ON p.id = m.player_id WHERE m.clan_id = $1", &[&clan])
        .await?;
    let mut v: Vec<(String, i16, i64, u8)> = rows
        .iter()
        .map(|r| {
            let d: Vec<String> = r.get(3);
            let refs: Vec<&str> = d.iter().map(String::as_str).collect();
            (
                r.get(0),
                r.get(1),
                r.get(2),
                member_distinction_priority(&refs),
            )
        })
        .collect();
    v.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(a.3.cmp(&b.3))
            .then(b.2.cmp(&a.2))
            .then_with(|| a.0.cmp(&b.0))
    });
    Ok(v.into_iter().map(|(n, r, f, _)| (n, r, f)).collect())
}

// ---------------------------------------------------------- tournaments

/// Creates a single-elimination tournament.
///
/// # Errors
/// Database errors.
pub async fn create_tournament(c: &Client, season: i32, name: &str, capacity: i32) -> Result<i64> {
    let size = i32::try_from(next_power_of_two(
        u64::try_from(capacity.max(2)).unwrap_or(2),
    ))
    .unwrap_or(i32::MAX);
    Ok(c.query_one("INSERT INTO achillea.tournament (season_id, name, stage_type, bracket_size) VALUES ($1, $2, 'single_elimination', $3) RETURNING id", &[&season, &name, &size]).await?.get(0))
}

/// Registers a player.
///
/// # Errors
/// Rule error when registration is closed or full; database errors.
pub async fn register_player(c: &Client, tournament: i64, player: i64) -> Result<()> {
    let r = c.query_one("SELECT state, bracket_size, (SELECT count(*) FROM achillea.tournament_entry WHERE tournament_id = $1) FROM achillea.tournament WHERE id = $1", &[&tournament]).await?;
    let (state, size, n): (String, i32, i64) = (r.get(0), r.get(1), r.get(2));
    if state != "registration" {
        return rule("registration is closed");
    }
    if n >= i64::from(size) {
        return rule("the tournament is full");
    }
    c.execute(
        "INSERT INTO achillea.tournament_entry (tournament_id, player_id) VALUES ($1, $2)",
        &[&tournament, &player],
    )
    .await?;
    Ok(())
}

async fn place(tx: &Transaction<'_>, t: i64, round: i32, slot: i32, player: i64) -> Result<()> {
    let col = if slot % 2 == 0 {
        "home_player_id"
    } else {
        "away_player_id"
    };
    let sql = format!(
        "UPDATE achillea.tournament_match SET {col} = $4 WHERE tournament_id = $1 AND round = $2 AND slot = $3"
    );
    tx.execute(&sql, &[&t, &(round + 1), &(slot / 2), &player])
        .await?;
    Ok(())
}

/// Seeds by AP (highest first), builds the whole tree and advances the byes.
///
/// # Errors
/// Rule error when fewer than 2 players are registered; database errors.
pub async fn start_tournament(c: &mut Client, tournament: i64) -> Result<usize> {
    let tx = c.transaction().await?;
    let (season, size, state): (Option<i32>, i32, String) = {
        let r = tx.query_one("SELECT season_id, bracket_size, state FROM achillea.tournament WHERE id = $1 FOR UPDATE", &[&tournament]).await?;
        (r.get(0), r.get(1), r.get(2))
    };
    if state != "registration" {
        return rule("the tournament already started");
    }
    let players: Vec<i64> = tx
        .query(
            "SELECT e.player_id FROM achillea.tournament_entry e LEFT JOIN achillea.rating r ON r.player_id = e.player_id AND r.season_id = $2 WHERE e.tournament_id = $1 ORDER BY coalesce(r.ap, 0) DESC, e.player_id",
            &[&tournament, &season],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    if players.len() < 2 {
        return rule("at least 2 players are required");
    }
    for (i, p) in players.iter().enumerate() {
        tx.execute("UPDATE achillea.tournament_entry SET seed = $3 WHERE tournament_id = $1 AND player_id = $2", &[&tournament, p, &(i as i32 + 1)]).await?;
    }
    let bracket = next_power_of_two(players.len() as u64).max(2);
    let rounds = single_elimination_rounds(bracket) as i32;
    let order = standard_seed_order(bracket);
    for r in 1..=rounds {
        for s in 0..(bracket as i32 >> r) {
            tx.execute("INSERT INTO achillea.tournament_match (tournament_id, round, slot) VALUES ($1, $2, $3)", &[&tournament, &r, &s]).await?;
        }
    }
    let seat = |seed: u64| players.get(seed as usize - 1).copied();
    for s in 0..(bracket as usize / 2) {
        let (a, b) = (seat(order[2 * s]), seat(order[2 * s + 1]));
        tx.execute("UPDATE achillea.tournament_match SET home_player_id = $4, away_player_id = $5 WHERE tournament_id = $1 AND round = $2 AND slot = $3", &[&tournament, &1_i32, &(s as i32), &a, &b]).await?;
        match (a, b) {
            (Some(w), None) | (None, Some(w)) => {
                tx.execute("UPDATE achillea.tournament_match SET winner_id = $4 WHERE tournament_id = $1 AND round = $2 AND slot = $3", &[&tournament, &1_i32, &(s as i32), &w]).await?;
                if rounds > 1 {
                    place(&tx, tournament, 1, s as i32, w).await?;
                }
            }
            _ => {}
        }
    }
    let _ = size;
    tx.execute(
        "UPDATE achillea.tournament SET state = 'running' WHERE id = $1",
        &[&tournament],
    )
    .await?;
    tx.commit().await?;
    Ok(players.len())
}

/// Reports a tournament match; advances the winner and, on the final, assigns placements and AP.
///
/// # Errors
/// Rule errors (draw, missing players, already reported); database errors.
pub async fn report_tournament_match(
    c: &mut Client,
    match_id: i64,
    home_score: i16,
    away_score: i16,
) -> Result<i64> {
    if home_score == away_score {
        return rule("a tournament match cannot end in a draw");
    }
    let tx = c.transaction().await?;
    let m = tx
        .query_opt("SELECT tournament_id, round, slot, home_player_id, away_player_id, winner_id FROM achillea.tournament_match WHERE id = $1 FOR UPDATE", &[&match_id])
        .await?;
    let Some(m) = m else {
        return rule("unknown match");
    };
    let (t, round, slot): (i64, i32, i32) = (m.get(0), m.get(1), m.get(2));
    let (Some(home), Some(away)): (Option<i64>, Option<i64>) = (m.get(3), m.get(4)) else {
        return rule("both players must be known");
    };
    if m.get::<_, Option<i64>>(5).is_some() {
        return rule("already reported");
    }
    let (winner, loser) = if home_score > away_score {
        (home, away)
    } else {
        (away, home)
    };
    tx.execute("UPDATE achillea.tournament_match SET home_score = $2, away_score = $3, winner_id = $4 WHERE id = $1", &[&match_id, &home_score, &away_score, &winner]).await?;
    let r = tx
        .query_one(
            "SELECT bracket_size, season_id FROM achillea.tournament WHERE id = $1",
            &[&t],
        )
        .await?;
    let (size, season): (i32, Option<i32>) = (r.get(0), r.get(1));
    let rounds = single_elimination_rounds(size as u64) as i32;
    if round < rounds {
        place(&tx, t, round, slot, winner).await?;
        if round == rounds - 1 {
            tx.execute("UPDATE achillea.tournament_entry SET placement = 3 WHERE tournament_id = $1 AND player_id = $2", &[&t, &loser]).await?;
        }
    } else {
        tx.execute("UPDATE achillea.tournament_entry SET placement = 1 WHERE tournament_id = $1 AND player_id = $2", &[&t, &winner]).await?;
        tx.execute("UPDATE achillea.tournament_entry SET placement = 2 WHERE tournament_id = $1 AND player_id = $2", &[&t, &loser]).await?;
        tx.execute(
            "UPDATE achillea.tournament SET state = 'finished' WHERE id = $1",
            &[&t],
        )
        .await?;
        if let Some(season) = season {
            let entries = tx.query("SELECT player_id, placement FROM achillea.tournament_entry WHERE tournament_id = $1 AND placement IS NOT NULL", &[&t]).await?;
            let n = tx
                .query_one(
                    "SELECT count(*) FROM achillea.tournament_entry WHERE tournament_id = $1",
                    &[&t],
                )
                .await?
                .get::<_, i64>(0);
            for e in &entries {
                let (p, place): (i64, i32) = (e.get(0), e.get(1));
                let ap = placement_prize_ap(n as f64, f64::from(place)) as i32;
                if ap > 0 {
                    ensure_rating(&tx, season, p).await?;
                    tx.execute("UPDATE achillea.rating SET ap = ap + $3 WHERE season_id = $1 AND player_id = $2", &[&season, &p, &ap]).await?;
                }
            }
        }
    }
    tx.commit().await?;
    Ok(winner)
}

// -------------------------------------------------------------- leagues

/// Creates a league (`scheme`: `asytral` or `football`) and returns its id.
///
/// # Errors
/// Database errors.
pub async fn create_league(
    c: &Client,
    name: &str,
    scheme: &str,
    double_legged: bool,
) -> Result<i64> {
    Ok(c.query_one("INSERT INTO achillea.league (name, points_scheme, double_legged) VALUES ($1, $2, $3) RETURNING id", &[&name, &scheme, &double_legged]).await?.get(0))
}

/// Adds a team.
///
/// # Errors
/// Database errors.
pub async fn add_league_team(
    c: &Client,
    league: i64,
    team_id: &str,
    name: &str,
    clan: Option<i64>,
) -> Result<()> {
    c.execute("INSERT INTO achillea.league_team (league_id, team_id, name, clan_id) VALUES ($1, $2, $3, $4)", &[&league, &team_id, &name, &clan]).await?;
    Ok(())
}

/// Generates the calendar (second leg mirrored when double legged); returns the match count.
///
/// # Errors
/// Database errors.
pub async fn generate_league_fixtures(c: &mut Client, league: i64) -> Result<usize> {
    let tx = c.transaction().await?;
    let double: bool = tx
        .query_one(
            "SELECT double_legged FROM achillea.league WHERE id = $1",
            &[&league],
        )
        .await?
        .get(0);
    let teams: Vec<String> = tx
        .query(
            "SELECT team_id FROM achillea.league_team WHERE league_id = $1 ORDER BY team_id",
            &[&league],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    let first = generate_single_round_robin(&teams);
    let legs = first
        .iter()
        .map(|f| (f.matchday, f.home_id.clone(), f.away_id.clone()))
        .collect::<Vec<_>>();
    let rounds = legs.iter().map(|l| l.0).max().unwrap_or(0);
    let mut all = legs.clone();
    if double {
        all.extend(
            legs.iter()
                .map(|(d, h, a)| (d + rounds, a.clone(), h.clone())),
        );
    }
    for (day, h, a) in &all {
        tx.execute("INSERT INTO achillea.league_match (league_id, matchday, home_id, away_id) VALUES ($1, $2, $3, $4)", &[&league, &(*day as i32), h, a]).await?;
    }
    tx.commit().await?;
    Ok(all.len())
}

/// Reports a league match.
///
/// # Errors
/// Database errors.
pub async fn report_league_match(
    c: &Client,
    match_id: i64,
    home: i32,
    away: i32,
    overtime: bool,
) -> Result<()> {
    c.execute("UPDATE achillea.league_match SET home_score = $2, away_score = $3, overtime = $4 WHERE id = $1", &[&match_id, &home, &away, &overtime]).await?;
    Ok(())
}

/// Standings computed by `achillea-core` from the stored results.
///
/// # Errors
/// Database errors.
pub async fn league_standings(c: &Client, league: i64) -> Result<Vec<LeagueStanding>> {
    let scheme: String = c
        .query_one(
            "SELECT points_scheme FROM achillea.league WHERE id = $1",
            &[&league],
        )
        .await?
        .get(0);
    let teams: Vec<String> = c
        .query(
            "SELECT team_id FROM achillea.league_team WHERE league_id = $1 ORDER BY team_id",
            &[&league],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    let results: Vec<LeagueMatchResult> = c
        .query("SELECT home_id, away_id, home_score, away_score, overtime FROM achillea.league_match WHERE league_id = $1 AND home_score IS NOT NULL AND away_score IS NOT NULL ORDER BY matchday, id", &[&league])
        .await?
        .iter()
        .map(|r| LeagueMatchResult { home_id: r.get(0), away_id: r.get(1), home_score: r.get(2), away_score: r.get(3), winner_id: None, overtime: r.get(4) })
        .collect();
    Ok(compute_league_table(
        &teams,
        &results,
        if scheme == "asytral" {
            ASYTRAL_POINTS
        } else {
            FOOTBALL_POINTS
        },
    ))
}

// -------------------------------------------------------- Ultimate Team

/// Rarity weights: `(rarity, name, weight)`.
///
/// # Errors
/// Database errors.
pub async fn set_rarities(c: &Client, rarities: &[(i16, &str, i32)]) -> Result<()> {
    for (r, n, w) in rarities {
        c.execute("INSERT INTO achillea.ut_rarity (rarity, name, weight) VALUES ($1, $2, $3) ON CONFLICT (rarity) DO UPDATE SET name = EXCLUDED.name, weight = EXCLUDED.weight", &[r, n, w]).await?;
    }
    Ok(())
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Builds the card pool from an imported game table (`schema.table` of the nie-cfgsql import):
/// one card per row, `hash_col` becomes `chara_hash`, `name_col` (any type, cast to text) the name.
///
/// # Errors
/// Database errors.
pub async fn import_cards(
    c: &Client,
    schema: &str,
    table: &str,
    hash_col: &str,
    name_col: &str,
    rarity: i16,
    vfs_path: &str,
) -> Result<u64> {
    let sql = format!(
        "INSERT INTO achillea.ut_card (name, rarity, vfs_path, chara_hash) SELECT {n}::text, $1, $2, {h} FROM {s}.{t} ON CONFLICT DO NOTHING",
        n = quote(name_col),
        h = quote(hash_col),
        s = quote(schema),
        t = quote(table)
    );
    Ok(c.execute(&sql, &[&rarity, &vfs_path]).await?)
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// Opens a pack of `count` cards, deterministically from `seed`; returns the card ids.
///
/// # Errors
/// Rule error when the pool is empty; database errors.
pub async fn open_pack(c: &mut Client, player: i64, count: usize, seed: u64) -> Result<Vec<i64>> {
    let tx = c.transaction().await?;
    let rar: Vec<(i16, i32)> = tx
        .query(
            "SELECT rarity, weight FROM achillea.ut_rarity ORDER BY rarity",
            &[],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    let cards: Vec<(i64, i16)> = tx
        .query("SELECT id, rarity FROM achillea.ut_card ORDER BY id", &[])
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    // Only rarities that have cards can be drawn.
    let live: Vec<(i16, i32)> = rar
        .into_iter()
        .filter(|(r, _)| cards.iter().any(|c| c.1 == *r))
        .collect();
    let total: u64 = live.iter().map(|(_, w)| *w as u64).sum();
    if total == 0 {
        return rule("the card pool is empty");
    }
    let mut rng = Rng(seed | 1);
    let mut got = Vec::with_capacity(count);
    for _ in 0..count {
        let mut pick = rng.next() % total;
        let mut chosen = live[0].0;
        for (r, w) in &live {
            if pick < *w as u64 {
                chosen = *r;
                break;
            }
            pick -= *w as u64;
        }
        let pool: Vec<i64> = cards
            .iter()
            .filter(|c| c.1 == chosen)
            .map(|c| c.0)
            .collect();
        got.push(pool[(rng.next() % pool.len() as u64) as usize]);
    }
    for id in &got {
        tx.execute("INSERT INTO achillea.ut_collection (player_id, card_id, quantity) VALUES ($1, $2, 1) ON CONFLICT (player_id, card_id) DO UPDATE SET quantity = achillea.ut_collection.quantity + 1", &[&player, id]).await?;
    }
    tx.execute(
        "INSERT INTO achillea.ut_pack_open (player_id, seed, card_ids) VALUES ($1, $2, $3)",
        &[&player, &(seed as i64), &got],
    )
    .await?;
    tx.commit().await?;
    Ok(got)
}
