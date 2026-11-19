# FFI unification: nie, iecode-cpp, iecode-dotnet into one native library, one C ABI

Status: design, no code changed. Grounded in `aphrody-labs/nie` at `26fa0e4b4`, rosegriffon `a8ed41ad` (last tree with `apps/iecode-dotnet` and `tools/iecode-cpp`), and live lookups on 2026-11-19 (crates.io, `gh api`).

Open verifications (not settled by this document):

1. Byte-exact status of the Rust audio, USM, menu, Lua and DXBC modules against C#/C++ (step M4).
2. LLVM version of the pinned rustc 1.100.0 on fr1 (step M0). The workstation shows rustc 1.102.0-nightly with LLVM 23.1.5 and clang 23.1.5.
3. Whether the tree-sitter grammar crates build against tree-sitter 0.27.2 (step M1).
4. The Cloudflare Workers bundle size limit, checked live before shipping wasm (step M8).
5. Ownership of `contract/openapi.json`, which holds the whole Rose Griffon monorepo spec (coordinate with the rosegriffon agent).

## 0. Starting state and headline decision

Five C ABIs exist today, and none of them is complete:

| ABI | Location | Exports | Style |
|---|---|---|---|
| `nie_*` | `crates/nie-ffi/src/lib.rs` (3101 LOC, `[lib] name = "iecode"`, cdylib+rlib) | about 75 | Hand-written. Values returned by value or through `_out`. `NieBytes{ptr,len,cap}`. Thread-local `LAST_ERROR`. One `catch_unwind`, at line 661. |
| `rg_*` | `crates/rg-ffi` (generated from `crates/rg-ffi-contract/contract.json`, `abi_version 2`) | 1 + `rg_ffi_abi_version` + `rg_string_free` | Contract-first. Emits a C header, a Rust extern layer, bun:ffi bindings and an OpenAPI document. Drift test in `crates/rg-ffi/tests/in_sync.rs`. |
| `iecode_*` | `apps/iecode-dotnet/Ffi/Native/IecodeNative.cs` (NativeAOT, `NativeLib=Shared`) | 10 | `UnmanagedCallersOnly`. Returns an int status. Uses `NativeMemory`. A second bridge, `NodeApiBridge.cs` (node-api-dotnet JSExport). |
| C++ | `tools/iecode-cpp` (`iecode_core STATIC`, `src/include/iecode/export.h`) | no C ABI, C++ API only | vcpkg feature `iecode-cpp` pulls bgfx, capstone, directxtex, openssl, cpp-httplib, tree-sitter, zstd, libwebp and others. |
| wasm-bindgen | `crates/nie-wasm/src/lib.rs` | 108 `#[wasm_bindgen]` items | A separate surface over the same crates. |

Decision: the single library is the Rust crate `nie-ffi`, with library name `nie` and one generated C ABI.

- C++ is ported to Rust capability by capability. It is not linked in.
- .NET is ported where it still holds something unique. Otherwise it becomes a consumer of `nie` and a byte-exact oracle in CI.
- Linking C++ or NativeAOT code into the same binary is technically possible. Section 2.4 explains why it is the wrong trade, and what "full LLVM" and "full LTO" can honestly cover.

## 1. Capability inventory

"Reference" means the implementation that is most complete and has been validated against the game. Wherever Rust is the reference, the C# and C++ code become frozen oracles (pinned commits used only in CI diffs) and are then retired from production use.

