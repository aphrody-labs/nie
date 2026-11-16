//! Bot logic that does not depend on Discord: the two-sided score report flow.

use std::collections::HashMap;

use achillea_core::ranked::{ScoreInput, ScoreOutcome, clamp_score, resolve_scores};

/// What the bot should do after a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportResult {
    /// First report stored, waiting for the opponent.
    Waiting,
    /// Both agree: record the match (`home` is the first reporter).
    Record {
        /// Home player (first reporter).
        home: u64,
        /// Away player.
        away: u64,
        /// Home score.
        home_score: u32,
        /// Away score.
        away_score: u32,
    },
    /// The reports disagree; both were cleared.
    Dispute,
}

/// Pending reports keyed by the unordered player pair.
#[derive(Debug, Default)]
pub struct PendingReports {
    pending: HashMap<(u64, u64), ScoreInput>,
}

fn pair(a: u64, b: u64) -> (u64, u64) {
    (a.min(b), a.max(b))
}

impl PendingReports {
    /// `reporter` says they scored `mine` against `opponent` who scored `theirs`.
    pub fn report(&mut self, reporter: u64, opponent: u64, mine: f64, theirs: f64) -> ReportResult {
        let key = pair(reporter, opponent);
        let this = ScoreInput { user_id: reporter.to_string(), my_score: clamp_score(mine), opp_score: clamp_score(theirs), submitted: true };
        match self.pending.remove(&key) {
            Some(first) if first.user_id != this.user_id => match resolve_scores(&first, &this, false) {
                ScoreOutcome::Agree { s1, s2 } => ReportResult::Record {
                    home: first.user_id.parse().unwrap_or(reporter),
                    away: reporter,
                    home_score: s1,
                    away_score: s2,
                },
                _ => ReportResult::Dispute,
            },
            // First report, or the same player correcting their own report.
            _ => {
                self.pending.insert(key, this);
                ReportResult::Waiting
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_reports_record_the_match() {
        let mut p = PendingReports::default();
        assert_eq!(p.report(1, 2, 3.0, 1.0), ReportResult::Waiting);
        assert_eq!(p.report(2, 1, 1.0, 3.0), ReportResult::Record { home: 1, away: 2, home_score: 3, away_score: 1 });
        assert_eq!(p.report(1, 2, 0.0, 0.0), ReportResult::Waiting, "pair is free again");
    }

    #[test]
    fn disagreement_is_a_dispute_and_clears() {
        let mut p = PendingReports::default();
        p.report(1, 2, 3.0, 1.0);
        assert_eq!(p.report(2, 1, 3.0, 1.0), ReportResult::Dispute);
        assert_eq!(p.report(2, 1, 1.0, 3.0), ReportResult::Waiting);
    }

    #[test]
    fn a_player_can_correct_their_own_report() {
        let mut p = PendingReports::default();
        p.report(1, 2, 3.0, 1.0);
        assert_eq!(p.report(1, 2, 2.0, 1.0), ReportResult::Waiting);
        assert_eq!(p.report(2, 1, 1.0, 2.0), ReportResult::Record { home: 1, away: 2, home_score: 2, away_score: 1 });
    }

    #[test]
    fn scores_are_clamped() {
        let mut p = PendingReports::default();
        p.report(1, 2, 500.0, -3.0);
        assert_eq!(p.report(2, 1, 0.0, 99.0), ReportResult::Record { home: 1, away: 2, home_score: 99, away_score: 0 });
    }
}
