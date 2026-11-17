//! League: points scales, round-robin fixtures, standings with Astral League tie-breakers.

use std::collections::BTreeMap;

/// Points scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaguePoints {
    /// Regulation win.
    pub win: i32,
    /// Draw.
    pub draw: i32,
    /// Regulation loss.
    pub loss: i32,
    /// Overtime win.
    pub win_overtime: i32,
    /// Overtime loss.
    pub loss_overtime: i32,
}

/// Asytral League scale.
pub const ASYTRAL_POINTS: LeaguePoints = LeaguePoints {
    win: 3,
    draw: 0,
    loss: 0,
    win_overtime: 2,
    loss_overtime: 1,
};
/// Classic football scale.
pub const FOOTBALL_POINTS: LeaguePoints = LeaguePoints {
    win: 3,
    draw: 1,
    loss: 0,
    win_overtime: 3,
    loss_overtime: 0,
};

/// A played league match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeagueMatchResult {
    /// Home team.
    pub home_id: String,
    /// Away team.
    pub away_id: String,
    /// Home score.
    pub home_score: i32,
    /// Away score.
    pub away_score: i32,
    /// Declared winner, if any.
    pub winner_id: Option<String>,
    /// Decided in overtime.
    pub overtime: bool,
}

/// A fixture of the calendar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeagueFixture {
    /// 1-based matchday.
    pub matchday: u32,
    /// Home team.
    pub home_id: String,
    /// Away team.
    pub away_id: String,
}

/// Standings row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeagueStanding {
    /// Team.
    pub team_id: String,
    /// 1-based rank.
    pub rank: usize,
    /// Matches played.
    pub played: i32,
    /// Regulation wins.
    pub wins: i32,
    /// Draws.
    pub draws: i32,
    /// Regulation losses.
    pub losses: i32,
    /// Overtime wins.
    pub wins_ot: i32,
    /// Overtime losses.
    pub losses_ot: i32,
    /// Championship points.
    pub points: i32,
    /// Score difference.
    pub map_diff: i32,
    /// Scored.
    pub maps_for: i32,
    /// Conceded.
    pub maps_against: i32,
}

/// Single round robin by the circle method, with a bye when the count is odd.
#[must_use]
pub fn generate_single_round_robin(team_ids: &[String]) -> Vec<LeagueFixture> {
    let mut teams: Vec<Option<&String>> = team_ids.iter().map(Some).collect();
    if teams.len() < 2 {
        return Vec::new();
    }
    if teams.len() % 2 == 1 {
        teams.push(None);
    }
    let n = teams.len();
    let half = n / 2;
    let mut rotation = teams;
    let mut out = Vec::new();
    for round in 0..(n - 1) {
        for i in 0..half {
            let (Some(a), Some(b)) = (rotation[i], rotation[n - 1 - i]) else {
                continue;
            };
            let home_first = (round + i) % 2 == 0;
            let (home, away) = if home_first { (a, b) } else { (b, a) };
            out.push(LeagueFixture {
                matchday: round as u32 + 1,
                home_id: home.clone(),
                away_id: away.clone(),
            });
        }
        let fixed = rotation[0];
        let mut rest: Vec<Option<&String>> = rotation[1..].to_vec();
        rest.rotate_right(1);
        rotation = std::iter::once(fixed).chain(rest).collect();
    }
    out
}

/// Winner of a match, `None` for a pure draw.
#[must_use]
pub fn match_winner_id(m: &LeagueMatchResult) -> Option<&str> {
    if let Some(w) = &m.winner_id
        && (*w == m.home_id || *w == m.away_id)
    {
        return Some(w);
    }
    if m.home_score > m.away_score {
        Some(&m.home_id)
    } else if m.away_score > m.home_score {
        Some(&m.away_id)
    } else {
        None
    }
}

/// +1 if `a` beat `b` (first meeting found), -1 if `b` beat `a`, else 0.
#[must_use]
pub fn head_to_head(a: &str, b: &str, results: &[LeagueMatchResult]) -> i32 {
    for m in results {
        let pair = (m.home_id == a && m.away_id == b) || (m.home_id == b && m.away_id == a);
        if !pair {
            continue;
        }
        match match_winner_id(m) {
            Some(w) if w == a => return 1,
            Some(w) if w == b => return -1,
            _ => {}
        }
    }
    0
}

