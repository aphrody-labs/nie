//! `dump_file <source> <internal_path> <out>`: writes one VFS file to disk (test fixtures for
//! consumers that read bytes, such as the bun:ffi test).

use nie_formats::vfs::Vfs;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [src, path, out] = a.as_slice() else {
        panic!("usage: dump_file <source> <internal_path> <out>");
    };
    let mut vfs = Vfs::new();
    vfs.init_source(src).expect("init VFS");
    let bytes = vfs.read(path).expect("read");
    std::fs::write(out, &bytes).expect("write");
    eprintln!("{path}: {} bytes", bytes.len());
}
