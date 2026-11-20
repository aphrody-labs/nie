# 2026-11-20: dependency audit

## Question

What does nie.exe import, what does the production kernel provide, which external programs does nie's own code still start, and which crates carry the VFS, Steam and edge?

## Method

```sh
nie deps audit --game-source <uri> [--kernel-config <path-or-uri>]   # writes this file and its JSON
nie deps audit --check                                              # CI: call sites and tracked crate versions only
```

Everything is read in process: the PE import table (goblin), printable strings of the binary, `config.gz`, the AST of every Rust file under the policy roots (syn) and `Cargo.lock`. The policy (`docs/specs/dependency-audit.policy.toml`) names the launch chain, the kernel options, the tracked crates and the reason each kept process call is kept.

## Environment

- Generated: 2026-11-20T18:59:54Z
- nie 2.0.0 at commit `9d1cdbd4271836e161ca7becb4fd3f281f7affb6`
- Toolchain channel: 1.100.0
- `Cargo.lock` sha256: `00df11c51b88455d9e0d2b4e5b72f1ee531b86adb7f0f95e3dc8c59781635588`
- Host `vps-203bea89`, linux x86_64, kernel 7.3.0-10-generic, 12 CPUs, Intel Core Processor (Haswell, no TSX), 47033 MiB

## External processes started by nie's code

985 Rust files scanned, 50 `Command::new` sites (25 outside tests).

| Program | Sites | Status |
|---|---|---|
| `bash` | 1 | not reviewed (`crates/nie-rgdeploy/src/main.rs:125`) |
| `dynamic` | 1 | not reviewed (`crates/nie-deploy/src/supervise.rs:204`) |
| `dynamic(bin)` | 2 | not reviewed (`crates/nie-formats/src/vfs_remote.rs:97`, `plugins/steam/nie-steam/src/depot/token_backend.rs:414`) |
| `dynamic(bin_name)` | 1 | not reviewed (`crates/nie-formats/src/vfs_remote.rs:81`) |
| `dynamic(cargo)` | 1 | not reviewed (`tools/nie-mcp-launch/src/main.rs:205`) |
| `dynamic(cl)` | 2 | MSVC compiler driven by the forge to rebuild nie.exe byte for byte; no Rust equivalent exists (`plugins/exe/nie-forge/src/cc.rs:178`, `plugins/exe/nie-forge/src/cc.rs:205`) |
| `dynamic(exe)` | 3 | not reviewed (`crates/nie-deploy/src/supervise.rs:601`, `crates/nie-edge/src/reload.rs:34`, `crates/nie-link/src/startup.rs:70`) |
| `dynamic(executable)` | 1 | not reviewed (`crates/nie-explore/src/cinema/browser_video.rs:124`) |
| `dynamic(program)` | 2 | not reviewed (`plugins/exe/nie-re/src/pipeline/util.rs:91`, `tools/nie-mcp-launch/src/main.rs:240`) |
| `dynamic(shell)` | 1 | not reviewed (`crates/nie-sshd/src/server.rs:235`) |
| `dynamic(vswhere)` | 1 | locates the MSVC installation for the forge (`plugins/exe/nie-forge/src/cc.rs:122`) |
| `ffmpeg` | 1 | not reviewed (`crates/nie-render3d/src/video.rs:236`) |
| `git` | 3 | not reviewed (`crates/nie-cli/src/exe_lint.rs:505`, `crates/nie-rgdeploy/src/main.rs:197`, `crates/nie-rgdeploy/src/main.rs:205`) |
| `powershell` | 1 | Windows host inventory in nie-specs, run only when the target host is Windows (`crates/nie-specs/src/hosts.rs:271`) |
| `sh` | 1 | not reviewed (`crates/nie-specs/src/hosts.rs:86`) |
| `ssh` | 1 | not reviewed (`crates/nie-rgdeploy/src/main.rs:129`) |
| `strace` | 2 | measurement tool of the Asterinas syscall audit; it records what a workload calls (`crates/nie-syscalls/src/report.rs:103`, `crates/nie-syscalls/src/workload.rs:97`) |

In tests: `crates/nie-abi/tests/crlib.rs:15` dynamic, `crates/nie-cli/tests/mcp_re_real.rs:125` dynamic, `crates/nie-cli/tests/mcp_stdio_smoke.rs:33` dynamic, `crates/nie-deploy/src/nftables.rs:143` nft, `crates/nie-explore/src/cinema/browser_video.rs:406` dynamic(current_exe()), `crates/nie-lua/tests/aot_object.rs:66` cc, `crates/nie-render3d/src/video.rs:380` ffmpeg, `crates/nie-rgdeploy/src/main.rs:325` dynamic(unwrap_or_else()), `crates/nie-rgdeploy/src/main.rs:334` dynamic(unwrap_or_else()), `crates/nie-syscalls/src/tests.rs:60` strace, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:12` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:26` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:50` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:58` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:71` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:99` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:115` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:148` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:161` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:180` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:193` dynamic, `plugins/exe/nie-dbxv2-re/tests/inventory_cli.rs:205` dynamic, `plugins/exe/nie-re/src/pipeline/util.rs:346` dynamic(current_exe()), `tools/nie-mcp-launch/tests/stdio.rs:62` dynamic(LAUNCHER), `tools/nie-mcp-launch/tests/stdio.rs:209` kill.

## Cargo.lock

1336 packages: 1264 from a registry, 16 from git, 56 local.

| Crate | Versions |
|---|---|
| steamroom | 0.3.0 |
| steamroom-client | 0.3.0 |
| russh | 0.65.2 |
| russh-sftp | 3.0.1 |
| opendal | 0.59.5 |
| pingora | absent |
| blake3 | 1.8.7 |
| bevy | absent |
| wgpu-types | 30.0.2 |
| rusqlite | 0.43.0 |
| wasm-bindgen | 0.2.130 |
| syn | 1.0.109, 2.0.119, 3.0.10 |
| goblin | 0.10.7 |
| iced-x86 | 1.22.0 |

## Limits

- Imports are the static import table; delay-loaded DLLs and COM vtable calls (D3D11) are not in it.
- Call sites are `Command::new` paths seen by the parser; processes started through other APIs, macros or shell strings are not counted.
- Interface strings are matched by shape, not by the call sites that use them.
