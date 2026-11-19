//! # nie-launcher
//!
//! Native Rust launcher, save editor, and mod package manager for **Inazuma Eleven: Victory Road**.
//!
//! Complete 100% Rust replacement for the legacy .NET `UTLauncher.exe` and `UltimateTeamLauncher.Core.dll`:
//! - Cryptographic team export decryption (`AES-256-GCM + PBKDF2-SHA256`)
//! - Mod package reader (`.utmod` / `UTMOD2`)
//! - Save container coordinator (`002AB8F4-USERDATALIVE`)
//! - In-memory VFS streaming IPC channel

pub mod error;
pub mod package;
pub mod save;
pub mod spirit;
pub mod team;
pub mod ut;
pub mod vfs;

pub use error::{LauncherError, Result};
pub use package::{PackageHeader, inspect_package_header};
pub use save::SaveSession;
pub use spirit::{
    SPECIAL_MOVES, SPIRIT_CARDS, SpecialMove, SpiritCard, all_special_moves, all_spirit_cards,
    filter_special_moves_by_category, find_special_move_by_hex, find_spirit_card_by_id,
    search_special_moves, search_spirit_cards,
};
pub use team::{
    DEFAULT_PASSPHRASE, TeamCharacter, TeamExportEnvelope, TeamLineup, decrypt_team_envelope,
    encrypt_team_envelope,
};
pub use ut::{
    DrawnCard, PackOpeningResult, SquadValuation, UtDatabase, UtFormationLayout, UtFormationSlot,
    UtPack, UtPlayer, UtTeam, calculate_squad_valuation, formation_layout, open_pack,
    open_pack_with_optional_seed, quick_sell_value,
};
pub use vfs::VfsServer;

/// The launcher plugin: the Ultimate Team launcher, `.utmod` mod containers and the Viola
/// archive operations.
pub struct LauncherPlugin;

impl nie_plugin::Plugin for LauncherPlugin {
    fn info(&self) -> nie_plugin::PluginInfo {
        nie_plugin::PluginInfo {
            id: "launcher",
            summary: "Ultimate Team launcher, .utmod mods and Viola archive operations",
            package: "nie-launcher",
            commands: &["launcher", "mod", "viola"],
        }
    }
}

#[cfg(feature = "crlib")]
#[allow(unsafe_code)]
mod abi_generated;
