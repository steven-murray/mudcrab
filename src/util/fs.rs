//! Filesystem helpers shared across install, archive extraction, and MO2 export.
//!
//! These were previously duplicated two-to-six times across `config/install.rs`
//! and `archive/mod.rs`; the copies had drifted only in error-message wording.

use crate::archive::ArchiveFilters;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// ASCII case-insensitive comparison.
///
/// Oblivion/Windows path and plugin-name semantics are case-insensitive, so
/// almost every filename comparison in the codebase needs this.
pub fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Find a direct child of `parent` whose name matches `child_name` ignoring case.
///
/// Deliberately does a literal comparison rather than a glob: Oblivion plugin
/// names routinely contain glob metacharacters (`Harvest [Flora] - DLCVileLair.esp`),
/// which a pattern match would silently read as a character class.
pub fn find_child_case_insensitive(parent: &Path, child_name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(parent).ok()?.flatten() {
        if eq_ci(entry.file_name().to_string_lossy().as_ref(), child_name) {
            return Some(entry.path());
        }
    }
    None
}

pub fn path_exists_case_insensitive(parent: &Path, child_name: &str) -> bool {
    find_child_case_insensitive(parent, child_name).is_some()
}

/// Resolve `path` on a case-sensitive filesystem when the caller's casing may
/// not match what is on disk (common for paths taken from Windows-authored
/// mod archives and INIs).
pub fn resolve_existing_path_case_insensitive(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_path_buf());
    }

    let mut resolved = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().ok()?
    };

    for component in path.components() {
        match component {
            Component::Prefix(_) => return None,
            Component::RootDir => resolved.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                let exact = resolved.join(name);
                if exact.exists() {
                    resolved = exact;
                    continue;
                }
                resolved = find_child_case_insensitive(&resolved, &name.to_string_lossy())?;
            }
        }
    }

    resolved.exists().then_some(resolved)
}

/// Normalise an archive-relative or config-relative path, rejecting anything
/// that could escape the destination root.
pub fn normalize_relative_path(value: &str) -> anyhow::Result<PathBuf> {
    let mut out = PathBuf::new();

    for component in Path::new(value).components() {
        match component {
            Component::Normal(seg) => out.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("invalid relative path '{value}'");
            }
        }
    }

    if out.as_os_str().is_empty() {
        anyhow::bail!("relative path must not be empty");
    }
    Ok(out)
}

pub fn write_text_file(path: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
    }
    std::fs::write(path, content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))
}

/// Prefixes of the scratch directories mudcrab creates under the temp dir.
const STAGING_PREFIXES: [&str; 2] = ["mudcrab-stage-", "mudcrab-sys-"];

/// Delete scratch directories left behind by runs that did not finish.
///
/// Both staging helpers remove their directory on success *and* on error, so
/// nothing leaks in normal operation -- but a killed process (Ctrl-C, a
/// timeout, an OOM) leaves the whole extraction behind, and these are the size
/// of the archive. Two abandoned runs of one 6.5 GB archive filled a 16 GB
/// `/tmp` and wedged the machine, which is how this was found.
///
/// Age-gated rather than pid-gated: a concurrent `mudcrab` may legitimately own
/// a fresh directory, and deleting it mid-extraction would corrupt that run.
/// Anything untouched for `max_age` belongs to nobody.
pub fn sweep_stale_staging_dirs(max_age: std::time::Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return 0;
    };
    let now = SystemTime::now();
    let mut removed = 0usize;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !STAGING_PREFIXES.iter().any(|prefix| name.starts_with(prefix)) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else { continue };
        if !metadata.is_dir() {
            continue;
        }
        let stale = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > max_age);
        if !stale {
            continue;
        }
        if std::fs::remove_dir_all(entry.path()).is_ok() {
            tracing::info!(
                path = %entry.path().display(),
                "removed a staging directory left by an interrupted run"
            );
            removed += 1;
        }
    }

    removed
}

