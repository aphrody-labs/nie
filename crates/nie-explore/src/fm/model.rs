//! The state machine of the explorer: three columns (parent, current, preview), a cursor per
//! directory, marks, a fuzzy search over every path, a glob filter, and the keymap.
//!
//! The model owns no terminal and no runtime. A renderer calls [`Fm::handle_key`] with the keys it
//! receives, [`Fm::poll`] on a timer while a search runs, [`Fm::view`] to draw, and reads
//! [`View::target`] to know which preview to request.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use nie_formats::vfs::Vfs;

use super::keymap::{Action, Feed, KeyMode, Keymap, SortKey, SortOrder, Step};
use super::keys::Key;
use super::search::{Filter, Progress, Searcher};
use super::tree::{NodeId, NodeKind, TreeMode, VfsTree};

/// Search results kept for display.
const MAX_HITS: u32 = 5000;
/// Rows of context kept around the cursor when the column scrolls.
const SCROLL_MARGIN: usize = 3;

/// The line being typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    /// The fuzzy search over every path.
    Search,
    /// The glob filter of the current directory.
    Filter,
}

/// One row of a column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The node.
    pub id: NodeId,
    /// The text to show: the name, or the full path for a search hit.
    pub name: String,
    /// Directory or file.
    pub kind: NodeKind,
    /// File size, 0 for directories.
    pub size: u64,
    /// Files below a directory.
    pub files: u32,
    /// Whether the row is marked.
    pub marked: bool,
}

/// The visible window of one column.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Column {
    /// What the column lists.
    pub title: String,
    /// The rows of the visible window.
    pub rows: Vec<Row>,
    /// Index of the cursor row inside `rows`.
    pub cursor: Option<usize>,
    /// Index of the first row of the window in the whole list.
    pub offset: usize,
    /// Rows in the whole list.
    pub total: usize,
}

/// What the preview column is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The node.
    pub id: NodeId,
    /// The logical VFS path.
    pub path: String,
    /// Directory or file.
    pub kind: NodeKind,
    /// Size of a file.
    pub size: u64,
    /// Files below a directory.
    pub files: u32,
    /// The archive of a file.
    pub cpk: Option<String>,
    /// Names of the children of a directory, for its preview.
    pub children: Vec<(String, NodeKind)>,
}

/// Everything a renderer draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// The parent directory.
    pub parent: Column,
    /// The current directory, or the search hits.
    pub current: Column,
    /// The entry under the cursor.
    pub target: Option<Target>,
    /// The directory path shown in the header.
    pub cwd: String,
    /// Path or CPK grouping.
    pub mode: TreeMode,
    /// The open input line.
    pub input: Option<(InputKind, String)>,
    /// The active glob filter.
    pub filter: Option<String>,
    /// The active search query when its results are shown.
    pub search: Option<String>,
    /// Search progress.
    pub progress: Option<Progress>,
    /// Marked entries.
    pub marked: usize,
    /// A one-line message.
    pub status: Option<String>,
    /// Whether the key help is open.
    pub help: bool,
    /// The sequence typed so far, such as `g`.
    pub pending: String,
    /// Total files in the VFS.
    pub files: u32,
}

/// The explorer state.
pub struct Fm {
    vfs: Option<Arc<Vfs>>,
    tree: Arc<VfsTree>,
    mode: TreeMode,
    cwd: NodeId,
    /// The child last under the cursor in each directory.
    cursors: HashMap<NodeId, NodeId>,
    entries: Vec<NodeId>,
    index: usize,
    offset: usize,
    marked: HashSet<NodeId>,
    filter: Option<Filter>,
    sort: SortKey,
    reverse: bool,
    input: Option<(InputKind, String)>,
    searcher: Option<Searcher>,
    search_query: Option<String>,
    hits: Vec<NodeId>,
    progress: Option<Progress>,
    pending: Vec<Key>,
    keymap: Keymap,
    height: usize,
    status: Option<String>,
    help: bool,
    quit: bool,
    waker: Arc<dyn Fn() + Send + Sync>,
    /// Preview state shared with the renderer.
    pub preview: PreviewState,
}

