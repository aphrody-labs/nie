# nie-wasm

WebAssembly bindings (wasm-bindgen, version pinned in the workspace) for the nie format readers
and the `nie-data` and `nie-explore` decoders. The exports are described in the crate docs
(`src/lib.rs`).

## Build and output location

Run on the build host (heavy build, sccache), never on the Windows workstation:

```sh
cargo build -p nie-wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/nie_wasm.wasm \
    --out-dir crates/nie-wasm/pkg --target web
```

The package lands in `crates/nie-wasm/pkg/` (`nie_wasm.js`, `nie_wasm_bg.wasm`,
`nie_wasm.d.ts`). The wasm-bindgen CLI version must match the workspace pin exactly. The package
never enters git.

## Features

| Feature   | Adds                                                              |
| --------- | ----------------------------------------------------------------- |
| `webgpu`  | `WebGpuViewer`: the `nie-render3d` WebGPU pass on a canvas        |
| `webgl`   | WebGL2 fallback for browsers without WebGPU (adds about 2 MiB)    |
| `bevy`    | Bevy asset and plugin integration of `nie-bevy`                   |
