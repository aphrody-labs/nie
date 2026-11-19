# os: nie on Asterinas

A standalone OSDK workspace (own `[workspace]`, `Cargo.lock` and `rust-toolchain.toml`), kept out of
the stable main workspace. It builds and boots only on the build host (`fr1`, KVM) or in CI.

| Path | What |
| --- | --- |
| `nie-os` | Minimal OSTD kernel on crates.io `ostd` 0.19.0, booted by `cargo osdk run`. |
| `nie-ostd-vfs` | OSTD library component for the guest-side VFS reader; ktests only. |
| `OSDK.toml`, `cargo xtask os qemu` | `-cpu EPYC-Genoa`, 8 CPUs, 8G, virtio-net, virtio-rng, optional virtio-blk and virtiofs; every value is an environment variable (`NIE_OS_*`). |
| `initramfs/init`, `cargo xtask os initramfs` | Reproducible initramfs: static busybox, the static musl `nie`, an init that mounts the virtiofs share `nie-game` on `/game` and runs `nie serve`. |
| `cargo xtask os boot` | Boots the Linux-ABI kernel of `vendor/asterinas` with that initramfs and `virtiofsd` over a game directory. |

The toolchain is the one of the Asterinas tag that matches the crates.io `ostd` and `cargo-osdk`
release (v0.19.0: `nightly-2026-07-21`); `vendor/asterinas` main moves on to a newer nightly and its
own `ostd`, so the Linux-ABI kernel is built from that checkout with its own `cargo-osdk`.

```sh
# on fr1
cargo install cargo-osdk --version 0.19.0 --locked
cd os/nie-os && cargo osdk build && cargo osdk run        # nie-os ISO, boots to "nie-os 0.1.0 booted"
cd ../nie-ostd-vfs && cargo osdk test                     # ktests

cargo run --release -p nie-formats --example synthetic_game -- /tmp/game   # synthetic VFS, no game bytes
cargo xtask os boot /tmp/game /path/to/static-musl/nie             # serves /health on 127.0.0.1:8790
```

`nie deps audit --syscalls` (policy `docs/specs/asterinas-syscalls.policy.toml`, result
`docs/specs/asterinas-syscalls.json`) lists the syscalls nie uses that the Asterinas SCML tables
do not cover. Records: `docs/research/log/2026-11-19-asterinas.md`.