/// The parts of the preview the keys drive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreviewState {
    /// Rows scrolled in a text, table or hex view.
    pub scroll: i32,
    /// Which texture, record or language of a multi-part file.
    pub index: i32,
    /// Which frame of a gallery or an animation.
    pub frame: i32,
    /// Whether the hex view is forced.
    pub hex: bool,
    /// Whether the animation plays.
    pub playing: bool,
}

impl Fm {
    /// An explorer over a mounted game.
    #[must_use]
    pub fn new(vfs: Arc<Vfs>) -> Self {
        let tree = Arc::new(VfsTree::from_vfs(&vfs, TreeMode::Path));
        let mut fm = Self::over(tree);
        fm.vfs = Some(vfs);
        fm
    }

    /// An explorer over a ready tree; the path and CPK grouping switch is disabled.
    #[must_use]
    pub fn over(tree: Arc<VfsTree>) -> Self {
        let mode = tree.mode();
        let mut fm = Self {
            vfs: None,
            tree,
            mode,
            cwd: VfsTree::ROOT,
            cursors: HashMap::new(),
            entries: Vec::new(),
            index: 0,
            offset: 0,
            marked: HashSet::new(),
            filter: None,
            sort: SortKey::Name,
            reverse: false,
            input: None,
            searcher: None,
            search_query: None,
            hits: Vec::new(),
            progress: None,
            pending: Vec::new(),
            keymap: Keymap::builtin(),
            height: 24,
            status: None,
            help: false,
            quit: false,
            waker: Arc::new(|| {}),
            preview: PreviewState::default(),
        };
        fm.refresh();
        fm
    }

    /// Replaces the keymap (the embedded one merged with a user file, for instance).
    pub fn set_keymap(&mut self, keymap: Keymap) {
        self.keymap = keymap;
    }

