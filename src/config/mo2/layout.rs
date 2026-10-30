//! Where an MO2 instance keeps things.
//!
//! An instance is its `ModOrganizer.ini`; mudcrab calls the folder holding it
//! the instance folder. The ini's `[Settings]` say where the rest is:
//! `base_directory` (by default the instance folder itself), and the mods,
//! profiles, overwrite and downloads folders (by default `mods`, `profiles`,
//! `overwrite` and `downloads` in the base directory; a value may say
//! `%BASE_DIR%` for it). On Linux, set up by Jackify or as a portable
//! instance, they are usually all one folder. On Windows they are often not:
//! the ini sits with MO2 itself, and `base_directory` names a folder
//! elsewhere holding only `mods`, `profiles`, `overwrite` and `webcache`.
//!
//! MO2 stores paths with forward slashes, and under Wine as `Z:` paths; any
//! other Wine drive is inside a prefix this side cannot see.

use std::path::{Path, PathBuf};

/// The folders of the instance whose `ModOrganizer.ini` is in `instance`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The folder holding `ModOrganizer.ini`.
    pub instance: PathBuf,
    pub base: PathBuf,
    pub mods: PathBuf,
    pub profiles: PathBuf,
    pub overwrite: PathBuf,
}

impl Layout {
    /// The layout `ModOrganizer.ini` in `instance` describes, or the
    /// defaults, all in `instance`, when it says nothing (or is not there).
    pub fn of(instance: &Path) -> Self {
        let text = std::fs::read_to_string(instance.join("ModOrganizer.ini")).unwrap_or_default();
        Self::parse(instance, &text, cfg!(windows))
    }

    fn parse(instance: &Path, ini: &str, windows: bool) -> Self {
        let setting = |key: &str| settings_value(ini, key).and_then(|v| local_path(&v, windows));
        let base = setting("base_directory").unwrap_or_else(|| instance.to_path_buf());
        let folder = |key: &str, default: &str| {
            settings_value(ini, key)
                .map(|value| value.replace("%BASE_DIR%", &base.to_string_lossy()))
                .and_then(|value| local_path(&value, windows))
                .unwrap_or_else(|| base.join(default))
        };
        Self {
            instance: instance.to_path_buf(),
            mods: folder("mod_directory", "mods"),
            profiles: folder("profiles_directory", "profiles"),
            overwrite: folder("overwrite_directory", "overwrite"),
            base,
        }
    }
}

/// The mods folder of the instance in `instance`.
pub fn mods_dir(instance: &Path) -> PathBuf {
    Layout::of(instance).mods
}

/// The profiles folder of the instance in `instance`.
pub fn profiles_dir(instance: &Path) -> PathBuf {
    Layout::of(instance).profiles
}

/// The overwrite folder of the instance in `instance`.
pub fn overwrite_dir(instance: &Path) -> PathBuf {
    Layout::of(instance).overwrite
}

/// A `[Settings]` value of a `ModOrganizer.ini`, non-empty.
pub(crate) fn settings_value(ini: &str, key: &str) -> Option<String> {
    let mut in_settings = false;
    for line in ini.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_settings = line.eq_ignore_ascii_case("[Settings]");
            continue;
        }
        if !in_settings {
            continue;
        }
        if let Some((name, value)) = line.split_once('=')
            && name.trim().eq_ignore_ascii_case(key)
        {
            let value = value.trim().trim_matches('"');
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
}

/// A path as MO2 writes it, as this system names it: as is on Windows; a
/// Wine `Z:` path as the Unix path it is, and `None` for another drive.
fn local_path(value: &str, windows: bool) -> Option<PathBuf> {
    if windows {
        return Some(PathBuf::from(value));
    }
    if value.starts_with('/') {
        return Some(PathBuf::from(value));
    }
    let unix = value
        .strip_prefix("Z:")
        .or_else(|| value.strip_prefix("z:"))?;
    Some(PathBuf::from(unix.replace('\\', "/")))
}

