# ADR: GPUI is the system toolkit of the nie OS

Status: accepted, 2026-11-19. Amends `docs/adr-ui-stack.md`: Bevy keeps game rendering and assets;
everything else nie draws (explorer, viewers, wiki, terminal, desktop shell) is GPUI.

## Context

- zed-industries/zed v1.29.2 split GPUI into `gpui`, `gpui_platform`, `gpui_linux` (Wayland, X11,
  layer-shell), `gpui_windows`, `gpui_wgpu` (wgpu renderer, WGSL, cosmic-text), `gpui_web` (canvas,
  IME, dispatcher) and `gpui_tokio`, all Apache-2.0. None of the split crates is on crates.io; the
  crates.io `gpui` 0.2.2 is the old blade monolith, with no wgpu renderer and no web backend.
- Zed pins wgpu to a git revision. nie uses wgpu 30.0.2 from crates.io.
- GPUI carries AccessKit (`role`, `aria_*`, `on_a11y_action`, `debug_a11y_tree_json`), which nie's
  accessibility requirement (screen readers, high contrast, scaling) needs.

## Decision

1. `vendor/zed` is a submodule of the aphrody-labs/zed fork, branch `nie`. `cargo xtask zed-branch`
   regenerates it from upstream main: only the Apache-2.0 GPUI closure computed by `cargo metadata`
   on linux, windows-msvc and wasm32; every dependency from crates.io; no `[patch]`; wgpu 30.0.2.
2. nie crates take path dependencies on `vendor/zed/crates/gpui*` (the one fork with source
   dependencies, see AGENTS.md). They live in `gui/`, a workspace of its own, so a checkout without
   the submodule still builds the core.
3. `gui/nie-ui` is the toolkit: a theme derived from the game's `FONT_COLOR` palette, tree, table,
   tabs, nucleo picker, image and frame surfaces, a terminal surface (rio-vt as the VT parser), the
   explorer view, and the AccessKit audit. `gui/nie-apps` holds the apps.
4. The explorer draws the `nie_explore::fm` view model that `nie fm` also uses. The browser build
   cannot link `fm` (tree-sitter, host VFS), so it draws the same `nie_ui::explorer::Frame` from
   `nie serve` over HTTP (`gui/nie-apps/src/http_explorer.rs`).

## Consequences

- wgpu 30.0.2 lacks two fixes that the pinned revision has: gfx-rs/wgpu#10230 (Vulkan surface drop
  panic) and #10348 (WebGL timeouts on resize). The branch lists them in `nie-closure.json`; they
  return with the next wgpu release.
- Three source edits adapt `gpui_wgpu` to the released wgpu (`DeviceDescriptor::default_queue`,
  `TextureFormat::has_srgb_suffix`); the generator fails when upstream moves the text.
- The rule is dropped when zed publishes `gpui_wgpu` and `gpui_web` on crates.io.
