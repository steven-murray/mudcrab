//! Resolving `conflicts_with` into the files it names.
//!
//! The guide keeps saying "hide the files that conflict with X". Read off a
//! finished install that is a list of paths; said properly it is a relationship
//! between two mods, and the paths follow from it. Shared by `file_prune` and
//! `file_hide` because the guide uses "hide or delete" interchangeably and only
//! the disposal differs.

use super::ActionCx;
use crate::config::install::index::staged_paths;
use crate::config::install::safe_mod_dir_name;
use std::collections::BTreeSet;
use std::path::Path;

/// MO2's suffix for a file taken out of the virtual file system. Matches
/// `file_hide`, which is what puts it there.
const HIDDEN_SUFFIX: &str = ".mohidden";

/// Files of this mod that a named mod also provides.
///
/// Returned relative to `mod_target`, spelled as they are on disk, so the
/// caller can delete or rename them.
pub(super) fn conflicting_files(
    cx: &ActionCx<'_>,
    mod_target: &Path,
    conflicts_with: &[String],
    under: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    let mut theirs = BTreeSet::new();
    let mut skipped = Vec::new();

    for id in conflicts_with {
        let Some(other) = cx.plan_mods.iter().find(|entry| &entry.id == id) else {
            // A typo, caught here rather than as a conflict list that comes
            // back empty. The guide's own names for these mods were wrong three
            // separate ways in one row, and every one cost a rebuild.
            anyhow::bail!(
                "{}: conflicts_with names '{id}', which is not a mod in this modlist",
                cx.owner
            );
        };

        let installed_at = cx.settings.mods_dir.join(safe_mod_dir_name(&other.id)?);
        match staged_paths(
            other,
            Some(&installed_at),
            cx.settings,
            cx.active_plugins,
        ) {
            Ok(paths) => theirs.extend(paths),
            // A partial build legitimately has not fetched a later section's
            // archives yet. That is a reason to say so, not to fail -- but the
            // resulting prune is incomplete, so it says so loudly.
            Err(err) if !cx.settings.filter.matches(&other.section, &other.id) => {
                tracing::warn!(
                    owner = cx.owner,
                    conflicts_with = %id,
                    reason = %err,
                    "conflicts_with: skipping a mod outside this run; the selection is incomplete"
                );
                skipped.push(id.clone());
            }
            Err(err) => return Err(err.context(format!(
                "{}: conflicts_with could not determine what '{id}' provides",
                cx.owner
            ))),
        }
    }

    let under = under.map(|value| {
        let trimmed = value.replace('\\', "/").trim_matches('/').to_lowercase();
        format!("{trimmed}/")
    });

    let mut matched = Vec::new();
    let mut already_handled = 0usize;
    for relative in crate::config::install::layout::bain::list_relative_paths(mod_target)? {
        let key = relative.to_lowercase();
        // A file this action hid on a previous run no longer answers to its own
        // name. Compared without the suffix it still counts as selected, which
        // is what makes re-running a no-op rather than a "selected no files"
        // failure.
        let (key, hidden) = match key.strip_suffix(HIDDEN_SUFFIX) {
            Some(unhidden) => (unhidden.to_string(), true),
            None => (key, false),
        };
        if let Some(prefix) = &under
            && !key.starts_with(prefix.as_str())
        {
            continue;
        }
        if !theirs.contains(&key) {
            continue;
        }
        if hidden {
            already_handled += 1;
        } else {
            matched.push(relative);
        }
    }

    // Same rule as a `file_prune` pattern that matches nothing: a selection
    // resolving to no files is the shape of the first failed pass at Part 9,
    // and it succeeds silently while leaving every conflicting file in place.
    if matched.is_empty() && already_handled == 0 && skipped.len() < conflicts_with.len() {
        anyhow::bail!(
            "{}: conflicts_with [{}]{} selected no files. Either the mods do not overlap or the \
             relationship is stated the wrong way round.",
            cx.owner,
            conflicts_with.join(", "),
            under
                .as_deref()
                .map(|prefix| format!(" under '{prefix}'"))
                .unwrap_or_default(),
        );
    }

    Ok(matched)
}
