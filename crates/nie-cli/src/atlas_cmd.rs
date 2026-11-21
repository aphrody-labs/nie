//! `nie atlas`: the single index of the repository's RE surfaces.
//!
//! One database (`var/nie-atlas.sqlite`) gathers the knowledge base,
//! the `data/re/` tree, the forge (`forge/`, `data/forge/`), the exports (C, ASM, EXE, JSON), the
//! Markdown documentation, the crates and the executable tools. A Redis mirror (`db4` by
//! default) makes it queryable without opening SQLite, and `atlas gaps`/`atlas next` give the
//! measured route to 100%.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{CommandFactory, Subcommand};
use nie_index::atlas::{Atlas, ScanConfig, ToolRecord};

/// Known chain binaries, as `(relative path, role)`.
const BINARIES: &[(&str, &str)] = &[
    ("nie.exe", "reference"),
    ("dist/nie.exe", "produced"),
];

#[derive(Debug, Subcommand)]
pub enum AtlasCmd {
    /// Build (or update) the full index: files, crates, docs, knowledge base, binaries, tools,
    /// metrics and gaps.
    Build {
        /// Target atlas database.
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        /// Repository root.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Knowledge base to digest.
        #[arg(long, default_value = "var/nie.sqlite")]
        kb: PathBuf,
        /// Skip the knowledge base digest (file scan only).
        #[arg(long)]
        no_kb: bool,
        /// Cap on rows counted per knowledge base table.
        #[arg(long, default_value_t = 20_000_000)]
        row_cap: i64,
        /// Skip symbol to Rust source matching (the most expensive build step).
        #[arg(long)]
        no_link: bool,
        /// Redis mirror to write afterwards.
        #[arg(long)]
        redis: Option<String>,
        /// JSON output.
        #[arg(long)]
        json: bool,
    },
    /// One measured status line.
    Status {
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Print the menu screen inventory and its coverage.
    Menu {
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Search the whole index at once (docs, symbols, tools, files, crates).
    Search {
        /// Search pattern.
        query: String,
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        /// Restrict to one kind (`doc`, `symbol`, `tool`, `artifact`, `crate`).
        #[arg(long)]
        kind: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// The route to 100%, ranked by remaining work x weight.
    Gaps {
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// The next work item (top-ranked gap), as JSON for the autonomous loop.
    Next {
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
    },
    /// Record a measurement in the timeline (source required: the command that produced it).
    Metric {
        /// Metric name (`re.classified`, `proofs.ok`, `forge.units`...).
        name: String,
        /// Measured value.
        value: f64,
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        /// Total, when the metric is a ratio.
        #[arg(long)]
        total: Option<f64>,
        /// Command or table that produced the number.
        #[arg(long)]
        source: String,
        /// Note libre.
        #[arg(long)]
        note: Option<String>,
        /// Recompute gaps after recording.
        #[arg(long)]
        refresh: bool,
    },
    /// Record one step of the autonomous loop.
    Run {
        /// Step executed.
        step: String,
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        /// Target area (`forge.identity`, `re.named`...).
        #[arg(long)]
        area: Option<String>,
        /// Step succeeded.
        #[arg(long)]
        ok: bool,
        /// Duration in milliseconds.
        #[arg(long, default_value_t = 0)]
        ms: i64,
        /// Associated JSON metrics.
        #[arg(long)]
        metrics: Option<String>,
        /// Last log lines.
        #[arg(long)]
        log: Option<String>,
    },
    /// Indexed documents, ranked by machine anchoring (addresses, symbols, fingerprints).
    Docs {
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        /// Only show documents with no machine reference.
        #[arg(long)]
        orphans: bool,
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
    /// Files that are byte-identical (same sha256) at several paths.
    Dupes {
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        /// Restrict to one zone (`re-data`, `doc`, `forge`, `crate`...).
        #[arg(long)]
        zone: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Redis mirror of the index (status, gaps, tools, symbol -> address).
    Sync {
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        #[arg(long, env = "NIE_ATLAS_REDIS", default_value = nie_index::atlas::DEFAULT_ATLAS_REDIS)]
        redis: String,
        #[arg(long, default_value = "atlas")]
        prefix: String,
    },
    /// Link and mark the knowledge base's named symbols that appear in the workspace's Rust source.
    Link {
        #[arg(long, env = "NIE_ATLAS", default_value = nie_index::atlas::DEFAULT_ATLAS_PATH)]
        db: PathBuf,
        /// Repository root.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
}

pub fn run(cmd: AtlasCmd) -> Result<()> {
    match cmd {
        AtlasCmd::Build {
            db,
            root,
            kb,
            no_kb,
            row_cap,
            no_link,
            redis,
            json,
        } => build(
            &db,
            &root,
            &kb,
            no_kb,
            row_cap,
            no_link,
            redis.as_deref(),
            json,
        ),
        AtlasCmd::Status { db, json } => status(&db, json),
        AtlasCmd::Menu { db, json } => menu_screens(&db, json),
        AtlasCmd::Search {
            query,
            db,
            kind,
            limit,
            json,
        } => search(&db, &query, kind.as_deref(), limit, json),
        AtlasCmd::Gaps { db, limit, json } => gaps(&db, limit, json),
        AtlasCmd::Next { db } => next(&db),
        AtlasCmd::Metric {
            name,
            value,
            db,
            total,
            source,
            note,
            refresh,
        } => metric(&db, &name, value, total, &source, note.as_deref(), refresh),
        AtlasCmd::Run {
            step,
            db,
            area,
            ok,
            ms,
            metrics,
            log,
        } => run_step(
            &db,
            &step,
            area.as_deref(),
            ok,
            ms,
            metrics.as_deref(),
            log.as_deref(),
        ),
        AtlasCmd::Docs { db, orphans, limit } => docs(&db, orphans, limit),
        AtlasCmd::Dupes { db, zone, limit } => dupes(&db, zone.as_deref(), limit),
        AtlasCmd::Sync { db, redis, prefix } => sync(&db, &redis, &prefix),
        AtlasCmd::Link { db, root } => link(&db, &root),
    }
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn build(
    db: &Path,
    root: &Path,
    kb: &Path,
    no_kb: bool,
    row_cap: i64,
    no_link: bool,
    redis: Option<&str>,
    json: bool,
) -> Result<()> {
    let started = std::time::Instant::now();
    let mut atlas = Atlas::open(db).context("opening atlas")?;
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    let tracked = git_tracked(&root);
    let scan = atlas
        .scan(&ScanConfig::new(&root), &tracked)
        .context("scanning repository")?;
    let crates = atlas.import_crates(&root).context("importing crates")?;
    let docs = atlas.import_docs(&root).context("importing documents")?;
    let binaries = atlas
        .import_binaries(
            &root,
            &BINARIES
                .iter()
                .map(|(p, r)| ((*p).to_string(), (*r).to_string()))
                .collect::<Vec<_>>(),
        )
        .context("importing binaries")?;
    let forge = atlas.import_forge(&root).context("importing forge")?;
    let menu_screens = atlas
        .import_menu(&root)
        .context("importing menu catalog")?;
    let mut tools = atlas
        .import_repo_tools(&root)
        .context("importing repository tools")?;
    tools += atlas
        .import_tools(&cli_tools())
        .context("importing nie subcommands")?;

    let mut kb_digest = None;
    if !no_kb {
        let kb_path = if kb.is_absolute() {
            kb.to_path_buf()
        } else {
            root.join(kb)
        };
        if kb_path.is_file() {
            let digest = atlas
                .import_kb(&kb_path, row_cap)
                .with_context(|| format!("digest of {}", kb_path.display()))?;
            record_kb_metrics(&atlas, &digest)?;
            kb_digest = Some(digest);
        } else {
            eprintln!(
                "atlas: knowledge base missing ({}); index built without its digest",
                kb_path.display()
            );
        }
    }

    let linked = if no_link {
        0
    } else {
        atlas
            .link_ported_symbols(&root)
            .context("matching symbols to Rust source")?
    };

    atlas.record_metric(
        "menu.screens",
        menu_screens as f64,
        Some(38.0),
        "data/menu/screen-inventory.json",
        None,
        None,
    )?;
    record_forge_metrics(&atlas)?;
    let gaps = atlas.refresh_gaps().context("computing gaps")?;
    let mirrored = match redis {
        Some(url) => atlas.sync_redis(url, "atlas").unwrap_or(0),
        None => 0,
    };

    let elapsed = started.elapsed().as_millis() as i64;
    let status = atlas.status()?;
    atlas.record_run(
        "atlas.build",
        None,
        true,
        elapsed,
        Some(&serde_json::to_string(&status)?),
        None,
    )?;

    if json {
        let payload = serde_json::json!({
            "scan": scan,
            "crates": crates,
            "docs": docs,
            "binaries": binaries,
            "forge": forge,
            "menu_screens": menu_screens,
            "tools": tools,
            "kb": kb_digest,
            "symbols_linked": linked,
            "gaps": gaps,
            "redis_keys": mirrored,
            "status": status,
            "ms": elapsed,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "atlas-build files={} bytes={} crates={} docs={} doc_refs={} binaries={} tools={} menu_screens={} symbols={} units={} units_lifted={} units_exact={} kb_tables={} kb_rows={} linked={} gaps={} redis={} ms={}",
            scan.files,
            scan.bytes,
            crates,
            docs,
            status.doc_refs,
            binaries,
            tools,
            menu_screens,
            status.symbols,
            status.units,
            forge.lifted,
            status.units_exact,
            status.kb_tables,
            status.kb_rows,
            linked,
            gaps,
            mirrored,
            elapsed
        );
    }
    Ok(())
}

/// Metrics derived from the knowledge base digest.
///
/// Coverage is not recomputed here: the last `kb.coverage` row, written by `nie rebuild`, is
/// taken as is, since it is the loop's honest measurement.
fn record_kb_metrics(atlas: &Atlas, digest: &nie_index::atlas::KbDigest) -> Result<()> {
    atlas.record_metric(
        "kb.rows",
        digest.rows as f64,
        None,
        "atlas import_kb (COUNT capped by --row-cap)",
        None,
        Some(&format!(
            "{} tables, {} capped",
            digest.tables, digest.capped
        )),
    )?;
    if let Some(cov) = digest.coverage {
        atlas.record_metric(
            "re.classified",
            cov.classified as f64,
            Some(cov.total as f64),
            "kb.coverage (last row)",
            None,
            None,
        )?;
        atlas.record_metric(
            "re.named",
            cov.named as f64,
            Some(cov.total as f64),
            "kb.coverage (last row)",
            None,
            None,
        )?;
        return Ok(());
    }
    // Without a coverage row, the only available measurement is the digest itself.
    let functions: Option<i64> = atlas
        .conn()
        .query_row(
            "SELECT n_rows FROM atlas_kb_table WHERE name = 'function'",
            [],
            |r| r.get(0),
        )
        .ok();
    if let Some(total) = functions
        && total > 0
    {
        atlas.record_metric(
            "re.named",
            digest.symbols as f64,
            Some(total as f64),
            "atlas import_kb (kb.function)",
            None,
            None,
        )?;
    }
    Ok(())
}

/// Forge metrics: identity of the produced binary, lifted units and byte-exact units.
fn record_forge_metrics(atlas: &Atlas) -> Result<()> {
    let conn = atlas.conn();
    let (units, exact, lifted): (i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(byte_exact), 0),
                COALESCE(SUM(CASE WHEN lang = 'asm' THEN 1 ELSE 0 END), 0)
         FROM atlas_unit",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if units > 0 {
        atlas.record_metric(
            "forge.units",
            exact as f64,
            Some(units as f64),
            "atlas_unit (byte_exact)",
            None,
            None,
        )?;
        atlas.record_metric(
            "forge.lifted",
            lifted as f64,
            Some(units as f64),
            "atlas_unit (lang=asm)",
            None,
            None,
        )?;
    }
    if let Ok((identical, sha)) = conn.query_row(
        "SELECT COALESCE(identical, 0), COALESCE(sha256, '') FROM atlas_binary
         WHERE role = 'produced' ORDER BY seen_at DESC LIMIT 1",
        [],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    ) {
        atlas.record_metric(
            "forge.identity",
            identical as f64 * 100.0,
            Some(100.0),
            "sha256 dist/nie.exe vs nie.exe",
            Some(&sha),
            None,
        )?;
    }
    Ok(())
}

fn status(db: &Path, json: bool) -> Result<()> {
    let atlas = Atlas::open(db)?;
    let status = atlas.status()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!(
            "atlas artifacts={} bytes={} crates={} docs={} doc_refs={} symbols={} units={} units_exact={} tools={} menu_screens={} kb_tables={} kb_rows={} gaps_open={} runs={}",
            status.artifacts,
            status.artifact_bytes,
            status.crates,
            status.docs,
            status.doc_refs,
            status.symbols,
            status.units,
            status.units_exact,
            status.tools,
            status.menu_screens,
            status.kb_tables,
            status.kb_rows,
            status.gaps_open,
            status.runs
        );
    }
    Ok(())
}

fn menu_screens(db: &Path, json: bool) -> Result<()> {
    let atlas = Atlas::open(db)?;
    let screens = atlas.menu_screens()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&screens)?);
    } else {
        println!(
            "{:<24} {:<32} {:<10} {:>5} {:>5} {:<5} {}",
            "SCREEN_ID", "CAPTURE_FILE", "STATUS", "REFS", "MISS", "LUA", "SETTING_CFG"
        );
        for s in &screens {
            println!(
                "{:<24} {:<32} {:<10} {:>5} {:>5} {:<5} {}",
                s.screen_id,
                s.capture_file,
                s.pairing_status,
                s.referenced_objbins,
                s.missing_objbins,
                if s.has_lua { "yes" } else { "no" },
                s.setting_cfg.as_deref().unwrap_or("-")
            );
        }
        println!("menu-screens total={} (catalog 38)", screens.len());
    }
    Ok(())
}

fn search(db: &Path, query: &str, kind: Option<&str>, limit: usize, json: bool) -> Result<()> {
    let atlas = Atlas::open(db)?;
    let hits: Vec<_> = atlas
        .search(query, limit * 4)?
        .into_iter()
        .filter(|h| kind.is_none_or(|k| h.kind == k))
        .take(limit)
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&hits)?);
    } else {
        for hit in &hits {
            println!(
                "{:<8} {:<48} {}",
                hit.kind,
                hit.key,
                hit.reference.clone().unwrap_or_default()
            );
        }
        println!("hits={}", hits.len());
    }
    Ok(())
}

fn gaps(db: &Path, limit: usize, json: bool) -> Result<()> {
    let atlas = Atlas::open(db)?;
    let gaps = atlas.gaps(limit)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&gaps)?);
        return Ok(());
    }
    for gap in &gaps {
        println!(
            "{:<18} {:>6.2}{} -> {:>6.2} score={:>7.2} {} [{}]",
            gap.area,
            gap.current,
            gap.unit,
            gap.target,
            gap.score,
            gap.action.clone().unwrap_or_default(),
            gap.evidence.clone().unwrap_or_default()
        );
    }
    println!("gaps={}", gaps.len());
    Ok(())
}

