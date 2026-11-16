//! Ranked transport rules: matchmaking window, queue pairing, score resolution.

/// Team side of a ranked match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamSide {
    /// Home.
    Team1,
    /// Away.
    Team2,
}

/// Side of the player at `index` in the room.
#[must_use]
pub fn team_for_index(index: usize) -> TeamSide {
    if index == 0 { TeamSide::Team1 } else { TeamSide::Team2 }
}

/// Bounds an untrusted score to 0..=99.
#[must_use]
pub fn clamp_score(value: f64) -> u32 {
    let s = value.floor();
    if !s.is_finite() {
        return 0;
    }
    s.clamp(0.0, 99.0) as u32
}

/// Only 1v1 (2 players) and 2v2 (4 players) are valid; anything else is 1v1.
#[must_use]
pub fn parse_mode(value: f64) -> u8 {
    if value.floor() == 4.0 { 4 } else { 2 }
}

/// Matching window: widens by 50 rating points every five seconds waited.
#[must_use]
pub fn elo_window(base: i64, waited_ms: i64) -> i64 {
    base + (waited_ms.max(0) / 5000) * 50
}

/// A player waiting in the queue.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueCandidate {
    /// Player id.
    pub user_id: String,
    /// Rating.
    pub elo: i64,
    /// Enqueue time, ms.
    pub enqueued_at: i64,
}

/// First compatible pair of distinct players in an Elo-sorted queue.
#[must_use]
pub fn select_queue_pair(sorted: &[QueueCandidate], now: i64, base_window: i64) -> Option<(usize, usize)> {
    for i in 0..sorted.len().saturating_sub(1) {
        let first = &sorted[i];
        for j in (i + 1)..sorted.len() {
            let second = &sorted[j];
            if first.user_id == second.user_id {
                continue;
            }
            let waited = (now - first.enqueued_at).max(now - second.enqueued_at);
            let window = elo_window(base_window, waited);
            if (first.elo - second.elo).abs() <= window {
                return Some((i, j));
            }
            if second.elo - first.elo > window {
                break;
            }
        }
    }
    None
}

/// One side's score report.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreInput {
    /// Reporter.
    pub user_id: String,
    /// Own score.
    pub my_score: u32,
    /// Opponent score.
    pub opp_score: u32,
    /// Whether the report was sent.
    pub submitted: bool,
}

/// Both reports submitted and mirrored.
#[must_use]
pub fn scores_agree(home: &ScoreInput, away: &ScoreInput) -> bool {
    home.submitted && away.submitted && home.my_score == away.opp_score && home.opp_score == away.my_score
}

/// `win`, `loss` or `draw` for a score pair.
#[must_use]
pub fn result_label(my: u32, opp: u32) -> &'static str {
    if my == opp {
        "draw"
    } else if my > opp {
        "win"
    } else {
        "loss"
    }
}

/// Outcome of the dual score reports.
#[derive(Debug, Clone, PartialEq)]
pub enum ScoreOutcome {
    /// Waiting for the other report.
    Wait,
    /// Agreement.
    Agree {
        /// Home score.
        s1: u32,
        /// Away score.
        s2: u32,
    },
    /// One side did not report in time.
    Forfeit {
        /// Home score.
        s1: u32,
        /// Away score.
        s2: u32,
        /// Player who forfeited.
        forfeit_user_id: String,
    },
    /// Reports disagree.
    Dispute,
}

/// Resolves the dual reports without touching storage, sockets or Discord.
#[must_use]
pub fn resolve_scores(home: &ScoreInput, away: &ScoreInput, forced: bool) -> ScoreOutcome {
    if !forced && (!home.submitted || !away.submitted) {
        return ScoreOutcome::Wait;
    }
    if scores_agree(home, away) {
        return ScoreOutcome::Agree { s1: home.my_score, s2: home.opp_score };
    }
    if forced && home.submitted && !away.submitted {
        return ScoreOutcome::Forfeit { s1: home.my_score, s2: home.opp_score, forfeit_user_id: away.user_id.clone() };
    }
    if forced && away.submitted && !home.submitted {
        return ScoreOutcome::Forfeit { s1: away.opp_score, s2: away.my_score, forfeit_user_id: home.user_id.clone() };
    }
    ScoreOutcome::Dispute
}
