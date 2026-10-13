//! What the user sees, and where everything else goes.
//!
//! Every run writes a full log -- every per-mod and per-archive line -- to a
//! file. The terminal gets only what a person running the command wants: a
//! progress bar per long phase, warnings, a short summary on stdout, and on
//! failure the error and where the log is. `--verbose` mirrors the full log to
//! stderr instead of the bars, which is also what tests that read log lines use.

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

static MULTI: OnceLock<MultiProgress> = OnceLock::new();
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static VERBOSE: OnceLock<bool> = OnceLock::new();

fn multi() -> &'static MultiProgress {
    MULTI.get_or_init(|| {
        let interactive = std::io::stderr().is_terminal() && !verbose();
        MultiProgress::with_draw_target(if interactive {
            ProgressDrawTarget::stderr()
        } else {
            ProgressDrawTarget::hidden()
        })
    })
}

fn verbose() -> bool {
    VERBOSE.get().copied().unwrap_or(false)
}

/// Where this run's full log is, if one is being written.
pub fn log_path() -> Option<&'static Path> {
    LOG_PATH.get().map(PathBuf::as_path)
}

/// `~/.local/state/mudcrab/logs` (`%LOCALAPPDATA%\mudcrab\logs` on Windows).
fn default_log_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("mudcrab").join("logs"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state"))
            })
            .map(|state| state.join("mudcrab").join("logs"))
    }
}

/// `YYYYmmdd-HHMMSS` in UTC, without a date crate.
fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil from days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Keep the newest `keep` logs in `dir`; old runs are not worth a full disk.
fn prune_logs(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    // Oldest first by modification time: names start with the command, so
    // sorting by name would evict every `check` log before any `install` one.
    let mut logs: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "log"))
        .filter_map(|p| Some((p.metadata().ok()?.modified().ok()?, p)))
        .collect();
    logs.sort();
    let logs: Vec<PathBuf> = logs.into_iter().map(|(_, p)| p).collect();
    let excess = logs.len().saturating_sub(keep);
    for old in logs.into_iter().take(excess) {
        let _ = std::fs::remove_file(old);
    }
}

/// Writes to stderr without tearing the progress bars.
#[derive(Clone, Copy)]
struct ConsoleWriter;

impl Write for ConsoleWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        multi().suspend(|| std::io::stderr().write_all(buf))?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::stderr().flush()
    }
}

/// Set up logging for a run of `command`.
///
/// The file gets everything at `info` (or `RUST_LOG`); the console gets
/// warnings and errors, or everything with `verbose`. A log file that cannot
/// be created is not fatal: the run goes ahead without one.
pub fn init(command: &str, verbose: bool, log_file: Option<PathBuf>) {
    let _ = VERBOSE.set(verbose);
    let file_filter =
        || EnvFilter::try_from_default_env().unwrap_or_else(|_| "mudcrab=info".into());

    let path = log_file.or_else(|| {
        let dir = default_log_dir()?;
        std::fs::create_dir_all(&dir).ok()?;
        prune_logs(&dir, 30);
        Some(dir.join(format!("{command}-{}.log", timestamp())))
    });
    let file_layer = path.as_ref().and_then(|path| {
        let file = std::fs::File::create(path).ok()?;
        let _ = LOG_PATH.set(path.clone());
        Some(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_target(false)
                .with_writer(Mutex::new(file))
                .with_filter(file_filter()),
        )
    });

    let console_filter = if verbose {
        file_filter()
    } else {
        EnvFilter::new("mudcrab=warn")
    };
    // Verbose mirrors the log as it is; otherwise a warning reads like the
    // rest of the output.
    let console_layer = if verbose {
        tracing_subscriber::fmt::layer()
            .with_target(false)
            .compact()
            .with_ansi(std::io::stderr().is_terminal())
            .with_writer(|| ConsoleWriter)
            .with_filter(console_filter)
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .event_format(Plain)
            .with_writer(|| ConsoleWriter)
            .with_filter(console_filter)
            .boxed()
    };

    let _ = tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .try_init();
}

