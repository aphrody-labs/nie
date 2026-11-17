//! `unreadable_probe <source>`: lists the `*.cfg.bin` files that `nie_cfgsql::from_bytes` rejects,
//! with size, first bytes and Shannon entropy (a value near 8 bits per byte means encrypted or
//! compressed data).

use nie_formats::vfs::Vfs;

fn entropy(b: &[u8]) -> f64 {
    if b.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for &x in b {
        counts[usize::from(x)] += 1;
    }
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / b.len() as f64;
            -p * p.log2()
        })
        .sum()
}

fn main() {
    let src = std::env::args()
        .nth(1)
        .expect("usage: unreadable_probe <source>");
    let mut vfs = Vfs::new();
    vfs.init_source(&src).expect("init VFS");
    vfs.discover_extra_cpks();
    let mut paths: Vec<String> = vfs
        .iter_all()
        .filter(|e| e.path.ends_with(".cfg.bin"))
        .map(|e| e.path.to_owned())
        .collect();
    paths.sort();
    std::panic::set_hook(Box::new(|_| {}));
    let mut bad = 0;
    for p in &paths {
        let Ok(bytes) = vfs.read(p) else {
            println!("READ_FAILED {p}");
            continue;
        };
        let ok =
            std::panic::catch_unwind(|| nie_cfgsql::from_bytes(&bytes).is_ok()).unwrap_or(false);
        if !ok {
            bad += 1;
            let head: Vec<String> = bytes.iter().take(24).map(|x| format!("{x:02x}")).collect();
            println!(
                "{p} len={} entropy={:.2} head={}",
                bytes.len(),
                entropy(&bytes),
                head.join(" ")
            );
        }
    }
    println!("total={} unreadable={bad}", paths.len());
}