| Capability | Rust (nie) | C# (iecode-dotnet @a8ed41ad) | C++ (iecode-cpp @a8ed41ad) | Reference | Retire / dedupe |
|---|---|---|---|---|---|
| Steam depot download | `plugins/steam/nie-steam/src/depot/*`, feature `depot` (steamroom 0.3.0). Port of `SteamDepotDownloader.cs`; 33 tests per the doc at a8ed41ad. | `Core/Steam/Content/*` (SteamKit2 3.4.0) | none (`modding/steam_helper.cpp` only finds the install) | Rust | C# becomes the oracle, then is retired. Not exposed to wasm. |
| CPK, @UTF, CRILAYLA, decrypt | `nie-formats/src/cpk.rs` (1773 LOC, 25 tests), `crilayla.rs` (942, 14), `cpk_encode.rs`. Validated byte-exact (300/300 g4tx per `docs/architecture-unifiee-ievr.md`). | `Formats/Criware/CriFs/*` (29 files), `Archives/CpkService.cs`, `Crypto/CriCryptoService.cs`, `Native/NativeCrypto.cs` | `formats/criware/{cpk_reader,cpk_writer,utf_parser}.cpp`, `compression/crilayla.cpp`, `crypto/cri_crypto.cpp` | Rust | Also delete the internal duplicate `nie-re/src/game/fs/{cpk,cpk_stream,cpk_extract,crilayla}.rs`; route `nie-re` through `nie-formats`. Bug to record in the C# oracle notes: `IecodeNative.CpkDecryptBlock` uses `0x1717E18E` as the default key. That is the cfg.bin key; pack keys are CRC32 of the filename (`cpk.rs` asserts this). |
| cfg.bin RDBN/T2B | `nie-formats/src/cfgbin.rs` (2631 LOC, 25 tests), `rdbn_patch.rs`, `t2b_patch.rs`, `nie-cfgsql` | `Formats/Level5/CfgBin/*`, `CfgBinTypesGenerator.cs` | `formats/level5/cfgbin.cpp`, `crypto/cfgbin_decrypt.cpp` | Rust | Drop `nie_decode_json`; keep only the out-param form. |
| G4TX / NXTCH / BCn decode | `g4tx.rs`, `g4tx_decode.rs` (image_dds 0.7, pure Rust), `nxtch.rs` (off-by-4 fixed; test `parse_header_offsets_alignes_sur_struct_csharp`) | `G4txParser.cs`, `NxtchParser.cs`, `TextureSwizzler.cs`, BCnEncoder.Net | `g4tx.cpp`, `nxtch.cpp`, `texture_swizzler.cpp`, `dds_parser.cpp`, DirectXTex | Rust (decode) | BCn encode (`textures-encode`, intel_tex_2 ISPC C++ objects, `nie-formats/build.rs`) is native-only and stays out of the musl, wasm and cloud features. |
| G4MD/G4MG/G4SK/G4PK/G4RA (+G4MT, G4CM, G4LA, G4MA, G4VS) | `g4md.rs`, `g4mg.rs`, `g4sk.rs`, `g4pk.rs`, `g4ra.rs`, and the others | `G4md/G4mg/G4sk/G4pk/G4raParser.cs` (`G4mgParser.cs:253` TODO) | `formats/level5/g4*.cpp`, plus g4bs, g4tg, g4nv | Rust | Port `g4bs.cpp` (no Rust found). `g4tg` exists only as probe examples (`nie-formats/examples/probe_g4tg*.rs`); promote it to a module. |
| Audio ADX/HCA/ACB/AWB | `cri_audio.rs` (1197 LOC: `adx_decode`, `acb_parse`, `acb_cues`, AWB, HCA through `cridecoder 0.3.7` behind `audio-decode`) | `Audio/AdxDecoder.cs`, `HcaDecoder.cs`, `Formats/Criware/{AcbReader,AwbReader}.cs` | `criware/{adx_decoder,acb_reader,awb_reader}.cpp` | Rust, pending an oracle diff | Only 3 unit tests in `cri_audio.rs`. ADX PCM16 must be diffed byte-exact against C# before C# is retired. |
| USM video | `usm.rs` (1517 LOC, 15 tests), `mp4.rs`, `webm.rs`, `ivf.rs` | `UsmDemuxer.cs`, `Converters/VideoConverter.cs` | `criware/usm_demuxer.cpp` | Rust | Transcoding (FFmpeg) is owned by Shenron (`tv.`) per the shared table, not part of this library. |
| Lua bytecode, VM, analysis | `nie-lua`: `bytecode.rs` (pure Rust, wasm-safe), feature `vm` (mlua 0.12 `lua52 vendored`), feature `analysis` (tree-sitter 0.27 + tree-sitter-lua 0.5) | `Formats/Lua/*`, `Scripting/Lua/*` (MoonSharp 2.0.0) | `third_party/lua-5.2.4` | Rust | MoonSharp is not the reference Lua 5.2 VM, so it is not a semantic oracle. |
| Menu G4PKM / OBJB | `menu.rs` (1538), `g4pkm.rs`, `g4pkm_motion.rs`, `objbin.rs` (1660), `menu_layout.rs`, `menu_screen.rs`, `menu_presentation.rs` | `Formats/Menu/{G4pkmLayout,G4pkmMotion,ObjbParser}.cs` | `g4pkm.cpp`, `objbin.cpp` | Rust | |
| DXBC / FXBIN shaders | `dxbc.rs` (213 LOC, 3 tests); fxbin referenced in 6 files | `Shader/DxbcReader.cs`, `Level5/FxbinParser.cs` | `vfx_inspector.cpp`, `mevbin.cpp` | Rust, thin | Diff the DXBC chunk list against C#. |
| MEVBIN, P3LIP, NAVM/G4NV, AGI, PXCL, COL | `mevbin.rs`, `lip.rs`, `navm.rs`, `col.rs`; PXCL and AGI referenced | `MevbinDocument.cs`, `P3lipParser.cs`, `NavmParser.cs`, `AgiParser.cs`, `PxclParser.cs` | `mevbin.cpp`, `p3lip.cpp`, `g4nv.cpp`, `agi.cpp`, `col.cpp` | Rust, per-format oracle diff | |
| Older-generation Level-5 compression (LZ10, Huffman, RLE, LZSS, Level5 compress, LZ4 block) | missing (0 hits for `lz10`/`Huffman`) | `Compression/{Lz10Decoder,Lz4Decoder}.cs`, AuroraLib.Compression | `compression/{lz10,huffman,rle,inazuma_lzss,level5_compress,lz4_block}.cpp` (+ tests) | C++, until ported | Port to `nie-formats/src/compress/*`. Needed for the DS/3DS games in scope (every Inazuma Eleven game). |
| DS ROM (NDS) and older-game formats | none | `Core/NdsSharp/*` (about 150 files), `NdsSharpPlugins` | `homebrew/{nds,3ds}` | C# | Large. Port later to a separate crate (`nie-nds`), or keep `iecode-dotnet` as a consumer. Not on the critical path. |
| `fnt`, `anm`, `map_block_list`, `tagged_bin`, `g4bs` | `font.rs`, `bitmap_font.rs`, `pose.rs` (partial) | n/a | `fnt_parser.cpp`, `anm_parser.cpp`, `map_block_list.cpp`, `tagged_bin.cpp`, `g4bs.cpp` | C++, until ported | Port each with byte-exact diffs on synthetic fixtures. |
| CDN on-demand (VFS read, menu PNG) | `nie-formats/src/{vfs,vfs_remote,vfs_manifest}.rs`, `nie serve` `/v1/vfs/read?decode=`, `nie-model-serve` (`mirror.rs`, `menu.rs`) | `Cdn/*` (`MenuPngService`, `CpkReaderPool`, `TextureManifestExport`). This is the current production CDN for `dx11/menu/**` PNG (image `ghcr.io/aphrody-labs/iecode-dotnet`, per rosegriffon AGENTS). | `vfs/vfs.cpp`, `services/dump_service.cpp` | C# in production today; Rust is the target | Before switching production and retiring `Cdn/*`, run a pixel-identical diff of the `dx11/menu/**` PNG export. |
| Game data (cfg.bin to typed entities) | `nie-data` (about 140 modules) | `GameData/*`, `Game/*` (7 types; delegated to inagle) | `gamedata/*.cpp` | Rust (`nie-data`) | Retire C# `GameData/` and C++ `gamedata/`. The triple stats implementation (TS, `nie-core`, `nie-data`) collapses to `nie-core`. |
| Format catalog | `nie_format_catalog_json_out` (nie-ffi), `nie-seed/src/format_catalog.rs` (ingest), `nie-seed/src/formats.rs` (hard-coded) | `Formats/FormatCatalog.cs` + `export-knowledge` (the original source) | `formats/format_detector.cpp`, `core/nie_re.cpp` (crypto constants table) | Rust, generated from the contract | Delete `nie-seed/src/formats.rs`. Import the C# `FormatCatalog` JSON once as a drift-test fixture, then stop following it live. |
| Viola (dump, pack, merge, crypto) | `nie-viola/src/*` | `Core/Viola/*` | `viola/{dump,merge,pack}.cpp` | Rust | |
| Ghidra index parsing (`nie.c`) | `nie-re`, `nie-index` | n/a | `decomp/src/nie_parser.cpp` | Rust | |
| Modding (GameBanana, profiles, conflicts), `pg_push` | `nie-cli/src/mod_cmd.rs` | Gui/IevrManager | `modding/*.cpp`, `db/pg_push.cpp` | n/a | Out of the FFI. Modding stays an app concern. `pg_push` duplicates inagle's push and should be retired. |

