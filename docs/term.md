# nie term: the GPU terminal window

`nie term` opens a window that hosts `nie fm` (default) or any command, on the rio crates. Images
reach the explorer through the terminal's own kitty graphics support, so `nie fm` runs with
`--protocol kitty` inside it.

```sh
cargo run -p nie-cli --features term -- term                         # nie fm in a window
nie term --cols 140 --rows 40 --font-size 18
nie term -- htop                                                      # any command after --
nie term --screenshot fm.png --wait 8                                 # one frame to PNG, no display needed
```

`term` is opt-in (`nie-cli/term`, `nie-tui/term`, `nie-explore/term`): sugarloaf's build script needs
`glslc` or `glslangValidator` on Linux. `nie plugins` lists the `term` command when the feature is on.

## Layers

| Layer | Where | What |
|---|---|---|
| Model | `crates/nie-explore/src/term.rs` (feature `term`) | `TermModel`: bytes in, `Frame` out (cells with resolved colours and attributes, cursor, scrollback viewport, title, image placements as RGBA with cell and pixel rectangles), selection, and the replies the terminal owes the child |
| Input encoders | `crates/nie-explore/src/term/input.rs` | keys (xterm, kitty keyboard protocol flags 1, 2, 4, 8, 16), mouse (X10, normal, drag, motion; UTF-8 and SGR), bracketed paste, focus, alternate scroll |
| Scene | `plugins/tui/nie-tui/src/term/scene.rs` | a `Frame` to drawing calls on a `Canvas` trait (rect, text, image) |
| pty | `plugins/tui/nie-tui/src/term/pty.rs` | child on a Unix pty or Windows ConPTY (`teletypewriter`), driven by a `corcovado` poll thread |
| Window | `plugins/tui/nie-tui/src/term/window.rs` | rio-window event loop, sugarloaf on wgpu as the `Canvas`, clipboard, resize, close |
| Screenshot | `plugins/tui/nie-tui/src/term/cpu.rs` | the same scene on a CPU `Canvas` (sugarloaf's text rasteriser), written as PNG |

The model depends on `rio-vt` only (default features off, `graphics` on): no sugarloaf, rio-window,
winit, GPU API or pty. A GPUI window replaces `window.rs` and keeps the model, the scene and the pty.

## What rio provides, and what nie takes

| Crate (0.5.32) | Used | Role |
|---|---|---|
| `rio-vt` | yes | `Crosswords` grid, scrollback, selection, `Processor` parser, kitty graphics, sixel, iTerm2, kitty keyboard state, `TermColors`; features `graphics` (model), `pty`, `clipboard`, `x11` (window) |
| `rio-graphics` | yes | `GraphicData`, `ColorType`, image key helpers (leaf crate under rio-vt) |
| `teletypewriter`, `corcovado` | yes, through `rio-vt/pty` | pty and poller |
| `rio-window` | yes | winit fork: event loop, keyboard, IME, raw window handles |
| `sugarloaf` | yes (`wgpu` feature) | `Text` (shaping, glyph atlas, CPU and wgpu rasteriser), rect quads, image overlays |
| `rio-backend` | no | adds rio's user config, themes and the application glue; nie needs only what it re-exports from rio-vt |
| `rioterm` | no | the Rio application (tabs, splits, island, command palette, hints, config files, audio, global hotkeys) and a binary |

## Limits

- Kitty Unicode-placeholder (virtual) placements are not drawn.
- Images draw under the text, whatever their `z_index`.
- The offscreen path is CPU only: sugarloaf's GPU contexts are bound to a window surface, so there is
  no headless GPU frame to read back.
