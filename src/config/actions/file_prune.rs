//! `file_prune`: delete staged files matching globs.
//!
//! Exists because `pack_bsa`'s `exclude` cannot express it: the loose files
//! have to survive long enough to be packed, and only then be removed. Actions
//! run in declaration order (`apply_all`), so a `file_prune` written after a
//! `pack_bsa` deletes exactly what that archive now contains.

use super::ActionCx;
use crate::archive::ArchiveFilters;
use crate::config::schema::FilePruneAction;
use std::path::Path;

pub(super) fn apply(action: &FilePruneAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!("{}: file_prune is only valid as a per-mod action", cx.owner);
    };

    // An empty pattern list would compile to a glob set that matches
    // everything, which would delete the entire staged mod.
    if action.paths.is_empty() {
        anyhow::bail!("{}: file_prune requires at least one path pattern", cx.owner);
    }

    for pattern in &action.paths {
        reject_escaping_pattern(cx.owner, pattern)?;
    }

    if cx.settings.dry_run {
        tracing::info!(
            owner = cx.owner,
            target = %mod_target.display(),
            paths = ?action.paths,
            "install dry-run file_prune action"
        );
        return Ok(());
    }

    // One filter per pattern, so a pattern that matches nothing can be named.
    // Cheap: the tree is walked once per pattern, and these trees are a staged
    // mod, not a game install.
    let mut deleted = 0usize;
    let mut barren = Vec::new();
    for pattern in &action.paths {
        // Staged-tree semantics, not archive-entry semantics: `/` separates,
        // and case does not matter. Both differences bit in Part 11 --
        // `NoMushroomStalks` matched nothing against the folded folder on disk,
        // and `textures/rocks/*.dds` matched straight through into the
        // `underwater/` folder the guide says to keep.
        let filters = ArchiveFilters::new_for_staged_tree(&expand_directory_pattern(pattern), &[])?;
        let mut matched = 0usize;
        prune(mod_target, mod_target, &filters, &mut matched)?;
        if matched == 0 {
            barren.push(pattern.clone());
        }
        deleted += matched;
    }

    // A prune that deletes nothing is always a mistake, and a silent one: the
    // install still succeeds and the loose files it was meant to remove stay in
    // the VFS, shadowing whatever the mod packed. Found exactly that way -- the
    // Oracle diff, not the install, is what noticed.
    if !barren.is_empty() {
        anyhow::bail!(
            "{}: file_prune pattern{} {} matched nothing under {}. Patterns are matched \
             against paths relative to the staged folder, case-insensitively.",
            cx.owner,
            if barren.len() == 1 { "" } else { "s" },
            barren
                .iter()
                .map(|pattern| format!("'{pattern}'"))
                .collect::<Vec<_>>()
                .join(", "),
            mod_target.display()
        );
    }

    tracing::info!(
        owner = cx.owner,
        target = %mod_target.display(),
        deleted,
        "pruned staged files"
    );
    Ok(())
}

/// A pattern naming a plain directory also matches everything below it.
///
/// `paths = ["meshes"]` means the folder, which is what the guide means every
/// time it says "delete the loose meshes & textures folders". As a raw glob it
/// would match only a *file* called `meshes` and silently delete nothing.
/// Patterns carrying glob syntax are left exactly as written.
fn expand_directory_pattern(pattern: &str) -> Vec<String> {
    let trimmed = pattern.trim_end_matches('/');
    if trimmed.is_empty() || trimmed.contains(['*', '?', '[', '{']) {
        return vec![pattern.to_string()];
    }
    vec![trimmed.to_string(), format!("{trimmed}/**")]
}

/// Patterns are matched against paths relative to the staged folder, so a
/// traversal cannot escape by construction -- but a pattern that tries to is a
/// mistake worth reporting rather than silently matching nothing.
fn reject_escaping_pattern(owner: &str, pattern: &str) -> anyhow::Result<()> {
    let normalized = pattern.replace('\\', "/");
    if normalized.starts_with('/') {
        anyhow::bail!("{owner}: file_prune pattern '{pattern}' must be relative");
    }
    if normalized.split('/').any(|segment| segment == "..") {
        anyhow::bail!(
            "{owner}: file_prune pattern '{pattern}' must stay inside the mod folder"
        );
    }
    Ok(())
}

/// Delete matching files below `current`, then remove directories left empty.
///
/// Returns whether `current` is empty afterwards, so the recursion can clean up
/// the folder tree a pack leaves behind.
fn prune(
    current: &Path,
    root: &Path,
    filters: &ArchiveFilters,
    deleted: &mut usize,
) -> anyhow::Result<bool> {
    let entries = std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?;

    let mut remaining = 0usize;
    for entry in entries {
        let entry = entry.map_err(|err| {
            anyhow::anyhow!("failed to iterate directory {}: {err}", current.display())
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            anyhow::anyhow!("failed to read file type for {}: {err}", path.display())
        })?;

        if file_type.is_dir() {
            if prune(&path, root, filters, deleted)? {
                // The directory is empty now, so the loose folder a pack
                // replaced does not linger.
                std::fs::remove_dir(&path).map_err(|err| {
                    anyhow::anyhow!("failed to remove {}: {err}", path.display())
                })?;
            } else {
                remaining += 1;
            }
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .map_err(|err| {
                anyhow::anyhow!("failed to compute relative path for {}: {err}", path.display())
            })?
            .to_string_lossy()
            .replace('\\', "/");

        if filters.should_extract(&relative) {
            std::fs::remove_file(&path)
                .map_err(|err| anyhow::anyhow!("failed to remove {}: {err}", path.display()))?;
            *deleted += 1;
        } else {
            remaining += 1;
        }
    }

    Ok(remaining == 0)
}