Not verified: byte-exact status of the Rust audio, USM, menu, Lua and DXBC modules. The capability matrix at a8ed41ad still lists them as "NON PORTE"; the current code has them, but no cross-implementation diff was found. Step M4 settles this.

## 2. Target architecture

### 2.1 Crates (in nie)

```
crates/nie-abi/            contract + generators (absorbs rg-ffi-contract)
  contract/nie.abi.json      single source: types, handles, enums, ops, HTTP mapping, JSON schemas
  src/lib.rs                 model + emitters (C, Rust, C#, C++, TS ffi, TS http, OpenAPI, wasm-bindgen)
  src/bin/nie-abi-gen.rs     `cargo run -p nie-abi --bin nie-abi-gen -- --write | --check`
crates/nie-ops/            safe Rust: one `fn op(&Ctx, In) -> Result<Out, NieError>` per contract op
                           no unsafe, no I/O in core features; `vfs`, `steam`, `lua-vm` features add hosts
crates/nie-ffi/            [lib] name = "nie", crate-type = ["cdylib","staticlib","rlib"]
  src/generated.rs           extern "C" wrappers (catch_unwind, arg validation, status mapping)
  src/runtime.rs             NieBytes, handles, last error, allocator boundary
crates/nie-abi-check/      drift and ABI tests (tree-sitter, cbindgen cross-check, layout asserts)
crates/nie-wasm/           keeps the game client; its byte-in/byte-out ops come from generated shims over nie-ops
crates/nie-cli/            `nie serve` routes generated from the contract's `http` block, dispatching to nie-ops
bindings/                  generated, committed, checked by `--check`
  c/include/nie.h
  cpp/include/nie.hpp        header-only RAII (unique handles, std::span, std::expected<_, nie::Error>)
  dotnet/Nie.Interop/        Nie.Interop.csproj, NativeMethods.g.cs (LibraryImport), SafeHandles, NieException
  ts/nie-ffi.ts              bun:ffi dlopen + typed wrappers
  ts/nie-http.ts             fetch client + types for `nie serve`
  openapi/nie.openapi.json
  schema/*.schema.json       JSON payload schemas (schemars, already a nie-cli dependency)
oracle/                    CI only, git submodules with relative URLs, pinned commits
  iecode-cpp    -> ../iecode-cpp.git
  iecode-dotnet -> ../iecode-dotnet.git
tools/oracle/diff.ts       runs nie vs oracle on synthetic fixtures, byte-compares outputs
```

