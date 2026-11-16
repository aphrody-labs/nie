//! `vfs_sitemap <source|inventory.ndjson> <out_dir> <base_url> <route_prefix>`
//!
//! `source` is a data dir or `ssh://host/path` (live VFS); a path ending in `.ndjson` is read as
//! an inventory instead. Writes `sitemap*.xml`, `robots.txt` and `tree.json` into `out_dir`.

use nie_formats::vfs::Vfs;
use nie_sitemap::{Config, Sitemap, VfsFile};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [src, out, base, prefix] = a.as_slice() else {
        panic!("usage: vfs_sitemap <source|inventory.ndjson> <out_dir> <base_url> <route_prefix>");
    };
    let files: Vec<VfsFile> = if src.ends_with(".ndjson") {
        let text = std::fs::read_to_string(src).expect("read inventory");
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).expect("json line");
                VfsFile { path: v["path"].as_str().expect("path").to_owned(), size: v["size"].as_u64().unwrap_or(0) }
            })
            .collect()
    } else {
        let mut vfs = Vfs::new();
        vfs.init_source(src).expect("init VFS");
        vfs.discover_extra_cpks();
        vfs.iter_all().map(|e| VfsFile { path: e.path.to_owned(), size: e.size }).collect()
    };
    let map = Sitemap::from_files(files.clone(), Config::new(base, prefix));
    std::fs::create_dir_all(out).expect("mkdir");
    for (name, xml) in map.render() {
        std::fs::write(format!("{out}/{name}"), xml).expect("write sitemap");
    }
    std::fs::write(format!("{out}/robots.txt"), map.robots_txt()).expect("write robots");
    std::fs::write(format!("{out}/tree.json"), map.tree_json(&files).to_string()).expect("write tree");
    eprintln!("files={} pages={} sitemaps={}", map.file_count(), map.pages().len(), map.chunk_count());
}
