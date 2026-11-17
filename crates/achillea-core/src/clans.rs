//! Clan rules: role hierarchy, distinctions, powers.

/// Clan member role; the numeric value is the stored one (Trial 0 to Captain 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClanMemberRole {
    /// On trial.
    Trial = 0,
    /// Full member.
    Member = 1,
    /// Vice captain.
    SubCaptain = 2,
    /// Captain (president).
    Captain = 3,
}

impl ClanMemberRole {
    /// Role from its stored value.
    #[must_use]
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Trial),
            1 => Some(Self::Member),
            2 => Some(Self::SubCaptain),
            3 => Some(Self::Captain),
            _ => None,
        }
    }
}

/// Hierarchy weight; does not rely on the enum order so reordering it cannot change rights.
#[must_use]
pub const fn role_weight(role: ClanMemberRole) -> u8 {
    match role {
        ClanMemberRole::Trial => 0,
        ClanMemberRole::Member => 1,
        ClanMemberRole::SubCaptain => 2,
        ClanMemberRole::Captain => 3,
    }
}

/// `role` is at least `minimum`.
#[must_use]
pub const fn has_role_at_least(role: ClanMemberRole, minimum: ClanMemberRole) -> bool {
    role_weight(role) >= role_weight(minimum)
}

/// Captain and vice captain manage recruitment.
#[must_use]
pub const fn can_manage_recruitment(role: ClanMemberRole) -> bool {
    has_role_at_least(role, ClanMemberRole::SubCaptain)
}

/// Only the captain dissolves the clan or hands over the captaincy.
#[must_use]
pub const fn can_dissolve_clan(role: ClanMemberRole) -> bool {
    matches!(role, ClanMemberRole::Captain)
}

/// A member acts only on a strictly lower role.
#[must_use]
pub const fn can_act_on_member(actor: ClanMemberRole, target: ClanMemberRole) -> bool {
    role_weight(actor) > role_weight(target)
}

/// Display priority of a distinction (stored French value or the legacy English key);
/// 99 when unknown.
#[must_use]
pub fn distinction_priority(distinction: &str) -> u8 {
    match distinction {
        "Staff" => 1,
        "Joueur" | "Player" => 2,
        "Artiste" | "Artist" => 3,
        "Créateur de contenu" | "ContentCreator" | "Content Creator" => 4,
        _ => 99,
    }
}

/// Priority of a member: the best of its distinctions, 99 when none.
#[must_use]
pub fn member_distinction_priority(distinctions: &[&str]) -> u8 {
    distinctions
        .iter()
        .map(|d| distinction_priority(d))
        .min()
        .unwrap_or(99)
}
