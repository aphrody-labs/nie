# 2026-11-19: prd converted in place from Arch to CachyOS, booting the 7.4-rc3 kernel

## Question

Can the production host prd be converted in place to CachyOS (znver4 repositories, CachyOS-Settings, Rust
toolchain) and run the newest release-candidate kernel, with every service back after the reboots, a fallback
kernel in GRUB, and a promotion step that only trusts a kernel after a health check?

## Environment (before)

- Host `srv2116886` (Hostinger VPS id 2116886, plan KVM 8), Arch Linux, kernel `7.2.14-arch1-1`, AMD EPYC 9354P,
  8 vCPU, 31 GiB, no `/dev/kvm`. glibc loader reports x86-64-v4 supported; `gcc -march=native` reports `znver4`.
- Disk `/dev/sda` GPT: `sda1` BIOS boot, `sda2` vfat 300 MiB (`/efi`, unused: the VPS boots BIOS, `/sys/firmware/efi`
  absent), `sda3` btrfs root (UUID `86a96c15-9d21-45be-a21b-c81a5423eac4`, `compress=zstd:1`), `sda4` XFS label `niegame`
  (UUID `32d2b6fb-28a2-48f5-9cad-87c5c9a42faf`) on `/srv/nie/steam`. GRUB 2.16 i386-pc, `GRUB_DEFAULT=0`, grubenv on
  btrfs with `env_block=512+1`.
- Inventory before any change (copies in `/root/cachyos-convert/` on prd: `running.baseline`, `ports.baseline`,
  `pacman-Q.before`, `pacman.conf.before`, `mirrorlist.before`, `grub.cfg.before`, `default-grub.before`, `fstab.before`,
  `sda.sfdisk`, `sysctl.d.before`):
  - running services: auto-server, ay-web, cloudflared-mig-prd, containerd, dbus-broker, docker, nginx, nie-link,
    obscura, qemu-guest-agent, shenron-cdn-origin, sshd, wii-os-shenron, systemd-*, getty, polkit; plus wg-quick@prod,
    wg-quick@vps, shenron-compose, fleet-firewall, fleet-tune (enabled).
  - containers: shenron-admin, -tv, -redis, -postgres, -gateway, -site, -bot, -cloudflared, rosegriffon-inagle-pg-1, nie-pg.
  - explicitly installed packages (`pacman -Qe`, full list in `pacman-Q.before`), foreign: paru-bin, repos core, extra, multilib, omarchy.
  - `rosegriffon-stack` does not exist as a unit on prd; the rosegriffon database is the container
    `rosegriffon-inagle-pg-1` (restart policy `no`).
- Snapshot: Hostinger action `126560799` (`vps_snapshots_create`), snapshot id `465422`, created
  2026-11-19T11:07:30Z, expires 2026-11-20T11:07:30Z, `restore_time` 1800 s. No snapshot existed before, so nothing
  was overwritten.

## Method

1. Snapshot, then inventory (above).
2. The official script: `https://mirror.cachyos.org/cachyos-repo.tar.xz`, sha256
   `5b3ba76080b570d9935e10ace5ef16f43667289b08752b03333a6546f0910020`; `cachyos-repo.sh` sha256
   `474c72a02359d128e6cf162ff5fc9b6df3ffd43167118a0f93b923c606b345bf`, `install-znver4-repo.awk` sha256
   `6a0ca1e054e767337a4b0de5400312639f3aed3d63c927be1ae8e4735337d89a`. The script was read before it ran (it adds the
   CachyOS keyring, mirrorlists and a rebuilt `pacman`, inserts `[cachyos-znver4]`, `[cachyos-core-znver4]`,
   `[cachyos-extra-znver4]`, `[cachyos]` above `[core]`, sets `Architecture = auto`, then runs `pacman -Syu`). It picked
   znver4 (`gcc -march=native` reported znver4).
3. First attempt failed at its first `pacman -U`: the CachyOS pacman package links `libcrypto.so=4` while the host had
   openssl 3.6.5. Fix at the source: `pacman -Syu --noconfirm --ignore linux` first (103 packages, openssl 4.1.0, kernel
   held so `/usr/lib/modules/7.2.14-arch1-1` stayed usable), then the script again (`yes | ./cachyos-repo.sh --install`,
   as a transient systemd unit so an SSH drop cannot interrupt pacman). It reinstalled 493 packages from the znver4
   repos, replaced `zlib` with `zlib-ng-compat`, and (it has no `--ignore`) upgraded `linux` to 7.3.4. `/usr/lib/libz.so.1`
   is missing for a few seconds inside that transaction: any new exec fails during that window, running processes are not
   affected. 714 packages after.
