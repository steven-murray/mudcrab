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
    if action.paths.is_empty() && action.conflicts_with.is_empty() {
        anyhow::bail!(
            "{}: file_prune requires at least one path pattern or a conflicts_with selection",
            cx.owner
        );
    }

    for pattern in &action.paths {
        reject_escaping_pattern(cx.owner, pattern)?;
    }

    if cx.settings.dry_run {
        tracing::info!(
            owner = cx.owner,
            target = %mod_target.display(),
            paths = ?action.paths,
            conflicts_with = ?action.conflicts_with,
            "install dry-run file_prune action"
        );
        return Ok(());
    }

    // Which exceptions the glob patterns account for, worked out before
    // anything is deleted so the tree still holds the files being asked about.
    // Whatever is left over belongs to the `conflicts_with` selection, which
    // rejects an exception it cannot place -- so between them every `except`
    // has to be honoured by somebody.
    let kept_by_patterns = exceptions_the_patterns_select(mod_target, action)?;
    // Compared in the normalised spelling on both sides: `kept_by_patterns`
    // holds the tree's own casing, for logging, and the guide writes an
    // `except` in the casing the mod page uses. Neither is authoritative.
    let placed: std::collections::BTreeSet<String> = kept_by_patterns
        .iter()
        .map(|path| normalize_except(path))
        .collect();
    let unplaced: Vec<String> = action
        .except
        .iter()
        .filter(|path| !placed.contains(&normalize_except(path)))
        .cloned()
        .collect();

    // Resolved first, and separately: these are exact paths another mod also
    // provides, not patterns, so they are deleted by name rather than fed
    // through the glob machinery below.
    let mut deleted_by_conflict = 0usize;
    if !action.conflicts_with.is_empty() {
        let files = super::conflicts::conflicting_files(
            cx,
            mod_target,
            &action.conflicts_with,
            action.under.as_deref(),
            &unplaced,
        )?;
        for relative in &files {
            let path = mod_target.join(relative);
            std::fs::remove_file(&path)
                .map_err(|err| anyhow::anyhow!("failed to remove {}: {err}", path.display()))?;
        }
        deleted_by_conflict = files.len();
        remove_empty_dirs(mod_target)?;
        tracing::info!(
            owner = cx.owner,
            conflicts_with = ?action.conflicts_with,
            deleted = deleted_by_conflict,
            "pruned files another mod provides"
        );
    }

    if action.paths.is_empty() {
        return Ok(());
    }

    // An exception nobody could place is stale in exactly the way a
    // `conflicts_with` one is: the archive moved, or the guide's carve-out no
    // longer describes anything, and keeping it records a reason for a thing
    // that does not happen. When there is a `conflicts_with`, it has already
    // had its say above.
    if action.conflicts_with.is_empty() && !unplaced.is_empty() {
        anyhow::bail!(
            "{}: file_prune `except` names {} that no pattern selects. Either the path is \
             wrong or the exception is no longer needed.",
            cx.owner,
            unplaced
                .iter()
                .map(|path| format!("'{path}'"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    for path in &kept_by_patterns {
        tracing::info!(
            owner = cx.owner,
            path = %path,
            "kept a file the prune patterns named, per `except`"
        );
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
        let filters = ArchiveFilters::new_for_staged_tree(
            &expand_directory_pattern(pattern),
            &action.except,
        )?;
        let mut matched = 0usize;
        prune(mod_target, mod_target, &filters, &mut matched)?;
        // A pattern whose every match was excepted still named something, so it
        // is not the silent no-op the check below is for.
        if matched == 0 && !selects_only_exceptions(pattern, &kept_by_patterns)? {
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
        deleted = deleted + deleted_by_conflict,
        "pruned staged files"
    );
    Ok(())
}

/// Spelling an `except` entry is compared in: `/`-separated and lowercased,
/// matching the case-insensitive way patterns are matched against the tree.
fn normalize_except(path: &str) -> String {
    path.replace('\\', "/").trim_matches('/').to_lowercase()
}

/// The `except` entries that the action's own patterns would otherwise delete.
///
/// Read off the staged tree before anything is removed, so an exception naming
/// a file that is not there is distinguishable from one the patterns simply do
/// not reach. Returned in the tree's own spelling, for logging.
fn exceptions_the_patterns_select(
    mod_target: &Path,
    action: &FilePruneAction,
) -> anyhow::Result<Vec<String>> {
    if action.except.is_empty() || action.paths.is_empty() {
        return Ok(Vec::new());
    }

    let expanded: Vec<String> = action
        .paths
        .iter()
        .flat_map(|pattern| expand_directory_pattern(pattern))
        .collect();
    let selection = ArchiveFilters::new_for_staged_tree(&expanded, &[])?;
    let excepted: std::collections::BTreeSet<String> =
        action.except.iter().map(|path| normalize_except(path)).collect();

    Ok(
        crate::config::install::layout::bain::list_relative_paths(mod_target)?
            .into_iter()
            .filter(|relative| {
                excepted.contains(&normalize_except(relative)) && selection.should_extract(relative)
            })
            .collect(),
    )
}

/// Whether every file this one pattern picked was held back by an `except`.
///
/// Without this a pattern like `*.esp` guarding a single kept plugin would look
/// like a pattern that matched nothing, which is an error for good reason
/// everywhere else.
fn selects_only_exceptions(pattern: &str, kept: &[String]) -> anyhow::Result<bool> {
    if kept.is_empty() {
        return Ok(false);
    }
    let filters = ArchiveFilters::new_for_staged_tree(&expand_directory_pattern(pattern), &[])?;
    Ok(kept.iter().any(|path| filters.should_extract(path)))
}

/// Drop directories a by-name deletion emptied, as the glob path already does.
fn remove_empty_dirs(current: &Path) -> anyhow::Result<bool> {
    let mut remaining = 0usize;
    for entry in std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?
    {
        let entry = entry
            .map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", current.display()))?;
        let path = entry.path();
        if path.is_dir() {
            if remove_empty_dirs(&path)? {
                std::fs::remove_dir(&path)
                    .map_err(|err| anyhow::anyhow!("failed to remove {}: {err}", path.display()))?;
            } else {
                remaining += 1;
            }
        } else {
            remaining += 1;
        }
    }
    Ok(remaining == 0)
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

#[cfg(test)]
mod except_tests {
    use super::super::conflicts::tests::{instance, settings};
    use super::*;
    use crate::config::actions::ActionCx;
    use crate::config::schema::FilePruneAction;

    fn action(paths: &[&str], except: &[&str]) -> FilePruneAction {
        FilePruneAction {
            paths: paths.iter().map(ToString::to_string).collect(),
            conflicts_with: Vec::new(),
            under: None,
            except: except.iter().map(ToString::to_string).collect(),
        }
    }

    /// Part 25 row 9: "delete every esp except DispMiscPatch_Ducks and Swans -
    /// Reedstand Patch.esp". Said as the guide says it, rather than as a
    /// hand-copied list of the other nine -- which would go stale the next time
    /// the collection gains a patch.
    #[test]
    fn a_pattern_can_carve_out_the_one_file_the_guide_keeps() {
        let dir = tempfile::tempdir().unwrap();
        let (mods_dir, subject) = instance(
            dir.path(),
            &[],
            &["a.esp", "DispMiscPatch_Keep.esp", "b.esp", "meshes/x.nif"],
        );
        let settings = settings(mods_dir);
        let cx = ActionCx {
            owner: "Subject",
            settings: &settings,
            mod_target: Some(&subject),
            plan_mods: &[],
            active_plugins: &Default::default(),
        };

        // Spelled in the guide's casing, which is not the tree's: the real row
        // is mixed-case on both sides and a case-sensitive compare passed every
        // test written in lowercase before failing on the actual archive.
        apply(&action(&["*.esp"], &["dispmiscpatch_keep.ESP"]), &cx).expect("prune");

        assert!(!subject.join("a.esp").exists());
        assert!(!subject.join("b.esp").exists());
        assert!(
            subject.join("DispMiscPatch_Keep.esp").exists(),
            "the carve-out survives"
        );
        assert!(subject.join("meshes/x.nif").exists(), "untouched by the pattern");
    }

    /// The invariant `conflicts_with` already holds, extended to patterns: an
    /// exception that carves nothing out records a reason for something that no
    /// longer happens.
    #[test]
    fn an_exception_no_pattern_selects_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let (mods_dir, subject) = instance(dir.path(), &[], &["a.esp", "keep.esp"]);
        let settings = settings(mods_dir);
        let cx = ActionCx {
            owner: "Subject",
            settings: &settings,
            mod_target: Some(&subject),
            plan_mods: &[],
            active_plugins: &Default::default(),
        };

        // Present on disk, but `*.nif` never reaches it.
        let err = apply(&action(&["*.nif"], &["keep.esp"]), &cx)
            .expect_err("the exception belongs to no pattern");
        assert!(err.to_string().contains("keep.esp"), "{err}");

        // Named but absent, which is the same mistake seen from the other side.
        let err = apply(&action(&["*.esp"], &["gone.esp"]), &cx)
            .expect_err("the exception names nothing on disk");
        assert!(err.to_string().contains("gone.esp"), "{err}");
    }

    /// A pattern whose only match is excepted deleted nothing, but it did not
    /// match nothing -- so the barren-pattern check must not fire on it.
    #[test]
    fn a_pattern_that_selects_only_its_exception_is_not_barren() {
        let dir = tempfile::tempdir().unwrap();
        let (mods_dir, subject) = instance(dir.path(), &[], &["keep.esp", "meshes/x.nif"]);
        let settings = settings(mods_dir);
        let cx = ActionCx {
            owner: "Subject",
            settings: &settings,
            mod_target: Some(&subject),
            plan_mods: &[],
            active_plugins: &Default::default(),
        };

        apply(&action(&["*.esp"], &["keep.esp"]), &cx)
            .expect("nothing to delete is not the same as nothing to match");
        assert!(subject.join("keep.esp").exists());
    }

    /// Unchanged: a pattern with no `except` at all that reaches nothing is
    /// still the silent no-op this action is built to reject.
    #[test]
    fn a_pattern_that_matches_nothing_is_still_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let (mods_dir, subject) = instance(dir.path(), &[], &["a.esp"]);
        let settings = settings(mods_dir);
        let cx = ActionCx {
            owner: "Subject",
            settings: &settings,
            mod_target: Some(&subject),
            plan_mods: &[],
            active_plugins: &Default::default(),
        };

        let err = apply(&action(&["textures/**"], &[]), &cx).expect_err("matched nothing");
        assert!(err.to_string().contains("matched nothing"), "{err}");
    }
}
