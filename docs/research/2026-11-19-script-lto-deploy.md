# Rust scripts, Bun, LTO/AOT and the deploy flow: survey (2026-11-19)

Question: what should the `nie-xtask` consolidation, the Cargo profiles and `nie-deploy` adopt, from live
sources. Method: `gh api`, crates.io API, `ssh fr1` (rustup, cargo, clang, apt), the rust-lang and oven-sh
repositories. Every fact carries a source id from the table at the end; "unverified" marks what no command
confirmed. Environment: fr1 `rustc 1.100.0 (0ef1bc386 2026-11-09)`, LLVM 23.1.5 [S1]; the PC built nothing.

## 1. Single-file Rust scripts (`cargo -Zscript`)

| Fact | Evidence |
|---|---|
| Not stable in 1.100.0: `cargo +1.100.0 -Zscript a.rs` fails with "the `-Z` flag is only accepted on the nightly channel"; `cargo script` is "no such command". | [S2] |
| Not stable in 1.101.0-beta.2 either: the stabilization PR rust-lang/cargo#16569 "Stabilize cargo script" is open, `S-waiting-on-review`, no milestone, last update 2026-06-07; the last human comment (2026-04-09, ehuss) raises an open concern "edition-behavior" (rust-lang/rust#152254) and a security concern (interaction with RFC 3279). | [S3] |
| Tracking issue: rust-lang/cargo#12207 (eRFC 3424, RFC 3502 and 3503), open, `S-waiting-on-feedback`, updated 2026-11-09. Unticked: `cargo clippy` support, rust-analyzer support, `cargo clean gc` vs the script lockfile. "Deferred / non-blocking": workspace support, `cargo publish`. | [S4] |
| Frontmatter in rustc is separate and also unstable: rust-lang/rust#136889 (open, F-frontmatter), stabilization PR rust-lang/rust#148051 open, `S-blocked`, FCP finished, last update 2026-05-17. `rustc +1.100.0` and `+beta` both give E0658 "frontmatters are experimental". | [S5][S2] |
| Works on nightly (cargo 1.102.0-nightly ede6cefea 2026-11-11): `cargo +nightly -Zscript a.rs` runs. | [S2] |
| Workspace: a script is its own workspace root. `workspace = true` inheritance fails (`workspace.dependencies was not defined`); a parent workspace is never joined. It cannot reuse `[workspace.dependencies]` of nie. | [S2] |
| Lockfile: yes, but not beside the script. It is written under `~/.cargo/build/<hash>/<hash>/Cargo.lock` (observed), so it is not committable or reviewable, and `--locked` cannot pin it to the repository. | [S2] |
| Config is read only from `CARGO_HOME`, not from the repository `.cargo/config.toml`; so the `xtask` alias, flags and `target-dir` of the repo do not apply. | [S4] |

Comparison with the xtask pattern for nie:

- xtask is a workspace member: it inherits `[workspace.dependencies]`, `[lints]`, the single `Cargo.lock`
  checked by `--locked`, the pinned `rust-toolchain.toml`, the `shear`/`deny`/`audit` gates and `MAP.tsv`.
  A `-Zscript` file has none of these and needs nightly, which conflicts with the pinned stable 1.100.0.
- Reproducibility rule of AGENTS.md ("`--locked`, pinned toolchain") cannot be met by a script today.
- Recommendation: keep `nie-xtask` as the only home (`.cargo/config.toml` alias `xtask = "run -q -p nie-xtask --"`
  already exists, `justfile` already delegates). Revisit scripts only after cargo#16569 merges and workspace
  support lands; track both in the forks-sync style watcher. A script is acceptable only for a throwaway
  outside the repo.
- One xtask improvement independent of scripts: build xtask once and reuse (`cargo xtask` recompiles on
  dependency change; `just` can call `target/debug/xtask` after a `cargo build -p nie-xtask` recipe).

## 2. Bun

- Latest: Bun v1.4.5, published 2026-11-19T08:47:28Z (previous v1.4.4 2026-10-22); `bun --version` on fr1 prints 1.4.5. [S6]
- Bun Shell (`Bun.$`): cross-platform bash-like shell (Windows, Linux, macOS) with native `ls`, `cd`, `rm`
  builtins, pipes, redirection (including to JS objects), globs (`**`, `{}`), automatic escaping of
  interpolated strings against injection, `.quiet()`, `.text()`, `.json()`, `.nothrow()`, `.cwd()`, `.env()`. [S7]
