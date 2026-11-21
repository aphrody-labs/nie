//! `nie find` and `nie grep`: file and content search **on disk**.
//!
//! Complements `nie vfs find`, which only sees inside the game's CPK archives. Here the target is
//! the working tree: sources, dumps, scratchpad.
//!
//! ## Why in the CLI rather than in a shell
//!
//! A simple question ("where is this symbol?", "which files have this extension?") otherwise
//! needs `find | xargs grep | sort | uniq` chains that are slow and full of pitfalls: `comm`
//! requires `LC_ALL=C sort`, `xargs wc -l` undercounts (one `total` line per invocation),
//! `git ls-files 'dir/**'` misses files at the root of `dir`. A dedicated command removes the
//! whole class of problems.
//!
//! ## Engine
//!
//! [`ignore`] and [`grep_searcher`]/[`grep_regex`], the libraries that power ripgrep and fd:
//! parallel walk, `.gitignore` support, line-by-line search without per-line allocation.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkState;
use nie_explore::depot::{construire_globs, parcours_disque};

/// Options for `nie find`.
pub struct FindArgs {
    /// Substring searched in the path (empty = list everything).
    pub pattern: String,
    /// Walk root.
    pub dir: PathBuf,
    /// Glob patterns (`**/*.rs`), cumulative with `ext`.
    pub globs: Vec<String>,
    /// Extensions (`rs`, `.toml`), shorthand for `**/*.<ext>`.
    pub exts: Vec<String>,
    /// `f` = files only, `d` = directories only, otherwise both.
    pub kind: Option<String>,
    /// Include hidden files.
    pub hidden: bool,
    /// Ignore `.gitignore` rules.
    pub no_ignore: bool,
    /// Maximum depth.
    pub depth: Option<usize>,
    /// Maximum number of results (0 = unlimited).
    pub limit: usize,
    /// Case-sensitive (default: insensitive).
    pub case_sensitive: bool,
    /// Print only the result count.
    pub count: bool,
}

/// Searches files by path. Returns the number of matches.
pub fn find(args: &FindArgs) -> Result<usize> {
    let set = construire_globs(&args.globs, &args.exts)?;
    let needle = if args.case_sensitive {
        args.pattern.clone()
    } else {
        args.pattern.to_lowercase()
    };
    let only_files = args.kind.as_deref() == Some("f");
    let only_dirs = args.kind.as_deref() == Some("d");

    let hits: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::new(AtomicUsize::new(0));

    parcours_disque(&args.dir, args.hidden, args.no_ignore, args.depth)
        .build_parallel()
        .run(|| {
            let hits = Arc::clone(&hits);
            let seen = Arc::clone(&seen);
            let needle = needle.clone();
            let set = set.clone();
            Box::new(move |entry| {
                let Ok(e) = entry else {
                    return WalkState::Continue;
                };
                let is_dir = e.file_type().is_some_and(|t| t.is_dir());
                if (only_files && is_dir) || (only_dirs && !is_dir) {
                    return WalkState::Continue;
                }
                let path = e.path();
                // A pattern filter applies to FILES. A directory never matches `**/*.rs`: rejecting
                // it in `is_match` would cut the descent, but letting it through would LIST it.
                // So it is skipped at emission time, without cutting the walk.
                if set.is_some() && is_dir {
                    return WalkState::Continue;
                }
                if let Some(s) = &set
                    && !s.is_match(path)
                {
                    return WalkState::Continue;
                }
                if !needle.is_empty() {
                    let hay = path.to_string_lossy();
                    let hay = if args_case(&needle) {
                        hay.to_string()
                    } else {
                        hay.to_lowercase()
                    };
                    if !hay.contains(&needle) {
                        return WalkState::Continue;
                    }
                }
                if args.limit > 0 && seen.fetch_add(1, Ordering::Relaxed) >= args.limit {
                    return WalkState::Quit;
                }
                if let Ok(mut v) = hits.lock() {
                    v.push(path.display().to_string());
                }
                WalkState::Continue
            })
        });

    let mut v = hits
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    v.sort_unstable();
    let n = v.len();
    if args.count {
        println!("{n}");
        return Ok(n);
    }
    for p in &v {
        println!("{p}");
    }
    Ok(n)
}

/// Case is already normalized upstream; this predicate keeps the closure readable.
fn args_case(needle: &str) -> bool {
    needle.chars().any(char::is_uppercase)
}

/// Options for `nie grep`.
pub struct GrepArgs {
    /// Regular expression searched in the content.
    pub pattern: String,
    /// Walk root.
    pub dir: PathBuf,
    /// Glob patterns restricting the visited files.
    pub globs: Vec<String>,
    /// Extensions restricting the visited files.
    pub exts: Vec<String>,
    /// Case-insensitive.
    pub ignore_case: bool,
    /// Include hidden files.
    pub hidden: bool,
    /// Ignore `.gitignore` rules.
    pub no_ignore: bool,
    /// Maximum number of lines printed (0 = unlimited).
    pub limit: usize,
    /// Print only the paths of files with at least one match.
    pub files_with_matches: bool,
}

/// Searches a pattern in file contents. Returns the number of matching lines (or of files in
/// `files_with_matches` mode).
pub fn grep(args: &GrepArgs) -> Result<usize> {
    let set = construire_globs(&args.globs, &args.exts)?;
    let matcher = RegexMatcher::new_line_matcher(&if args.ignore_case {
        format!("(?i){}", args.pattern)
    } else {
        args.pattern.clone()
    })
    .with_context(|| format!("invalid regular expression: {}", args.pattern))?;

    let out: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::new(AtomicUsize::new(0));

    parcours_disque(&args.dir, args.hidden, args.no_ignore, None)
        .build_parallel()
        .run(|| {
            let out = Arc::clone(&out);
            let seen = Arc::clone(&seen);
            let matcher = matcher.clone();
            let set = set.clone();
            // `BinaryDetection::quit` avoids dumping bytes from a CPK or an .exe.
            let mut searcher = SearcherBuilder::new()
                .binary_detection(BinaryDetection::quit(0))
                .line_number(true)
                .build();
            Box::new(move |entry| {
                let Ok(e) = entry else {
                    return WalkState::Continue;
                };
                if !e.file_type().is_some_and(|t| t.is_file()) {
                    return WalkState::Continue;
                }
                let path = e.path();
                if let Some(s) = &set
                    && !s.is_match(path)
                {
                    return WalkState::Continue;
                }
                let mut file_hits = 0usize;
                let display = path.display().to_string();
                let res = searcher.search_path(
                    &matcher,
                    path,
                    UTF8(|lineno, line| {
                        file_hits += 1;
                        if !args.files_with_matches {
                            if args.limit > 0 && seen.fetch_add(1, Ordering::Relaxed) >= args.limit
                            {
                                return Ok(false);
                            }
                            if let Ok(mut v) = out.lock() {
                                v.push(format!("{display}:{lineno}:{}", line.trim_end()));
                            }
                        }
                        Ok(true)
                    }),
                );
                // An unreadable file (permission, broken link) must not stop the search.
                if res.is_ok() && args.files_with_matches && file_hits > 0 {
                    if args.limit > 0 && seen.fetch_add(1, Ordering::Relaxed) >= args.limit {
                        return WalkState::Quit;
                    }
                    if let Ok(mut v) = out.lock() {
                        v.push(display);
                    }
                }
                WalkState::Continue
            })
        });

    let mut v = out
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    v.sort_unstable();
    let n = v.len();
    for line in &v {
        println!("{line}");
    }
    Ok(n)
}