/// The instance folder for `dir`: `dir` itself when it holds
/// `ModOrganizer.ini` (or is a new folder for one); for a folder that is an
/// instance's base directory (`mods`, `profiles` and no ini, as
/// `base_directory` makes one), the folder of the ini that names it, where
/// that can be found -- MO2's global instances, in
/// `%LOCALAPPDATA%\ModOrganizer` on Windows -- and otherwise an error saying
/// to give that folder instead.
pub fn find_instance(dir: &Path) -> anyhow::Result<PathBuf> {
    if dir.join("ModOrganizer.ini").is_file() || dir.join("ModOrganizer.exe").is_file() {
        return Ok(dir.to_path_buf());
    }
    let looks_like_data = dir.join("mods").is_dir() || dir.join("profiles").is_dir();
    if looks_like_data
        && let Some(found) = global_instances()
            .into_iter()
            .find(|instance| same_dir(&Layout::of(instance).base, dir))
    {
        return Ok(found);
    }
    anyhow::ensure!(
        !looks_like_data,
        "{} holds an instance's mods and profiles, but not its ModOrganizer.ini, which says where \
         they are: give the folder with ModOrganizer.ini in it instead (with MO2 installed on \
         Windows, often the folder ModOrganizer.exe is in)",
        dir.display()
    );
    Ok(dir.to_path_buf())
}

/// Why `dir` is not an MO2 instance to choose, if it is not: one with
/// neither `ModOrganizer.exe` nor `ModOrganizer.ini` would get a mods folder
/// no MO2 reads.
pub fn not_an_instance(dir: &Path) -> Option<String> {
    let has = |name: &str| dir.join(name).is_file();
    (!has("ModOrganizer.exe") && !has("ModOrganizer.ini")).then(|| {
        format!(
            "{} does not look like a Mod Organizer 2 instance: it has neither ModOrganizer.exe \
             nor ModOrganizer.ini. Choose the folder with ModOrganizer.ini in it (for a \
             portable instance, the folder MO2 is in).",
            dir.display()
        )
    })
}

/// The folders of MO2's global instances.
fn global_instances() -> Vec<PathBuf> {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return Vec::new();
    };
    std::fs::read_dir(PathBuf::from(local).join("ModOrganizer"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("ModOrganizer.ini").is_file())
        .collect()
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_is_in_the_instance_folder_by_default() {
        let layout = Layout::parse(Path::new("/mo2"), "[General]\n", false);
        assert_eq!(layout.base, Path::new("/mo2"));
        assert_eq!(layout.mods, Path::new("/mo2/mods"));
        assert_eq!(layout.profiles, Path::new("/mo2/profiles"));
        assert_eq!(layout.overwrite, Path::new("/mo2/overwrite"));
    }

    #[test]
    fn a_base_directory_moves_the_folders_with_it() {
        let ini = "[Settings]\nbase_directory=C:/Modding/sampler\nlanguage=en\n";
        let layout = Layout::parse(Path::new("C:/MO2"), ini, true);
        assert_eq!(layout.base, Path::new("C:/Modding/sampler"));
        assert_eq!(layout.mods, Path::new("C:/Modding/sampler").join("mods"));
        assert_eq!(
            layout.profiles,
            Path::new("C:/Modding/sampler").join("profiles")
        );
    }

    #[test]
    fn a_folder_can_be_named_from_the_base_or_anywhere() {
        let ini = "[Settings]\nbase_directory=Z:/home/me/mo2\nmod_directory=%BASE_DIR%/my mods\n\
                   overwrite_directory=Z:/elsewhere/overwrite\n";
        let layout = Layout::parse(Path::new("/opt/mo2"), ini, false);
        assert_eq!(layout.base, Path::new("/home/me/mo2"));
        assert_eq!(layout.mods, Path::new("/home/me/mo2/my mods"));
        assert_eq!(layout.overwrite, Path::new("/elsewhere/overwrite"));
        assert_eq!(layout.profiles, Path::new("/home/me/mo2/profiles"));
    }

    #[test]
    fn a_wine_drive_this_side_cannot_see_is_left_at_the_default() {
        let ini = "[Settings]\nbase_directory=D:/Modding\n";
        let layout = Layout::parse(Path::new("/mo2"), ini, false);
        assert_eq!(layout.base, Path::new("/mo2"));
    }

    #[test]
    fn a_base_directory_without_its_ini_is_named_as_such() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("mods")).unwrap();
        let err = find_instance(dir.path()).unwrap_err().to_string();
        assert!(err.contains("not its ModOrganizer.ini"), "{err}");
        // A new folder is one to make an instance in.
        let new = dir.path().join("new");
        assert_eq!(find_instance(&new).unwrap(), new);
        std::fs::write(dir.path().join("ModOrganizer.ini"), "").unwrap();
        assert_eq!(find_instance(dir.path()).unwrap(), dir.path());
    }
}