Naming:

- The library is `nie`: `libnie.so`, `libnie.a`, `nie.dll`/`nie.lib`, `nie.wasm`.
- The symbol prefix is `nie_`, and domains follow `nie_<domain>_<verb>` (`nie_cpk_*`, `nie_g4tx_*`, `nie_audio_*`, `nie_rank_*`, and so on).
- The current `[lib] name = "iecode"` in `nie-ffi/Cargo.toml` is renamed.
- The old `iecode_*` and `rg_*` symbols survive for one minor release as generated aliases (contract `aliases: [...]`). They are removed in ABI 2.0.

### 2.2 C ABI rules (enforced by the generator)

- **Calling shape.** Every fallible op returns `nie_status` (`int32_t`), and results go to out-params. Nothing is returned by value, and no struct is passed in registers or through sret. This fixes the x86-64 sret problem with `NieBytes` documented for the `_out` variants in `nie-ffi/src/lib.rs`, and removes the by-value/`_out` duplicate pairs.
- **Status codes.** One enum, append-only: `NIE_OK=0`, `NIE_E_INVALID_ARG`, `NIE_E_INVALID_UTF8`, `NIE_E_NULL`, `NIE_E_UNSUPPORTED_FORMAT`, `NIE_E_DECODE`, `NIE_E_NOT_FOUND`, `NIE_E_IO`, `NIE_E_CRYPTO`, `NIE_E_LIMIT`, `NIE_E_PANIC`, `NIE_E_ABI`.
- **Error message.** `nie_last_error(nie_bytes *out)` returns a thread-local UTF-8 message. It generalizes the existing `LAST_ERROR`.
- **No panics or exceptions across the boundary.**
  - Each generated wrapper runs inside `std::panic::catch_unwind` and maps a panic to `NIE_E_PANIC`. This is required because a panic escaping an `extern "C"` function aborts the process on stable Rust.
  - An `ffi-release` profile keeps `panic = "unwind"`.
  - The wasm profiles keep `panic = "abort"`, and their shim maps errors to JS exceptions.
- **Ownership.**
  - `nie_bytes { uint8_t *ptr; size_t len; }` is backed by `Box<[u8]>`, so the `cap` field goes away. Free it exactly once with `nie_bytes_free(nie_bytes*)`; NULL or empty is a no-op.
  - Handles are opaque and typed: `nie_vfs*`, `nie_font*`, `nie_steam*`, `nie_lua*`. Each has a matching `nie_<h>_free`.
  - Static strings, such as format names, are marked `ownership: "static"` in the contract and are never freed.
  - The C++ and C# wrappers encode these rules in their type systems (RAII, SafeHandle).
- **Strings.** Inputs are `(const uint8_t*, size_t)` UTF-8, not NUL-terminated (friendlier to bun:ffi), and are validated, returning `NIE_E_INVALID_UTF8` on failure. Outputs are `nie_bytes` holding UTF-8.
- **Versioning.**
  - `uint32_t nie_abi_version(void)` returns `(major<<16)|minor`.
  - `nie.h` defines `NIE_ABI_VERSION_MAJOR` and `NIE_ABI_VERSION_MINOR`.
  - `nie_build_info(nie_bytes*)` returns JSON with the crate version, git sha, target, enabled features and LLVM version.
  - Adding an op bumps the minor version. Removing an op or changing a signature bumps the major version.
  - Bindings refuse to load on a major mismatch; the bun, .NET and C++ wrappers check this at init.
- **Threading.** Each handle declares `thread_safe` in the contract. `nie_vfs` is `Send + Sync`; it has an internal cache, sized with `nie_vfs_cache_budget`. The other handles are single-thread.
- **Struct layout.** Every `#[repr(C)]` struct (for example `NieCacheStats`) gets generated size and alignment asserts in both Rust and C (`_Static_assert`).

### 2.3 How C++ and .NET fold in

