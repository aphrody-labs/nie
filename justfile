# nie: the one human and CI entrypoint. `just` lists the recipes.
#
# Heavy recipes dispatch to the build host of deploy/build.toml through `cargo xtask` (NIE_BUILD_HOST and
# NIE_BUILD_DIR override it; --local forces this machine). `just fr1 <recipe>` runs any recipe on the build
# host in a detached tmux session. The `ci-*` recipes are the jobs of .github/workflows, one recipe each.

set shell := ["bash", "-euo", "pipefail", "-c"]
set windows-shell := ["bash", "-euo", "pipefail", "-c"]

# Features of the shipped binaries: `host` is the full CLI (MCP, Lua, wiki, RE).
cli_features := "nie-cli/host,nie-model-serve/host"
# The framework packages that every target builds and tests.
framework := "-p nie-plugin -p nie-abi -p nie-bindgen -p nie-scaffold -p hello"

# List the recipes.
default:
    @just --list --unsorted

# fmt check, cargo check, the bevy feature check and the generated-file drift tests.
check *args:
    cargo xtask check {{ args }}

# clippy, cargo shear and gitleaks.
lint *args:
    cargo xtask lint {{ args }}

# rustfmt and taplo (--check to verify only).
fmt *args:
    cargo xtask fmt {{ args }}

# The CI test job, the whole workspace (--workspace), or coverage (--coverage).
test *args:
    cargo xtask test {{ args }}

# Release, musl, windows, wasm and crlib builds (--release --musl --windows --wasm --crlib --all).
compile *args:
    cargo xtask compile {{ args }}

# Regenerate MAP.tsv (--check fails when it is stale).
map *args:
    cargo xtask map {{ args }}

# Regenerate the command, MCP tool and plugin table of the nie skill (--check fails when it is stale).
skill *args:
    cargo xtask skill {{ args }}

# Generate the language bindings from a C ABI contract (--check fails on drift).
bindgen *args:
    cargo run --locked -p nie-cli --bin nie -- bindgen {{ args }}

# Two builds of HEAD in two directories, BLAKE3 of every artifact, docs/specs/repro-manifest.json.
repro *args:
    cargo xtask repro {{ args }}

# Build the browser app bundle that nie-app embeds.
app-pack out="dist/app" *args:
    cargo xtask app-pack --out {{ out }} {{ args }}

# PE import closure of the launch chain over a LiberNT build tree.
libernt-closure *args:
    cargo xtask libernt-closure {{ args }}

# Run a recipe on the build host in a detached tmux session, logged under its build directory.
fr1 recipe *args:
    cargo xtask remote {{ recipe }} {{ args }}

# Ship binaries and deploy/supervise.toml, start the supervisor and wait for health.
deploy *args:
    cargo run --release --locked -p nie-deploy -- up {{ args }}

# Kernel.org mainline watcher: the decision as JSON (exit 0 build, 10 up to date, 11 wait).
kernel-watch *args:
    cargo xtask kernel watch {{ args }}

# One build step (source, config, package, repro) of a mainline tag.
kernel-build step tag *args:
    cargo xtask kernel build {{ step }} {{ tag }} {{ args }}

# Boot test of a kernel build under QEMU/KVM.
kernel-boot-test *args:
    cargo xtask kernel boot-test {{ args }}

# One pipeline run: watch, signed tag, two builds, same image hash, boot test, record.
kernel-cycle *args:
    cargo xtask kernel cycle {{ args }}

# Promote the running kernel to the GRUB default once the host is healthy.
kernel-promote *args:
    cargo xtask kernel promote {{ args }}

# Reproducible initramfs: static busybox, the static musl nie, os/initramfs/init.
os-initramfs *args:
    cargo xtask os initramfs {{ args }}

# QEMU arguments for cargo osdk, from the host and NIE_OS_*.
os-qemu:
    cargo xtask os qemu

