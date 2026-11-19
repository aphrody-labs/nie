# Session handoff, 2026-11-19

Method: every number below comes from a command run on fr1 (`/srv/xbuild/nie-final`, a copy of the PC
working tree, `rustc 1.100.0`) or on the PC during this session.

## Verified on fr1 on the working tree (not committed)

| Check | Command | Result |
|---|---|---|
| Build | `cargo check --workspace --all-targets` | passes |
| Lint | `cargo clippy --workspace --all-targets --keep-going -- -D warnings` | no error; `unused dependency` warnings remain (cranelift-jit, libloading, wasm-bindgen in nie-wasm, nie-formats in nie-bevy) |
| Tests | `cargo nextest run --workspace` | not run (interrupted by the owner) |

Fixes made to get there: duplicate `nie-tls` lines in three manifests, `corosensei` declared in
`[workspace.dependencies]`, `russh` 0.65.2 with the `aws-lc-rs` backend (it has `compile_error!` without
`ring` or `aws-lc-rs`; `deny.toml` allows `aws-lc-rs` only under `russh`), `Client` visibility in nie-ssh,
`RemoteStorage` made public, `gix_index` entry path call, `ChannelOpenHandle::reject` signature, the lost
`cfgbin_to_iecode_root` and `cfgbin_to_t2b_iecode_root` restored in `nie-model-serve/src/decode.rs`,
test-only dependencies of `nie-model-serve`, one non-ASCII identifier, `FontSize` in the nie-tui test, and
the clippy findings.

## State of main

- `origin/main` is at 684313301 and does not build: it carries nie-browser without the root manifest and
  lock lines. The working tree on the PC fixes it; it has about 400 uncommitted files from the 19 agents.
- Release run 44525397183 failed for that reason, on both targets.
- Locale dictionary fix 4785db34d (every shipped language, not fr/en/es) is pushed.
- Linux release is static `x86_64-unknown-linux-musl` (684313301).

## Production

- prd runs kernel 7.4.0-rc3-1-cachyos-rc. `nie-serve` and `nie-model` answer `/health` with
  `vfsReady: true`, 936 CPK and 255628 entries; the Cloudflare tunnel unit is inactive, so
  nie.aphrody.app is not reachable. The PC reaches prd only through fr1 (10.200.77.5).
- `deploy/supervise.toml` and the `nie-deploy supervise` command replace the units; the cut-over was not
  done.

## Owner orders recorded this session

Memory entries: full-low-level, long-commands-in-tmux, perf-parity-gate, minimal-json-context,
prd-shenron-lock. Rules added to AGENTS.md: UI and performance gate (120 fps at 1280x720, pixel parity
with test-tui, agent-desktop and agent-browser, hyperfine and JetStream), tmux for long commands, no
Docker, systemd, HTML, CSS or JS for nie.

## Cleaned

- fr1: nie scratch dirs under `/srv/xbuild` (`nie-*`, `nie/target*`, `nie/tree-*`, `fw*`, `gui-*`):
  4.6 GB free before, 15 GB after the first minutes of deletion, 77 GB later.
- PC: `%LOCALAPPDATA%\nie\link` game cache (64 GB) and the rosegriffon `node_modules`, `.local/video` and
  `.local/logs`; C: has 251 GB free. `C:\src\rosegriffon\.local\archive` (2.7 GB, a backup of the prd
  cleanup) is kept.

## Left

1. Commit the working tree (one commit), run `cargo nextest run --workspace`, push, tag, watch the release.
2. `cargo deny check bans`, `cargo shear`, `gitleaks detect` before the push.
3. Cut prd over to `nie-deploy supervise`, restore the public route, prove it in Chrome.
4. Tasks 10, 12, 13 (cargo check of 4785db34d done as part of the workspace check), 15, 18, 28, 29, 30.
5. Measure the 120 fps gate and pixel parity; nothing is measured yet.
