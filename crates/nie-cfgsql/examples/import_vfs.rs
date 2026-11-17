//! `import_vfs <source> <out.sql>`: decodes every `*.cfg.bin` of the VFS and writes the whole
//! PostgreSQL import script (provenance, COPY batches, keys). Load it with
//! `psql -v ON_ERROR_STOP=1 -f out.sql`.

use std::io::BufWriter;
use std::time::Instant;

use nie_cfgsql::pg::{FileMeta, PgImporter};
use nie_formats::vfs::Vfs;
use nie_formats::vfs_manifest::sha256_hex;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [src, out] = a.as_slice() else {
        panic!("usage: import_vfs <source> <out.sql>");
    };
    let t0 = Instant::now();
    let mut vfs = Vfs::new();
    vfs.init_source(src).expect("init VFS");
    vfs.discover_extra_cpks();
    let all: Vec<(String, String, u64)> = vfs
        .iter_all()
        .map(|e| (e.path.to_owned(), e.cpk.to_owned(), e.size))
        .collect();
    let mut files: Vec<(String, String)> = vfs
        .iter_all()
        .filter(|e| e.path.ends_with(".cfg.bin"))
        .map(|e| (e.path.to_owned(), e.cpk.to_owned()))
        .collect();
    files.sort();
    eprintln!("cfg.bin files: {}", files.len());
    let file = std::fs::File::create(out).expect("create output");
    let mut imp = PgImporter::new(BufWriter::with_capacity(1 << 20, file)).expect("header");
    let mut all = all;
    all.sort();
    for (p, k, s) in &all {
        imp.add_listing(p, k, *s).expect("listing");
    }
    eprintln!("listing entries: {}", all.len());
    std::panic::set_hook(Box::new(|_| {}));
    for (i, (path, pack)) in files.iter().enumerate() {
        let (meta, decoded) = match vfs.read(path) {
            Ok(bytes) => {
                let meta = FileMeta {
                    path: path.clone(),
                    pack: pack.clone(),
                    size: bytes.len() as u64,
                    sha256: sha256_hex(&bytes),
                };
                let decoded = std::panic::catch_unwind(|| nie_cfgsql::from_bytes(&bytes))
                    .unwrap_or_else(|_| Err(nie_cfgsql::Error::Parse("parser panicked".into())));
                (meta, decoded)
            }
            Err(e) => (
                FileMeta {
                    path: path.clone(),
                    pack: pack.clone(),
                    size: 0,
                    sha256: String::new(),
                },
                Err(nie_cfgsql::Error::Parse(format!("read failed: {e}"))),
            ),
        };
        imp.add(&meta, decoded).expect("write");
        if (i + 1) % 10_000 == 0 {
            eprintln!("{} files, {:.1}s", i + 1, t0.elapsed().as_secs_f32());
        }
    }
    let counts = imp.counts.clone();
    let fks = imp.inferred_fks;
    imp.finish().expect("finish");
    eprintln!(
        "generated in {:.1}s counts={counts:?} inferred_fks={fks}",
        t0.elapsed().as_secs_f32()
    );
}