# Boot the Linux-ABI Asterinas kernel with nie in the initramfs and a game directory over virtiofs.
os-boot *args:
    cargo xtask os boot {{ args }}

# nie-os and nie-ostd-vfs with the nightly pinned in os/rust-toolchain.toml.
os-build:
    cd os/nie-os && cargo osdk build
    cd os/nie-ostd-vfs && cargo osdk build

# Kernel unit tests under QEMU (fr1 only).
os-test:
    cd os/nie-os && cargo osdk test

# Content-hashed browser bundle of the GPUI apps.
web-pack *args:
    cargo xtask web pack {{ args }}

# Serve a bundle and forward /v1 and /tex to a running `nie serve`.
web-serve *args:
    cargo xtask web serve {{ args }}

# Headless Chrome proof of a page: navigator.gpu, console errors, screenshot.
web-shot *args:
    cargo xtask web shot {{ args }}

# ---------------------------------------------------------------------------------------------------
# CI jobs. Each recipe is the whole job of a workflow: setup, build and test.
# ---------------------------------------------------------------------------------------------------

_toolchain:
    rustup show active-toolchain || rustup toolchain install

# The sibling checkout of the Zed fork that gui/ path-depends on (one checkout per fork).
_gpui:
    test -d ../aphrody/crates/gpui || git clone --depth 1 https://github.com/aphrody-labs/aphrody ../aphrody

# wasm-bindgen CLI at the version a Cargo.lock pins.
_wasm-bindgen lock="Cargo.lock":
    cargo install --locked wasm-bindgen-cli --version "$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/[^0-9.]/,"",$3); print $3; exit}' {{ lock }})"

# The pinned toolchain of a release job (RUSTUP_TOOLCHAIN), with an optional target.
_release-toolchain target="":
    rustup toolchain install "${RUSTUP_TOOLCHAIN}" --profile minimal --no-self-update {{ if target != "" { "--target " + target } else { "" } }}

# ci.yml native: synthetic CPK through the asset server, VFS and remote transports.
ci-native: _toolchain
    cargo xtask test --local

# ci.yml launcher: resolution order and stdio round trips against a fake nie-mcp, warnings denied.
ci-launcher: _toolchain
    cargo test --locked --manifest-path tools/nie-mcp-launch/Cargo.toml
    cargo clippy --locked --manifest-path tools/nie-mcp-launch/Cargo.toml --all-targets -- -D warnings

# ci.yml workspace: lint, bevy assets, dependency audit, xtask tests, MAP.tsv.
ci-workspace: _toolchain
    sudo apt-get update && sudo apt-get install -y --no-install-recommends pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libgl1-mesa-dev
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo check --locked -p nie-bevy --features bevy --all-targets
    cargo run --locked -p nie-cli --bin nie -- deps audit --check
    cargo test --locked -p nie-xtask
    cargo xtask map --check

# ci.yml os: the no_std core on bare metal and the OSDK ISO.
ci-os: _toolchain
    rustup target add x86_64-unknown-none
    cargo check --locked -p nie-formats --no-default-features --target x86_64-unknown-none
    sudo apt-get update && sudo apt-get install -y --no-install-recommends grub-pc-bin grub-efi-amd64-bin xorriso mtools
    cd os && (rustup show active-toolchain || rustup toolchain install)
    cargo install cargo-osdk --version 0.19.0 --locked
    just os-build

# ci.yml deny: aws-lc-sys, aws-lc-rs, ring, openssl-sys and libssh2-sys stay out of the graph.
ci-deny:
    cargo deny check bans

# app.yml wasm-core: the browser core builds for wasm32 with the host features off.
ci-wasm-core: _toolchain
    rustup target add wasm32-unknown-unknown
    cargo check --locked --target wasm32-unknown-unknown -p nie-formats --no-default-features --features std,serde,textures,audio-decode
    cargo check --locked --target wasm32-unknown-unknown -p nie-data
    cargo check --locked --target wasm32-unknown-unknown -p nie-assets --no-default-features
    cargo check --locked --target wasm32-unknown-unknown -p nie-explore --no-default-features
    cargo check --locked --target wasm32-unknown-unknown -p nie-wiki-vfs
    cargo check --locked --target wasm32-unknown-unknown -p nie-render3d --no-default-features
    cargo check --locked --target wasm32-unknown-unknown -p nie-wasm --features webgl
    cargo test --locked -p nie-formats --lib -- synthetic cpk_list byte_source

