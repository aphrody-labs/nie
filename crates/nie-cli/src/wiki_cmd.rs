//! `nie wiki`: the game's characters, skills and teams computed from the VFS (`nie-wiki-vfs`).

use std::path::PathBuf;

use anyhow::Context as _;
use clap::{Args, Subcommand};
use nie_wiki_vfs::{Localized, Provenance, Query, Wiki};

#[derive(Debug, Args)]
pub struct WikiArgs {
    #[command(subcommand)]
    pub op: WikiOp,
    /// Game root, `data` directory or remote URI (default: automatic resolution, `NIE_GAME_SOURCE`).
    #[arg(long, global = true)]
    pub game_dir: Option<PathBuf>,
    /// Languages of the labels, folders or BCP 47 tags, comma separated (default: every language).
    #[arg(long, global = true, value_delimiter = ',')]
    pub lang: Vec<String>,
    /// Full records as JSON instead of one line per record.
    #[arg(long, global = true)]
    pub json: bool,
    /// List filter: case-insensitive substring of names and codes.
    #[arg(short = 'q', long, global = true)]
    pub query: Option<String>,
    /// List window size (default: every match).
    #[arg(long, global = true)]
    pub limit: Option<usize>,
    /// List window start.
    #[arg(long, global = true, default_value_t = 0)]
    pub offset: usize,
}

#[derive(Debug, Subcommand)]
pub enum WikiOp {
    /// Characters: `chara_param` joined to `chara_base`, with names, texts and team.
    Characters,
    /// One character by `chara_param` id (`0xXXXXXXXX`) or internal code (`c01000010`).
    Character { key: String },
    /// Skills of `skill_config` with their texts.
    Skills,
    /// One skill by `skillID` (`0xXXXXXXXX`) or code (`whs00010`).
    Skill { key: String },
    /// Teams of `belong_team_config`, emblems resolved by file-name hash.
    Teams,
    /// One team by `belongTeamId` (`0xXXXXXXXX`).
    Team { id: String },
    /// The text languages the installation ships.
    Languages,
    /// The families the VFS gives no source for.
    Unsourced,
}