    /// The keymap, for the help screen.
    #[must_use]
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Registers the function a background search calls when its results change.
    pub fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.waker = waker;
    }

    /// Tells the model how many rows the current column shows.
    pub fn set_height(&mut self, height: usize) {
        self.height = height.max(1);
        self.scroll_into_view();
    }

    /// The tree being browsed.
    #[must_use]
    pub fn tree(&self) -> &Arc<VfsTree> {
        &self.tree
    }

    /// The mounted game, when there is one.
    #[must_use]
    pub fn vfs(&self) -> Option<&Arc<Vfs>> {
        self.vfs.as_ref()
    }

    /// Whether `q` was pressed.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Whether a search is running, so the renderer keeps polling.
    #[must_use]
    pub fn is_searching(&self) -> bool {
        self.progress.is_some_and(|p| p.running)
    }

    /// The node under the cursor.
    #[must_use]
    pub fn selected(&self) -> Option<NodeId> {
        self.list().get(self.index).copied()
    }

    /// The directory being browsed.
    #[must_use]
    pub fn cwd(&self) -> NodeId {
        self.cwd
    }

    fn list(&self) -> &[NodeId] {
        if self.search_query.is_some() {
            &self.hits
        } else {
            &self.entries
        }
    }

    /// Recomputes the visible entries of the current directory.
    fn refresh(&mut self) {
        let tree = &self.tree;
        let mut entries: Vec<NodeId> = tree
            .node(self.cwd)
            .children
            .iter()
            .copied()
            .filter(|id| {
                let node = tree.node(*id);
                self.filter
                    .as_ref()
                    .is_none_or(|f| f.accepts(&node.name, node.kind))
            })
            .collect();
        let key = |id: &NodeId| {
            let node = tree.node(*id);
            (node.kind == NodeKind::File, node.name.clone(), node.size, ext_of(&node.name))
        };
        match self.sort {
            SortKey::Name => {}
            SortKey::Size => entries.sort_by(|a, b| {
                let (ka, kb) = (key(a), key(b));
                (ka.0, std::cmp::Reverse(ka.2), ka.1).cmp(&(kb.0, std::cmp::Reverse(kb.2), kb.1))
            }),
            SortKey::Ext => entries.sort_by(|a, b| {
                let (ka, kb) = (key(a), key(b));
                (ka.0, ka.3, ka.1).cmp(&(kb.0, kb.3, kb.1))
            }),
        }
        if self.reverse {
            entries.reverse();
        }
        self.entries = entries;
        let remembered = self.cursors.get(&self.cwd).copied();
        self.index = remembered
            .and_then(|id| self.entries.iter().position(|e| *e == id))
            .unwrap_or(0);
        self.scroll_into_view();
        self.remember();
    }

    fn remember(&mut self) {
        if self.search_query.is_none()
            && let Some(id) = self.entries.get(self.index)
        {
            self.cursors.insert(self.cwd, *id);
        }
        self.preview = PreviewState {
            playing: self.preview.playing,
            ..PreviewState::default()
        };
    }

    fn scroll_into_view(&mut self) {
        let height = self.height;
        let margin = SCROLL_MARGIN.min(height.saturating_sub(1) / 2);
        if self.index < self.offset + margin {
            self.offset = self.index.saturating_sub(margin);
        } else if self.index + margin + 1 > self.offset + height {
            self.offset = self.index + margin + 1 - height;
        }
        let total = self.list().len();
        self.offset = self.offset.min(total.saturating_sub(height));
    }

    fn move_to(&mut self, index: usize) {
        let total = self.list().len();
        let index = index.min(total.saturating_sub(1));
        if index != self.index {
            self.index = index;
            self.scroll_into_view();
            self.remember();
        }
    }

    fn arrow(&mut self, step: Step) {
        let total = self.list().len();
        if total == 0 {
            return;
        }
        let height = i64::try_from(self.height).unwrap_or(24);
        let target = match step {
            Step::Top => 0,
            Step::Bottom => total - 1,
            Step::Lines(n) => offset_index(self.index, i64::from(n), total),
            Step::Percent(p) => offset_index(self.index, height * i64::from(p) / 100, total),
        };
        self.move_to(target);
    }

    fn go_to(&mut self, dir: NodeId, select: Option<NodeId>) {
        self.cwd = dir;
        if let Some(id) = select {
            self.cursors.insert(dir, id);
        }
        self.refresh();
    }

    fn enter(&mut self) {
        let Some(id) = self.selected() else { return };
        if self.search_query.is_some() {
            self.reveal();
            return;
        }
        if self.tree.node(id).kind == NodeKind::Dir {
            self.go_to(id, None);
        } else {
            self.status = Some("read-only explorer: the preview is the open action".into());
        }
    }

    fn leave(&mut self) {
        if self.search_query.is_some() {
            self.clear_search();
            return;
        }
        if let Some(parent) = self.tree.node(self.cwd).parent {
            let from = self.cwd;
            self.go_to(parent, Some(from));
        }
    }

    fn reveal(&mut self) {
        let Some(id) = self.selected() else { return };
        let dir = self.tree.node(id).parent.unwrap_or(VfsTree::ROOT);
        self.clear_search();
        self.filter = None;
        self.go_to(dir, Some(id));
    }

    fn clear_search(&mut self) {
        self.search_query = None;
        self.hits.clear();
        self.progress = None;
        self.searcher = None;
        self.input = None;
        self.index = 0;
        self.refresh();
    }

    fn open_input(&mut self, kind: InputKind) {
        let text = match kind {
            InputKind::Search => self.search_query.clone().unwrap_or_default(),
            InputKind::Filter => self
                .filter
                .as_ref()
                .map(|f| f.source().to_owned())
                .unwrap_or_default(),
        };
        if kind == InputKind::Search && self.searcher.is_none() {
            self.searcher = Some(Searcher::new(Arc::clone(&self.tree), Arc::clone(&self.waker)));
        }
        self.input = Some((kind, text));
        self.apply_input();
    }

    /// Applies the text typed so far.
    fn apply_input(&mut self) {
        let Some((kind, text)) = self.input.clone() else {
            return;
        };
        match kind {
            InputKind::Search => {
                if text.is_empty() {
                    self.search_query = None;
                    self.hits.clear();
                    self.progress = None;
                    self.index = 0;
                    self.refresh();
                    return;
                }
                if let Some(searcher) = &mut self.searcher {
                    searcher.set_query(&text);
                }
                self.search_query = Some(text);
                self.index = 0;
                self.offset = 0;
                self.poll();
            }
            InputKind::Filter => {
                if text.trim().is_empty() {
                    self.filter = None;
                    self.refresh();
                    return;
                }
                match Filter::parse(&text) {
                    Ok(filter) => {
                        self.filter = Some(filter);
                        self.status = None;
                        self.refresh();
                    }
                    Err(error) => self.status = Some(format!("filter: {error}")),
                }
            }
        }
    }

    /// Lets a running search publish its results. Returns true when the list changed.
    pub fn poll(&mut self) -> bool {
        let Some(searcher) = &mut self.searcher else {
            return false;
        };
        let progress = searcher.tick(Duration::from_millis(5));
        let hits = searcher.hits(MAX_HITS);
        let changed = hits != self.hits || Some(progress) != self.progress;
        self.progress = Some(progress);
        if self.search_query.is_some() && hits != self.hits {
            self.hits = hits;
            self.index = self.index.min(self.hits.len().saturating_sub(1));
            self.scroll_into_view();
            self.remember();
        }
        changed
    }

    fn set_mode(&mut self) {
        let Some(vfs) = self.vfs.clone() else {
            self.status = Some("the grouping switch needs a mounted game".into());
            return;
        };
        let keep = self.selected().map(|id| self.tree.vfs_path(id));
        let keep_cpk = self.selected().and_then(|id| self.tree.cpk_of(id).map(str::to_owned));
        self.mode = match self.mode {
            TreeMode::Path => TreeMode::Cpk,
            TreeMode::Cpk => TreeMode::Path,
        };
        self.tree = Arc::new(VfsTree::from_vfs(&vfs, self.mode));
        self.cursors.clear();
        self.marked.clear();
        self.searcher = None;
        self.search_query = None;
        self.hits.clear();
        self.progress = None;
        let resolved = keep.and_then(|path| match self.mode {
            TreeMode::Path => self.tree.resolve(&path),
            TreeMode::Cpk => {
                let cpk = keep_cpk.unwrap_or_else(|| super::tree::LOOSE_CPK.to_owned());
                self.tree.resolve(&format!("{cpk}/{path}"))
            }
        });
        match resolved {
            Some(id) => {
                let dir = self.tree.node(id).parent.unwrap_or(VfsTree::ROOT);
                self.go_to(dir, Some(id));
            }
            None => self.go_to(VfsTree::ROOT, None),
        }
        self.status = Some(match self.mode {
            TreeMode::Path => "grouped by path".into(),
            TreeMode::Cpk => format!("grouped by CPK archive ({})", self.tree.cpks().len()),
        });
    }

    /// Runs one action.
    pub fn run(&mut self, action: &Action) {
        self.status = None;
        match action {
            Action::Arrow(step) => self.arrow(*step),
            Action::Leave => self.leave(),
            Action::Enter => self.enter(),
            Action::Cd => {
                self.clear_search();
                self.go_to(VfsTree::ROOT, None);
            }
            Action::Quit => self.quit = true,
            Action::Escape => {
                if self.search_query.is_some() {
                    self.clear_search();
                } else if self.filter.is_some() {
                    self.filter = None;
                    self.refresh();
                } else if self.help {
                    self.help = false;
                } else if !self.marked.is_empty() {
                    self.marked.clear();
                }
            }
            Action::Toggle => {
                if let Some(id) = self.selected()
                    && !self.marked.remove(&id)
                {
                    self.marked.insert(id);
                }
            }
            Action::ToggleAll => {
                let all: Vec<NodeId> = self.list().to_vec();
                if all.iter().all(|id| self.marked.contains(id)) {
                    for id in all {
                        self.marked.remove(&id);
                    }
                } else {
                    self.marked.extend(all);
                }
            }
            Action::Search => self.open_input(InputKind::Search),
            Action::Filter => self.open_input(InputKind::Filter),
            Action::Reveal => self.reveal(),
            Action::Mode => self.set_mode(),
            Action::Sort(order) => {
                match order {
                    SortOrder::By(key) => self.sort = *key,
                    SortOrder::Reverse => self.reverse = !self.reverse,
                }
                let keep = self.selected();
                self.refresh();
                if let Some(id) = keep
                    && let Some(at) = self.entries.iter().position(|e| *e == id)
                {
                    self.move_to(at);
                }
            }
            Action::Preview(rows) => {
                self.preview.scroll = (self.preview.scroll + rows).max(0);
            }
            Action::Frame(step) => self.preview.frame += step,
            Action::Item(step) => {
                self.preview.index += step;
                self.preview.frame = 0;
                self.preview.scroll = 0;
            }
            Action::Play => self.preview.playing = !self.preview.playing,
            Action::Hex => {
                self.preview.hex = !self.preview.hex;
                self.preview.scroll = 0;
                self.preview.frame = 0;
            }
            Action::Help => self.help = !self.help,
            Action::Input(c) => {
                if let Some((_, text)) = &mut self.input {
                    text.push(*c);
                    self.apply_input();
                }
            }
            Action::Backspace => {
                if let Some((_, text)) = &mut self.input {
                    text.pop();
                    self.apply_input();
                }
            }
            Action::Clear => {
                if let Some((_, text)) = &mut self.input {
                    text.clear();
                    self.apply_input();
                }
            }
            Action::Commit => {
                self.input = None;
            }
            Action::Cancel => {
                match self.input.take() {
                    Some((InputKind::Search, _)) => self.clear_search(),
                    Some((InputKind::Filter, _)) => {
                        self.filter = None;
                        self.refresh();
                    }
                    None => {}
                }
            }
        }
    }

    /// Feeds one key through the keymap.
    pub fn handle_key(&mut self, key: Key) {
        let mode = if self.input.is_some() {
            KeyMode::Input
        } else {
            KeyMode::Manager
        };
        match self.keymap.feed(mode, &mut self.pending, key) {
            Feed::Run(actions) => {
                for action in &actions {
                    self.run(action);
                }
            }
            Feed::Wait => {}
            Feed::Miss => {
                if mode == KeyMode::Input
                    && let Some(c) = key.typed()
                {
                    self.run(&Action::Input(c));
                }
            }
        }
    }

    /// Jumps to a path of the current tree and puts the cursor on it.
    pub fn reveal_path(&mut self, path: &str) -> bool {
        let Some(id) = self.tree.resolve(path) else {
            return false;
        };
        self.clear_search();
        self.filter = None;
        let dir = self.tree.node(id).parent.unwrap_or(VfsTree::ROOT);
        self.go_to(dir, Some(id));
        true
    }

    fn row(&self, id: NodeId, full_path: bool) -> Row {
        let node = self.tree.node(id);
        Row {
            id,
            name: if full_path {
                self.tree.path(id)
            } else {
                node.name.to_string()
            },
            kind: node.kind,
            size: node.size,
            files: node.files,
            marked: self.marked.contains(&id),
        }
    }

    fn window(&self, ids: &[NodeId], cursor: usize, offset: usize, full_path: bool, title: String) -> Column {
        let end = (offset + self.height).min(ids.len());
        let start = offset.min(end);
        Column {
            title,
            rows: ids[start..end].iter().map(|id| self.row(*id, full_path)).collect(),
            cursor: (cursor >= start && cursor < end).then(|| cursor - start),
            offset: start,
            total: ids.len(),
        }
    }

    /// What to draw now.
    #[must_use]
    pub fn view(&self) -> View {
        let searching = self.search_query.is_some();
        let cwd_path = self.tree.path(self.cwd);
        let current = self.window(
            self.list(),
            self.index,
            self.offset,
            searching,
            if searching {
                "search".to_owned()
            } else {
                cwd_path.rsplit('/').next().unwrap_or("").to_owned()
            },
        );
        let parent = match self.tree.node(self.cwd).parent {
            Some(parent) if !searching => {
                let siblings = &self.tree.node(parent).children;
                let at = siblings.iter().position(|id| *id == self.cwd).unwrap_or(0);
                let offset = at.saturating_sub(self.height / 2).min(siblings.len().saturating_sub(self.height));
                self.window(siblings, at, offset, false, self.tree.path(parent))
            }
            _ => Column::default(),
        };
        let target = self.selected().map(|id| {
            let node = self.tree.node(id);
            Target {
                id,
                path: self.tree.vfs_path(id),
                kind: node.kind,
                size: node.size,
                files: node.files,
                cpk: self.tree.cpk_of(id).map(str::to_owned),
                children: if node.kind == NodeKind::Dir {
                    node.children
                        .iter()
                        .take(200)
                        .map(|c| (self.tree.node(*c).name.to_string(), self.tree.node(*c).kind))
                        .collect()
                } else {
                    Vec::new()
                },
            }
        });
        View {
            parent,
            current,
            target,
            cwd: if cwd_path.is_empty() { "/".to_owned() } else { cwd_path },
            mode: self.mode,
            input: self.input.clone(),
            filter: self.filter.as_ref().map(|f| f.source().to_owned()),
            search: self.search_query.clone(),
            progress: self.progress,
            marked: self.marked.len(),
            status: self.status.clone(),
            help: self.help,
            pending: self.pending.iter().map(ToString::to_string).collect(),
            files: self.tree.file_count(),
        }
    }
}

