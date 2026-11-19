//! The preview scheduler: one job in flight, the newest request wins, a stale one is cancelled.
//!
//! The cursor can move through hundreds of entries a second while one preview takes tens of
//! milliseconds or seconds. A request waits [`DEBOUNCE`] before it starts, so a key held down only
//! previews where it stops; starting a new request raises the cancel flag of the previous one,
//! which the previewer polls between its stages.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nie_explore::fm::preview::{self, Context, Preview, PreviewRequest};
use nie_explore::fm::thumbs::ThumbCache;
use nie_formats::vfs::Vfs;
use tokio::sync::mpsc::UnboundedSender;

/// How long a request waits for the cursor to settle.
pub const DEBOUNCE: Duration = Duration::from_millis(40);

/// A finished request.
pub struct Done {
    /// The request number, to drop stale results.
    pub id: u64,
    /// The preview.
    pub preview: Preview,
    /// Time from the request to the result, debounce included.
    pub latency: Duration,
}

/// Runs previews on the tokio blocking pool.
pub struct Scheduler {
    vfs: Arc<Vfs>,
    cache: Option<Arc<ThumbCache>>,
    results: UnboundedSender<Done>,
    cancel: Option<Arc<AtomicBool>>,
    next: u64,
}

impl Scheduler {
    /// A scheduler that sends results to `results`.
    #[must_use]
    pub fn new(vfs: Arc<Vfs>, cache: Option<Arc<ThumbCache>>, results: UnboundedSender<Done>) -> Self {
        Self {
            vfs,
            cache,
            results,
            cancel: None,
            next: 0,
        }
    }

    /// Cancels whatever runs now.
    pub fn cancel(&mut self) {
        if let Some(flag) = self.cancel.take() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Starts a request and cancels the previous one. Returns the request number.
    pub fn request(&mut self, request: PreviewRequest) -> u64 {
        self.cancel();
        self.next += 1;
        let id = self.next;
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel = Some(Arc::clone(&flag));
        let (vfs, cache, results) = (Arc::clone(&self.vfs), self.cache.clone(), self.results.clone());
        let started = Instant::now();
        tokio::spawn(async move {
            tokio::time::sleep(DEBOUNCE).await;
            if flag.load(Ordering::Relaxed) {
                return;
            }
            let cancelled = Arc::clone(&flag);
            let preview = tokio::task::spawn_blocking(move || {
                let poll = || cancelled.load(Ordering::Relaxed);
                let ctx = Context {
                    vfs: &vfs,
                    cache: cache.as_deref(),
                    cancelled: &poll,
                };
                preview::build(&ctx, &request)
            })
            .await;
            if let Ok(preview) = preview
                && !flag.load(Ordering::Relaxed)
            {
                let _ = results.send(Done {
                    id,
                    preview,
                    latency: started.elapsed(),
                });
            }
        });
        id
    }
}
