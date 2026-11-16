//! Bracket arithmetic: size, rounds, byes, match counts, seeding.

/// Stage format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageType {
    /// Everyone plays everyone.
    RoundRobin,
    /// Single elimination tree.
    SingleElimination,
    /// Double elimination tree.
    DoubleElimination,
    /// League calendar with a points table.
    League,
}

/// Match format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchFormat {
    /// Best of 1.
    Bo1,
    /// Best of 3.
    Bo3,
    /// Best of 5.
    Bo5,
    /// Two legs, decided on aggregate.
    DoubleLeg,
    /// Custom number of games.
    Custom,
}

/// Smallest power of two `>= size` (minimum 1).
#[must_use]
pub fn next_power_of_two(size: u64) -> u64 {
    size.max(1).next_power_of_two()
}

/// Exactly a power of two.
#[must_use]
pub fn is_power_of_two(size: u64) -> bool {
    size > 0 && size & (size - 1) == 0
}

/// First-round byes for `participants`.
#[must_use]
pub fn bye_count(participants: u64) -> u64 {
    if participants <= 1 { 0 } else { next_power_of_two(participants) - participants }
}

/// Rounds of a single elimination tree.
#[must_use]
pub fn single_elimination_rounds(participants: u64) -> u64 {
    if participants <= 1 { 0 } else { u64::from(participants.next_power_of_two().trailing_zeros()) }
}

/// Rounds of a double elimination, grand final included.
#[must_use]
pub fn double_elimination_rounds(participants: u64) -> u64 {
    2 * single_elimination_rounds(participants)
}

/// Matchdays of a round robin (circle algorithm).
#[must_use]
pub fn round_robin_rounds(participants: u64, double_legged: bool) -> u64 {
    if participants <= 1 {
        return 0;
    }
    let rounds = if participants % 2 == 0 { participants - 1 } else { participants };
    if double_legged { rounds * 2 } else { rounds }
}

/// Total rounds for a format.
#[must_use]
pub fn stage_rounds(kind: StageType, participants: u64, double_legged: bool) -> u64 {
    match kind {
        StageType::SingleElimination => single_elimination_rounds(participants),
        StageType::DoubleElimination => double_elimination_rounds(participants),
        StageType::RoundRobin | StageType::League => round_robin_rounds(participants, double_legged),
    }
}

/// Total matches for a format.
#[must_use]
pub fn stage_match_count(kind: StageType, participants: u64, double_legged: bool) -> u64 {
    let n = participants;
    if n <= 1 {
        return 0;
    }
    match kind {
        StageType::SingleElimination => n - 1,
        StageType::DoubleElimination => 2 * n - 2,
        StageType::RoundRobin | StageType::League => {
            if double_legged { n * (n - 1) } else { n * (n - 1) / 2 }
        }
    }
}

/// Games in a match.
#[must_use]
pub fn match_game_count(format: MatchFormat, custom: Option<u64>) -> u64 {
    match format {
        MatchFormat::Bo1 => 1,
        MatchFormat::Bo3 => 3,
        MatchFormat::Bo5 => 5,
        MatchFormat::DoubleLeg => 2,
        MatchFormat::Custom => custom.unwrap_or(1).max(1),
    }
}

/// Games needed to win the match.
#[must_use]
pub fn games_to_win(format: MatchFormat, custom: Option<u64>) -> u64 {
    if format == MatchFormat::DoubleLeg { 2 } else { match_game_count(format, custom).div_ceil(2) }
}

/// Standard seeding: top seeds meet as late as possible (1 v 16, 8 v 9, ...). Seeds are 1-based.
#[must_use]
pub fn standard_seed_order(bracket_size: u64) -> Vec<u64> {
    let size = next_power_of_two(bracket_size);
    let mut order = vec![1u64];
    while (order.len() as u64) < size {
        let round = order.len() as u64 * 2;
        order = order.iter().flat_map(|&s| [s, round + 1 - s]).collect();
    }
    order
}

/// Round name counted from the end (1 = final).
#[must_use]
pub fn round_label_from_end(remaining: u64) -> String {
    match remaining {
        1 => "Finale".into(),
        2 => "Demi-finales".into(),
        3 => "Quarts de finale".into(),
        4 => "Huitiemes de finale".into(),
        5 => "Seiziemes de finale".into(),
        n => format!("Tour {n}"),
    }
}
