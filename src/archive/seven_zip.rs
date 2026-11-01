//! Finding 7-Zip, which reads the archive formats mudcrab cannot itself
//! (7z and RAR, and a named subset of a solid archive).
//!
//! On Linux it is on `PATH` as `7z` (p7zip) or `7zz` (7-Zip's own build). On
//! Windows the 7-Zip installer puts `7z.exe` in `Program Files\7-Zip` and does
//! not add it to `PATH`, so looking for `7z` there finds nothing even with
//! 7-Zip installed. `MUDCRAB_7Z` names it outright, for anywhere else.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The 7-Zip to run: found once, then remembered. Where none is found, the
/// bare name, so running it fails as a missing program would.
pub fn program() -> &'static Path {
    static FOUND: OnceLock<PathBuf> = OnceLock::new();
    FOUND.get_or_init(|| find().unwrap_or_else(|| PathBuf::from("7z")))
}

fn find() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("MUDCRAB_7Z").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(named));
    }
    on_path().or_else(installed)
}

/// `7z`, `7zz` or `7za` on `PATH`.
fn on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let names: &[&str] = if cfg!(windows) {
        &["7z.exe", "7zz.exe", "7za.exe"]
    } else {
        &["7z", "7zz", "7za"]
    };
    std::env::split_paths(&path)
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| candidate.is_file())
}

/// Where the 7-Zip installer puts it on Windows: Program Files (64- or
/// 32-bit), or the user's own programs for a per-user install.
fn installed() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
        .iter()
        .filter_map(|var| std::env::var_os(var).map(PathBuf::from))
        .chain(std::env::var_os("LOCALAPPDATA").map(|local| PathBuf::from(local).join("Programs")))
        .map(|dir| dir.join("7-Zip").join("7z.exe"))
        .find(|candidate| candidate.is_file())
}

/// What to say when 7-Zip could not be run at all.
pub fn missing(err: &std::io::Error) -> String {
    if err.kind() == std::io::ErrorKind::NotFound {
        "7-Zip was not found: install it (https://7-zip.org), or set MUDCRAB_7Z to its 7z \
         program"
            .to_string()
    } else {
        format!("failed to run 7-Zip ({}): {err}", program().display())
    }
}
