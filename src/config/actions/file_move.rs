//! `file_move`: relocate a staged file or folder within the mod.
//!
//! The guide's phrasing is "move X to the optional folder": a plugin that ships
//! active but which this list does not want in the load order, parked where MO2
//! shows it as available rather than deleted. `optional/` is a convention, not a
//! special case, so this is a plain rename inside the staged folder.

use super::ActionCx;
use crate::config::schema::FileMoveAction;
use crate::util::fs::{lowercase_dir_components, normalize_relative_path};
use std::path::Path;

pub(super) fn apply(action: &FileMoveAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!("{}: file_move is only valid as a per-mod action", cx.owner);
    };

    let from = mod_target.join(normalize_relative_path(&action.from)?);
    // Directories we create fold to lowercase like every other staged folder.
    let to = mod_target.join(lowercase_dir_components(&normalize_relative_path(&action.to)?));

    if cx.settings.dry_run {
        tracing::info!(
            owner = cx.owner,
            from = %from.display(),
            to = %to.display(),
            "install dry-run file_move action"
        );
        return Ok(());
    }

    let Some(source) = resolve_case_insensitive(mod_target, Path::new(&normalize_relative_path(&action.from)?))
    else {
        anyhow::bail!(
            "{}: file_move found no '{}' under {}. The path is relative to the staged \
             folder and matched case-insensitively.",
            cx.owner,
            action.from,
            mod_target.display()
        );
    };

    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
    }

    // Already moved: a re-run must not fail on the second pass.
    if to.exists() && source == to {
        return Ok(());
    }

    std::fs::rename(&source, &to).map_err(|err| {
        anyhow::anyhow!(
            "failed to move {} to {}: {err}",
            source.display(),
            to.display()
        )
    })?;

    tracing::info!(
        owner = cx.owner,
        from = %source.display(),
        to = %to.display(),
        "moved staged file"
    );
    Ok(())
}

/// Walk the path a segment at a time, matching each case-insensitively, so a
/// path transcribed from the guide finds a Windows-cased archive entry.
fn resolve_case_insensitive(root: &Path, relative: &Path) -> Option<std::path::PathBuf> {
    let mut current = root.to_path_buf();
    for segment in relative.components() {
        let wanted = segment.as_os_str().to_str()?.to_ascii_lowercase();
        let matched = std::fs::read_dir(&current).ok()?.flatten().find(|entry| {
            entry.file_name().to_string_lossy().to_ascii_lowercase() == wanted
        })?;
        current = matched.path();
    }
    Some(current)
}
