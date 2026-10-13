//! mudcrab's events, sent on to the page.
//!
//! The library reports through `mudcrab::events`; this sink emits each event
//! to the window as `mudcrab://event`, and each progress bar's changes as
//! `mudcrab://progress`, at most every 80 ms a bar so a fast loop does not
//! flood the page.

use mudcrab::events::{Event, Progress, ProgressSink, Sink};
use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};

pub struct GuiSink<R: Runtime> {
    app: AppHandle<R>,
    next: AtomicU64,
}

impl<R: Runtime> GuiSink<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            next: AtomicU64::new(1),
        }
    }
}

impl<R: Runtime> Sink for GuiSink<R> {
    fn event(&self, event: Event) {
        let _ = self.app.emit("mudcrab://event", &event);
    }

    fn progress(&self, len: u64, label: &str) -> Progress {
        let bar = Bar {
            app: self.app.clone(),
            id: self.next.fetch_add(1, Ordering::Relaxed),
            label: label.to_string(),
            len,
            pos: AtomicU64::new(0),
            state: Mutex::new((String::new(), Instant::now() - Duration::from_secs(1))),
        };
        bar.send(false);
        Progress::new(bar)
    }
}

/// What the page is told about a bar.
#[derive(Serialize, Clone)]
struct BarState<'a> {
    id: u64,
    label: &'a str,
    len: u64,
    pos: u64,
    message: &'a str,
    done: bool,
}

struct Bar<R: Runtime> {
    app: AppHandle<R>,
    id: u64,
    label: String,
    len: u64,
    pos: AtomicU64,
    /// The current message, and when the page was last told.
    state: Mutex<(String, Instant)>,
}

impl<R: Runtime> Bar<R> {
    fn send(&self, done: bool) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let pos = self.pos.load(Ordering::Relaxed);
        if !done && pos < self.len && state.1.elapsed() < Duration::from_millis(80) {
            return;
        }
        state.1 = Instant::now();
        let _ = self.app.emit(
            "mudcrab://progress",
            BarState {
                id: self.id,
                label: &self.label,
                len: self.len,
                pos,
                message: &state.0,
                done,
            },
        );
    }
}

impl<R: Runtime> ProgressSink for Bar<R> {
    fn inc(&self, steps: u64) {
        self.pos.fetch_add(steps, Ordering::Relaxed);
        self.send(false);
    }

    fn set_message(&self, message: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.0 = message.to_string();
        }
        self.send(false);
    }

    fn finish(&self) {
        self.send(true);
    }
}
