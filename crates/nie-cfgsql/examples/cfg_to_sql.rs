//! `cfg_to_sql <source> <internal_path> <postgres|sqlite>`: reads one cfg.bin through the VFS
//! (`source` is a data dir or `ssh://host/path`) and prints the SQL script.

use nie_cfgsql::{Dialect, from_bytes};
use nie_formats::vfs::Vfs;

fn main() {
    let mut a = std::env::args().skip(1);
    let (src, path, dialect) = (
        a.next().expect("source"),
        a.next().expect("internal path"),
        a.next().unwrap_or_else(|| "sqlite".into()),
    );
    let mut vfs = Vfs::new();
    vfs.init_source(&src).expect("init VFS");
    let bytes = vfs.read(&path).expect("read");
    let data = from_bytes(&bytes).expect("decode");
    let dialect = if dialect == "postgres" { Dialect::Postgres } else { Dialect::Sqlite };
    print!("{}", data.to_sql(dialect));
}
