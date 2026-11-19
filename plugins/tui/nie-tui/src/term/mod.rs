//! `nie term`: a GPU terminal window built on the rio crates that hosts `nie fm` or any command.
//!
//! | Part | Crate |
//! |---|---|
//! | VT parser, grid, scrollback, graphics protocols | `rio-vt`, through the `nie_explore::term` model |
//! | pty (Unix pty, Windows ConPTY) and its poller | `teletypewriter`, `corcovado` (re-exported by `rio-vt`) |
//! | window and events | `rio-window` |
//! | text and image drawing | `sugarloaf` on wgpu |
//!
//! The model has no window or GPU dependency, so a GPUI view can take over the window later and
//! keep the same model, scene and pty code.

mod cpu;
mod keys;
mod pty;
mod scene;
mod window;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use nie_explore::term::TermModel;
use sugarloaf::font::FontLibrary;
use sugarloaf::font::fonts::SugarloafFonts;

pub use pty::{CommandSpec, PtyEvent, PtyHandle, PtySize};

/// Vertical offset of a glyph inside its cell in logical pixels: sugarloaf's text recorder takes
/// the top of the line box.
pub(crate) const TEXT_DY: f32 = 0.0;

/// Options of `nie term`.
#[derive(Clone, Debug)]
pub struct TermOptions {
    pub cols: u16,
    pub rows: u16,
    pub font_size: f32,
    /// Render one frame offscreen to this PNG and exit.
    pub screenshot: Option<PathBuf>,
    /// Seconds to let the child run before the screenshot.
    pub wait: f32,
    /// The command to run; empty means `nie fm --protocol kitty` with the current executable.
    pub command: Vec<String>,
}

/// The default child: the current executable running the explorer, with its pictures going
/// through the kitty graphics protocol of this terminal.
pub fn default_command() -> Result<CommandSpec> {
    let exe = std::env::current_exe().context("locate the current executable")?;
    Ok(CommandSpec {
        program: exe.to_string_lossy().into_owned(),
        args: vec!["fm".into(), "--protocol".into(), "kitty".into()],
        cwd: None,
    })
}

fn command_spec(command: &[String]) -> Result<CommandSpec> {
    match command.split_first() {
        None => default_command(),
        Some((program, args)) => Ok(CommandSpec {
            program: program.clone(),
            args: args.to_vec(),
            cwd: None,
        }),
    }
}

/// Runs `nie term`.
pub fn run(opts: &TermOptions) -> Result<()> {
    if opts.cols < 2 || opts.rows < 1 {
        bail!("--cols must be at least 2 and --rows at least 1");
    }
    let command = command_spec(&opts.command)?;
    match &opts.screenshot {
        Some(png) => screenshot(opts, command, png),
        None => window::run(window::WindowOptions {
            cols: opts.cols,
            rows: opts.rows,
            font_size: opts.font_size,
            command,
        }),
    }
}

/// Runs the child on a pty for `opts.wait` seconds with the same model and scene as the window,
/// draws one frame on the CPU and writes it as PNG.
fn screenshot(opts: &TermOptions, command: CommandSpec, png: &std::path::Path) -> Result<()> {
    let (fonts, _missing) = FontLibrary::new(SugarloafFonts {
        size: opts.font_size,
        ..SugarloafFonts::default()
    });
    let metrics = sugarloaf::layout::compute_cell_metrics(&fonts, opts.font_size, 1.0, 1.0).1;
    let (cell_w, cell_h) = (metrics.cell_width.max(1), metrics.cell_height.max(1));
    let (cols, rows) = (usize::from(opts.cols), usize::from(opts.rows));
    let mut model = TermModel::new(cols, rows);
    model.resize(cols, rows, cell_w, cell_h);
    let size = PtySize {
        cols: opts.cols,
        rows: opts.rows,
        width: (u32::from(opts.cols) * cell_w) as u16,
        height: (u32::from(opts.rows) * cell_h) as u16,
    };
    let pty = PtyHandle::spawn(&command, size, Arc::new(|| {}))?;
    let deadline = Instant::now() + Duration::from_secs_f32(opts.wait.max(0.0));
    let mut exited = false;
    while Instant::now() < deadline && !exited {
        match pty.events.recv_timeout(Duration::from_millis(50)) {
            Ok(PtyEvent::Output(bytes)) => model.feed(&bytes),
            Ok(PtyEvent::Exited(_)) => exited = true,
            Err(_) => {}
        }
        let replies = model.take_replies();
        if !replies.is_empty() {
            pty.write(replies);
        }
    }
    // Output queued before the exit notice is already in the channel.
    while let Ok(event) = pty.events.try_recv() {
        if let PtyEvent::Output(bytes) = event {
            model.feed(&bytes);
        }
    }
    let frame = model.frame();
    let (width, height) = (cols as u32 * cell_w, rows as u32 * cell_h);
    let mut canvas = cpu::CpuCanvas::new(width, height, frame.default_bg, &fonts, opts.font_size, TEXT_DY);
    let layout = scene::Layout {
        cell_w: cell_w as f32,
        cell_h: cell_h as f32,
        origin_x: 0.0,
        origin_y: 0.0,
    };
    scene::draw_frame(
        &mut canvas,
        &frame,
        &layout,
        scene::Look {
            focused: true,
            cursor_visible: true,
        },
    );
    let (w, h, rgb) = canvas.finish();
    cpu::write_png(png, w, h, &rgb)?;
    println!(
        "{}: {w}x{h} px, {cols}x{rows} cells of {cell_w}x{cell_h}, title {:?}, {} image placement(s), child {}",
        png.display(),
        frame.title,
        frame.placements.len(),
        if exited { "exited" } else { "running" }
    );
    Ok(())
}