pub fn run(args: WikiArgs) -> anyhow::Result<()> {
    let source = args.game_dir.as_ref().map(|path| path.to_string_lossy());
    let wiki = Wiki::open(source.as_deref()).context("mounting the game")?;
    let query = Query {
        languages: args.lang.clone(),
        text: args.query.clone(),
        offset: args.offset,
        limit: args.limit,
    };
    let json = args.json;
    let lang = wiki
        .resolve_languages(&args.lang)?
        .into_iter()
        .next()
        .unwrap_or_default();

    match args.op {
        WikiOp::Languages => emit(json, &wiki.languages(), |l| {
            format!("{}\t{}", l.folder, l.tag)
        }),
        WikiOp::Unsourced => emit(json, &wiki.unsourced(), |s| {
            format!("{}\t{}", s.family, s.reason)
        }),
        WikiOp::Characters => {
            let page = wiki.characters(&query)?;
            list(json, &page, |c| {
                format!(
                    "{}\t{}\t{}\telement {}\tposition {}/{}\t{}",
                    c.chara_param_id,
                    c.internal_code,
                    text(c.name.as_ref(), &lang).unwrap_or("-"),
                    c.element.value,
                    c.main_position.value,
                    c.sub_position.value,
                    c.team
                        .as_ref()
                        .and_then(|t| text(t.name.as_ref(), &lang))
                        .unwrap_or("-"),
                )
            })
        }
        WikiOp::Character { key } => {
            let c = wiki.character(&key, &args.lang)?;
            one(json, c, &key, |c| {
                let mut out = format!(
                    "{}\n  code {}\n  gender {}\n  element {} {}\n  position {}/{}\n  growth pattern {}\n",
                    text(c.name.as_ref(), &lang).unwrap_or(&c.internal_code),
                    c.internal_code,
                    c.gender
                        .as_ref()
                        .map_or("absent".to_owned(), |g| g.value.to_string()),
                    c.element.value,
                    text(c.element.label.as_ref(), &lang).unwrap_or(""),
                    c.main_position.value,
                    c.sub_position.value,
                    c.growth_pattern,
                );
                if let Some(team) = &c.team {
                    out += &format!(
                        "  team {} {}\n",
                        team.belong_team_id,
                        text(team.name.as_ref(), &lang).unwrap_or("")
                    );
                }
                for skill in &c.skills {
                    out += &format!(
                        "  skill {} level {} {}\n",
                        skill.skill_id,
                        skill.learn_level,
                        text(skill.name.as_ref(), &lang).unwrap_or("")
                    );
                }
                out += &provenance("  param", &c.param_source);
                out += &provenance("  base", &c.base_source);
                out
            })
        }
        WikiOp::Skills => {
            let page = wiki.skills(&query)?;
            list(json, &page, |s| {
                format!(
                    "{}\t{}\t{}\telement {}\tcategory {}",
                    s.skill_id,
                    s.skill_id_str,
                    text(s.name.as_ref(), &lang).unwrap_or("-"),
                    s.element
                        .as_ref()
                        .map_or(String::new(), |e| e.value.to_string()),
                    s.category
                        .as_ref()
                        .map_or(String::new(), |c| c.value.to_string()),
                )
            })
        }
        WikiOp::Skill { key } => {
            let s = wiki.skill(&key, &args.lang)?;
            one(json, s, &key, |s| {
                format!(
                    "{}\n  code {}\n  {}\n{}",
                    text(s.name.as_ref(), &lang).unwrap_or(&s.skill_id_str),
                    s.skill_id_str,
                    text(s.description.as_ref(), &lang).unwrap_or(""),
                    provenance("  source", &s.source)
                )
            })
        }
        WikiOp::Teams => {
            let page = wiki.teams(&query)?;
            list(json, &page, |t| {
                format!(
                    "{}\t{}\t{} affiliated\t{} emblem(s)",
                    t.belong_team_id,
                    text(t.name.as_ref(), &lang).unwrap_or("-"),
                    t.affiliated_characters.len(),
                    t.emblems.values().filter(|e| !e.files.is_empty()).count(),
                )
            })
        }
        WikiOp::Team { id } => {
            let t = wiki.team(&id, &args.lang)?;
            one(json, t, &id, |t| {
                let mut out = format!(
                    "{}\n  id {}\n",
                    text(t.name.as_ref(), &lang).unwrap_or(""),
                    t.belong_team_id
                );
                for (field, emblem) in &t.emblems {
                    out += &format!(
                        "  {field} {} {}\n",
                        emblem.emblem_id,
                        if emblem.files.is_empty() {
                            "(no file)".to_owned()
                        } else {
                            emblem.files.join(", ")
                        }
                    );
                }
                out += &format!(
                    "  {} affiliated characters\n",
                    t.affiliated_characters.len()
                );
                out += &format!("  roster: no source ({})\n", t.roster.reason);
                out += &provenance("  source", &t.source);
                out
            })
        }
    }
}

fn text<'a>(localized: Option<&'a Localized>, lang: &str) -> Option<&'a str> {
    let localized = localized?;
    localized.text(lang).or_else(|| {
        let first = localized.texts.keys().next()?;
        localized.text(first)
    })
}

fn provenance(label: &str, p: &Provenance) -> String {
    format!("{label}: {} {} entry {}\n", p.vfs_path, p.table, p.index)
}

fn emit<T: serde::Serialize>(
    json: bool,
    items: &&[T],
    line: impl Fn(&T) -> String,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(items)?);
    } else {
        for item in items.iter() {
            println!("{}", line(item));
        }
    }
    Ok(())
}

fn list<T: serde::Serialize>(
    json: bool,
    page: &nie_wiki_vfs::Page<T>,
    line: impl Fn(&T) -> String,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(page)?);
    } else {
        for item in &page.items {
            println!("{}", line(item));
        }
        eprintln!("{} match(es), offset {}", page.total, page.offset);
    }
    Ok(())
}

fn one<T: serde::Serialize>(
    json: bool,
    record: Option<T>,
    key: &str,
    render: impl Fn(&T) -> String,
) -> anyhow::Result<()> {
    let record = record.with_context(|| format!("no entry matches `{key}`"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&record)?);
    } else {
        print!("{}", render(&record));
    }
    Ok(())
}
