# 2026-11-19: nie on Asterinas (OSDK), boot and syscall audit

## Question

Can nie run as a Linux-ABI process on a Rust kernel, with the game directory shared from the host,
and which syscalls does nie still need that the kernel does not implement?

## Environment

- Build host `fr1`: Ubuntu 26.10, kernel 7.3.0-10-generic, Intel Core Haswell (no TSX) guest with
  nested KVM (`/dev/kvm`), 12 vCPU, 45 GiB, QEMU 11.0.3, grub-mkrescue 2.14, OVMF 2026.05,
  virtiofsd 1.14.0 (apt), strace (apt). The host load average was 50 to 60 during the runs, so
  build times are not benchmarks.
- Fork `aphrody-labs/asterinas`, synced to upstream main `2a3ceaea0f9c50e64118e4ab155d7be438242b6d`
  (v0.19.0 is `c4b7ed575b7b32773475b194d2ad41fb6d03765b`; tags pushed to the fork).
- `os/` toolchain `nightly-2026-07-21` (the one of the v0.19.0 tag), `cargo-osdk` 0.19.0 and `ostd`
  0.19.0 from crates.io. The Linux-ABI kernel builds from the fork main with
  `nightly-2026-11-16` and `cargo-osdk` installed from that checkout.
- Static musl `nie` 2.0.0 (feature `cloud`, `x86_64-unknown-linux-musl`, `static-pie`), built on
  fr1 from the committed tree.
- Game data: none copied. The guest serves a synthetic VFS written by
  `cargo run -p nie-formats --example synthetic_game -- <dir>` (64 files, 285,339 byte CPK).

## Method and results

See the sections below; every hash and log line was read from the command output of this session.
