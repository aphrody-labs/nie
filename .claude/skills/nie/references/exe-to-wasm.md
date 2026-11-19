# Workflow: nie.exe to wasm (task #18)

Goal: replace Wine/Proton by statically recompiling `nie.exe` against a Rust Win32/D3D11 runtime and run it in a
browser. Status is read from the code as of this page; re-check each "exists" line with `cargo check` and the commands
before building on it, and update this page when a step moves.

Boundaries that apply to every step:

- Run on the build host (`deploy/build.toml`: `NIE_BUILD_HOST`, `NIE_BUILD_DIR`) in tmux, in your own scratch
  directory, deleted at the end. Never on the PC.
- The input is the user's own `nie.exe`, read in place: a local path or `ssh://<host>/<abs path>` (`NIE_EXE`,
  `--exe`). Never copied into the repository, never committed, never shipped.
- Decompiled, lifted and recompiled output (`recomp.o`, `lift.json`, generated Rust or C) stays on the build host,
  outside the repository. Only the generator, a proof manifest and content hashes are committed
  (`docs/research/level5-questions.md`, questions 5 and 7: publishing derived code is an open licensing question).
- Record every measurement in `docs/research/log/YYYY-MM-DD-<topic>.md` with the environment fingerprint
  (`docs/research/README.md`). Do not write a coverage number that a command did not print this session.

## Steps

| # | step | recipe or command today | gate | status |
|---|---|---|---|---|
| 1 | index and RTTI | `nie index --exe E --db D`, `nie rtti`, `nie pdata`, `nie disasm`, `nie strings`, `nie rebuild`; no `just` recipe, proposed `just re-index` (open) | `nie coverage --db D` and `nie exe lint --db D` print numbers; `pdata_func`, `rtti_class` non-empty ([re-index.md](re-index.md)) | exists |
| 2a | measure the PE | `cargo run -p nie-recomp -- --exe E inventory --out F` (outside the repository) | JSON inventory is produced; unit tests of `pdata` and `source` pass: `cargo test -p nie-recomp` | exists |
| 2b | lift x86-64 to Cranelift and emit an object | `cargo run --release -p nie-recomp -- --exe E lift --out-dir O [--limit N] [--verify]` | the printed report: `percent_of_roots`, `failed`, `trap_stubs`, `failure_classes`; `--verify` runs the Cranelift IR verifier. Target ISA is hard-wired to x86_64-unknown-linux-gnu in `emit::host_isa` | exists, native object only |
| 2c | differential verification of lifted code against the original | none: `nie-recomp` has no `verify` command and no test that runs lifted code against the binary | a lifted function and the original return the same registers and memory on recorded inputs (`STYLE.md`: differential test against recorded outputs) | open |
| 2d | independent path: `nie-re` pipeline (`nie-re-core pipeline`, feature `pipeline`): acquire, identify, analyze, lift x86 PE to IR, emit C and Rust, rebuild with clang or rustc, verify | `nie-re-core pipeline ...`, `nie-re-core decompile` (feature `decompile`, Ghidra P-Code and Pseudo-C) | the pipeline report (`PipelineReport`, one `StageReport` per stage) has no failed stage | exists; wasm32 rebuild is wired only for console images (`pipeline/recomp.rs`), not for `nie.exe` |
| 3 | Rust Win32/D3D11 runtime | `plugins/exe/nie-win32`: `Cpu` register file, flat guest memory at the preferred image base, `Machine` dispatch for calls leaving the lifted code, `kernel32` import table (`imports::implemented()`); `cargo test -p nie-win32` | every import the lifted code calls resolves through `imports::lookup`; a Direct3D 11 call lands on a wgpu implementation checked against `docs/specs/d3d11.md` and `dxbc.md` | partial: kernel32 only, `cfg(unix)` modules. user32, gdi32, dxgi, d3d11, audio, input, DXBC to WGSL: open |
| 4 | wasm32 build | `just ci-wasm-core` (nie-formats, nie-data, nie-assets, nie-explore, nie-wiki-vfs, nie-render3d, nie-wasm), `just ci-gui-wasm` (GPUI apps), `just ci-targets wasm32-unknown-unknown check`; there is no `just ci-wasm` and no recipe for the recompiled game | the recipes pass; for the recompiled game: `cargo check --target wasm32-unknown-unknown` of the generated crate and of `nie-win32` with its `cfg(unix)` modules replaced | exists for the framework, open for the recompiled game |
| 5 | browser proof | `just web-pack`, `just web-serve`, `just web-shot <url> <png>` (headless Chrome: `navigator.gpu`, console errors, screenshot), `just ci-browser`; agent-browser for DOM probe and screenshot; `nie img diff` for the pixel comparison | screenshots inspected by eye, pixel difference across targets (`test-tui`, agent-desktop, agent-browser) within the agreed tolerance, and a scripted scene at 1280x720 with a frame time of at most 8.33 ms (120 fps); results in `docs/research/log` | exists for the GPUI apps; open for the recompiled game |

Steps 2c, 3 (beyond kernel32), 4 (recompiled game) and 5 (recompiled game) are the open work of task #18. Nothing in
this chain is "done" until its gate has run and the output was read.

## Design facts the code forces

- The guest address space is the host address space: `nie-win32` maps the PE image at its preferred base
  (`0x1_4000_0000` in `crates/nie-cli/src/main.rs`, `NIE_IMAGE_BASE`) and a guest pointer is a host pointer. A wasm32
  linear memory is 32-bit, so that base does not fit. A wasm32 target therefore needs guest-to-host address
  translation (or memory64) in the runtime; decide this before emitting code for it. Task #24 (qemu-wasm, wasm64 TCG) is
  the neighbouring effort.
- Cranelift has no wasm32 backend, so `nie-recomp`'s object path is native only. A wasm output comes from an emitter
  to Rust or C (the `nie-re` pipeline's `emit_rust`, `emit_c`) compiled with `rustc --target wasm32-unknown-unknown`, or
  from a wasm emitter that does not exist yet.
- `nie-forge` is a separate goal: reproduce the original binary byte for byte from Rust units and report the share
  generated by Rust (`nie-forge split|build|verify|report|match|unit`). It is not the wasm path; its `verify` compares two
  binaries, not lifted semantics.
- Online modes and anti-cheat stay off: nothing spoofs, patches or skips an EAC check (AGENTS.md).

## Gates for the browser step

- Pixel parity: render the same scripted scene on `test-tui`, agent-desktop (native) and agent-browser (wasm in Chrome),
  compare with `nie img diff` (identity, DeltaE2000, SSIM per region, difference map).
- Frame time: scripted scene at 1280x720, at most 8.33 ms per frame. CLI and decoder speed use
  `hyperfine --warmup 3 --runs 20 --export-json`; browser and wasm engine speed use JetStream.
- Both go to `docs/research/log` with the environment fingerprint, or the UI is not done (AGENTS.md).
