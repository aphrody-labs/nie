//! The directory tree of the VFS, built once from the entry index.
//!
//! The VFS is a flat list of 255,628 `(path, cpk, size)` rows. The explorer needs `ls` and
//! "parent of" in constant time, so the rows are folded into an arena of nodes. Two groupings
//! exist: by path (`data/common/menu/...`) and by CPK (`<cpk>/data/common/menu/...`), the second
//! one answering "what does this archive hold".

use std::collections::HashMap;

/// Index of a node in [`VfsTree`].
pub type NodeId = u32;

/// What a node is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// A directory (or, in CPK mode, an archive).
    Dir,
    /// A file of the VFS.
    File,
}

/// How the tree groups the entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TreeMode {
    /// The logical game paths.
    #[default]
    Path,
    /// One top-level directory per CPK archive.
    Cpk,
}

/// One input row of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Logical VFS path (`data/common/menu/x.cfg.bin`).
    pub path: String,
    /// Archive that holds the file; empty for a loose file.
    pub cpk: String,
    /// Extracted size in bytes.
    pub size: u64,
}

/// One node of the arena.
#[derive(Debug, Clone)]
pub struct Node {
    /// The path segment.
    pub name: Box<str>,
    /// The parent directory; `None` for the root.
    pub parent: Option<NodeId>,
    /// Directory or file.
    pub kind: NodeKind,
    /// File size in bytes (0 for directories).
    pub size: u64,
    /// Index into [`VfsTree::cpks`] (files only).
    pub cpk: u32,
    /// Number of files below this node (1 for a file).
    pub files: u32,
    /// Children: directories first, then files, each by name.
    pub children: Vec<NodeId>,
}

/// Name shown for loose files in CPK mode.
pub const LOOSE_CPK: &str = "(loose)";

/// The folded VFS.
#[derive(Debug, Clone)]
pub struct VfsTree {
    nodes: Vec<Node>,
    cpks: Vec<Box<str>>,
    mode: TreeMode,
}

impl VfsTree {
    /// The root directory.
    pub const ROOT: NodeId = 0;

    /// Folds `rows` into a tree.
    #[must_use]
    pub fn build(mut rows: Vec<Row>, mode: TreeMode) -> Self {
        let mut cpk_index: HashMap<String, u32> = HashMap::new();
        let mut cpks: Vec<Box<str>> = Vec::new();
        let mut keyed: Vec<(String, u32, u64)> = Vec::with_capacity(rows.len());
        for row in rows.drain(..) {
            let cpk_name = if row.cpk.is_empty() {
                LOOSE_CPK
            } else {
                row.cpk.as_str()
            };
            let id = *cpk_index.entry(cpk_name.to_owned()).or_insert_with(|| {
                cpks.push(cpk_name.into());
                u32::try_from(cpks.len() - 1).unwrap_or(u32::MAX)
            });
            let key = match mode {
                TreeMode::Path => row.path,
                TreeMode::Cpk => format!("{cpk_name}/{}", row.path),
            };
            keyed.push((key, id, row.size));
        }
        keyed.sort_unstable_by(|a, b| a.0.cmp(&b.0));

        let mut nodes = vec![Node {
            name: "".into(),
            parent: None,
            kind: NodeKind::Dir,
            size: 0,
            cpk: u32::MAX,
            files: 0,
            children: Vec::new(),
        }];
        // The open directories from the root to the deepest one, as (segment, node).
        let mut stack: Vec<(String, NodeId)> = Vec::new();
        for (key, cpk, size) in keyed {
            let mut segments = key.split('/').filter(|s| !s.is_empty()).peekable();
            let mut depth = 0;
            while let Some(segment) = segments.next() {
                let last = segments.peek().is_none();
                if last {
                    stack.truncate(depth);
                    let parent = stack.last().map_or(Self::ROOT, |(_, id)| *id);
                    let id = push(&mut nodes, parent, segment, NodeKind::File, size, cpk);
                    nodes[id as usize].files = 1;
                    break;
                }
                if stack.get(depth).is_some_and(|(name, _)| name == segment) {
                    depth += 1;
                    continue;
                }
                stack.truncate(depth);
                let parent = stack.last().map_or(Self::ROOT, |(_, id)| *id);
                let id = push(&mut nodes, parent, segment, NodeKind::Dir, 0, u32::MAX);
                stack.push((segment.to_owned(), id));
                depth += 1;
            }
            // Stay on the directory path of this entry: a following sibling keeps the stack.
            stack.truncate(depth);
        }

        for index in (1..nodes.len()).rev() {
            let (parent, files) = (nodes[index].parent, nodes[index].files);
            if let Some(parent) = parent {
                nodes[parent as usize].files += files;
            }
        }
        let kinds: Vec<(NodeKind, Box<str>)> =
            nodes.iter().map(|n| (n.kind, n.name.clone())).collect();
        for node in &mut nodes {
            node.children.sort_by(|a, b| {
                let (ka, na) = &kinds[*a as usize];
                let (kb, nb) = &kinds[*b as usize];
                (*ka == NodeKind::File, na).cmp(&(*kb == NodeKind::File, nb))
            });
        }
        Self { nodes, cpks, mode }
    }

    /// Folds the entries of a mounted VFS.
    #[must_use]
    pub fn from_vfs(vfs: &nie_formats::vfs::Vfs, mode: TreeMode) -> Self {
        let rows = vfs
            .iter_all()
            .map(|e| Row {
                path: e.path.to_owned(),
                cpk: e.cpk.to_owned(),
                size: e.size,
            })
            .collect();
        Self::build(rows, mode)
    }

    /// The grouping of this tree.
    #[must_use]
    pub fn mode(&self) -> TreeMode {
        self.mode
    }