fn empty(team: &str) -> LeagueStanding {
    LeagueStanding {
        team_id: team.to_owned(),
        rank: 0,
        played: 0,
        wins: 0,
        draws: 0,
        losses: 0,
        wins_ot: 0,
        losses_ot: 0,
        points: 0,
        map_diff: 0,
        maps_for: 0,
        maps_against: 0,
    }
}

/// Standings. Tie-breakers: points, score difference, head to head, scored, wins, team id.
#[must_use]
pub fn compute_league_table(
    team_ids: &[String],
    results: &[LeagueMatchResult],
    cfg: LeaguePoints,
) -> Vec<LeagueStanding> {
    let mut table: BTreeMap<String, LeagueStanding> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut touch = |id: &str, table: &mut BTreeMap<String, LeagueStanding>| {
        if !table.contains_key(id) {
            table.insert(id.to_owned(), empty(id));
            order.push(id.to_owned());
        }
    };
    for id in team_ids {
        touch(id, &mut table);
    }
    for m in results {
        touch(&m.home_id, &mut table);
        touch(&m.away_id, &mut table);
        {
            let h = table.get_mut(&m.home_id).expect("home");
            h.played += 1;
            h.maps_for += m.home_score;
            h.maps_against += m.away_score;
            h.map_diff = h.maps_for - h.maps_against;
        }
        {
            let a = table.get_mut(&m.away_id).expect("away");
            a.played += 1;
            a.maps_for += m.away_score;
            a.maps_against += m.home_score;
            a.map_diff = a.maps_for - a.maps_against;
        }
        let winner = match_winner_id(m).map(str::to_owned);
        match winner {
            Some(w) => {
                let loser = if w == m.home_id {
                    &m.away_id
                } else {
                    &m.home_id
                };
                {
                    let win = table.get_mut(&w).expect("winner");
                    if m.overtime {
                        win.wins_ot += 1;
                        win.points += cfg.win_overtime;
                    } else {
                        win.wins += 1;
                        win.points += cfg.win;
                    }
                }
                let lose = table.get_mut(loser).expect("loser");
                if m.overtime {
                    lose.losses_ot += 1;
                    lose.points += cfg.loss_overtime;
                } else {
                    lose.losses += 1;
                    lose.points += cfg.loss;
                }
            }
            None => {
                for id in [&m.home_id, &m.away_id] {
                    let t = table.get_mut(id).expect("team");
                    t.draws += 1;
                    t.points += cfg.draw;
                }
            }
        }
    }
    // Insertion order first so that a stable sort mirrors the JavaScript Map iteration order.
    let mut rows: Vec<LeagueStanding> = order.iter().map(|id| table[id].clone()).collect();
    // Insertion sort: stable and never panics, even though head-to-head is not a total order.
    let cmp = |a: &LeagueStanding, b: &LeagueStanding| {
        b.points
            .cmp(&a.points)
            .then(b.map_diff.cmp(&a.map_diff))
            .then_with(|| (-head_to_head(&a.team_id, &b.team_id, results)).cmp(&0))
            .then(b.maps_for.cmp(&a.maps_for))
            .then((b.wins + b.wins_ot).cmp(&(a.wins + a.wins_ot)))
            .then_with(|| a.team_id.cmp(&b.team_id))
    };
    for i in 1..rows.len() {
        let mut j = i;
        while j > 0 && cmp(&rows[j - 1], &rows[j]) == std::cmp::Ordering::Greater {
            rows.swap(j - 1, j);
            j -= 1;
        }
    }
    for (i, r) in rows.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    rows
}

/// Matchdays of a single round robin.
#[must_use]
pub fn matchday_count(team_count: u32) -> u32 {
    if team_count < 2 {
        0
    } else if team_count.is_multiple_of(2) {
        team_count - 1
    } else {
        team_count
    }
}
