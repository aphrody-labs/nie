//! Inventaire complet du VFS : une ligne NDJSON par fichier logique servi par `Vfs::read`
//! (index `cpk_list.cfg.bin` + packs absents de l'index), avec son pack et sa taille.
//!
//! Usage : `vfs_inventory <game_data_dir> > vfs.ndjson`

use nie_formats::vfs::Vfs;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: vfs_inventory <game_data_dir>");
    let mut vfs = Vfs::new();
    vfs.init(&dir).expect("init VFS");
    let extra = vfs.discover_extra_cpks();
    eprintln!(
        "entrees={} paquets={} extra_paquets={extra}",
        vfs.asset_count(),
        vfs.cpk_count()
    );
    let mut lignes: Vec<_> = vfs.iter_all().collect();
    lignes.sort_by(|a, b| a.path.cmp(b.path));
    let mut sortie = std::io::BufWriter::new(std::io::stdout().lock());
    for e in lignes {
        use std::io::Write;
        let ligne = serde_json::json!({"path": e.path, "pack": e.cpk, "size": e.size, "extra": e.extra});
        writeln!(sortie, "{ligne}").expect("écriture");
    }
}
