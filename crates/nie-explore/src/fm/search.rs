//! Fuzzy search over every path (nucleo) and glob filters of one directory (globset).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo, Utf32String};

use super::tree::{NodeId, NodeKind, VfsTree};

/// A glob filter such as `*.g4tx, !*_l.*`: patterns are separated by commas or spaces, a leading
/// `!` excludes. Directories stay visible so the filter never blocks navigation, unless excluded.
#[derive(Debug, Clone)]
pub struct Filter {
    source: String,
    include: Option<GlobSet>,
    exclude: GlobSet,
}

impl Filter {
    /// Compiles a filter.
    ///
    /// # Errors
    /// Fails on an invalid glob; the message names it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut include = GlobSetBuilder::new();
        let mut exclude = GlobSetBuilder::new();
        let mut any_include = false;
        for token in text.split([',', ' ']).filter(|t| !t.is_empty()) {
            let (negated, pattern) = token
                .strip_prefix('!')
                .map_or((false, token), |rest| (true, rest));
            // A bare word filters by substring, like the search line.
            let pattern = if pattern.contains(['*', '?', '[', '{']) {
                pattern.to_owned()
            } else {
                format!("*{pattern}*")
            };
            let glob = GlobBuilder::new(&pattern)
                .case_insensitive(true)
                .literal_separator(false)
                .build()
                .map_err(|e| format!("{token}: {}", e.kind()))?;
            if negated {
                exclude.add(glob);
            } else {
                include.add(glob);
                any_include = true;
            }
        }
        let build = |b: GlobSetBuilder| b.build().map_err(|e| e.to_string());
        Ok(Self {
            source: text.trim().to_owned(),
            include: if any_include { Some(build(include)?) } else { None },
            exclude: build(exclude)?,
        })
    }

    /// The text the filter was built from.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether an entry stays visible.
    #[must_use]
    pub fn accepts(&self, name: &str, kind: NodeKind) -> bool {
        if self.exclude.is_match(name) {
            return false;
        }
        match (&self.include, kind) {
            (Some(include), NodeKind::File) => include.is_match(name),
            _ => true,
        }
    }
}

/// The running state of a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Matches for the current query.
    pub matched: u32,
    /// Entries injected so far.
    pub total: u32,
    /// Whether the matcher threads are still working.
    pub running: bool,
}

/// A nucleo matcher over the paths of one tree. Entries are injected from a background thread so
/// opening the search never waits for the 255,628 paths; results refine live as they arrive.
pub struct Searcher {
    nucleo: Nucleo<NodeId>,
    query: String,
    injecting: Arc<AtomicBool>,
}

impl Searcher {
    /// Starts a searcher over every node of `tree`; `notify` is called when results change.
    #[must_use]
    pub fn new(tree: Arc<VfsTree>, notify: Arc<dyn Fn() + Send + Sync>) -> Self {
        let nucleo = Nucleo::new(Config::DEFAULT.match_paths(), notify, None, 1);
        let injector = nucleo.injector();
        let injecting = Arc::new(AtomicBool::new(true));
        let done = Arc::clone(&injecting);
        let spawned = std::thread::Builder::new()
            .name("nie-fm-inject".into())
            .spawn(move || {
                for id in tree.ids() {
                    let path = tree.path(id);
                    injector.push(id, |_, cols| cols[0] = Utf32String::from(path.as_str()));
                }
                done.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            injecting.store(false, Ordering::Release);
        }
        Self {
            nucleo,
            query: String::new(),
            injecting,
        }
    }

    /// Sets the query (an empty query matches everything).
    pub fn set_query(&mut self, query: &str) {
        let append = query.starts_with(&self.query) && !self.query.is_empty();
        self.nucleo
            .pattern
            .reparse(0, query, CaseMatching::Smart, Normalization::Smart, append);
        query.clone_into(&mut self.query);
    }

    /// Lets the matcher publish a snapshot, waiting at most `timeout`.
    pub fn tick(&mut self, timeout: Duration) -> Progress {
        let status = self
            .nucleo
            .tick(u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX));
        let snapshot = self.nucleo.snapshot();
        Progress {
            matched: snapshot.matched_item_count(),
            total: snapshot.item_count(),
            running: status.running || self.injecting.load(Ordering::Acquire),
        }
    }

    /// The best `limit` hits of the last snapshot, best first.
    #[must_use]
    pub fn hits(&self, limit: u32) -> Vec<NodeId> {
        let snapshot = self.nucleo.snapshot();
        let end = snapshot.matched_item_count().min(limit);
        snapshot.matched_items(..end).map(|item| *item.data).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tree::{Row, TreeMode};
    use super::*;

    fn tree() -> Arc<VfsTree> {
        let r = |p: &str| Row {
            path: p.into(),
            cpk: "c.cpk".into(),
            size: 1,
        };
        Arc::new(VfsTree::build(
            vec![
                r("data/common/menu/main_menu.cfg.bin"),
                r("data/common/menu/option.cfg.bin"),
                r("data/dx11/chr/c01000010_l.g4tx"),
                r("data/dx11/chr/c01000020_l.g4tx"),
                r("data/common/text/en/mission.cfg.bin"),
            ],
            TreeMode::Path,
        ))
    }

    fn settle(searcher: &mut Searcher) -> Progress {
        loop {
            let progress = searcher.tick(Duration::from_millis(50));
            if !progress.running && progress.total > 0 {
                return progress;
            }
        }
    }

    #[test]
    fn fuzzy_search_ranks_the_closest_path_first() {
        let tree = tree();
        let mut searcher = Searcher::new(Arc::clone(&tree), Arc::new(|| {}));
        searcher.set_query("mainmenu");
        settle(&mut searcher);
        let hits = searcher.hits(5);
        assert_eq!(tree.path(hits[0]), "data/common/menu/main_menu.cfg.bin");
    }

    #[test]
    fn a_query_spanning_directories_matches_the_full_path() {
        let tree = tree();
        let mut searcher = Searcher::new(Arc::clone(&tree), Arc::new(|| {}));
        searcher.set_query("dx11 c0100002");
        settle(&mut searcher);
        let hits = searcher.hits(5);
        assert_eq!(hits.len(), 1);
        assert_eq!(tree.path(hits[0]), "data/dx11/chr/c01000020_l.g4tx");
    }

    #[test]
    fn filters_apply_to_files_and_keep_directories() {
        let filter = Filter::parse("*.g4tx, !*_l*").unwrap();
        assert!(!filter.accepts("c01_l.g4tx", NodeKind::File));
        assert!(filter.accepts("c01.g4tx", NodeKind::File));
        assert!(!filter.accepts("a.cfg.bin", NodeKind::File));
        assert!(filter.accepts("chr", NodeKind::Dir));
        let word = Filter::parse("menu").unwrap();
        assert!(word.accepts("MAIN_MENU.bin", NodeKind::File));
        assert!(Filter::parse("[").is_err());
    }
}