4. `pacman -S cachyos-settings linux-cachyos-rc linux-cachyos-server rustup lld clang`, later `linux-cachyos-bore
   schbench-git rt-tests fio` for the scheduler measurement.
5. Rust: rustup system-wide in `/opt/rust` (`RUSTUP_HOME`, `CARGO_HOME` appended to `/etc/environment`), `stable`
   (rustc 1.100.0, LLVM 23.1.5) with `rust-src`, `llvm-tools`, `clippy`, `rustfmt`. `/opt/rust/cargo/config.toml`:
   `linker = "clang"`, `rustflags = ["-C", "target-cpu=native", "-C", "link-arg=-fuse-ld=mold"]` (sccache already comes
   from `/etc/environment`). A fresh `cargo init` release build runs and `readelf -p .comment` shows mold 3.0.0.
6. GRUB: `GRUB_DEFAULT=saved`, `GRUB_DISABLE_SUBMENU=y`, `GRUB_TIMEOUT=3`, `grub-install --target=i386-pc /dev/sda`,
   `grub-mkconfig`. Entry ids are `gnulinux-<kernel>-advanced-<root uuid>`. Boot order of the test: `grub-set-default`
   stock, `grub-reboot` server kernel (proves the one-shot clears on this btrfs grubenv: `next_entry` was empty after
   boot), then `grub-reboot` the RC kernel.
7. Promotion: `cargo xtask kernel promote` (installed as `/usr/local/bin/xtask`, unit
   `nie-kernel-promote.service`, required list in `/etc/kernel-promote/required`: the 12 services of the order plus the
   `/srv/nie/steam` mount). It derives the running entry from `/proc/cmdline`, and when it is not the saved default and
   the list is healthy it runs `grub-set-default`; unhealthy for 300 s, it reboots into the saved default.

## Result

Kernels installed (GRUB entries, in `grub.cfg` order): `linux-cachyos-server` 7.3.5-1, `linux-cachyos-rc` 7.4.rc3-1,
`linux-cachyos-bore` 7.3.5-1, stock `linux` 7.3.4.arch1-2. Running now: `7.4.0-rc3-1-cachyos-rc` (clang 23.1.5,
ThinLTO, `CONFIG_MZEN4=y`, full preemption, HZ 1000, `CONFIG_IO_URING=y`, `CONFIG_SCHED_CLASS_EXT=y`, EEVDF).
kernel.org mainline at the time: 7.4-rc3 (`finger_banner`). The RC was booted by `grub-reboot`, `nie-kernel-promote` ran
`grub-set-default` after the health check, and `grub-editenv list` shows `saved_entry=...cachyos-rc...`.

Health, RC kernel, 100 s after boot (`/root/cachyos-convert/health.sh`): auto-server, ay-web, cloudflared-mig-prd,
containerd, docker, nginx, nie-link, obscura, qemu-guest-agent, shenron-cdn-origin, sshd, wii-os-shenron all `active`;
wg-quick@prod, wg-quick@vps, fleet-firewall, fleet-tune, shenron-compose, shenron-stack `active`; `systemctl --failed`
empty; `/srv/nie/steam` on `/dev/sda4` xfs `rw,noatime,largeio,inode64,logbufs=8,logbsize=256k`; loopback probes
3410, 8780, 8788, 8790, 9222, 5432, 6379 open.

Faults found and fixed at the source:

- `cloudflared-mig-prd` exited `203/EXEC` after the first reboot: the drop-in
  `/etc/systemd/system/cloudflared-mig-prd.service.d/20-token-credential.conf` (dated 2026-11-13) points at
  `/usr/local/bin/cloudflared`, which does not exist; the process that was "active" before the reboot predates the
  drop-in. Changed to `/usr/bin/cloudflared` (the pacman binary); `readyConnections: 3`. Backup in
  `/root/cachyos-convert/20-token-credential.conf.before`.
- `shenron-compose` and `shenron-stack` failed on the first two boots: `/srv/shenron/compose.yaml` references images
  (`localhost/shenron:<tag>`, `localhost/shenron-gateway:<sha>`, `aphrody:toolbox-<sha>`) that are not in the local
  image store, while the running containers use untagged image ids. Not caused by the conversion (the units were
  "active" only because they had run at the previous boot). By the RC boot both were `active`; Shenron owns that stack,
  nothing was changed there.
- `ananicy-cpp.service` was enabled by the CachyOS-Settings dependency and re-niced production processes: disabled.

