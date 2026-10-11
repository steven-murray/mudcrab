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
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")))
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
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut logs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "log"))
        .collect();
    logs.sort();
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
    let file_filter = || EnvFilter::try_from_default_env().unwrap_or_else(|_| "mudcrab=info".into());

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

    let console_filter = if verbose { file_filter() } else { EnvFilter::new("mudcrab=warn") };
    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact()
        .with_ansi(std::io::stderr().is_terminal())
        .with_writer(|| ConsoleWriter)
        .with_filter(console_filter);

    let _ = tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .try_init();
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

/// Print a failed run's error for a person: the message and its causes, then
/// where the full log is.
pub fn report_error(err: &anyhow::Error) {
    multi().suspend(|| {
        eprintln!("error: {err}");
        for cause in err.chain().skip(1) {
            eprintln!("  caused by: {cause}");
        }
        if let Some(path) = log_path() {
            eprintln!("full log: {}", path.display());
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
