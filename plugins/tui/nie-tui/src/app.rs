//! `nie fm`: the interactive loop and the headless snapshot.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use nie_explore::fm::keymap::Keymap;
use nie_explore::fm::thumbs::ThumbCache;
use nie_explore::fm::Action;
use nie_formats::vfs::Vfs;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{self, Event};
use ratatui_image::picker::{Picker, ProtocolType};
use tokio::sync::mpsc::unbounded_channel;

use crate::session::{Sample, Session, from_crossterm};

/// Where the user keymap lives.
pub const KEYMAP_ENV: &str = "NIE_FM_KEYMAP";

/// The options of `nie fm`.
#[derive(Debug, Clone, Default)]
pub struct FmOptions {
    /// Game location (`ssh://`, `rclone://`, a path); `NIE_GAME_SOURCE` and discovery otherwise.
    pub source: Option<String>,
    /// A keymap merged over the default one.
    pub keymap: Option<PathBuf>,
    /// `kitty`, `sixel`, `iterm2` or `halfblocks`; detected from the terminal otherwise.
    pub protocol: Option<String>,
    /// Do not read or write the thumbnail cache.
    pub no_cache: bool,
    /// Open this directory first.
    pub cd: Option<String>,
    /// Put the cursor on this file first.
    pub select: Option<String>,
    /// Render one frame of this size (`120x40`) to standard output, without a terminal.
    pub snapshot: Option<String>,
    /// Write the preview timings as JSON to this file when the session ends.
    pub metrics: Option<PathBuf>,
}

/// Mounts the game through `nie-assets`.
///
/// # Errors
/// Fails when no game source can be mounted.
pub fn open_vfs(source: Option<&str>) -> Result<Arc<Vfs>> {
    let mut config = nie_assets::AssetsConfig::from_env().context("assets configuration")?;
    if let Some(source) = source {
        config.source = source.to_owned();
    }
    let vfs = config.open_vfs().context("mount the game")?;
    Ok(Arc::new(vfs))
}

fn keymap(options: &FmOptions) -> Result<Keymap> {
    let mut keymap = Keymap::builtin();
    let user = options
        .keymap
        .clone()
        .or_else(|| std::env::var_os(KEYMAP_ENV).map(PathBuf::from))
        .or_else(|| dirs_config().map(|d| d.join("nie").join("fm-keymap.toml")).filter(|p| p.is_file()));
    if let Some(path) = user {
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        keymap.merge(Keymap::from_toml(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?);
    }
    Ok(keymap)
}

fn dirs_config() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

fn picker_for(options: &FmOptions, query: bool) -> Picker {
    let mut picker = if query {
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
    } else {
        Picker::halfblocks()
    };
    if let Some(name) = options.protocol.as_deref() {
        let kind = match name {
            "kitty" => Some(ProtocolType::Kitty),
            "sixel" => Some(ProtocolType::Sixel),
            "iterm2" => Some(ProtocolType::Iterm2),
            "halfblocks" => Some(ProtocolType::Halfblocks),
            _ => None,
        };
        if let Some(kind) = kind {
            picker.set_protocol_type(kind);
        }
    }
    picker
}

fn cache_for(vfs: &Vfs, options: &FmOptions) -> Option<Arc<ThumbCache>> {
    if options.no_cache {
        return None;
    }
    match ThumbCache::open(Some(vfs.game_data_dir())) {
        Ok(cache) => {
            let pruned = cache.clone();
            std::thread::spawn(move || {
                pruned.prune();
            });
            Some(Arc::new(cache))
        }
        Err(error) => {
            eprintln!("nie fm: thumbnail cache disabled: {error}");
            None
        }
    }
}

fn start(session: &mut Session, options: &FmOptions) {
    if let Some(dir) = &options.cd
        && session.fm.reveal_path(dir)
    {
        session.fm.run(&Action::Enter);
    }
    if let Some(path) = &options.select {
        session.fm.reveal_path(path);
    }
    session.sync();
}

/// Runs `nie fm`.
///
/// # Errors
/// Fails when the game cannot be mounted or the terminal cannot be driven.
pub fn run_fm(options: &FmOptions) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .context("tokio runtime")?;
    runtime.block_on(async {
        let vfs = open_vfs(options.source.as_deref())?;
        let cache = cache_for(&vfs, options);
        let keymap = keymap(options)?;
        if let Some(size) = &options.snapshot {
            return snapshot(options, size, vfs, cache, keymap).await;
        }
        interactive(options, vfs, cache, keymap).await
    })
}