/// Build a unique temporary staging directory path for `target_root`.
///
/// Includes both PID and nanosecond timestamp: a timestamp alone collides when
/// two extractions start within the same millisecond.
///
/// NOTE this lives under the system temp dir, which on many Linux setups is a
/// tmpfs -- i.e. RAM. Extraction is archive-sized, so a multi-gigabyte mod is
/// unpacked into memory. Staging beside the destination instead would be
/// better on both counts, but it is a change of contract for every caller.
pub fn staging_dir_for(target_root: &Path) -> anyhow::Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| anyhow::anyhow!("system clock error: {err}"))?
        .as_nanos();

    let name: String = target_root
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("mod")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();

    Ok(std::env::temp_dir().join(format!(
        "mudcrab-stage-{name}-{}-{stamp}",
        std::process::id()
    )))
}

/// Hard-link `source` to `destination`, falling back to a copy across devices.
pub fn link_or_copy(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
    }

    if destination.exists() {
        std::fs::remove_file(destination)
            .map_err(|err| anyhow::anyhow!("failed to replace {}: {err}", destination.display()))?;
    }

    match std::fs::hard_link(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => std::fs::copy(source, destination).map(|_| ()).map_err(|err| {
            anyhow::anyhow!(
                "failed to stage {} to {}: {err}",
                source.display(),
                destination.display()
            )
        }),
    }
}

/// Lowercase every directory component of a relative path, leaving the file
/// name alone.
///
/// Oblivion and MO2 both treat asset paths case-insensitively, and mod archives
/// are authored on Windows where that is free. On Linux it is not: two archives
/// contributing `Sound/` and `sound/` to the same mod produce two directories
/// that never overlay, and the game sees whichever one it happens to look in.
/// Part 9's OOO voice files hit exactly this, twice.
///
/// Staging everything into lowercase directories makes the overlay behave the
/// way the mod authors assumed it would. File names are left as they are: they
/// are what BSA packing and the Oracle comparison both key on, and nothing
/// merges two files whose names differ only in case.
pub fn lowercase_dir_components(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let mut components: Vec<_> = path.components().collect();
    let file_name = components.pop();

    for component in components {
        out.push(component.as_os_str().to_string_lossy().to_lowercase());
    }
    if let Some(file_name) = file_name {
        out.push(file_name.as_os_str());
    }
    out
}

/// Lowercase every component of a relative path that names only directories.
pub fn lowercase_path(path: &Path) -> PathBuf {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

/// Whether a copy folds directory names to lowercase as it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirCase {
    /// Copy paths exactly as they are.
    ///
    /// Required anywhere the tree will be *read again* -- unpacking into a
    /// staging directory, or assembling build layers -- because FOMOD and BAIN
    /// scripts name their sources in the archive's own casing, and folding it
    /// first makes them unfindable.
    Preserve,
    /// Fold directory names to lowercase.
    ///
    /// Used on the last hop, into the mod's own folder, where nothing reads the
    /// tree by name again. That is where two archives contributing `Sound/` and
    /// `sound/` otherwise become two directories that never overlay.
    Fold,
}

/// Recursively copy `source_root` into `destination_root`, applying `filters`
/// to each file's root-relative path. Returns the number of files copied.
///
/// Paths are copied as they are; see `copy_filtered_tree_folded` for the
/// variant that lowercases directory names.
pub fn copy_filtered_tree(
    source_root: &Path,
    destination_root: &Path,
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    copy_filtered_tree_with_case(source_root, destination_root, filters, DirCase::Preserve)
}

/// As `copy_filtered_tree`, folding every directory name to lowercase.
pub fn copy_filtered_tree_folded(
    source_root: &Path,
    destination_root: &Path,
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    copy_filtered_tree_with_case(source_root, destination_root, filters, DirCase::Fold)
}

pub fn copy_filtered_tree_with_case(
    source_root: &Path,
    destination_root: &Path,
    filters: &ArchiveFilters,
    case: DirCase,
) -> anyhow::Result<usize> {
    let mut copied = 0usize;
    copy_tree_recursive(source_root, source_root, destination_root, filters, case, &mut copied)?;
    Ok(copied)
}