# app.yml browser: bundle, embed, serve a synthetic installation and prove the page in headless Chrome.
ci-browser out="dist/shots": _toolchain (_wasm-bindgen "Cargo.lock")
    rustup target add wasm32-unknown-unknown
    sudo apt-get update && sudo apt-get install -y --no-install-recommends binaryen
    cargo xtask app-pack --out dist/app
    NIE_APP_DIST="$PWD/dist/app" cargo build --locked --release -p nie-cli --no-default-features --features cloud --bin nie
    cargo run --locked -p nie-wiki-vfs --features testing --example synth_install -- "$PWD/dist/fixture" 50
    mkdir -p {{ out }}
    NIE_RAW_VFS_TOKEN="$(head -c 24 /dev/urandom | base64 | tr -d '/+=' | head -c 40)" NIE_GAME_SOURCE="$PWD/dist/fixture/install" NIE_PORT=8790 cargo xtask web shot http://127.0.0.1:8790/ {{ out }}/app.png --spawn "target/release/nie serve" --ready http://127.0.0.1:8790/ready

# gui.yml linux: GPUI snapshots and the AccessKit audit.
ci-gui-linux: _toolchain _gpui
    sudo apt-get update && sudo apt-get install -y libwayland-dev libxkbcommon-x11-dev libxcb1-dev libfontconfig-dev libfreetype-dev mesa-vulkan-drivers
    cargo clippy --locked --manifest-path gui/Cargo.toml --workspace --all-targets -- -D warnings
    cargo test --locked --manifest-path gui/Cargo.toml -p nie-ui --features test-support

# gui.yml windows: GPUI check on windows-msvc.
ci-gui-windows: _toolchain _gpui
    cargo check --locked --manifest-path gui/Cargo.toml -p nie-ui --all-targets

# gui.yml wasm: the browser build of the GPUI apps.
ci-gui-wasm: _toolchain _gpui
    cargo check --locked --manifest-path gui/Cargo.toml -p nie-ui -p nie-apps --lib --target wasm32-unknown-unknown

# release.yml version: the workspace version and the toolchain, written to $GITHUB_OUTPUT.
ci-release-version:
    #!/usr/bin/env bash
    set -euo pipefail
    version=$(awk '/^\[workspace.package\]/{p=1;next} /^\[/{p=0} p && $1=="version"{gsub(/"/,"",$3); print $3; exit}' Cargo.toml)
    toolchain=$(awk -F'"' '/^channel/{print $2; exit}' rust-toolchain.toml)
    test -n "$version" && test -n "$toolchain"
    if [ "${GITHUB_REF_TYPE:-}" = "tag" ] && [ "${GITHUB_REF_NAME:-}" != "v${version}" ]; then
      echo "tag ${GITHUB_REF_NAME} does not match workspace version ${version}" >&2
      exit 1
    fi
    printf 'version=%s\ntoolchain=%s\n' "$version" "$toolchain" >> "${GITHUB_OUTPUT:-/dev/stdout}"

# release.yml app: the browser app bundle (nie-wasm, wasm-bindgen, wasm-opt, BLAKE3 names, brotli).
ci-release-app: (_release-toolchain "wasm32-unknown-unknown") (_wasm-bindgen "Cargo.lock")
    sudo apt-get update && sudo apt-get install -y --no-install-recommends binaryen
    cargo xtask app-pack --out dist/app

