# nie: agent contract

nie is an ecosystem of original football games on Bevy, a native GUI (egui) and Dioxus, built as
Craft Anything jobs: a TOML manifest in `games/` selects shared engine crates and a frontend.
See `README.md` for the layout and the manifest format.

## Rules

- Rust 2024 workspace, versions read live from crates.io (`curl -A nie https://crates.io/api/v1/crates/<name>`). No vendored forks.
- Content is original. Never add names, characters, art, music or levels from an existing commercial game.
- `nie-sim` stays deterministic: integer RNG, `+ - * /` and `sqrt` only, stable entity order, no wall-clock time. A change that moves the pinned score or checksum in `crates/nie-sim/tests/headless.rs` is a gameplay change and must say so.
- New game behavior goes into engine crates and is selected through the manifest; do not fork a game into its own crate.
- Heavy builds run on the build host or in CI. Commit by path on `main`, no AI attribution, check `gh run list -R aphrody-labs/nie` after pushing.
- English, no emoji.