fn copy_tree_recursive(
    current: &Path,
    source_root: &Path,
    destination_root: &Path,
    filters: &ArchiveFilters,
    case: DirCase,
    copied: &mut usize,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?
    {
        let entry = entry.map_err(|err| {
            anyhow::anyhow!("failed to iterate directory {}: {err}", current.display())
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            anyhow::anyhow!("failed to read file type for {}: {err}", path.display())
        })?;

        if file_type.is_dir() {
            copy_tree_recursive(&path, source_root, destination_root, filters, case, copied)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let rel = path.strip_prefix(source_root).map_err(|err| {
            anyhow::anyhow!(
                "failed to compute relative path for {} from {}: {err}",
                path.display(),
                source_root.display()
            )
        })?;
        if !filters.should_extract(&rel.to_string_lossy().replace('\\', "/")) {
            continue;
        }

        let destination = match case {
            DirCase::Preserve => destination_root.join(rel),
            DirCase::Fold => destination_root.join(lowercase_dir_components(rel)),
        };
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
        }
        std::fs::copy(&path, &destination).map_err(|err| {
            anyhow::anyhow!(
                "failed to copy {} to {}: {err}",
                path.display(),
                destination.display()
            )
        })?;
        *copied += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercase_dir_components_leaves_the_file_name_alone() {
        assert_eq!(
            lowercase_dir_components(Path::new("Sound/Voice/Line.mp3")),
            PathBuf::from("sound/voice/Line.mp3")
        );
        assert_eq!(
            lowercase_dir_components(Path::new("MESHES/Characters/_Male/skeleton.nif")),
            PathBuf::from("meshes/characters/_male/skeleton.nif")
        );
        // A bare file name has no directories to fold.
        assert_eq!(
            lowercase_dir_components(Path::new("Readme.txt")),
            PathBuf::from("Readme.txt")
        );
        assert_eq!(lowercase_dir_components(Path::new("")), PathBuf::from(""));
    }

    #[test]
    fn lowercase_dir_components_is_for_relative_paths_only() {
        // Applying it to an absolute destination folds the tempdir and the mod
        // folder along with the asset directories, and the copy then lands
        // somewhere that does not exist. Callers must fold the relative part and
        // join it, never fold the joined result.
        let folded = lowercase_dir_components(Path::new("/tmp/MyInstance/Mods/Example/Textures/a.dds"));
        assert_eq!(
            folded,
            PathBuf::from("/tmp/myinstance/mods/example/textures/a.dds"),
            "documents the hazard: every component folds, including ones that are \
             not the mod's own asset directories"
        );
    }

    #[test]
    fn normalize_rejects_escapes_and_absolutes() {
        assert!(normalize_relative_path("../evil").is_err());
        assert!(normalize_relative_path("/etc/passwd").is_err());
        assert!(normalize_relative_path("").is_err());
        assert_eq!(
            normalize_relative_path("./a/b").unwrap(),
            PathBuf::from("a/b")
        );
    }

    #[test]
    fn staging_dir_includes_pid_and_is_unique() {
        let a = staging_dir_for(Path::new("/tmp/Some Mod")).unwrap();
        let b = staging_dir_for(Path::new("/tmp/Some Mod")).unwrap();
        assert_ne!(a, b, "two staging dirs in quick succession must not collide");
        assert!(a.to_string_lossy().contains(&std::process::id().to_string()));
        // non-alphanumerics in the mod name are sanitised
        assert!(a.file_name().unwrap().to_string_lossy().contains("Some_Mod"));
    }

    #[test]
    fn case_insensitive_lookup_handles_glob_metacharacters() {
        let dir = tempfile::tempdir().unwrap();
        // A real MOFAM plugin name: '[Flora]' is a glob character class.
        let name = "Harvest [Flora] - DLCVileLair.esp";
        std::fs::write(dir.path().join(name), b"x").unwrap();

        assert!(find_child_case_insensitive(dir.path(), name).is_some());
        assert!(find_child_case_insensitive(dir.path(), &name.to_uppercase()).is_some());
        assert!(find_child_case_insensitive(dir.path(), "Harvest F.esp").is_none());
    }
}