fn next(db: &Path) -> Result<()> {
    let atlas = Atlas::open(db)?;
    match atlas.next_gap()? {
        Some(gap) => println!("{}", serde_json::to_string(&gap)?),
        None => println!("{{\"area\":null}}"),
    }
    Ok(())
}

fn metric(
    db: &Path,
    name: &str,
    value: f64,
    total: Option<f64>,
    source: &str,
    note: Option<&str>,
    refresh: bool,
) -> Result<()> {
    let atlas = Atlas::open(db)?;
    atlas.record_metric(name, value, total, source, None, note)?;
    let gaps = if refresh { atlas.refresh_gaps()? } else { 0 };
    println!(
        "metric name={name} value={value} total={} gaps={gaps}",
        total.unwrap_or(0.0)
    );
    Ok(())
}

fn run_step(
    db: &Path,
    step: &str,
    area: Option<&str>,
    ok: bool,
    ms: i64,
    metrics: Option<&str>,
    log: Option<&str>,
) -> Result<()> {
    let atlas = Atlas::open(db)?;
    let id = atlas.record_run(step, area, ok, ms, metrics, log)?;
    println!("run id={id} step={step} ok={ok} ms={ms}");
    Ok(())
}

fn docs(db: &Path, orphans: bool, limit: usize) -> Result<()> {
    let atlas = Atlas::open(db)?;
    let sql = if orphans {
        "SELECT path, COALESCE(title,''), re_score, words FROM atlas_doc
         WHERE re_score = 0 ORDER BY words DESC LIMIT ?1"
    } else {
        "SELECT path, COALESCE(title,''), re_score, words FROM atlas_doc
         ORDER BY re_score DESC, words DESC LIMIT ?1"
    };
    let conn = atlas.conn();
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([limit as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    let mut count = 0usize;
    for row in rows {
        let (path, title, score, words) = row?;
        println!("{score:>6} {words:>7} {path} — {title}");
        count += 1;
    }
    println!("docs={count}");
    Ok(())
}

/// Strict duplicates: same sha256, several paths. The repository has some by construction
/// (`refs/re/` and `data/re/50-source/upstream/` are two copies of the same corpus); measuring
/// them is the condition for deciding which one is authoritative.
fn dupes(db: &Path, zone: Option<&str>, limit: usize) -> Result<()> {
    let atlas = Atlas::open(db)?;
    let conn = atlas.conn();
    let mut stmt = conn.prepare(
        "SELECT sha256, COUNT(*) AS n, SUM(bytes) AS total, GROUP_CONCAT(path, ' | ')
         FROM atlas_artifact
         WHERE sha256 IS NOT NULL AND (?1 IS NULL OR zone = ?1)
         GROUP BY sha256 HAVING n > 1
         ORDER BY total DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(nie_index::rusqlite::params![zone, limit as i64], |r| {
        Ok((
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut groups = 0usize;
    let mut wasted = 0i64;
    for row in rows {
        let (n, total, paths) = row?;
        wasted += total - total / n;
        println!("{n:>3}x {:>10} B  {paths}", total / n);
        groups += 1;
    }
    println!("dupes groups={groups} redundant_bytes={wasted}");
    Ok(())
}

fn sync(db: &Path, redis: &str, prefix: &str) -> Result<()> {
    let atlas = Atlas::open(db)?;
    let keys = atlas
        .sync_redis(redis, prefix)
        .with_context(|| format!("miroir redis {redis}"))?;
    println!("atlas-sync redis={redis} prefix={prefix} keys={keys}");
    Ok(())
}

fn link(db: &Path, root: &Path) -> Result<()> {
    let mut atlas = Atlas::open(db)?;
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let linked = atlas
        .link_ported_symbols(&root)
        .context("matching symbols to Rust source")?;
    let gaps = atlas.refresh_gaps().context("computing gaps")?;
    println!("atlas-link linked={linked} gaps_updated={gaps}");
    Ok(())
}

/// Paths tracked by git under `root` (relative to `root`), read from the repository index:
/// distinguishes versioned files from local material (`data/re/`). Empty outside a repository.
fn git_tracked(root: &Path) -> HashSet<String> {
    let Some(top) = root.ancestors().find(|dir| dir.join(".git").is_dir()) else {
        return HashSet::new();
    };
    let Ok(index) = gix_index::File::at(
        top.join(".git").join("index"),
        gix_hash::Kind::Sha1,
        false,
        gix_index::decode::Options::default(),
    ) else {
        return HashSet::new();
    };
    let prefix = root
        .strip_prefix(top)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let state: &gix_index::State = &index;
    state
        .entries()
        .iter()
        .filter_map(|entry| {
            let path = entry.path(state).to_string();
            if prefix.is_empty() {
                Some(path)
            } else {
                path.strip_prefix(&format!("{prefix}/")).map(str::to_owned)
            }
        })
        .collect()
}

/// The `nie` subcommands, read from the clap definition itself, so the index cannot drift from
/// the real CLI.
fn cli_tools() -> Vec<ToolRecord> {
    let cmd = crate::Cli::command();
    cmd.get_subcommands()
        .map(|sub| ToolRecord {
            name: sub.get_name().to_string(),
            kind: "cli-cmd".to_string(),
            path: Some("crates/nie/nie-cli".to_string()),
            summary: sub.get_about().map(ToString::to_string),
        })
        .collect()
}
