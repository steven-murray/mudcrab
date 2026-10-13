//! What a long operation tells whoever is running it.
//!
//! The library does not print. `check`, `download`, `install` and the rest
//! report through a [`Sink`]: a line of status, a titled group of findings, a
//! step left to do by hand, a progress bar. The CLI's sink is the terminal
//! (`crate::ui::Terminal`), which draws them as styled lines and bars; a GUI
//! sets its own with [`set_sink`] and shows the same events in its views.
//!
//! What is reported is data, not styled text: a [`Text`] marks the names in a
//! message (a mod, a plugin, a file) so each front end can set them off its
//! own way, and an [`Item`] says which mod it is about, so a GUI can link to
//! it.
//!
//! The full log still goes through `tracing`, as before; a warning logged
//! there reaches the terminal through `crate::ui`'s formatter, and a GUI can
//! add a `tracing` layer of its own to see them.

use std::sync::{Arc, RwLock};

/// How a line or a group reads: done, worth a look, wrong, or for
/// information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Good,
    Warning,
    Problem,
    Note,
}

/// One run of a message.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", content = "text", rename_all = "lowercase")]
pub enum Span {
    Plain(String),
    /// A name the user might look for: a mod, a plugin, a file.
    Name(String),
    /// Secondary: a path, a hint.
    Dim(String),
}

/// A message, with its names marked.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct Text(pub Vec<Span>);

impl Text {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn plain(mut self, text: impl Into<String>) -> Self {
        self.push(Span::Plain(text.into()));
        self
    }

    pub fn name(mut self, text: impl Into<String>) -> Self {
        self.push(Span::Name(text.into()));
        self
    }

    pub fn dim(mut self, text: impl Into<String>) -> Self {
        self.push(Span::Dim(text.into()));
        self
    }

    /// This message, then `other`.
    pub fn append(mut self, other: Text) -> Self {
        self.0.extend(other.0);
        self
    }

    fn push(&mut self, span: Span) {
        let empty = match &span {
            Span::Plain(text) | Span::Name(text) | Span::Dim(text) => text.is_empty(),
        };
        if !empty {
            self.0.push(span);
        }
    }

    /// The message without its marks.
    pub fn to_plain(&self) -> String {
        self.0
            .iter()
            .map(|span| match span {
                Span::Plain(text) | Span::Name(text) | Span::Dim(text) => text.as_str(),
            })
            .collect()
    }
}

impl std::fmt::Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_plain())
    }
}

impl From<String> for Text {
    fn from(text: String) -> Self {
        Text::new().plain(text)
    }
}

impl From<&str> for Text {
    fn from(text: &str) -> Self {
        Text::new().plain(text)
    }
}

/// One entry in a group: the message, and the mod it is about, if one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Item {
    pub text: Text,
    pub mod_id: Option<String>,
}

impl Item {
    pub fn new(text: impl Into<Text>) -> Self {
        Self {
            text: text.into(),
            mod_id: None,
        }
    }

    pub fn about(mut self, mod_id: impl Into<String>) -> Self {
        self.mod_id = Some(mod_id.into());
        self
    }
}

/// Something to tell the user.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A one-line outcome: what was done, or what is wrong.
    Status { tone: Tone, text: Text },
    /// A titled group, such as one kind of `check` finding. Never empty.
    Section {
        tone: Tone,
        heading: String,
        items: Vec<Item>,
    },
    /// Something left for the user to do by hand, after an install.
    ManualStep { title: String, steps: Vec<String> },
}

/// Where events go.
pub trait Sink: Send + Sync {
    fn event(&self, event: Event);
    /// A bar for `len` steps of `label`, such as "checking".
    fn progress(&self, len: u64, label: &str) -> Progress;
}

/// What a progress bar is told.
pub trait ProgressSink: Send + Sync {
    fn inc(&self, steps: u64);
    /// What is being worked on now, such as a mod id.
    fn set_message(&self, message: &str);
    /// The work is over; the bar can go.
    fn finish(&self);
}

/// A progress bar, shared between the threads doing the work.
#[derive(Clone)]
pub struct Progress(Arc<dyn ProgressSink>);

impl Progress {
    pub fn new(sink: impl ProgressSink + 'static) -> Self {
        Self(Arc::new(sink))
    }

    /// A bar that shows nothing.
    pub fn hidden() -> Self {
        Self::new(Hidden)
    }

    pub fn inc(&self, steps: u64) {
        self.0.inc(steps);
    }

    pub fn set_message(&self, message: impl AsRef<str>) {
        self.0.set_message(message.as_ref());
    }

    pub fn finish(&self) {
        self.0.finish();
    }
}

struct Hidden;

impl ProgressSink for Hidden {
    fn inc(&self, _: u64) {}
    fn set_message(&self, _: &str) {}
    fn finish(&self) {}
}

/// A sink that drops everything; for running the library where nobody is
/// watching.
pub struct Silent;

impl Sink for Silent {
    fn event(&self, _: Event) {}
    fn progress(&self, _: u64, _: &str) -> Progress {
        Progress::hidden()
    }
}

/// A sink that keeps every event, for a caller that shows them afterwards (and
/// for tests).
#[derive(Default)]
pub struct Collect(std::sync::Mutex<Vec<Event>>);

impl Collect {
    pub fn events(&self) -> Vec<Event> {
        self.0
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }
}

impl Sink for Collect {
    fn event(&self, event: Event) {
        if let Ok(mut events) = self.0.lock() {
            events.push(event);
        }
    }
    fn progress(&self, _: u64, _: &str) -> Progress {
        Progress::hidden()
    }
}

static SINK: RwLock<Option<Arc<dyn Sink>>> = RwLock::new(None);

/// Send every later event to `sink`. Until this is called, events go to the
/// terminal.
pub fn set_sink(sink: Arc<dyn Sink>) {
    if let Ok(mut current) = SINK.write() {
        *current = Some(sink);
    }
}

fn sink() -> Arc<dyn Sink> {
    SINK.read()
        .ok()
        .and_then(|current| current.clone())
        .unwrap_or_else(|| Arc::new(crate::ui::Terminal))
}

/// Report `event`.
pub fn emit(event: Event) {
    sink().event(event);
}

/// Report a one-line status.
pub fn status(tone: Tone, text: impl Into<Text>) {
    emit(Event::Status {
        tone,
        text: text.into(),
    });
}

/// Report a titled group; nothing for an empty one.
pub fn section(tone: Tone, heading: impl Into<String>, items: Vec<Item>) {
    if items.is_empty() {
        return;
    }
    emit(Event::Section {
        tone,
        heading: heading.into(),
        items,
    });
}

/// A progress bar for `len` steps of `label`.
pub fn progress(len: u64, label: &str) -> Progress {
    sink().progress(len, label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_keeps_its_names_and_drops_empty_runs() {
        let text = Text::new()
            .plain("moved ")
            .name("Foo.esp")
            .plain("")
            .dim(" (x)");
        assert_eq!(text.0.len(), 3);
        assert_eq!(text.to_plain(), "moved Foo.esp (x)");
    }

    #[test]
    fn events_serialise_for_a_front_end() {
        let event = Event::Status {
            tone: Tone::Good,
            text: Text::new().plain("Moved ").name("X"),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"event":"status","tone":"good","text":[{"kind":"plain","text":"Moved "},{"kind":"name","text":"X"}]}"#
        );
    }
}
