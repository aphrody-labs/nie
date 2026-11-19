//! nie-launcher CLI entrypoint.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use nie_launcher::{
    DEFAULT_PASSPHRASE, SaveSession, TeamExportEnvelope, decrypt_team_envelope,
    inspect_package_header,
};

#[derive(Parser)]
#[command(name = "nie-launcher")]
#[command(about = "Native Rust launcher, save editor, and mod package manager for IEVR")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Team lineup export management
    Team {
        #[command(subcommand)]
        cmd: TeamCommands,
    },
    /// Mod package (.utmod) inspection
    Package {
        #[command(subcommand)]
        cmd: PackageCommands,
    },
    /// Save session operations
    Save {
        #[command(subcommand)]
        cmd: SaveCommands,
    },
    /// Spirit cards (142) and special moves (1,299) roster
    Spirit {
        #[command(subcommand)]
        cmd: SpiritCommands,
    },
}

#[derive(Subcommand)]
enum TeamCommands {
    /// Decrypt an exported team JSON from the web platform
    Decrypt {
        /// Path to encrypted team JSON file
        file: PathBuf,
        /// Custom passphrase if different from canonical
        #[arg(long, default_value = DEFAULT_PASSPHRASE)]
        passphrase: String,
    },
}

#[derive(Subcommand)]
enum PackageCommands {
    /// Inspect a .utmod package header
    Info {
        /// Path to .utmod file
        file: PathBuf,
    },
}

#[derive(Subcommand)]
enum SaveCommands {
    /// Safely park the live Steam save to .bk and install a mod save
    Park {
        /// Live save path (e.g. 002AB8F4-USERDATALIVE)
        live_save: PathBuf,
        /// Mod save path
        mod_save: PathBuf,
    },
    /// Restore the parked .bk save
    Restore {
        /// Live save path
        live_save: PathBuf,
        /// Mod save path
        mod_save: PathBuf,
    },
}

#[derive(Subcommand)]
enum SpiritCommands {
    /// Search or list 142 spirit cards
    Cards {
        /// Optional query filter
        #[arg(long, short = 'q')]
        query: Option<String>,
        /// Output JSON
        #[arg(long)]
        json: bool,
    },
    /// Search or list 1,299 special moves
    Moves {
        /// Optional query filter
        #[arg(long, short = 'q')]
        query: Option<String>,
        /// Category filter (Shot, Catch, Dribble, Block)
        #[arg(long, short = 'c')]
        category: Option<String>,
        /// Output JSON
        #[arg(long)]
        json: bool,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Team { cmd } => match cmd {
            TeamCommands::Decrypt { file, passphrase } => {
                let content = std::fs::read_to_string(&file)?;
                let envelope: TeamExportEnvelope = serde_json::from_str(&content)?;
                let lineup = decrypt_team_envelope(&envelope, &passphrase)?;
                println!("{}", serde_json::to_string_pretty(&lineup)?);
            }
        },
        Commands::Package { cmd } => match cmd {
            PackageCommands::Info { file } => {
                let data = std::fs::read(&file)?;
                let header = inspect_package_header(&data)?;
                println!(
                    "Package GUID: {}\nFile size: {} bytes\nPayload offset: 0x{:X}",
                    header.package_id, header.file_size, header.payload_offset
                );
            }
        },
        Commands::Save { cmd } => match cmd {
            SaveCommands::Park {
                live_save,
                mod_save,
            } => {
                let mut session = SaveSession::new(live_save, mod_save);
                session.park_and_install_mod_save()?;
                // Forget drop so session stays active
                std::mem::forget(session);
                println!("Mod save installed and original save parked successfully.");
            }
            SaveCommands::Restore {
                live_save,
                mod_save,
            } => {
                let mut session = SaveSession::new(live_save, mod_save);
                session.is_active = true;
                session.restore_original_save()?;
                println!("Original save restored successfully.");
            }
        },
        Commands::Spirit { cmd } => match cmd {
            SpiritCommands::Cards { query, json } => {
                let cards: Vec<&nie_launcher::SpiritCard> = if let Some(q) = query {
                    nie_launcher::search_spirit_cards(&q)
                } else {
                    nie_launcher::all_spirit_cards().iter().collect()
                };
                if json {
                    println!("{}", serde_json::to_string_pretty(&cards)?);
                } else {
                    println!("Spirit Cards ({}) :", cards.len());
                    for c in cards {
                        println!("  0x{} | {:<30} | {}", c.hex_id, c.name, c.variant);
                    }
                }
            }
            SpiritCommands::Moves {
                query,
                category,
                json,
            } => {
                let mut moves: Vec<&nie_launcher::SpecialMove> = if let Some(q) = query {
                    nie_launcher::search_special_moves(&q)
                } else {
                    nie_launcher::all_special_moves().iter().collect()
                };
                if let Some(cat) = category {
                    moves.retain(|m| m.category.eq_ignore_ascii_case(&cat));
                }
                if json {
                    println!("{}", serde_json::to_string_pretty(&moves)?);
                } else {
                    println!("Special Moves ({}) :", moves.len());
                    for m in moves {
                        println!("  0x{} | {:<35} | {}", m.hex_id, m.name, m.category);
                    }
                }
            }
        },
    }

    Ok(())
}