| Part | Fate | Why |
|---|---|---|
| iecode-cpp formats, compression, crypto, viola, gamedata | Port to Rust (the "C++ until ported" rows in section 1), then keep the C++ as an oracle | `iecode_core` depends on DirectXTex, bgfx, openssl, cpp-httplib, capstone, libwebp and tinygltf (`src/CMakeLists.txt`). Linking it would block musl, wasm32 and Workers, and would add a C++ runtime (the same `__gxx_personality_v0` problem `nie-formats/build.rs` works around for ISPC). The algorithms (LZ10, Huffman, RLE, LZSS, fnt, anm, g4bs) are small and pure, so porting costs less than linking. |
| iecode-cpp engine and game (`src/game/*`, `src/engine/*`) | Not part of the FFI. The football and game layer was removed from nie. | `IECODE_BUILD_ENGINE` and `IECODE_BUILD_GAME` are already `OFF`, with headers missing. |
| iecode-dotnet Core formats, Steam, Cdn, GameData | Port where unique, otherwise retire. C# becomes a pinned oracle in `oracle/iecode-dotnet`. The menu PNG CDN stays in production until its parity diff passes (M6). | Rust already covers almost all of it (section 1). |
| iecode-dotnet NdsSharp and GUI (Avalonia apps, IevrManager, SaveEditor, Lynx, MapEditor and the others) | Consumers. They load `nie` through the generated `Nie.Interop` (LibraryImport). A NativeAOT app can also link `libnie.a`/`nie.lib` statically through `<NativeLibrary Include=...>` + `<DirectPInvoke Include="nie">`, which gives one self-contained AOT binary with no dlopen. | This puts .NET in the host role and Rust inside it, not the other way round. |
| `IECODE.Ffi` (`IecodeNative.cs`, `NodeApiBridge.cs`) | Delete after the alias release. | Duplicates `nie_detect`, `nie_crc32`, crilayla and rdbn. Bun uses bun:ffi or HTTP, not node-api-dotnet. |

### 2.4 What "full LLVM" and "full LTO" can and cannot cover

**Rust and C in one binary: yes.**

- mlua's vendored Lua 5.2 is built by `cc`. Building it with `CC=clang CFLAGS=-flto=thin`, plus `RUSTFLAGS="-Clinker-plugin-lto -Clinker=clang -Clink-arg=-fuse-ld=lld"`, gives cross-language ThinLTO.
- On Windows MSVC, use clang-cl with `-flto=thin` and `lld-link`.
- Requirement: clang's LLVM major version must equal rustc's.
  - The workstation shows rustc 1.102.0-nightly with LLVM 23.1.5, and clang 23.1.5.
  - The pinned toolchain (`rust-toolchain.toml` = 1.100.0) must be checked on fr1 with `rustc +1.100.0 -vV` before choosing the clang version.
  - CI asserts the two majors are equal, and `nie_build_info` records them.

**Rust-only parts.** A new `ffi-release` profile uses `lto = "fat"` and `codegen-units = 1`. The current `release` profile is `thin`; `wasm-release` and `web-craft` are already `fat`.

**What LTO does not cover:**

- **ISPC objects** (intel_tex_2, feature `textures-encode`). They are prebuilt machine code, not bitcode, so they stay out of LTO and out of the cloud, musl and wasm features.
- **.NET NativeAOT.** Its code is produced by RyuJIT/ILC, not LLVM, so it never joins an LLVM LTO unit.
  - Live check: `dotnet/runtimelab` branch `feature/NativeAOT-LLVM` (last commit 2026-09-22) says in its README that it is an "experimental fork" and that "Currently, the only supported target is WebAssembly".
  - The official NativeAOT static-library documentation (`dotnet/samples` `core/nativeaot/NativeLibrary/README.md`) says shared libraries are preferred and that linked code must be built with compatible compilers. It also says "it's also not possible to mix multiple native AOT compiled static libraries within the same loadable module".
  - Linking a NativeAOT `.a` into `libnie` would also embed a second GC and runtime into every consumer, Bun included.

Therefore .NET stays out of the single binary. "Full LTO" applies to Rust plus C/clang only, and the documentation says exactly that.

**AOT.**

- Rust is AOT by construction.
- The .NET consumer apps use `PublishAot=true` and link `libnie.a` statically through DirectPInvoke.
- The C++ oracle builds with clang `-flto=thin` and has no link-time coupling to nie.

## 3. Type safety end to end

1. **One contract: `crates/nie-abi/contract/nie.abi.json`.** It extends the `rg-ffi-contract/contract.json` schema.
   - Types: `bool`, `i32`, `u32`, `u64`, `f64`, `bytes_in`, `str_in`, `bytes_out`, `json_out<Schema>`, `handle<T>`, `enum<T>`, `struct<T>`.
   - Each op has `name`, `doc`, `domain`, `args`, `out`, `errors`, `since`, `aliases`, `features`, `thread_safe`, `http` (method, path, body or query mapping) and `wasm: true|false` (pure byte ops allowed in Workers).
   - `json_out` points to a JSON Schema derived with schemars from the Rust output type, so JSON payloads are typed for TS consumers too.
2. **Generated, committed, and checked with `nie-abi-gen --check`.** This replaces `rg-ffi/tests/in_sync.rs`. The generated files are:
   - `nie-ffi/src/generated.rs`
   - `bindings/c/include/nie.h`
   - `bindings/cpp/include/nie.hpp`
   - `bindings/dotnet/Nie.Interop/NativeMethods.g.cs`: `[LibraryImport]`, one `SafeHandle` per handle, a `NieStatus` enum, `ReadOnlySpan<byte>` inputs, AOT- and trim-safe
   - `bindings/ts/nie-ffi.ts`: bun:ffi symbols with `FFIType`, and wrappers that free through `nie_bytes_free`
   - `bindings/ts/nie-http.ts`: request and response types plus a fetch client
   - `bindings/openapi/nie.openapi.json`
   - the `nie serve` route table
   - the wasm-bindgen shim for `wasm: true` ops
