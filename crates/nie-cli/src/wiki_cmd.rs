//! `nie wiki`: the game's characters, skills, teams and tables computed from the VFS
//! (`nie-wiki-vfs`), and the conformance check of a wiki against that VFS.

use std::path::PathBuf;

use anyhow::Context as _;
use clap::{Args, Subcommand};
use nie_wiki_vfs::{Coded, Localized, Provenance, Query, Wiki};

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
    /// Character class filter: param_and_base, param_without_base or base_without_param.
    #[arg(long, global = true)]
    pub class: Option<String>,
    /// List window size (default: every match).
    #[arg(long, global = true)]
    pub limit: Option<usize>,
    /// List window start.
    #[arg(long, global = true, default_value_t = 0)]
    pub offset: usize,
}

#[derive(Debug, Subcommand)]
pub enum WikiOp {
    /// Characters: every `chara_param` and `chara_base` entry, with names, texts and team.
    Characters,
    /// One character by `chara_param` id (`0xXXXXXXXX`), `base-<n>`, base id or internal code.
    Character { key: String },
    /// Skills of `skill_config` with their texts.
    Skills,
    /// One skill by `skillID` (`0xXXXXXXXX`) or code (`whs00010`).
    Skill { key: String },
    /// Teams of `belong_team_config`, emblems resolved by file-name hash.
    Teams,
    /// One team by `belongTeamId` (`0xXXXXXXXX`).
    Team { id: String },
    /// Every decoded table with its node classes.
    Tables,
    /// The entries of one table class as stored (`chara_param` `STORY_FIX_SKILL_INFO`).
    Table { table: String, class: String },
    /// Every gamedata file and whether the wiki decodes it.
    Catalog,
    /// The text languages the installation ships.
    Languages,
    /// The families the VFS gives no source for.
    Unsourced,
    /// Compare what a wiki serves, field by field, to the raw decode of the VFS entries.
    Verify(crate::wiki_verify::VerifyArgs),
}

pub fn run(args: WikiArgs) -> anyhow::Result<()> {
    if let WikiOp::Verify(verify) = &args.op {
        return crate::wiki_verify::run(verify, args.game_dir.as_deref());
    }
    let source = args.game_dir.as_ref().map(|path| path.to_string_lossy());
    let wiki = Wiki::open(source.as_deref()).context("mounting the game")?;
    let query = Query {
        languages: args.lang.clone(),
        text: args.query.clone(),
        class: args.class.clone(),
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
        WikiOp::Verify(_) => unreachable!("handled above"),
        WikiOp::Languages => emit(json, wiki.languages(), |l| {
            format!("{}\t{}", l.folder, l.tag)
        }),
        WikiOp::Unsourced => emit(json, wiki.unsourced(), |s| {
            format!("{}\t{}", s.family, s.reason)
        }),
        WikiOp::Catalog => emit(json, &wiki.catalog()?, |e| {
            format!("{}\t{}", e.status, e.vfs_path)
        }),
        WikiOp::Tables => emit(json, &wiki.tables()?, |t| {
            let classes: Vec<String> = t
                .classes
                .iter()
                .map(|c| format!("{}={}", c.name, c.entries))
                .collect();
            format!("{}\t{}\t{}", t.table, t.vfs_path, classes.join(" "))
        }),
        WikiOp::Table { table, class } => {
            let page = wiki
                .table_records(&table, &class, &query)?
                .with_context(|| format!("no table class `{table}/{class}`"))?;
            list(json, &page, |r| {
                let stored = match (&r.fields, &r.variables) {
                    (Some(f), _) => serde_json::to_string(f).unwrap_or_default(),
                    (None, Some(v)) => serde_json::to_string(v).unwrap_or_default(),
                    (None, None) => String::new(),
                };
                let labels: Vec<String> = r
                    .labels
                    .iter()
                    .filter_map(|(k, l)| Some(format!("{k}={}", text(Some(l), &lang)?)))
                    .collect();
                format!("{}\t{}\t{}", r.index, stored, labels.join(" "))
            })
        }
        WikiOp::Characters => {
            let page = wiki.characters(&query)?;
            list(json, &page, |c| {
                format!(
                    "{}\t{}\t{}\t{:?}\telement {}\tposition {}/{}\t{}",
                    c.chara_param_id
                        .map_or_else(|| c.chara_base_id.to_string(), |id| id.to_string()),
                    c.internal_code.as_deref().unwrap_or("-"),
                    text(c.name.as_ref(), &lang).unwrap_or("-"),
                    c.class,
                    code(c.element.as_ref()),
                    code(c.main_position.as_ref()),
                    code(c.sub_position.as_ref()),
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
                    "{}\n  class {:?}\n  code {}\n  gender {}\n  element {} {}\n  position {}/{}\n  growth pattern {}\n",
                    text(c.name.as_ref(), &lang).unwrap_or("-"),
                    c.class,
                    c.internal_code.as_deref().unwrap_or("-"),
                    code(c.gender.as_ref()),
                    code(c.element.as_ref()),
                    c.element
                        .as_ref()
                        .and_then(|e| text(e.label.as_ref(), &lang))
                        .unwrap_or(""),
                    code(c.main_position.as_ref()),
                    code(c.sub_position.as_ref()),
                    c.growth_pattern.map_or("-".to_owned(), |g| g.to_string()),
                );
                if let Some(roma) = text(c.name_roma.as_ref(), &lang) {
                    out += &format!("  romanized {roma}\n");
                }
                if let Some(series) = &c.series {
                    out += &format!(
                        "  series {} {}\n",
                        series.series_id,
                        text(series.name.as_ref(), &lang).unwrap_or("(no name text)")
                    );
                }
                if let Some(team) = &c.team {
                    out += &format!(
                        "  team {} {}\n",
                        team.belong_team_id,
                        text(team.name.as_ref(), &lang).unwrap_or("")
                    );
                }
                for skill in &c.skills {
                    out += &format!(
                        "  skill {} level {} {:?} {}\n",
                        skill.skill_id,
                        skill.learn_level,
                        skill.family,
                        text(skill.name.as_ref(), &lang).unwrap_or("(no name text)")
                    );
                }
                for source in [&c.param_source, &c.base_source].into_iter().flatten() {
                    out += &provenance("  source", source);
                }
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
                    code(s.element.as_ref()),
                    code(s.category.as_ref()),
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

fn code(value: Option<&Coded>) -> String {
    value.map_or_else(|| "-".to_owned(), |v| v.value.to_string())
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
    items: &[T],
    line: impl Fn(&T) -> String,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(items)?);
    } else {
        for item in items {
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