- `Bun.spawn` / `Bun.spawnSync` (child processes with streams, IPC, timeouts); docs at docs/runtime/child-process.mdx. [S8]
- `bun build --compile`: single-file executable embedding the runtime plus the bundle. Targets:
  `bun-linux-x64`, `bun-linux-arm64`, `bun-windows-x64`, `bun-windows-arm64`, `bun-darwin-x64`,
  `bun-darwin-arm64`, `bun-linux-x64-musl`, `bun-linux-arm64-musl` (`-baseline`/`-modern` suffixes are accepted
  and map to the same binary). Cross-compilation is a flag. `--bytecode` moves JavaScriptCore parsing to build
  time (docs claim `tsc` starts 2x faster); `--bytecode-order=<profile>` is a profile-guided bytecode layout
  recorded with `BUN_BYTECODE_ORDER_OUT`; `--minify`, `--sourcemap` (zstd), `--compile-exec-argv`. [S9]
- Honest assessment (nie rules forbid hand-written JS/TS for nie itself; this applies to repo tooling that is
  not nie, such as the Claude profile scripts under `~/.claude`, which are outside the repo):
  - Bun wins for: glue that is mostly process orchestration and text (Bun Shell is shorter than
    `std::process::Command` plus error plumbing), JSON munging, Windows/Linux portability without bash,
    startup of small scripts without a compile step (no 24 s release build; the xtask first build on fr1 is
    minutes of `reqwest`/`clap`/`regex`), and prototyping before a Rust port.
  - Rust wins for: everything that must be reproducible and gated here (`--locked`, one `Cargo.lock`,
    `cargo shear`, `cargo deny`, layering test, `MAP.tsv`), anything sharing types with `nie-*` crates (the
    contract, `repro` manifest, `supervise.toml`), no second runtime to pin on every host and in CI, and the
    AGENTS.md rule "no hand-written JS". A `bun build --compile` binary embeds the Bun runtime (tens of MB;
    size not measured here, unverified) and is not byte-reproducible by `nie repro` today (unverified).
  - Decision: no Bun in the repo. `bun build --compile` is AOT only in the sense of packaging bytecode; it is
    not native compilation of the script, so it does not meet the AOT bar the Lua task (#31) sets.

## 3. LTO and AOT across the workspace

Current state [S10][S11]:

- `Cargo.toml`: `[profile.release] lto = "thin"`, `codegen-units = 1`, `debug = "limited"`; no `panic`, no `strip`.
  `wasm-release` and `web-craft` inherit release with `lto = "fat"`, `panic = "abort"`, `strip = true`, `opt-level = 3`/`"z"`.
- `release.yml` sets only env: `CARGO_PROFILE_RELEASE_DEBUG=false`, `CARGO_PROFILE_RELEASE_STRIP=true`,
  `CARGO_INCREMENTAL=0`; Linux builds `x86_64-unknown-linux-musl` with `musl-tools`, Windows builds msvc.
- `xtask repro` builds `--release --locked` with `--remap-path-prefix`, `SOURCE_DATE_EPOCH`, no `target-cpu=native`.
- `crates/nie-abi` generates `catch_unwind` guards in every C-ABI export (`crates/nie-abi/src/lib.rs:200,280,285`, `crlib.rs:223`).

Findings:

1. `fat` vs `thin` [S12]: cargo documents `fat` (`true`) as whole-graph optimization and `thin` as "substantially less
   time ... similar gains". `codegen-units = 1` removes intra-crate parallelism and makes `thin` local LTO a
   no-op. Which is faster for nie is not measured: decide with `hyperfine --warmup 3 --runs 20 --export-json`
   (AGENTS.md gate), not from folklore. Fat is justified for the shipped binaries (the CI builds are
   `--locked` and uncached for release; fr1 does the heavy work), thin keeps iteration cheap.
2. `panic = "abort"` must not apply to the cdylib/crlib: the generated exports rely on `catch_unwind` to return a
   failure value instead of crossing the FFI boundary; with abort a Rust panic kills the host process.
   `panic` is ignored in `[profile.*.package.*]` overrides, so use a separate profile for binaries and keep
   `release` (unwind) for the `nie-ffi` cdylib and tests. Tests also need unwind.
3. `strip`: `release.yml` strips by env; the repro build does not, so repro hashes unstripped binaries with
   `debug = "limited"` while the shipped ones are stripped: the proven artifact is not the shipped one. Put
   `strip`/`debug` in a profile so both paths use the same one.
4. `trim-paths`: not stable in cargo 1.100.0 (the build of a probe crate fails: "requires the Cargo feature called
   `trim-paths`, but that feature is not stabilized in this version of Cargo"), it builds on 1.101.0-beta.2 and
   1.102 nightly. Keep `--remap-path-prefix` now; switch to `trim-paths = "object"` when the pin moves to 1.101. [S2]
5. Cross-language LTO [S13]: `-Clinker-plugin-lto` makes rustc emit bitcode; C/C++ must be built with
   `-flto=thin` (or `-flto=full` with `-Clto=fat`) by clang, linked with `-fuse-ld=lld`; "make sure your rust major
   LLVM version matches your installed LLVM tooling". fr1 has rustc LLVM 23.1.5 [S1] and `clang 23.1.6`, `ld.lld-23` [S14]:
   major matches. The workspace itself has no first-party C sources (`grep cc::Build|cc =` over crates, plugins, apps
   found nothing), so the only candidates are dependencies with `build.rs` C code; the benefit is unmeasured.
   Do not enable it globally: a `RUSTFLAGS` change invalidates the cache and the repro flags are cleared by xtask.
   Add it only to a dedicated `just lto-cross` experiment on fr1 once a C-ABI consumer benchmark exists.
6. PGO/BOLT via cargo-pgo [S15]: latest 0.3.0 (published 2026-01-25). PGO needs `llvm-profdata` (rustup component
   `llvm-tools` is installed on 1.100.0 on fr1; `/usr/lib/llvm-23/bin/llvm-profdata` also exists). BOLT needs
   `llvm-bolt` and `merge-fdata`; fr1 has only the 21 build (`llvm-bolt-21`) while apt offers `bolt-23`
   `1:23.1.6` (not installed). cargo-pgo marks BOLT and Docker "experimental"; BOLT also works on Linux ELF
   only, so Windows and wasm are out. PGO profiles are build inputs: to stay reproducible, the `.profdata` must be
   committed (or content-addressed in the artifact cache) and its BLAKE3 recorded in the repro manifest.
7. musl and LTO: rustc LTO covers Rust crates; musl's libc comes as precompiled `libc.a` from `rust-std-*-musl`
   (installed on fr1), so it is outside the LTO graph and nothing breaks: static linking and LTO are independent. The
   known cost of musl is the allocator (slow malloc under threads); the workspace uses no `mimalloc`/`jemalloc`
   for first-party binaries (grep found only vendor/kitesurf). Whether it matters for `nie serve` is unmeasured
   (unverified). BOLT on a static-musl binary is unverified.

Proposed changes (exact):

```toml
# Cargo.toml
[profile.release]            # keeps unwind: nie-ffi cdylib, crlib exports, tests
lto = "thin"
codegen-units = 1
debug = "limited"

# Shipped and proven binaries: one profile for release.yml, xtask repro and nie-deploy.
[profile.dist]
inherits = "release"
lto = "fat"
codegen-units = 1
panic = "abort"      # binaries only; never used to build libnie / crlib
strip = "symbols"
debug = false
incremental = false
```

- `release.yml`: drop the `CARGO_PROFILE_RELEASE_*` env, build with `--profile dist` for the `nie`, `nie-model-serve`,
  `nie-deploy` binaries; build `libnie` with `--release`. `just ci-release-linux/windows` and `xtask repro` gain
  `--profile dist` (artifact path becomes `target/<triple>/dist/`; `release archive --bin-dir` follows).
- Before merging, measure thin vs fat and unwind vs abort on `nie vfs` workloads with hyperfine on fr1 and write the
  result to `docs/research/log` with the environment fingerprint; keep thin if fat is within noise.
- Optional later: `cargo pgo` recipe `just pgo` on fr1 (needs `apt install bolt-23` for BOLT, `cargo binstall cargo-pgo@0.3.0`),
  profile committed under `docs/research/log/pgo/<commit>.profdata` with its hash.

## 4. Deploy flow

Current state [S16]: `nie-deploy up --host` ships each binary with `ship()` (`crates/nie-deploy/src/up.rs:243`):
SHA-256 compare over `sha256sum`, upload through `cat > dest.new && chmod 0755 && mv -f`, then runs `nie-deploy up`
remotely; `up` refreshes the supervisor, installs a boot hook (cron.d/rc.local), `wait_healthy()` (up.rs:150).
`supervise.rs`: setsid, uid drop, rlimits, restart with exponential backoff, rotated logs, HTTP probes with
`grace_secs`, SIGHUP restarts every service (documented as "new binaries are picked up"), flock single instance,
`status.json`. `deploy/supervise.toml` has three services plus a periodic one; each probes `/health`.
Gaps: the swap keeps no previous binary (an overwritten `nie` cannot be rolled back); health failure after `up`
only reports, it does not revert; the whole binary is `read` into memory and shipped every time; no artifact is
verified against the release manifest; restart of `nie-serve` drops the listening socket (SIGTERM, then relaunch).

Proposed features, all Rust, no systemd or Docker. Crate versions read from crates.io 2026-11-19 [S17]:

| Feature | Design | Crate and version |
|---|---|---|
| Atomic swap with rollback | Install into `<install_dir>/.nie/<blake3>/<bin>`, flip a `current` symlink with `rename(2)` (`renameat2` `RENAME_EXCHANGE` when both are symlinks/dirs), keep the previous target as `previous`; `rollback` re-points it. Binaries stay immutable, so no "text file busy". | `rustix` 1.1.6 (`fs`: `renameat_with`, no libc) or the existing `nix` (workspace pins 0.31.4; 0.32.0 is out); `atomic-write-file` 0.3.1 for the manifest/status writes |
| Health-gated promotion | `up` starts the new generation, waits `wait_healthy`, and flips `current` only after the probes pass (`contains = vfsReady:true`); on failure, restore `previous` and exit non-zero. Already half there in supervise `grace_secs`. | none (existing `nie-health`) |
| Content-addressed artifact cache | Key by BLAKE3 of each binary under `/var/cache/nie/artifacts/<hash>`; the client sends hashes first, the host answers which are missing (replaces `sha256sum` per file, supports rsync-like delta of nothing but whole files), and fr1 can seed prd. Reuse the nie-link CAS format (task #17). | `blake3` 1.8.7 (already workspace) |
| Reproducible release manifests | Extend `docs/specs/repro-manifest.json` (toolchain, `Cargo.lock` hash, BLAKE3 per artifact) into the signed deploy input: `nie-deploy up --manifest release.json` refuses a binary whose hash is not in it. Embed the dependency list in the binary for audit. | `cargo-auditable` 0.7.7 (binary SBOM); `cyclonedx-bom` 0.9.0 if a CycloneDX file is required |
| Signed artifacts | Two options that keep `deny.toml` clean: (a) minisign: `minisign-verify` 0.3.0 (zero dependencies) on the host, `minisign` 0.10.0 (ct-codecs, getrandom, rpassword, scrypt) in xtask to sign; (b) SSHSIG over the same key as `nie-ssh`: `ssh-key` 0.6.7 (RustCrypto, sha2, signature). Sigstore: `sigstore` 0.15.0 has default features `full` and `native-tls` and optional `aws-lc-rs`/`openssl`, both banned by `deny.toml`; whether a feature set exists without them is unverified, and keyless needs Fulcio/Rekor network trust. Recommend (a) for the supervisor host (verify-only crate with no dependencies), key id in `deploy/supervise.toml`, signature in the release asset. | `minisign-verify` 0.3.0, `minisign` 0.10.0 or `ssh-key` 0.6.7 |
| Zero-downtime socket handover | Two proven models: (1) systemd-style `LISTEN_FDS` inheritance: the supervisor owns the listening socket and passes the fd to each generation, so a restart never closes it; `listenfd` 1.0.2 reads it in the service (the supervisor side is a few `nix` calls: bind, clear `CLOEXEC`, set `LISTEN_FDS`/`LISTEN_PID`). (2) Pingora's graceful upgrade (already in the dependency graph, `pingora-core` 0.9.1): start the new instance with `--upgrade`, SIGQUIT to the old, fds transferred over a Unix socket; `nie-edge` can use it directly. Recommend (1) for `nie-serve`/`nie-model` (plain HTTP servers owned by the supervisor), (2) for `nie-edge`. | `listenfd` 1.0.2 (last release 2025-01-19, stable API); `sd-listen-fds` 0.2.0 is older (2023-08-27), skip; `nix` for the supervisor side |
| Self-update of `nie-deploy` | The supervisor binary must replace itself on update, then re-exec: `self-replace` 1.5.1 (Windows aware) is not needed on Linux-only prd; plain atomic rename plus `exec` suffices. | none |
| xtask/just recipe shape | Add `xtask deploy {build,sign,push,promote,rollback,status}`; `just deploy` becomes `build` (fr1, `--profile dist`) then `sign` then `push --host prd` then `promote`. `just fr1 deploy-prd` runs it in the tmux session of the job. Keep `nie-deploy` as the on-host binary (it must run without the repo); `xtask` only orchestrates builds and signing. | `minisign` in xtask; no new crate for the recipes |

Order of work (smallest risk first): immutable generation directories plus `rollback` and health-gated flip
(no new crate); manifest hash check plus `minisign-verify`; CAS cache; `LISTEN_FDS` handover; `--profile dist`.

## 5. Limits and unverified items

- Bun binary size, reproducibility of `bun build --compile`, and BOLT on musl were not measured.
- No benchmark compares thin and fat LTO or abort and unwind for nie: the proposal gates the switch on hyperfine.
- `sigstore` without banned TLS backends was not resolved (`cargo tree` not run).
- The nightly-script observations come from one tiny script on fr1; the lockfile location is the path `find`
  returned, not a documented guarantee.
- Side effect on fr1: `rustup` auto-installed the `beta` toolchain (1.101.0-beta.2) while probing; it can be removed
  with `rustup toolchain uninstall beta`.

## Sources (retrieved 2026-11-19)

| Id | Source | Command or URL |
|---|---|---|
| S1 | rustc 1.100.0 (0ef1bc386 2026-11-09), LLVM 23.1.5; default nightly 2026-09-14 LLVM 23.1.1 | `ssh fr1 rustc +1.100.0 -vV` |
| S2 | Probes: script under stable (error), beta/stable frontmatter (E0658), nightly script run, workspace inheritance error, lockfile path, trim-paths probe | `ssh fr1` in `/tmp/scr`, `/tmp/tp` |
| S3 | rust-lang/cargo#16569 state, labels, comments | `gh api repos/rust-lang/cargo/pulls/16569`, `.../issues/16569/comments` |
| S4 | rust-lang/cargo#12207 tracking issue body (checklist, deferred items, considerations) | `gh api repos/rust-lang/cargo/issues/12207` |
| S5 | rust-lang/rust#136889 and #148051 | `gh api repos/rust-lang/rust/issues/136889`, `.../pulls/148051` |
| S6 | Bun v1.4.5, 2026-11-19; `bun --version` 1.4.5 | `gh api repos/oven-sh/bun/releases`; `ssh fr1 bun --version` |
| S7 | Bun Shell docs | `oven-sh/bun` `docs/runtime/shell.mdx` |
| S8 | Bun child process docs | `oven-sh/bun` `docs/runtime/child-process.mdx` |
| S9 | Bun single-file executables, targets, bytecode | `oven-sh/bun` `docs/compiler/executables.mdx` |
| S10 | nie profiles | `C:\src\nie\Cargo.toml:171-212` |
| S11 | Release workflow and recipes | `C:\src\nie\.github\workflows\release.yml:17-21`, `justfile` `ci-release-*`, `xtask/src/repro.rs` |
| S12 | Cargo profile reference (lto, codegen-units, panic, strip, trim-paths) | `rust-lang/cargo` `doc/book/src/reference/profiles.md` |
| S13 | rustc book, linker-plugin-lto, toolchain compatibility | `rust-lang/rust` `src/doc/rustc/src/linker-plugin-lto.md` |
| S14 | clang 23.1.6, `ld.lld-23`, `llvm-bolt-21` only, `bolt-23` candidate 23.1.6, `llvm-tools` component installed | `ssh fr1` (`clang --version`, `apt-cache policy bolt-23`, `rustup component list`) |
| S15 | cargo-pgo 0.3.0 (2026-01-25), README requirements | `gh api repos/Kobzol/cargo-pgo/releases/latest`, readme |
| S16 | nie-deploy sources and manifests | `crates/nie-deploy/src/{up,supervise,main}.rs`, `deploy/supervise.toml`, `deploy/build.toml` |
| S17 | crate versions and dependencies (minisign-verify 0.3.0, minisign 0.10.0, sigstore 0.15.0, ssh-key 0.6.7, rustix 1.1.6, nix 0.32.0, listenfd 1.0.2, sd-listen-fds 0.2.0, atomic-write-file 0.3.1, self-replace 1.5.1, cargo-auditable 0.7.7, cyclonedx-bom 0.9.0, blake3 1.8.7, just 1.59.0) | `curl -A nie https://crates.io/api/v1/crates/<name>[/<version>/dependencies]` |
| S18 | Pingora graceful upgrade | `cloudflare/pingora` `docs/user_guide/graceful.md` |
