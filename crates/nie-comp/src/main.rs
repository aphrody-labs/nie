//! `nie-comp`: the nie session compositor.
//!
//! ```text
//! nie-comp --headless --socket nie-0 --screenshot out.png --size 1920x1080
//! ```
//!
//! Headless needs no GPU, DRM or display. SIGUSR1 writes the screenshot, SIGINT and SIGTERM write
//! it and stop; `--after-ms` writes it once after that delay and stops.

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("nie-comp runs on Linux only");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    linux::run()
}

#[cfg(target_os = "linux")]
mod linux {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result};
    use calloop::signals::{Signal, Signals};
    use clap::Parser;
    use nie_comp::{Config, Headless};
    use tracing_subscriber::EnvFilter;

    #[derive(Parser)]
    #[command(version, about = "The nie session compositor (Wayland, smithay)")]
    struct Args {
        /// Composite into memory: no GPU, DRM or display (the default backend).
        #[arg(long, conflicts_with_all = ["winit", "drm"])]
        headless: bool,
        /// Run in a window on the host Wayland or X11 session (feature `winit`).
        #[arg(long, conflicts_with = "drm")]
        winit: bool,
        /// Drive DRM/KMS and libinput on the current seat (feature `drm`).
        #[arg(long)]
        drm: bool,
        /// Socket name under XDG_RUNTIME_DIR, or an absolute path.
        #[arg(long, env = "NIE_COMP_SOCKET", default_value = "nie-0")]
        socket: String,
        /// Output size as WIDTHxHEIGHT (headless and nested).
        #[arg(long, default_value = "1920x1080", value_parser = parse_size)]
        size: (i32, i32),
        /// Write the whole output as PNG: on SIGUSR1, on exit, or after --after-ms.
        #[arg(long)]
        screenshot: Option<PathBuf>,
        /// Spawn XWayland for X11 clients (feature `xwayland`).
        #[arg(long)]
        xwayland: bool,
        /// Headless only: write the screenshot after this delay and exit.
        #[arg(long, requires = "screenshot")]
        after_ms: Option<u64>,
    }

    fn parse_size(value: &str) -> Result<(i32, i32), String> {
        let (w, h) = value.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
        let (w, h) = (w.parse::<i32>().map_err(|e| e.to_string())?, h.parse::<i32>().map_err(|e| e.to_string())?);
        if w <= 0 || h <= 0 {
            return Err("size must be positive".into());
        }
        Ok((w, h))
    }

    pub fn run() -> Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
            .init();
        let args = Args::parse();
        let config = Config { socket: args.socket.clone(), size: args.size, xwayland: args.xwayland, ..Config::default() };

        if args.drm {
            #[cfg(feature = "drm")]
            return nie_comp::drm::run(&config);
            #[cfg(not(feature = "drm"))]
            anyhow::bail!("built without the `drm` feature");
        }
        if args.winit {
            #[cfg(feature = "winit")]
            return nie_comp::winit::run(&config);
            #[cfg(not(feature = "winit"))]
            anyhow::bail!("built without the `winit` feature");
        }
        run_headless(&config, &args)
    }

    fn run_headless(config: &Config, args: &Args) -> Result<()> {
        let mut headless = Headless::new(config)?;
        headless.compositor().quit_on_signals()?;
        let shot = Arc::new(AtomicBool::new(false));
        let signals = Signals::new(&[Signal::SIGUSR1]).context("signals")?;
        let shot_flag = shot.clone();
        headless
            .compositor()
            .event_loop
            .handle()
            .insert_source(signals, move |_, _, _| shot_flag.store(true, Ordering::Relaxed))
            .map_err(|e| anyhow::anyhow!("signal source: {e}"))?;

        let deadline = args.after_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
        let write = |headless: &mut Headless| -> Result<()> {
            if let Some(path) = &args.screenshot {
                headless.screenshot()?.write_png(path)?;
                tracing::info!(path = %path.display(), "screenshot written");
            }
            Ok(())
        };
        loop {
            headless.dispatch(Duration::from_millis(16))?;
            if shot.swap(false, Ordering::Relaxed) {
                write(&mut headless)?;
            }
            if headless.compositor().state.quit || deadline.is_some_and(|d| Instant::now() >= d) {
                break;
            }
        }
        write(&mut headless)
    }
}
