//! `file_hide`: take staged files out of the virtual file system, MO2-style.
//!
//! Mod Organizer 2 "hides" a file by renaming it to `<name>.mohidden`, which
//! drops it out of the VFS while leaving it on disk to be un-hidden later. It
//! does the same to a directory, hiding everything below it in one rename.
//!
//! The guide asks for this constantly -- "once installed, hide or delete X" --
//! and hiding is the better half of that choice: reversible, visible in MO2's
//! own UI, and what a hand-built instance actually contains. `file_prune` is
//! for the cases where the instruction is specifically to delete.

use super::ActionCx;
use crate::config::schema::FileHideAction;
use crate::util::fs::normalize_relative_path;
use std::path::Path;

/// The suffix MO2 appends. Matches `install::merge`, which hides merged
/// plugins the same way.
const MO2_HIDDEN_SUFFIX: &str = ".mohidden";

pub(super) fn apply(action: &FileHideAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!("{}: file_hide is only valid as a per-mod action", cx.owner);
    };

    if action.paths.is_empty() && action.conflicts_with.is_empty() {
        anyhow::bail!(
            "{}: file_hide requires at least one path or a conflicts_with selection",
            cx.owner
        );
    }

    if cx.settings.dry_run {
        tracing::info!(
            owner = cx.owner,
            target = %mod_target.display(),
            paths = ?action.paths,
            conflicts_with = ?action.conflicts_with,
            "install dry-run file_hide action"
        );
        return Ok(());
    }

    // Files another mod also provides, named by the relationship rather than
    // one at a time. These are exact paths, so they need none of the
    // case-insensitive resolution the guide-written ones below do.
    let mut hidden_by_conflict = 0usize;
    if !action.conflicts_with.is_empty() {
        for relative in super::conflicts::conflicting_files(
            cx,
            mod_target,
            &action.conflicts_with,
            action.under.as_deref(),
            &action.except,
        )? {
            hide_path(&mod_target.join(&relative))?;
            hidden_by_conflict += 1;
        }
        tracing::info!(
            owner = cx.owner,
            conflicts_with = ?action.conflicts_with,
            hidden = hidden_by_conflict,
            "hid files another mod provides"
        );
    }

    if action.paths.is_empty() {
        return Ok(());
    }

    let mut hidden = 0usize;
    let mut missing = Vec::new();
    for path in &action.paths {
        let relative = normalize_relative_path(path)?;
        let Some(source) = resolve_case_insensitive(mod_target, &relative) else {
            missing.push(path.clone());
            continue;
        };

        // Already hidden: a re-run must not produce `x.mohidden.mohidden`.
        if source
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.to_ascii_lowercase().ends_with(MO2_HIDDEN_SUFFIX))
        {
            hidden += 1;
            continue;
        }

        hide_path(&source)?;
        hidden += 1;
    }

    // Named paths, not globs: every one is something the guide pointed at, so a
    // path that is not there means the archive changed or the entry has a typo.
    // Either way the install would silently keep a file it was told to remove.
    if !missing.is_empty() {
        anyhow::bail!(
            "{}: file_hide found no {} under {}. Paths are relative to the staged \
             folder and matched case-insensitively.",
            cx.owner,
            missing
                .iter()
                .map(|path| format!("'{path}'"))
                .collect::<Vec<_>>()
                .join(", "),
            mod_target.display()
        );
    }

    tracing::info!(
        owner = cx.owner,
        target = %mod_target.display(),
        hidden = hidden + hidden_by_conflict,
        "hid staged files"
    );
    Ok(())
}

/// Rename to `<name>.mohidden`, which is how MO2 drops a file from the VFS.
fn hide_path(source: &Path) -> anyhow::Result<()> {
    // A re-run must not produce `x.mohidden.mohidden`.
    if source
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(MO2_HIDDEN_SUFFIX))
    {
        return Ok(());
    }

    let mut hidden_name = source.as_os_str().to_os_string();
    hidden_name.push(MO2_HIDDEN_SUFFIX);
    std::fs::rename(source, Path::new(&hidden_name))
        .map_err(|err| anyhow::anyhow!("failed to hide {}: {err}", source.display()))
}

/// Walk the path a segment at a time, matching each case-insensitively.
///
/// Mod archives are built on Windows and the guide transcribes paths by eye, so
/// `Textures/Characters/Nuska/Hair` and `textures/characters/nuska/hair` have to
/// find the same folder. A segment may also already carry `.mohidden` from an
/// earlier run.
fn resolve_case_insensitive(root: &Path, relative: &Path) -> Option<std::path::PathBuf> {
    let mut current = root.to_path_buf();

    for segment in relative.components() {
        let wanted = segment.as_os_str().to_str()?.to_ascii_lowercase();
        let hidden = format!("{wanted}{MO2_HIDDEN_SUFFIX}");

        let matched = std::fs::read_dir(&current).ok()?.flatten().find(|entry| {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            name == wanted || name == hidden
        })?;

        current = matched.path();
    }

    Some(current)
}
