//! The renderer-independent state of the shell: the apps the launcher offers, the notification
//! queue and the panel geometry. Plain data and pure functions, tested without a window.

use std::time::{Duration, Instant};

/// An app of the nie desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// Stable identifier.
    pub id: &'static str,
    /// The name shown and announced.
    pub name: &'static str,
    /// What the app does, announced by screen readers.
    pub summary: &'static str,
    /// The executable, looked up next to the shell and then on `PATH`.
    pub binary: &'static str,
    /// Arguments.
    pub args: &'static [&'static str],
}

/// The apps of this build.
pub const APPS: &[App] = &[
    App { id: "explorer", name: "Explorer", summary: "Browse the game's files", binary: "nie-explorer", args: &[] },
    App { id: "viewer", name: "Viewer", summary: "Pictures, sprite sheets, films and models", binary: "nie-viewer", args: &[] },
    App { id: "wiki", name: "Wiki", summary: "Characters, skills and teams from the game's data", binary: "nie-wiki", args: &[] },
];

/// One notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Identifier assigned by the queue.
    pub id: u64,
    /// The app or service that sent it.
    pub source: String,
    /// The text, also its accessible name.
    pub text: String,
    /// Moment after which the notification disappears.
    pub expires: Instant,
}

/// The notification queue: newest first, at most `capacity` shown.
#[derive(Debug, Clone)]
pub struct Notifications {
    items: Vec<Notification>,
    next: u64,
    capacity: usize,
    ttl: Duration,
}

impl Notifications {
    /// An empty queue.
    #[must_use]
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self { items: Vec::new(), next: 1, capacity: capacity.max(1), ttl }
    }

    /// Adds a notification and returns its id.
    pub fn push(&mut self, source: impl Into<String>, text: impl Into<String>, now: Instant) -> u64 {
        let id = self.next;
        self.next += 1;
        self.items.insert(0, Notification { id, source: source.into(), text: text.into(), expires: now + self.ttl });
        self.items.truncate(self.capacity);
        id
    }

    /// Removes one notification.
    pub fn dismiss(&mut self, id: u64) {
        self.items.retain(|n| n.id != id);
    }

    /// Drops the expired ones; true when something changed.
    pub fn expire(&mut self, now: Instant) -> bool {
        let before = self.items.len();
        self.items.retain(|n| n.expires > now);
        self.items.len() != before
    }

    /// The notifications to show, newest first.
    #[must_use]
    pub fn items(&self) -> &[Notification] {
        &self.items
    }
}

/// Height of the top panel in logical pixels at scale 1.
pub const PANEL_HEIGHT: f32 = 34.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_queue_keeps_the_newest_and_expires() {
        let t0 = Instant::now();
        let mut q = Notifications::new(2, Duration::from_secs(5));
        let a = q.push("nie", "first", t0);
        q.push("nie", "second", t0);
        q.push("nie", "third", t0 + Duration::from_secs(1));
        assert_eq!(q.items().iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), ["third", "second"]);
        assert!(!q.items().iter().any(|n| n.id == a));
        assert!(q.expire(t0 + Duration::from_secs(5)));
        assert_eq!(q.items().len(), 1);
        q.dismiss(q.items()[0].id);
        assert!(q.items().is_empty());
    }

    #[test]
    fn every_app_has_a_name_and_a_summary() {
        for app in APPS {
            assert!(!app.name.is_empty() && !app.summary.is_empty() && !app.binary.is_empty());
        }
    }
}