/// A warning or error as a person reads it: a symbol, the message, and its
/// details dimmed after it.
struct Plain;

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for Plain
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        #[derive(Default)]
        struct Fields {
            message: String,
            rest: Vec<String>,
        }
        impl tracing::field::Visit for Fields {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message = format!("{value:?}");
                } else {
                    self.rest.push(format!("{}={value:?}", field.name()));
                }
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    self.message = value.to_string();
                } else {
                    self.rest.push(format!("{}={value}", field.name()));
                }
            }
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        let tone = if *event.metadata().level() == tracing::Level::ERROR {
            Tone::Problem
        } else if *event.metadata().level() == tracing::Level::WARN {
            Tone::Warning
        } else {
            Tone::Note
        };
        let colour = std::io::stderr().is_terminal() && console::colors_enabled_stderr();
        let symbol = tone.symbol().to_string();
        let details = if fields.rest.is_empty() {
            String::new()
        } else {
            format!(" ({})", fields.rest.join(", "))
        };
        if colour {
            writeln!(
                writer,
                "{} {}{}",
                tone.paint(symbol).bold().force_styling(true),
                tone.paint(fields.message).force_styling(true),
                console::style(details).dim().force_styling(true)
            )
        } else {
            writeln!(writer, "{symbol} {}{details}", fields.message)
        }
    }
}

/// A progress bar for `len` steps of `label`. Hidden when stderr is not a
/// terminal or in verbose mode, so it costs nothing in scripts and tests.
pub fn progress(len: u64, label: &str) -> ProgressBar {
    let bar = multi().add(ProgressBar::new(len));
    bar.set_style(
        ProgressStyle::with_template("{prefix:>10} [{bar:30}] {pos}/{len} {wide_msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=> "),
    );
    bar.set_prefix(label.to_string());
    bar
}

/// A line for the user on stdout, around any progress bars.
pub fn say(message: impl std::fmt::Display) {
    multi().suspend(|| println!("{message}"));
}

pub use crate::events::Tone;

/// How a tone looks on a terminal: a symbol and a colour. Colour and the
/// symbols' Unicode forms follow the terminal -- none when output is piped,
/// `NO_COLOR` is set, or the terminal cannot show them.
trait Look {
    fn symbol(self) -> console::Emoji<'static, 'static>;
    fn paint<D>(self, text: D) -> console::StyledObject<D>;
}

impl Look for Tone {
    fn symbol(self) -> console::Emoji<'static, 'static> {
        match self {
            Tone::Good => console::Emoji("✔", "ok"),
            Tone::Warning => console::Emoji("⚠", "!"),
            Tone::Problem => console::Emoji("✖", "x"),
            Tone::Note => console::Emoji("ℹ", "i"),
        }
    }

    fn paint<D>(self, text: D) -> console::StyledObject<D> {
        let styled = console::style(text);
        match self {
            Tone::Good => styled.green(),
            Tone::Warning => styled.yellow(),
            Tone::Problem => styled.red(),
            Tone::Note => styled.cyan(),
        }
    }
}

/// A message as the terminal shows it: names bold, secondary text dim.
pub fn render(text: &crate::events::Text) -> String {
    use crate::events::Span;
    text.0
        .iter()
        .map(|span| match span {
            Span::Plain(text) => text.clone(),
            Span::Name(text) => name(text),
            Span::Dim(text) => dim(text),
        })
        .collect()
}

/// The CLI's [`crate::events::Sink`]: events as styled lines on stdout, progress
/// as bars on stderr.
pub struct Terminal;

impl crate::events::Sink for Terminal {
    fn event(&self, event: crate::events::Event) {
        use crate::events::Event;
        // A status after a group stands apart from it.
        let after_group = AFTER_GROUP.swap(
            matches!(event, Event::Section { .. } | Event::ManualStep { .. }),
            std::sync::atomic::Ordering::Relaxed,
        );
        match event {
            Event::Status { tone, text } => {
                if after_group {
                    say("");
                }
                status(tone, render(&text))
            }
            Event::Section {
                tone,
                heading,
                items,
            } => {
                let items: Vec<String> = items.iter().map(|item| render(&item.text)).collect();
                section(tone, &heading, &items);
            }
            Event::ManualStep { title, steps } => {
                self::heading(Tone::Warning, format!("Still to do by hand: {title}"));
                for line in steps {
                    say(format!("  {line}"));
                }
            }
        }
    }

    fn progress(&self, len: u64, label: &str) -> crate::events::Progress {
        crate::events::Progress::new(Bar(progress(len, label)))
    }
}

