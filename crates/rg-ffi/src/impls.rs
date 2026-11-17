//! Behaviour behind each exported symbol, named exactly like the contract entry.

use achillea_core::brackets::{bye_count, single_elimination_rounds};
use achillea_core::clans::{ClanMemberRole, can_act_on_member};
use achillea_core::elo::{expected_score, placement_prize_ap, progressive_elo_delta};
use nie_cfgsql::Dialect;

/// `rg_elo_expected_score`
#[must_use]
pub fn rg_elo_expected_score(player: f64, opponent: f64) -> f64 {
    expected_score(player, opponent)
}

/// `rg_elo_progressive_delta`
#[must_use]
pub fn rg_elo_progressive_delta(
    player: f64,
    opponent: f64,
    actual: f64,
    tier: f64,
    season_k: f64,
) -> f64 {
    progressive_elo_delta(player, opponent, actual, tier, season_k)
}

/// `rg_tournament_placement_prize_ap`
#[must_use]
pub fn rg_tournament_placement_prize_ap(size: f64, rank: f64) -> u32 {
    placement_prize_ap(size, rank)
}

/// `rg_bracket_single_elimination_rounds`
#[must_use]
pub fn rg_bracket_single_elimination_rounds(participants: u64) -> u64 {
    single_elimination_rounds(participants)
}

/// `rg_bracket_bye_count`
#[must_use]
pub fn rg_bracket_bye_count(participants: u64) -> u64 {
    bye_count(participants)
}

/// `rg_clan_can_act_on_member`; unknown roles never grant power.
#[must_use]
pub fn rg_clan_can_act_on_member(actor_role: i32, target_role: i32) -> bool {
    match (
        ClanMemberRole::from_i32(actor_role),
        ClanMemberRole::from_i32(target_role),
    ) {
        (Some(a), Some(t)) => can_act_on_member(a, t),
        _ => false,
    }
}

/// `rg_sitemap_encode_path`
#[must_use]
pub fn rg_sitemap_encode_path(path: &str) -> Option<String> {
    Some(nie_sitemap::encode_path(path))
}

/// `rg_cfgbin_to_sql`
#[must_use]
pub fn rg_cfgbin_to_sql(data: &[u8], dialect: i32) -> Option<String> {
    let dialect = if dialect == 0 {
        Dialect::Postgres
    } else {
        Dialect::Sqlite
    };
    nie_cfgsql::from_bytes(data).ok().map(|d| d.to_sql(dialect))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behaviours() {
        assert_eq!(
            rg_elo_progressive_delta(1000.0, 1000.0, 1.0, 0.0, 32.0),
            20.0
        );
        assert_eq!(rg_tournament_placement_prize_ap(32.0, 1.0), 50);
        assert_eq!(rg_bracket_single_elimination_rounds(6), 3);
        assert_eq!(rg_bracket_bye_count(6), 2);
        assert!(
            rg_clan_can_act_on_member(3, 2)
                && !rg_clan_can_act_on_member(2, 2)
                && !rg_clan_can_act_on_member(9, 0)
        );
        assert_eq!(
            rg_sitemap_encode_path("a b/c&d").as_deref(),
            Some("a%20b/c%26d")
        );
        assert!(rg_cfgbin_to_sql(b"nope", 0).is_none());
    }
}