## Scheduler choice (measured, same script on three kernels)

`/root/cachyos-convert/bench.sh`: `hackbench -g 16 -l 4000 -s 512 -T` x5, `schbench -m 2 -t 4` idle x3, `cargo install
ripgrep 15.2.0` (3 sequential builds, no sccache, offline) alone x3 and concurrent with `schbench` x3. Production load was
running during all runs (load 0.9 to 1.5 at start), so differences under 10 percent are noise.

| kernel | scheduler, preemption | idle wake p99 (us) | wake p99 during build (us) | build alone (s) | build with schbench (s) | hackbench (s, median of runs 2-5) |
|---|---|---|---|---|---|---|
| 7.4.0-rc3 cachyos-rc | EEVDF, full, HZ 1000 | 919 | 3300 | 43.4 | 68.0 | 17.2 |
| 7.3.5 cachyos-bore | BORE, full, HZ 1000 | 929 | 2156 | 46.1 | 74.5 | 18.7 |
| 7.3.5 cachyos-server | EEVDF, lazy, HZ 300 | 1982 | 5576 | 44.9 | 64.8 | 19.1 |

Decision: the default is the RC kernel, which ships EEVDF only (no BORE patch in `linux-cachyos-rc`); BORE kept as
the first fallback because it has the lowest wake latency under a build (2.2 ms against 3.3 ms), at 6 to 10 percent lower
build throughput. Builds run on fr1; prd serves, and the RC's EEVDF idle latency (0.9 ms) equals BORE's. Raw files:
`/root/cachyos-convert/bench/<uname -r>.txt`.

## Tuning for nie

- io_uring: `kernel.io_uring_disabled = 0`, `kernel.io_uring_group = -1` (`/etc/sysctl.d/96-nie-prd.conf`, also
  `kernel.panic = 10`). Read-only `fio --direct=1` on one 2.78 GB CPK on the XFS partition (nothing written, 10 s
  each): psync 1 MiB qd1 954 MB/s; io_uring 1 MiB qd1 805 MB/s, qd16 7,980 MB/s (8.4x); psync 4 KiB random qd1 3,314
  IOPS; io_uring 4 KiB random qd32 53,224 IOPS (16x).
- XFS mount options kept: `noatime,inode64,logbsize=256k,largeio,nofail,x-systemd.device-timeout=30s` (fstab
  unchanged). Block queue: scheduler `none`, `read_ahead_kb=1024`, `nr_requests=256`, non-rotational, left as CachyOS
  sets them. `xfs_info`: crc, reflink, finobt, bigtime, 8 allocation groups.
- sysctl effective after boot (`/root/cachyos-convert/sysctl.effective`): `vm.swappiness 150` (zram first, from
  `95-fleet-tune.conf`), `vm.page-cluster 0`, `vm.dirty_bytes 536870912`, `vm.dirty_background_bytes 134217728`,
  `vm.vfs_cache_pressure 50`, `net.ipv4.tcp_congestion_control bbr`, `net.core.default_qdisc fq`,
  `kernel.sched_autogroup_enabled 0`, `kernel.split_lock_mitigate 0`, `fs.aio-max-nr 1048576`. The CachyOS-Settings file
  `/usr/lib/sysctl.d/70-cachyos-settings.conf` sorts before every fleet file, so the fleet values win.

## Limits

- The RC kernel has not run the full 24 hours of production traffic; the saved default is the RC, and a kernel that
  panics after boot reboots after 10 s into the same default. A kernel that fails to start at all needs the Hostinger
  console to pick another GRUB entry (all four stay in the menu) or the snapshot.
- The benchmark ran next to production services; run-to-run spread is up to 30 percent for hackbench.
- Services outside the order (shenron containers, nie-pg, nie-model, nie-serve) were compared by name only.

## Rollback

1. Boot an older entry: Hostinger console, GRUB menu, `Arch Linux, with Linux linux` (stock 7.3.4) or
   `linux-cachyos-server`; or from a shell `grub-set-default gnulinux-linux-cachyos-server-advanced-<root uuid>` and
   reboot.
2. Remove CachyOS: `/root/cachyos-convert/dl/cachyos-repo/cachyos-repo.sh --remove` (restores `pacman.conf.bak`,
   reinstalls native packages from Arch), `/etc/pacman.conf` original in `/root/cachyos-convert/pacman.conf.before`.
3. Last resort: Hostinger snapshot `465422` (`vps_snapshots_restore`, expires 2026-11-20T11:07:30Z).