3. **Independent cross-check with cbindgen 0.29.4** (crates.io max stable, read live). cbindgen regenerates a header from `nie-ffi/src` alone, and `nie-abi-check` compares it to `nie.h` declaration by declaration. If the contract and the Rust code diverge, CI fails, even when both were edited by hand.
4. **tree-sitter for parsing and verification, not for codegen.** Crate versions read live: tree-sitter 0.27.2, tree-sitter-c 0.24.2, tree-sitter-cpp 0.23.4, tree-sitter-c-sharp 0.24.0, tree-sitter-typescript 0.23.2, tree-sitter-lua 0.5.0. Step M1 confirms each grammar builds against the tree-sitter 0.27 language ABI and pins any that lag.
   - **tree-sitter-c** parses `nie.h` and the cbindgen header into normalized signatures. These feed the diff in point 3 and an ABI snapshot (`bindings/c/abi.snapshot.json`). The snapshot diff enforces the semver rules of 2.2: a removal or type change without a major bump fails CI.
   - **tree-sitter-c-sharp** parses `NativeMethods.g.cs` and asserts that each `LibraryImport` matches the C types (sizes, `nint` vs `nuint`, out-params). It also parses `oracle/iecode-dotnet` to list the C# parser classes that have no Rust equivalent; this is the port-coverage report.
   - **tree-sitter-cpp** parses `nie.hpp` to check that every op is wrapped. It also parses `oracle/iecode-cpp/src/formats` and `oracle/iecode-cpp/src/compression` for the same port-coverage report, which drives the "until ported" rows of section 1 to zero.
   - **tree-sitter-typescript** parses `nie-ffi.ts` and `nie-http.ts`. It checks that every op is present, that each `FFIType` matches the C type, and that the HTTP client covers every `http` op.
   - **tree-sitter-lua** already powers the `nie-lua` feature `analysis`. It is unchanged and exposed as the op `nie_lua_analyze`.
5. **Runtime type checks.** Generated tests call every op with NULL, empty, invalid UTF-8 and truncated inputs, and expect a status, not a crash. The Rust wrappers run under Miri, and the C header test runs under ASan and UBSan.

## 4. AOT and cloud readiness

### Build matrix

Release and CI builds run on GitHub runners and on fr1 at `/srv/xbuild/nie`, never on the workstation.

| Target | Artifacts | Feature set |
|---|---|---|
| `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` | `libnie.so`, `libnie.a`, `nie` (cli/serve) | `host` (vfs, steam, lua-vm, audio-decode, textures; textures-encode optional) |
| `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` | `libnie.a`, static `nie` | `cloud` (pure Rust + vendored Lua C under clang; no ISPC) |
| `x86_64-pc-windows-msvc` | `nie.dll`, `nie.lib` (import lib), `nie_static.lib`, `nie.exe` | `host` |
| `wasm32-unknown-unknown` | `nie_ops.wasm` + wasm-bindgen JS | `wasm` (only `wasm: true` ops: detect, crc32, crilayla, cfgbin to json/sql, g4tx to png/rgba, utf table, lua bytecode listing, rank math) |

- The current `.github/workflows/release.yml` covers only linux-gnu x86_64 and Windows. The aarch64 and musl jobs are new.
- Every artifact gets a smoke test: load the library, check `nie_abi_version`, and run one op on a synthetic fixture.

### Container

- Keep `Dockerfile`.
- For the musl `nie` build, change the runtime base to `gcr.io/distroless/static`, or `scratch` plus CA certificates.
- The ssh and rclone clients stay only in the variant that mounts remote game sources (`NIE_GAME_SOURCE`).
- Build multi-arch images (`linux/amd64`, `linux/arm64`) and keep publishing to `ghcr.io/aphrody-labs/nie`.

### HTTP parity

- `nie serve` (`crates/nie-cli/src/serve_cmd.rs`, a hand-rolled router today) gets its `/v1/...` routes from the contract's `http` blocks. Each route dispatches into the same `nie-ops` function the C ABI calls, so the two surfaces cannot drift.
- Byte inputs travel as `application/octet-stream` bodies. Handles become server-side resources; the VFS is mounted from `NIE_GAME_SOURCE`.
- `bindings/openapi/nie.openapi.json` replaces `contract/openapi.json`, which today holds the whole Rose Griffon monorepo spec (title "Rose Griffon Monorepo Unified API"). That file is rosegriffon's, so it moves there by coordination rather than being overwritten.
- Rose Griffon (TypeScript, cloud) uses only `bindings/ts/nie-http.ts` through `RG_NIE_URL`. This matches nie AGENTS ("consumers such as rosegriffon call it over HTTP and never embed its Rust").
- Workers and edge use the wasm build for `wasm: true` ops only. Check the Workers bundle size limit live before shipping. Inputs are VFS bytes fetched from R2.

## 5. Migration steps