static AFTER_GROUP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct Bar(ProgressBar);

impl crate::events::ProgressSink for Bar {
    fn inc(&self, steps: u64) {
        self.0.inc(steps);
    }
    fn set_message(&self, message: &str) {
        self.0.set_message(message.to_string());
    }
    fn finish(&self) {
        self.0.finish_and_clear();
    }
}

/// A one-line status: symbol, then the message, in the tone's colour.
pub fn status(tone: Tone, message: impl std::fmt::Display) {
    say(format!(
        "{} {}",
        tone.paint(tone.symbol()).bold(),
        tone.paint(message.to_string())
    ));
}

/// `status`, on stderr: for a command whose stdout is data to pipe or paste
/// (a list of files, a TOML snippet), so its messages stay out of it.
pub fn status_err(tone: Tone, message: impl std::fmt::Display) {
    multi().suspend(|| {
        eprintln!(
            "{} {}",
            tone.paint(tone.symbol()).bold().for_stderr(),
            tone.paint(message.to_string()).for_stderr()
        )
    });
}

/// A titled list: a heading with its symbol and count, then one bullet per
/// item, blank line before. Nothing for an empty list.
pub fn section(tone: Tone, heading: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    say("");
    say(format!(
        "{} {} {}",
        tone.paint(tone.symbol()).bold(),
        console::style(heading).bold(),
        console::style(format!("({})", items.len())).dim()
    ));
    for item in items {
        // Continuation lines line up under the bullet's text.
        let item = item.replace('\n', "\n    ");
        say(format!("  {} {item}", tone.paint(console::Emoji("•", "-"))));
    }
}

/// A question put to the user, ready for `print!`: a bold cyan `?` and the
/// question in bold.
pub fn question(text: impl std::fmt::Display) -> String {
    format!(
        "{} {}",
        console::style("?").cyan().bold(),
        console::style(text.to_string()).bold()
    )
}

/// Secondary text -- a path, a hint, a default -- dimmed.
pub fn dim(text: impl std::fmt::Display) -> String {
    console::style(text.to_string()).dim().to_string()
}

/// "Goes to": an arrow, or `->` where Unicode will not show.
pub fn arrow() -> String {
    dim(console::Emoji("→", "->"))
}

/// A numbered option in a list to pick from: the number set off, the default
/// marked.
pub fn option(number: usize, text: impl std::fmt::Display, default: bool) -> String {
    format!(
        "  {} {text}{}",
        console::style(format!("{number:>2}.")).cyan(),
        if default {
            format!(" {}", dim("(default)"))
        } else {
            String::new()
        }
    )
}

/// A line under a heading: a bullet, then the text.
pub fn bullet(tone: Tone, text: impl std::fmt::Display) {
    say(format!("  {} {text}", tone.paint(console::Emoji("•", "-"))));
}

/// A heading on its own, with its symbol, after a blank line.
pub fn heading(tone: Tone, text: impl std::fmt::Display) {
    say("");
    say(format!(
        "{} {}",
        tone.paint(tone.symbol()).bold(),
        console::style(text.to_string()).bold()
    ));
}

/// A name the user might look for -- a mod, a plugin, a file -- set off from
/// the words around it.
pub fn name(text: impl std::fmt::Display) -> String {
    console::style(text.to_string()).bold().to_string()
}

/// Print a failed run's error for a person: the message and its causes, then
/// where the full log is.
pub fn report_error(err: &anyhow::Error) {
    multi().suspend(|| {
        let problem = Tone::Problem;
        eprintln!(
            "{} {err}",
            problem
                .paint(format!("{} error:", problem.symbol()))
                .bold()
                .for_stderr()
        );
        for cause in err.chain().skip(1) {
            eprintln!(
                "  {} {cause}",
                console::style("caused by:").dim().for_stderr()
            );
        }
        if let Some(path) = log_path() {
            eprintln!(
                "{}",
                console::style(format!("full log: {}", path.display()))
                    .dim()
                    .for_stderr()
            );
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn timestamp_has_the_expected_shape() {
        let stamp = super::timestamp();
        assert_eq!(stamp.len(), 15, "{stamp}");
        assert_eq!(&stamp[8..9], "-");
        assert!(stamp.starts_with("20"), "{stamp}");
    }
}
