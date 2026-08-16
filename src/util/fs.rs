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

/// Build a unique temporary staging directory path for `target_root`.
///
/// Includes both PID and nanosecond timestamp: a timestamp alone collides when
/// two extractions start within the same millisecond.
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

/// Recursively copy `source_root` into `destination_root`, applying `filters`
/// to each file's root-relative path. Returns the number of files copied.
pub fn copy_filtered_tree(
    source_root: &Path,
    destination_root: &Path,
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    let mut copied = 0usize;
    copy_tree_recursive(source_root, source_root, destination_root, filters, &mut copied)?;
    Ok(copied)
}

fn copy_tree_recursive(
    current: &Path,
    source_root: &Path,
    destination_root: &Path,
    filters: &ArchiveFilters,
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
            copy_tree_recursive(&path, source_root, destination_root, filters, copied)?;
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

        let destination = destination_root.join(rel);
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
