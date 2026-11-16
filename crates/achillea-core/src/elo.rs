//! Elo, progressive AP by tier, tournament podium prizes.

use crate::js_round;

/// Season K-factor when none is set.
pub const DEFAULT_K_FACTOR: f64 = 32.0;
/// Starting rating when the season sets none.
pub const DEFAULT_BASE_RATING: i64 = 0;

/// Probability (0..1) that the player beats the opponent.
#[must_use]
pub fn expected_score(player: f64, opponent: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf((opponent - player) / 400.0))
}

/// Rounded Elo change after a match (`actual`: 1 win, 0.5 draw, 0 loss).
#[must_use]
pub fn elo_delta(player: f64, opponent: f64, actual: f64, k: f64) -> f64 {
    js_round(k * (actual - expected_score(player, opponent)))
}

/// Directional K factors of a tier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TierEloFactors {
    /// K on a win.
    pub win_k: f64,
    /// K on a loss.
    pub loss_k: f64,
}

/// The 11 tiers, Fer (0) to Legendaire (10).
pub const TIER_ELO_FACTORS: [TierEloFactors; 11] = [
    TierEloFactors { win_k: 40.0, loss_k: 10.0 },
    TierEloFactors { win_k: 40.0, loss_k: 12.0 },
    TierEloFactors { win_k: 38.0, loss_k: 16.0 },
    TierEloFactors { win_k: 38.0, loss_k: 18.0 },
    TierEloFactors { win_k: 36.0, loss_k: 20.0 },
    TierEloFactors { win_k: 36.0, loss_k: 24.0 },
    TierEloFactors { win_k: 34.0, loss_k: 26.0 },
    TierEloFactors { win_k: 34.0, loss_k: 28.0 },
    TierEloFactors { win_k: 32.0, loss_k: 32.0 },
    TierEloFactors { win_k: 32.0, loss_k: 32.0 },
    TierEloFactors { win_k: 32.0, loss_k: 32.0 },
];

/// Factors of a tier index, clamped to the table (non-finite or negative gives tier 0).
#[must_use]
pub fn tier_elo_factors(tier: f64) -> TierEloFactors {
    if !tier.is_finite() || tier <= 0.0 {
        return TIER_ELO_FACTORS[0];
    }
    let i = (tier.floor() as usize).min(TIER_ELO_FACTORS.len() - 1);
    TIER_ELO_FACTORS[i]
}

/// Progressive AP change: directional K by tier times the Elo-gap modulation.
#[must_use]
pub fn progressive_elo_delta(player: f64, opponent: f64, actual: f64, tier: f64, season_k: f64) -> f64 {
    let f = tier_elo_factors(tier);
    let dir_k = if actual >= 1.0 {
        f.win_k
    } else if actual <= 0.0 {
        f.loss_k
    } else {
        (f.win_k + f.loss_k) / 2.0
    };
    let scaled = dir_k * (season_k / DEFAULT_K_FACTOR);
    js_round(scaled * (actual - expected_score(player, opponent)))
}

/// Applies a delta with a floor of 0; returns `(new rating, applied delta)`.
#[must_use]
pub fn apply_elo_update(player: f64, delta: f64) -> (f64, f64) {
    let new = (player + delta).max(0.0);
    (new, new - player)
}

/// Result of a winner/loser duel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EloMatchResult {
    /// New winner rating.
    pub new_winner: f64,
    /// New loser rating (floored at 0).
    pub new_loser: f64,
    /// Winner's gain.
    pub delta: f64,
}

/// Classic duel.
#[must_use]
pub fn compute_match_elo(winner: f64, loser: f64, k: f64) -> EloMatchResult {
    let expected_winner = expected_score(winner, loser);
    let expected_loser = 1.0 - expected_winner;
    let delta = js_round(k * (1.0 - expected_winner));
    let delta_loser = js_round(k * (0.0 - expected_loser));
    EloMatchResult { new_winner: winner + delta, new_loser: (loser + delta_loser).max(0.0), delta }
}

/// AP for the first, second and each semi-finalist of a tournament.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrizeTable {
    /// Winner.
    pub first: u32,
    /// Runner-up.
    pub second: u32,
    /// Each semi-finalist.
    pub semi: u32,
}

/// Prize table by tournament size.
#[must_use]
pub fn prize_table_for_size(size: f64) -> PrizeTable {
    let n = if size.is_finite() { size.floor().max(0.0) } else { 0.0 };
    let (first, second, semi) = if n <= 8.0 {
        (25, 12, 5)
    } else if n <= 16.0 {
        (35, 18, 8)
    } else if n <= 32.0 {
        (50, 25, 10)
    } else if n <= 64.0 {
        (80, 40, 15)
    } else if n <= 128.0 {
        (120, 60, 25)
    } else {
        (180, 90, 35)
    };
    PrizeTable { first, second, semi }
}

/// AP for a final placement (0 outside the top 4 or for an invalid rank).
#[must_use]
pub fn placement_prize_ap(size: f64, rank: f64) -> u32 {
    if !rank.is_finite() || rank < 1.0 {
        return 0;
    }
    let t = prize_table_for_size(size);
    if rank == 1.0 {
        t.first
    } else if rank == 2.0 {
        t.second
    } else if rank <= 4.0 {
        t.semi
    } else {
        0
    }
}

/// Ranked tier names of the French ladder, index 0 to 10.
pub const TIER_NAMES: [&str; 11] =
    ["Fer", "Bronze", "Argent", "Or", "Platine", "Diamant", "Emeraude", "Rubis", "Divin", "Supernova", "Legendaire"];