# release.yml linux: static musl binaries with the app embedded (NIE_APP_DIST), archived.
ci-release-linux version: (_release-toolchain "x86_64-unknown-linux-musl")
    sudo apt-get update && sudo apt-get install -y --no-install-recommends musl-tools
    cargo build --locked --release --target x86_64-unknown-linux-musl --no-default-features -p nie-cli -p nie-model-serve -p nie-deploy --features "{{ cli_features }}" --bin nie --bin nie-model-serve --bin nie-deploy
    for b in nie nie-model-serve nie-deploy; do target/x86_64-unknown-linux-musl/release/$b --version; file target/x86_64-unknown-linux-musl/release/$b | grep -q "statically linked"; done
    cargo xtask release archive --name "nie-v{{ version }}-x86_64-unknown-linux-musl" --bin-dir target/x86_64-unknown-linux-musl/release --bins nie,nie-model-serve,nie-deploy

# release.yml windows: nie.exe and nie-model-serve.exe with the app embedded (NIE_APP_DIST), zipped.
ci-release-windows version: (_release-toolchain "")
    cargo build --locked --release --no-default-features -p nie-cli -p nie-model-serve --features "{{ cli_features }}" --bin nie --bin nie-model-serve
    ./target/release/nie.exe --version
    ./target/release/nie-model-serve.exe --version
    cargo xtask release archive --name "nie-v{{ version }}-x86_64-pc-windows-msvc" --bin-dir target/release --bins nie.exe,nie-model-serve.exe --zip

# release.yml release: checksums of the downloaded assets, then the GitHub Release (tags only).
ci-release-publish version:
    cd assets && sha256sum -- * > SHA256SUMS && cat SHA256SUMS
    gh release create "${GITHUB_REF_NAME}" assets/* --repo "${GITHUB_REPOSITORY}" --verify-tag --generate-notes --title "nie {{ version }}"

# repro.yml: two builds in two directories, the manifest written to the given path.
ci-repro manifest="dist/repro-manifest.json": _toolchain
    cargo xtask repro --manifest {{ manifest }}

# specs.yml: unit tests, sync of the official sources, commit of docs/specs when it changed.
ci-specs: _toolchain
    #!/usr/bin/env bash
    set -euo pipefail
    cargo test --locked -p nie-specs
    cargo run --locked --release -p nie-specs -- sync
    [ "${GITHUB_EVENT_NAME:-}" != "pull_request" ] || exit 0
    if git diff --quiet -- docs/specs; then
      echo "specs unchanged"
      exit 0
    fi
    git config user.name "nie-specs"
    git config user.email "nie-specs@users.noreply.github.com"
    git add docs/specs
    git commit -m "Specs sync: $(git diff --cached --name-only -- docs/specs | sed 's|docs/specs/||' | tr '\n' ' ')"
    git push origin HEAD:main

# targets.yml targets: the framework and the format readers on one target ("test" also runs the tests).
ci-targets target mode packages="": _toolchain
    {{ if target =~ "musl" { "sudo apt-get update && sudo apt-get install -y --no-install-recommends musl-tools" } else { "true" } }}
    rustup target add {{ target }}
    cargo check --locked --target {{ target }} {{ if packages == "" { framework } else { packages } }} -p nie-formats --no-default-features --features nie-formats/std,nie-formats/serde
    {{ if mode == "test" { "cargo test --locked --target " + target + " " + framework } else { "true" } }}
    {{ if target == "wasm32-unknown-unknown" { "cargo check --locked --target wasm32-unknown-unknown -p nie-wasm" } else { "true" } }}

# targets.yml nostd: the no_std crates that build on bare metal, alloc only.
ci-nostd: _toolchain
    rustup target add x86_64-unknown-none
    cargo check --locked -p nie-data --no-default-features --target x86_64-unknown-none

# targets.yml bindings: every generated binding against the cdylib of the contract.
ci-bindings: _toolchain
    cargo test --locked -p nie-bindgen
    cargo build --locked -p nie-ffi
    cargo xtask bindings-smoke --lib target/debug/libnie.so
