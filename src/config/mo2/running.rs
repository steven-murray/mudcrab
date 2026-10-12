//! Is Mod Organizer 2 running this instance?
//!
//! MO2 keeps the active profile's mod list in memory and writes it back over
//! `modlist.txt` when it exits or refreshes. An `install` that rewrites the
//! profile while MO2 is open is therefore silently undone -- in testing, MO2
//! re-enabled one ENB binary and dropped its replacement, and a whole test ran
//! against the wrong one. So `install` refuses while MO2 has the instance open.
//!
//! Detection reads `/proc`, which on Linux covers MO2 under Wine/Proton: the
//! Windows process is an ordinary one there, with its `Z:\...` executable path
//! in its command line. Only an MO2 whose executable sits in *this* instance
//! counts, so a second instance open alongside is no obstacle. Elsewhere no
//! check is made.

use std::path::{Path, PathBuf};

/// Process ids of MO2 processes running from `instance`.
pub fn running_in(instance: &Path) -> Vec<u32> {
    #[cfg(target_os = "linux")]
    {
        let Ok(instance) = std::fs::canonicalize(instance) else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        let mut pids = Vec::new();
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|n| n.parse::<u32>().ok())
            else {
                continue;
            };
            if let Ok(cmdline) = std::fs::read(entry.path().join("cmdline"))
                && cmdline_runs_instance(&cmdline, &instance)
            {
                pids.push(pid);
            }
        }
        pids
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = instance;
        Vec::new()
    }
}

/// The instance folder of every MO2 that is running. `None` where this cannot
/// be told (anywhere but Linux).
pub fn running_instances() -> Option<Vec<PathBuf>> {
    #[cfg(target_os = "linux")]
    {
        let mut out = Vec::new();
        for entry in std::fs::read_dir("/proc").ok()?.flatten() {
            if entry
                .file_name()
                .to_str()
                .and_then(|n| n.parse::<u32>().ok())
                .is_none()
            {
                continue;
            }
            let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else {
                continue;
            };
            if let Some(dir) = mo2_instance_of(&cmdline)
                && !out.contains(&dir)
            {
                out.push(dir);
            }
        }
        Some(out)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// The folder `ModOrganizer.exe` runs from, when a command line's program is
/// MO2.
fn mo2_instance_of(cmdline: &[u8]) -> Option<PathBuf> {
    let program = cmdline
        .split(|byte| *byte == 0)
        .next()
        .and_then(|arg| std::str::from_utf8(arg).ok())
        .and_then(as_unix_path)?;
    let is_mo2 = program.file_name().is_some_and(|name| {
        name.to_string_lossy()
            .eq_ignore_ascii_case("modorganizer.exe")
    });
    is_mo2
        .then(|| program.parent().map(Path::to_path_buf))
        .flatten()
}

/// Whether a NUL-separated command line's program is `ModOrganizer.exe` inside
/// `instance`, as a Unix path or a Wine `Z:\` one.
///
/// Only the first argument: Steam's launcher and runtime wrappers also carry
/// MO2's path, further along, and are not MO2.
fn cmdline_runs_instance(cmdline: &[u8], instance: &Path) -> bool {
    let instance = normalise(&instance.to_string_lossy());
    cmdline
        .split(|byte| *byte == 0)
        .next()
        .and_then(|arg| std::str::from_utf8(arg).ok())
        .and_then(as_unix_path)
        .is_some_and(|path| {
            let path = normalise(&path.to_string_lossy());
            path.rsplit_once('/')
                .is_some_and(|(dir, exe)| exe == "modorganizer.exe" && dir == instance)
        })
}

/// `Z:\home\x` -> `/home/x`; a Unix path as-is; anything else (another drive,
/// a bare name) is not something this can place.
fn as_unix_path(arg: &str) -> Option<PathBuf> {
    if let Some(rest) = arg
        .strip_prefix("Z:\\")
        .or_else(|| arg.strip_prefix("z:\\"))
    {
        return Some(PathBuf::from(format!("/{}", rest.replace('\\', "/"))));
    }
    arg.starts_with('/').then(|| PathBuf::from(arg))
}

/// Compared case-insensitively, since Wine hands paths around in whatever case
/// the Windows side chose.
fn normalise(path: &str) -> String {
    path.trim_end_matches('/').to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::cmdline_runs_instance;
    use std::path::Path;

    const INSTANCE: &str = "/home/u/Games/MudCrab Test";

    #[test]
    fn a_wine_path_to_this_instances_mo2_matches() {
        let cmdline = b"Z:\\home\\u\\Games\\MudCrab Test\\ModOrganizer.exe\0";
        assert!(cmdline_runs_instance(cmdline, Path::new(INSTANCE)));
    }

    #[test]
    fn a_unix_path_matches_case_insensitively() {
        let cmdline = b"/home/u/Games/mudcrab test/modorganizer.exe\0-p\0Default\0";
        assert!(cmdline_runs_instance(cmdline, Path::new(INSTANCE)));
    }

    #[test]
    fn any_mo2_is_named_by_its_folder() {
        let cmdline = b"Z:\\home\\u\\Games\\MudCrab Test\\ModOrganizer.exe\0";
        assert_eq!(
            super::mo2_instance_of(cmdline),
            Some(std::path::PathBuf::from(INSTANCE))
        );
        assert_eq!(super::mo2_instance_of(b"pgrep\0ModOrganizer.exe\0"), None);
    }

    #[test]
    fn another_instance_or_another_program_does_not() {
        let other = b"Z:\\home\\u\\Games\\MOFAM-03.25\\ModOrganizer.exe\0";
        assert!(!cmdline_runs_instance(other, Path::new(INSTANCE)));
        let game = b"Z:\\home\\u\\Games\\MudCrab Test\\usvfs_proxy_x86.exe\0";
        assert!(!cmdline_runs_instance(game, Path::new(INSTANCE)));
        let grep = b"pgrep\0-f\0ModOrganizer.exe\0";
        assert!(!cmdline_runs_instance(grep, Path::new(INSTANCE)));
        // Steam's wrapper names MO2 as an argument, not as itself.
        let wrapper =
            b"c:\\windows\\system32\\steam.exe\0/home/u/Games/MudCrab Test/ModOrganizer.exe\0";
        assert!(!cmdline_runs_instance(wrapper, Path::new(INSTANCE)));
    }
}