fn ext_of(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

fn offset_index(index: usize, delta: i64, total: usize) -> usize {
    let target = i64::try_from(index).unwrap_or(0) + delta;
    usize::try_from(target.clamp(0, i64::try_from(total).unwrap_or(i64::MAX) - 1)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::super::keys::KeyCode;
    use super::super::tree::Row as TreeRow;
    use super::*;

    fn fixture() -> Fm {
        let r = |p: &str, size| TreeRow {
            path: p.into(),
            cpk: "common.cpk".into(),
            size,
        };
        let tree = VfsTree::build(
            vec![
                r("data/common/menu/main.cfg.bin", 300),
                r("data/common/menu/option.cfg.bin", 100),
                r("data/common/menu/icon.g4tx", 900),
                r("data/common/text/en/words.cfg.bin", 50),
                r("data/dx11/chr/c01.g4tx", 700),
            ],
            TreeMode::Path,
        );
        let mut fm = Fm::over(Arc::new(tree));
        fm.set_height(10);
        fm
    }

    fn keys(fm: &mut Fm, text: &str) {
        for key in Key::parse_sequence(text).unwrap() {
            fm.handle_key(key);
        }
    }

    fn names(column: &Column) -> Vec<&str> {
        column.rows.iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn navigates_down_and_up_the_tree() {
        let mut fm = fixture();
        assert_eq!(names(&fm.view().current), ["data"]);
        keys(&mut fm, "l");
        assert_eq!(names(&fm.view().current), ["common", "dx11"]);
        keys(&mut fm, "jl");
        assert_eq!(fm.view().cwd, "data/dx11");
        assert_eq!(names(&fm.view().parent), ["common", "dx11"]);
        assert_eq!(fm.view().parent.cursor, Some(1));
        keys(&mut fm, "h");
        assert_eq!(fm.view().cwd, "data");
        // The cursor returns to the directory just left.
        assert_eq!(fm.view().current.cursor, Some(1));
    }

    #[test]
    fn the_cursor_is_remembered_per_directory() {
        let mut fm = fixture();
        keys(&mut fm, "ll");
        assert_eq!(names(&fm.view().current), ["menu", "text"]);
        keys(&mut fm, "jhh");
        keys(&mut fm, "ll");
        assert_eq!(fm.view().current.cursor, Some(1));
    }

    #[test]
    fn the_preview_target_follows_the_cursor() {
        let mut fm = fixture();
        keys(&mut fm, "lll");
        let target = fm.view().target.unwrap();
        assert_eq!(target.path, "data/common/menu/icon.g4tx");
        assert_eq!(target.size, 900);
        assert_eq!(target.cpk.as_deref(), Some("common.cpk"));
        keys(&mut fm, "j");
        assert_eq!(fm.view().target.unwrap().path, "data/common/menu/main.cfg.bin");
    }

    #[test]
    fn sorting_by_size_puts_the_largest_file_first() {
        let mut fm = fixture();
        keys(&mut fm, "lll");
        keys(&mut fm, ",s");
        let view = fm.view();
        assert_eq!(names(&view.current), ["icon.g4tx", "main.cfg.bin", "option.cfg.bin"]);
        assert_eq!(view.target.unwrap().path, "data/common/menu/icon.g4tx");
        keys(&mut fm, ",r");
        assert_eq!(names(&fm.view().current)[0], "option.cfg.bin");
    }

    #[test]
    fn the_glob_filter_hides_non_matching_files() {
        let mut fm = fixture();
        keys(&mut fm, "lll");
        keys(&mut fm, "f");
        for c in "*.g4tx".chars() {
            fm.handle_key(Key::char(c));
        }
        fm.handle_key(Key::plain(KeyCode::Enter));
        assert_eq!(names(&fm.view().current), ["icon.g4tx"]);
        assert_eq!(fm.view().filter.as_deref(), Some("*.g4tx"));
        keys(&mut fm, "<Esc>");
        assert_eq!(fm.view().current.total, 3);
    }

    #[test]
    fn marks_toggle_and_count() {
        let mut fm = fixture();
        keys(&mut fm, "lll");
        keys(&mut fm, "<Space>");
        assert_eq!(fm.view().marked, 1);
        assert!(fm.view().current.rows[0].marked);
        keys(&mut fm, "<C-a>");
        assert_eq!(fm.view().marked, 3);
        keys(&mut fm, "<C-a>");
        assert_eq!(fm.view().marked, 0);
    }

    #[test]
    fn search_finds_across_directories_and_reveals_the_hit() {
        let mut fm = fixture();
        keys(&mut fm, "/");
        for c in "c01 g4tx".chars() {
            fm.handle_key(Key::char(c));
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while fm.view().current.total == 0 && std::time::Instant::now() < deadline {
            fm.poll();
        }
        let view = fm.view();
        assert_eq!(view.current.rows[0].name, "data/dx11/chr/c01.g4tx");
        keys(&mut fm, "<Enter>");
        keys(&mut fm, "<Enter>");
        let view = fm.view();
        assert_eq!(view.cwd, "data/dx11/chr");
        assert_eq!(view.target.unwrap().path, "data/dx11/chr/c01.g4tx");
        assert!(view.search.is_none());
    }

    #[test]
    fn percent_steps_use_the_visible_height() {
        let mut fm = fixture();
        fm.set_height(2);
        keys(&mut fm, "lll");
        keys(&mut fm, "<C-f>");
        assert_eq!(fm.view().target.unwrap().path, "data/common/menu/option.cfg.bin");
        keys(&mut fm, "gg");
        assert_eq!(fm.view().target.unwrap().path, "data/common/menu/icon.g4tx");
    }
}