Each step is one or more logical commits on `main`, builds on fr1 or in CI, and comes with a proof. Fixtures are synthetic and built in temp directories; no game asset is ever committed.

- **M0. Toolchain pin.**
  - On fr1, record `rustc +1.100.0 -vV` and the matching clang.
  - Add a CI job that fails on an LLVM major mismatch.
  - Proof: the job output.
- **M1. `nie-abi` crate.**
  - Move `rg-ffi-contract` into `crates/nie-abi` and extend the type model (section 3.1).
  - Port the 8 existing `rg_*` ops unchanged, so the generator emits the same `rg_ffi.h`, `rg_ffi.ts` and OpenAPI as today.
  - Confirm the tree-sitter grammar compatibility.
  - Proof: output byte-identical to the committed `crates/rg-ffi/{include,bindings,openapi}`.
- **M2. `nie-ops`.**
  - Extract the bodies of the 75 `nie-ffi` exports into safe functions, with no behavior change; `nie-ffi` calls `nie-ops`.
  - Proof: the existing `nie-ffi` tests (about line 2959 onward) pass, and the golden JSON of `nie_format_catalog_json_out` is identical.
- **M3. Generated `nie` ABI v1.**
  - Describe every op in `nie.abi.json`.
  - Generate `nie-ffi/src/generated.rs` with `catch_unwind` and status codes.
  - Generate aliases for the old `nie_*` by-value/`_out` names, the `rg_*` names and the 10 `iecode_*` names.
  - Rename the lib to `nie` and add `staticlib`.
  - Proof:
    - `nie-abi-check` is green (cbindgen diff, tree-sitter checks, layout asserts).
    - A C test program links against `libnie.a` and `libnie.so` under ASan.
    - A bun:ffi smoke test (`bun test bindings/ts`) returns real decoded JSON from a synthetic CPK.
- **M4. Oracle harness.**
  - Add `oracle/iecode-cpp` and `oracle/iecode-dotnet` as submodules with relative URLs, pinned to the extraction commits now in `aphrody-labs/iecode-{cpp,dotnet}` (pushed 2026-11-18).
  - `tools/oracle/diff.ts` builds the oracles on fr1 (clang; `dotnet publish -p:PublishAot=true`).
  - It generates synthetic fixtures: CRILAYLA round-trip, @UTF tables, an ADX sine wave, USM with fake chunks, G4TX BCn blocks, NXTCH headers with trap fields, minimal menu G4PKM/OBJB files, and Lua 5.2 bytecode compiled from source.
  - It byte-compares the outputs of nie against C# and C++.
  - Proof: a per-format report under nie `docs/proofs/`. Each mismatch becomes an issue before its oracle is retired.
- **M5. Ports from C++.**
  - Port LZ10, Huffman, RLE, Inazuma LZSS, Level-5 compress and LZ4 block into `nie-formats/src/compress/`.
  - Then port `g4bs`, `fnt`, `anm`, `map_block_list` and `tagged_bin`, and promote `g4tg` from the probe examples to a module.
  - Each port lands with its own oracle diff, adds ops to the contract (a minor version bump), and shrinks the tree-sitter port-coverage report.