    /// Number of nodes, root included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree has no entry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// Total number of files.
    #[must_use]
    pub fn file_count(&self) -> u32 {
        self.nodes[0].files
    }

    /// The archives, in first-seen order.
    #[must_use]
    pub fn cpks(&self) -> &[Box<str>] {
        &self.cpks
    }

    /// One node.
    #[must_use]
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    /// Every node id, root first.
    pub fn ids(&self) -> impl Iterator<Item = NodeId> {
        1..u32::try_from(self.nodes.len()).unwrap_or(u32::MAX)
    }

    /// The archive of a file.
    #[must_use]
    pub fn cpk_of(&self, id: NodeId) -> Option<&str> {
        let node = self.node(id);
        (node.kind == NodeKind::File)
            .then(|| self.cpks.get(node.cpk as usize))
            .flatten()
            .map(AsRef::as_ref)
    }

    /// The path of a node inside this tree (`data/common/x`, or `<cpk>/data/common/x`).
    #[must_use]
    pub fn path(&self, id: NodeId) -> String {
        let mut parts: Vec<&str> = Vec::new();
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            let node = self.node(current);
            if node.parent.is_some() {
                parts.push(&node.name);
            }
            cursor = node.parent;
        }
        parts.reverse();
        parts.join("/")
    }

    /// The logical VFS path of a node (the CPK prefix of CPK mode removed).
    #[must_use]
    pub fn vfs_path(&self, id: NodeId) -> String {
        let path = self.path(id);
        match self.mode {
            TreeMode::Path => path,
            TreeMode::Cpk => path.split_once('/').map(|(_, rest)| rest.to_owned()).unwrap_or_default(),
        }
    }

    /// Resolves a path of this tree to a node.
    #[must_use]
    pub fn resolve(&self, path: &str) -> Option<NodeId> {
        let mut current = Self::ROOT;
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            current = *self
                .node(current)
                .children
                .iter()
                .find(|c| &*self.node(**c).name == segment)?;
        }
        Some(current)
    }

    /// The directory a node lives in (itself when it is a directory).
    #[must_use]
    pub fn dir_of(&self, id: NodeId) -> NodeId {
        let node = self.node(id);
        match node.kind {
            NodeKind::Dir => id,
            NodeKind::File => node.parent.unwrap_or(Self::ROOT),
        }
    }
}

fn push(
    nodes: &mut Vec<Node>,
    parent: NodeId,
    name: &str,
    kind: NodeKind,
    size: u64,
    cpk: u32,
) -> NodeId {
    let id = u32::try_from(nodes.len()).unwrap_or(u32::MAX);
    nodes.push(Node {
        name: name.into(),
        parent: Some(parent),
        kind,
        size,
        cpk,
        files: 0,
        children: Vec::new(),
    });
    nodes[parent as usize].children.push(id);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Row> {
        let r = |p: &str, c: &str, s| Row {
            path: p.into(),
            cpk: c.into(),
            size: s,
        };
        vec![
            r("data/common/menu/a.cfg.bin", "common.cpk", 10),
            r("data/common/menu/b.g4tx", "common.cpk", 20),
            r("data/common/menu.x/c.bin", "common.cpk", 5),
            r("data/dx11/model/m.g4md", "dx11.cpk", 30),
            r("data/common/text/en/t.cfg.bin", "common.cpk", 7),
            r("data/loose.txt", "", 1),
        ]
    }

    #[test]
    fn folds_paths_into_directories() {
        let tree = VfsTree::build(rows(), TreeMode::Path);
        assert_eq!(tree.file_count(), 6);
        let menu = tree.resolve("data/common/menu").unwrap();
        assert_eq!(tree.node(menu).files, 2);
        let names: Vec<_> = tree
            .node(menu)
            .children
            .iter()
            .map(|c| &*tree.node(*c).name)
            .collect();
        assert_eq!(names, ["a.cfg.bin", "b.g4tx"]);
        let data = tree.resolve("data").unwrap();
        let top: Vec<_> = tree
            .node(data)
            .children
            .iter()
            .map(|c| (&*tree.node(*c).name, tree.node(*c).kind))
            .collect();
        assert_eq!(
            top,
            [
                ("common", NodeKind::Dir),
                ("dx11", NodeKind::Dir),
                ("loose.txt", NodeKind::File)
            ]
        );
    }

    #[test]
    fn a_dotted_sibling_does_not_split_a_directory() {
        let tree = VfsTree::build(rows(), TreeMode::Path);
        let common = tree.resolve("data/common").unwrap();
        let dirs: Vec<_> = tree
            .node(common)
            .children
            .iter()
            .map(|c| &*tree.node(*c).name)
            .collect();
        assert_eq!(dirs, ["menu", "menu.x", "text"]);
    }

    #[test]
    fn cpk_mode_groups_by_archive_and_maps_back() {
        let tree = VfsTree::build(rows(), TreeMode::Cpk);
        let top: Vec<_> = tree
            .node(VfsTree::ROOT)
            .children
            .iter()
            .map(|c| &*tree.node(*c).name)
            .collect();
        assert_eq!(top, ["(loose)", "common.cpk", "dx11.cpk"]);
        let file = tree.resolve("dx11.cpk/data/dx11/model/m.g4md").unwrap();
        assert_eq!(tree.vfs_path(file), "data/dx11/model/m.g4md");
        assert_eq!(tree.cpk_of(file), Some("dx11.cpk"));
    }

    #[test]
    fn path_round_trips() {
        let tree = VfsTree::build(rows(), TreeMode::Path);
        for id in tree.ids() {
            assert_eq!(tree.resolve(&tree.path(id)), Some(id));
        }
    }
}