async fn interactive(
    options: &FmOptions,
    vfs: Arc<Vfs>,
    cache: Option<Arc<ThumbCache>>,
    keymap: Keymap,
) -> Result<()> {
    let mut terminal = ratatui::init();
    // The picker queries the terminal, which needs raw mode and the alternate screen.
    let picker = picker_for(options, options.protocol.is_none());
    let mut session = Session::new(vfs, cache, picker, keymap);
    start(&mut session, options);

    let (tx, mut events) = unbounded_channel::<Event>();
    std::thread::Builder::new()
        .name("nie-fm-input".into())
        .spawn(move || {
            while let Ok(event) = event::read() {
                if tx.send(event).is_err() {
                    break;
                }
            }
        })
        .context("input thread")?;

    let outcome = async {
        loop {
            terminal.draw(|frame| session.draw(frame))?;
            if session.fm.should_quit() {
                return Ok::<(), anyhow::Error>(());
            }
            let wait = session.next_tick().unwrap_or(Duration::from_secs(3600));
            tokio::select! {
                event = events.recv() => match event {
                    Some(Event::Key(key)) => {
                        if let Some(key) = from_crossterm(key) {
                            session.key(key);
                        }
                    }
                    Some(Event::Resize(..)) => {}
                    Some(_) => {}
                    None => return Ok(()),
                },
                () = tokio::time::sleep(wait) => {
                    session.tick();
                }
            }
            session.drain();
        }
    }
    .await;
    ratatui::restore();
    write_metrics(options, &session.samples)?;
    outcome
}

fn parse_size(text: &str) -> Result<(u16, u16)> {
    let (w, h) = text
        .split_once('x')
        .with_context(|| format!("`{text}` is not WIDTHxHEIGHT"))?;
    Ok((w.trim().parse()?, h.trim().parse()?))
}

async fn snapshot(
    options: &FmOptions,
    size: &str,
    vfs: Arc<Vfs>,
    cache: Option<Arc<ThumbCache>>,
    keymap: Keymap,
) -> Result<()> {
    let (width, height) = parse_size(size)?;
    let picker = picker_for(options, false);
    let mut session = Session::new(vfs, cache, picker, keymap);
    start(&mut session, options);
    let settled = session.settle(Duration::from_secs(120)).await;
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| session.draw(frame))?;
    let buffer = terminal.backend().buffer().clone();
    let mut out = std::io::stdout().lock();
    for y in 0..buffer.area.height {
        let line: String = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol().to_owned())
            .collect();
        writeln!(out, "{}", line.trim_end())?;
    }
    out.flush()?;
    if !settled {
        eprintln!("nie fm: the preview did not finish within 120 s");
    }
    write_metrics(options, &session.samples)?;
    Ok(())
}

fn write_metrics(options: &FmOptions, samples: &[Sample]) -> Result<()> {
    let Some(path) = &options.metrics else {
        return Ok(());
    };
    let rows: Vec<_> = samples
        .iter()
        .map(|s| {
            serde_json::json!({
                "path": s.path,
                "kind": s.kind,
                "latency_ms": s.latency_ms,
                "build_ms": s.build_ms,
                "cached": s.cached,
            })
        })
        .collect();
    std::fs::write(path, serde_json::to_vec_pretty(&rows)?)
        .with_context(|| format!("write {}", path.display()))
}