- **M6. Ports and parity from C#.**
  - Close any audio, DXBC or menu-PNG mismatch found in M4.
  - Run a pixel-identical diff of the `dx11/menu/**` PNG export (C# `MenuPngService` vs nie) against a local install, without committing outputs. Only then may the production CDN move from iecode-dotnet to nie.
  - Delete `nie-seed/src/formats.rs`; the catalog is generated from the contract.
- **M7. HTTP from the contract.**
  - Generate the `nie serve` routes and `bindings/openapi/nie.openapi.json`. `/health` and `/ready` stay hand-written.
  - Proof: real responses checked by content (decoded JSON fields, PNG dimensions), plus the existing `route_request` tests.
- **M8. wasm.**
  - Generate the wasm-bindgen shim for `wasm: true` ops in `nie-wasm`, and remove the duplicate hand-written exports there.
  - Proof: `cargo check --target wasm32-unknown-unknown -p nie-wasm`, plus a wasm test of crilayla and g4tx to png in workerd/`wrangler dev` on a synthetic fixture.
- **M9. Consumer bindings.**
  - Publish `Nie.Interop` (C#) and `nie.hpp`.
  - In their own repos: switch the iecode-dotnet GUI apps to `Nie.Interop` and delete `IECODE.Ffi`. This is coordination with those repos, not edits from nie.
- **M10. Build matrix and image.** Add the aarch64 and musl jobs, switch to the distroless/static image, build multi-arch, and run the smoke tests.
- **M11. ABI 2.0.**
  - Remove the `iecode_*` and `rg_*` aliases and the old `nie_*` by-value variants.
  - Delete `crates/rg-ffi` and the `nie-re/src/game/fs` duplicates.
  - Freeze the oracles: CI only, with no production consumers.

### Risks

- rustc and clang LLVM versions can drift (guarded by M0).
- tree-sitter grammar crates may lag the 0.27 core ABI (pin them in M1).
- mlua's vendored C under musl with cross-LTO may need `-Clinker-plugin-lto` disabled for that target only.
- `cridecoder` HCA output may not be bit-identical to the C# path, which used vgmstream-cli per the doc. The target then becomes PCM within a tolerance, or vgmstream itself as the oracle.
- The NdsSharp scope (about 150 C# files) has no Rust equivalent, so iecode-dotnet stays alive for DS-era games.
- `contract/openapi.json` ownership needs the rosegriffon agent.
- `catch_unwind` does not catch aborts coming from C. mlua contains Lua's `longjmp`.
- NativeAOT static linking in .NET consumers requires a compatible MSVC/clang C runtime, per the official documentation.
- The production menu CDN is iecode-dotnet today, so switching it before M6 parity would regress `cdn.rosegriffon.fr`.

## 6. Files and crates

### Create

- `crates/nie-abi/{Cargo.toml,contract/nie.abi.json,src/lib.rs,src/bin/nie-abi-gen.rs}`
- `crates/nie-ops/{Cargo.toml,src/lib.rs,src/<domain>.rs}`
- `crates/nie-abi-check/{Cargo.toml,tests/*.rs}`
- `crates/nie-ffi/src/{generated.rs,runtime.rs}`
- `crates/nie-formats/src/compress/{lz10,huffman,rle,lzss,level5,lz4_block}.rs`
- `crates/nie-formats/src/{g4bs,g4tg,anm,map_block_list,tagged_bin}.rs`
- `bindings/{c,cpp,dotnet,ts,openapi,schema}/...`
- `oracle/iecode-cpp`, `oracle/iecode-dotnet` (submodules, relative URLs)
- `tools/oracle/diff.ts`
- `.github/workflows/ffi.yml` (matrix, oracle diff, abi-check)
- `docs/ffi-unification.md` (this document)

### Change

- `Cargo.toml` (`ffi-release` profile, workspace members)
- `crates/nie-ffi/Cargo.toml` (lib name `nie`, add `staticlib`)
- `crates/nie-ffi/src/lib.rs` (shrinks to module wiring)
- `crates/nie-cli/src/serve_cmd.rs` (generated routes)
- `crates/nie-wasm/src/lib.rs` (generated byte ops)
- `plugins/exe/nie-re/src/game/mod.rs` (use the `nie-formats` CPK)
- `Dockerfile`
- `.github/workflows/release.yml`
- `README.md`

### Delete

- `crates/rg-ffi/` and `crates/rg-ffi-contract/`, merged into `nie-abi` and `nie-ffi`
- The hand-written exports in `crates/nie-ffi/src/lib.rs`
- `plugins/exe/nie-re/src/game/fs/{cpk.rs,cpk_stream.rs,cpk_extract.rs,crilayla.rs}`, after a caller check
- `plugins/exe/nie-seed/src/formats.rs`
- `contract/openapi.json`, moved to rosegriffon by coordination

In the other repos, through their own agents:

- `IECODE.Ffi` in iecode-dotnet.
- C++ `formats/`, `compression/` and `gamedata/` in iecode-cpp, once frozen as oracles.

## 7. Licensing questions for the Level-5 partner admin

No licensing decision is taken here. These questions are listed for review:

1. Is distributing a library that decrypts and decodes Level-5 and CRI formats (CPK, cfg.bin, HCA with the IEVR key, USM) acceptable in binary form (cdylib, static library, container image, wasm on public Workers)? Is it acceptable in source form?
2. May the keys and constants (`0x1717E18E`, the IEVR HCA key, the CRC32 key derivation) appear in a public header, contract or generated binding? Or must they stay server-side in `nie serve` only?
3. May the wasm build that decodes bytes run in end users' browsers, or must decoding stay behind the self-hosted service?
4. May synthetic fixtures that mimic format layouts (no game bytes) be published in a public repository?
5. Licensing of the combined code:
   - May the oracle repos be submoduled into nie? `iecode-dotnet` carries an MIT `LICENSE.upstream` ("Yohan Pierre" + "Aphrody Labs").
   - The nie workspace is `MIT OR Apache-2.0`, while `nie-ffi` and `rg-ffi` declare `Apache-2.0` only. Which license do the unified `nie` library and its generated bindings carry?
   - GitHub reports `NOASSERTION` for `iecode-dotnet`, `iecode-cpp` and `nie`.
6. Vendored third-party code:
   - The components are `third_party/lua-5.2.4` (MIT, iecode-cpp), mlua's vendored Lua, `cridecoder`, intel_tex_2/ISPC, `steamroom`, and the NativeAOT runtime redistributed in .NET consumers.
   - Are there attribution or notice requirements for the combined binary and the container image?
7. May the Steam depot download op (App 2799860) ship in the public library, or only in the self-hosted `host` build?
8. For ported NdsSharp and DS/3DS-era decoders: the same questions as 1 and 2, for the older games.
9. Does the metadata/reference-only rule in rosegriffon AGENTS allow on-demand decoded menu PNGs served from `cdn.rosegriffon.fr`? This applies to both the current iecode-dotnet CDN and a future nie one.
