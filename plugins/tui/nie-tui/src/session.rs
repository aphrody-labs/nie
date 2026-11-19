//! One explorer session: the model, the preview pane and the scheduler wired together. The
//! interactive loop and the headless snapshot both drive it, so what the tests render is what the
//! terminal shows.

use std::sync::Arc;
use std::time::{Duration, Instant};

use nie_explore::fm::keymap::Keymap;
use nie_explore::fm::keys::{Key, KeyCode, Mods};
use nie_explore::fm::preview::PreviewRequest;
use nie_explore::fm::thumbs::ThumbCache;
use nie_explore::fm::{Fm, NodeKind};
use nie_formats::vfs::Vfs;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode as CtCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui_image::picker::Picker;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::pane::Pane;
use crate::sched::{Done, Scheduler};
use crate::ui::{self, Status};

/// What identifies the preview on screen or on its way.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Wanted {
    path: String,
    hex: bool,
    index: i32,
}

/// One timed preview, for the research log.
#[derive(Debug, Clone)]
pub struct Sample {
    /// The VFS path.
    pub path: String,
    /// The previewer kind.
    pub kind: &'static str,
    /// Request to result, debounce included, in milliseconds.
    pub latency_ms: f64,
    /// Time inside the previewer, in milliseconds.
    pub build_ms: f64,
    /// Whether the thumbnail cache answered.
    pub cached: bool,
}

/// The session.
pub struct Session {
    /// The model.
    pub fm: Fm,
    /// The preview column.
    pub pane: Pane,
    scheduler: Scheduler,
    results: UnboundedReceiver<Done>,
    latest: u64,
    wanted: Option<Wanted>,
    shown: Option<Wanted>,
    status: Status,
    show_timings: bool,
    /// Every finished preview, in order.
    pub samples: Vec<Sample>,
}

impl Session {
    /// A session over a mounted game.
    #[must_use]
    pub fn new(vfs: Arc<Vfs>, cache: Option<Arc<ThumbCache>>, picker: Picker, keymap: Keymap) -> Self {
        let (tx, results): (UnboundedSender<Done>, _) = unbounded_channel();
        let mut fm = Fm::new(Arc::clone(&vfs));
        fm.set_keymap(keymap);
        let status = Status {
            protocol: String::new(),
            ..Status::default()
        };
        let mut session = Self {
            fm,
            pane: Pane::new(picker),
            scheduler: Scheduler::new(vfs, cache, tx),
            results,
            latest: 0,
            wanted: None,
            shown: None,
            status,
            show_timings: true,
            samples: Vec::new(),
        };
        session.status.protocol = session.pane.protocol_name();
        session
    }

    /// Hides the preview timing from the status bar, so a rendered frame is reproducible.
    pub fn show_timings(&mut self, show: bool) {
        self.show_timings = show;
    }

    /// Feeds a key and updates the preview request.
    pub fn key(&mut self, key: Key) {
        self.fm.handle_key(key);
        self.sync();
    }

    /// Asks for the preview of the entry under the cursor when it changed.
    pub fn sync(&mut self) {
        let view = self.fm.view();
        let wanted = view.target.as_ref().and_then(|t| {
            (t.kind == NodeKind::File).then(|| Wanted {
                path: t.path.clone(),
                hex: self.fm.preview.hex,
                index: self.fm.preview.index,
            })
        });
        if wanted == self.wanted {
            return;
        }
        self.wanted = wanted.clone();
        match (wanted, view.target) {
            (Some(wanted), Some(target)) => {
                self.pane.loading(wanted.path.clone());
                self.status.busy = true;
                self.latest = self.scheduler.request(PreviewRequest {
                    path: wanted.path,
                    size: target.size,
                    hex: wanted.hex,
                    index: wanted.index,
                });
            }
            _ => {
                self.scheduler.cancel();
                self.pane.clear();
                self.shown = None;
                self.status.busy = false;
            }
        }
    }

    /// Takes finished previews. Returns true when something changed on screen.
    pub fn drain(&mut self) -> bool {
        let mut changed = self.fm.poll();
        while let Ok(done) = self.results.try_recv() {
            if done.id != self.latest {
                continue;
            }
            self.samples.push(Sample {
                path: done.preview.path.clone(),
                kind: done.preview.kind,
                latency_ms: done.latency.as_secs_f64() * 1000.0,
                build_ms: done.preview.took.as_secs_f64() * 1000.0,
                cached: done.preview.cached,
            });
            self.status.preview_ms = self.show_timings.then_some(done.latency.as_millis());
            self.status.busy = false;
            self.shown = self.wanted.clone();
            self.pane.set(done.preview);
            changed = true;
        }
        changed
    }

    /// Waits until the requested preview arrived, or `timeout` elapsed.
    pub async fn settle(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            self.drain();
            if self.wanted.is_none() || self.shown == self.wanted {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Whether the loop needs to wake on a timer.
    #[must_use]
    pub fn next_tick(&self) -> Option<Duration> {
        if self.fm.is_searching() || self.status.busy {
            return Some(Duration::from_millis(30));
        }
        if self.playing() {
            return self.pane.next_tick();
        }
        None
    }

    /// Films and turntables play unless paused; galleries play when asked. `p` inverts the default.
    fn playing(&self) -> bool {
        self.pane.animates() != self.fm.preview.playing
    }

    /// Advances animations. Returns true when the frame changed.
    pub fn tick(&mut self) -> bool {
        let mut changed = self.drain();
        // `[` and `]` pick a frame by hand; playing advances by itself.
        if self.playing() {
            changed |= self.pane.tick(Instant::now());
        } else if self.pane.frame_count() > 0 {
            self.pane.show_frame(self.fm.preview.frame);
        }
        changed
    }

    /// Draws the screen.
    pub fn draw(&mut self, frame: &mut Frame<'_>) {
        let height = usize::from(frame.area().height.saturating_sub(3));
        self.fm.set_height(height);
        let view = self.fm.view();
        let scroll = usize::try_from(self.fm.preview.scroll).unwrap_or(0);
        ui::draw(frame, &view, &mut self.pane, self.fm.keymap(), &self.status, scroll);
    }
}

/// Converts a crossterm key event; releases and repeats of unknown keys are dropped.
#[must_use]
pub fn from_crossterm(event: KeyEvent) -> Option<Key> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    let mods = Mods {
        ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
        alt: event.modifiers.contains(KeyModifiers::ALT),
        shift: event.modifiers.contains(KeyModifiers::SHIFT),
    };
    let code = match event.code {
        CtCode::Char(' ') => KeyCode::Space,
        CtCode::Char(c) => {
            return Some(Key {
                code: KeyCode::Char(c),
                // Shift is already folded into the character.
                mods: Mods {
                    shift: false,
                    ..mods
                },
            });
        }
        CtCode::Enter => KeyCode::Enter,
        CtCode::Esc => KeyCode::Esc,
        CtCode::Backspace => KeyCode::Backspace,
        CtCode::Delete => KeyCode::Delete,
        CtCode::Tab => KeyCode::Tab,
        CtCode::BackTab => {
            return Some(Key {
                code: KeyCode::Tab,
                mods: Mods { shift: true, ..mods },
            });
        }
        CtCode::Up => KeyCode::Up,
        CtCode::Down => KeyCode::Down,
        CtCode::Left => KeyCode::Left,
        CtCode::Right => KeyCode::Right,
        CtCode::PageUp => KeyCode::PageUp,
        CtCode::PageDown => KeyCode::PageDown,
        CtCode::Home => KeyCode::Home,
        CtCode::End => KeyCode::End,
        CtCode::F(n) => KeyCode::F(n),
        _ => return None,
    };
    Some(Key { code, mods })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossterm_keys_map_to_model_keys() {
        let key = |code, modifiers| from_crossterm(KeyEvent::new(code, modifiers));
        assert_eq!(key(CtCode::Char('j'), KeyModifiers::NONE), Some(Key::char('j')));
        assert_eq!(key(CtCode::Char('G'), KeyModifiers::SHIFT), Some(Key::char('G')));
        assert_eq!(
            key(CtCode::Char('u'), KeyModifiers::CONTROL),
            Some(Key {
                code: KeyCode::Char('u'),
                mods: Mods::CTRL
            })
        );
        assert_eq!(key(CtCode::Char(' '), KeyModifiers::NONE), Some(Key::plain(KeyCode::Space)));
        assert_eq!(key(CtCode::Enter, KeyModifiers::NONE), Some(Key::plain(KeyCode::Enter)));
    }
}
