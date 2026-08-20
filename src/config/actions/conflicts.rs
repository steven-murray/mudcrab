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
    except: &[String],
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
            // Reported, not silent: this is the branch that lets the emptiness
            // check below pass, so a run that looks like it did nothing should
            // say why it was allowed to.
            already_handled += 1;
        } else {
            matched.push(relative);
        }
    }

    // A carve-out that carves nothing out is stale -- the archives moved, or
    // the relationship changed -- and silently keeping it would leave a reason
    // recorded for a thing that no longer happens.
    let excepted: BTreeSet<String> = except.iter().map(|path| path.to_lowercase()).collect();
    let mut unused: Vec<&String> = except.iter().collect();
    matched.retain(|relative| {
        let key = relative.to_lowercase();
        if excepted.contains(&key) {
            unused.retain(|path| path.to_lowercase() != key);
            tracing::info!(
                owner = cx.owner,
                path = %relative,
                "kept a file the conflict selection named, per `except`"
            );
            return false;
        }
        true
    });
    if !unused.is_empty() {
        anyhow::bail!(
            "{}: conflicts_with `except` names {} that the selection did not pick. \
             Either the path is wrong or the exception is no longer needed.",
            cx.owner,
            unused
                .iter()
                .map(|path| format!("'{path}'"))
                .collect::<Vec<_>>()
                .join(", ")
        );
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

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::config::install::InstallSettings;
    use crate::config::schema::PersonalizedMod;
    use crate::config::tools::ToolsConfig;
    use std::path::PathBuf;

    /// A partner mod that exists on disk, so `staged_paths` reads its folder
    /// rather than trying to predict it from archives that are not there.
    pub(super) fn instance(
        root: &std::path::Path,
        partner_files: &[&str],
        subject_files: &[&str],
    ) -> (PathBuf, PathBuf) {
        let mods_dir = root.join("mods");
        for (mod_name, files) in [("Partner", partner_files), ("Subject", subject_files)] {
            for file in files {
                let full = mods_dir.join(mod_name).join(file);
                std::fs::create_dir_all(full.parent().unwrap()).unwrap();
                std::fs::write(full, b"DATA").unwrap();
            }
        }
        (mods_dir.clone(), mods_dir.join("Subject"))
    }

    pub(super) fn settings(mods_dir: PathBuf) -> InstallSettings {
        InstallSettings {
            cache_dir: mods_dir.join("cache"),
            mods_dir,
            mo2_instance_dir: None,
            profile_name: String::new(),
            game_dir: None,
            game_root_dir: None,
            execute_actions: true,
            dry_run: false,
            tools: ToolsConfig::default(),
            filter: Default::default(),
            archive_search_paths: Vec::new(),
            force_merges: false,
            force: false,
        }
    }

    pub(super) fn partner() -> PersonalizedMod {
        PersonalizedMod {
            id: "Partner".to_string(),
            oracle_name: None,
            section: Vec::new(),
            mod_type: None,
            merge: None,
            archives: Vec::new(),
            files: Vec::new(),
            actions: Vec::new(),
        }
    }

    #[test]
    fn a_selection_finds_the_files_the_partner_also_provides() {
        let dir = tempfile::tempdir().unwrap();
        let (mods_dir, subject) = instance(
            dir.path(),
            &["meshes/a.nif", "meshes/shared.nif"],
            &["meshes/shared.nif", "textures/mine.dds"],
        );
        let settings = settings(mods_dir);
        let plan = [partner()];
        let cx = ActionCx {
            owner: "Subject",
            settings: &settings,
            mod_target: Some(&subject),
            plan_mods: &plan,
            active_plugins: &Default::default(),
        };

        let files =
            conflicting_files(&cx, &subject, &["Partner".to_string()], None, &[]).unwrap();
        assert_eq!(files, ["meshes/shared.nif"]);
    }

    /// The bug a rerun of Part 18 found. A file this action hid last time is
    /// called `x.nif.mohidden`, so it stops matching its own name, the selection
    /// comes back empty, and the "selected no files" check -- which exists to
    /// catch a selector that was *wrong* -- fires on one that already worked.
    #[test]
    fn rerunning_a_selection_that_already_hid_its_files_is_not_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let (mods_dir, subject) = instance(
            dir.path(),
            &["meshes/shared.nif"],
            &["meshes/shared.nif.mohidden", "textures/mine.dds"],
        );
        let settings = settings(mods_dir);
        let plan = [partner()];
        let cx = ActionCx {
            owner: "Subject",
            settings: &settings,
            mod_target: Some(&subject),
            plan_mods: &plan,
            active_plugins: &Default::default(),
        };

        let files = conflicting_files(&cx, &subject, &["Partner".to_string()], None, &[])
            .expect("a selection that already ran is not a failure");
        assert!(files.is_empty(), "nothing left to hide, and nothing rehidden");
    }

    /// The check the fix must not weaken: two mods that genuinely do not
    /// overlap, which is what a selector stated the wrong way round looks like.
    #[test]
    fn a_selection_that_overlaps_nothing_is_still_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (mods_dir, subject) = instance(
            dir.path(),
            &["meshes/theirs.nif"],
            &["meshes/mine.nif", "meshes/hidden.nif.mohidden"],
        );
        let settings = settings(mods_dir);
        let plan = [partner()];
        let cx = ActionCx {
            owner: "Subject",
            settings: &settings,
            mod_target: Some(&subject),
            plan_mods: &plan,
            active_plugins: &Default::default(),
        };

        // Note the subject has a hidden file: being hidden is not on its own
        // enough to satisfy the check, only being hidden *and* provided by the
        // named mod.
        let err = conflicting_files(&cx, &subject, &["Partner".to_string()], None, &[])
            .expect_err("no overlap at all");
        assert!(err.to_string().contains("selected no files"), "{err}");
    }

    #[test]
    fn under_restricts_the_selection_to_one_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let (mods_dir, subject) = instance(
            dir.path(),
            &["meshes/shared.nif", "textures/shared.dds"],
            &["meshes/shared.nif", "textures/shared.dds"],
        );
        let settings = settings(mods_dir);
        let plan = [partner()];
        let cx = ActionCx {
            owner: "Subject",
            settings: &settings,
            mod_target: Some(&subject),
            plan_mods: &plan,
            active_plugins: &Default::default(),
        };

        let files =
            conflicting_files(&cx, &subject, &["Partner".to_string()], Some("textures"), &[]).unwrap();
        assert_eq!(files, ["textures/shared.dds"]);
    }
}

#[cfg(test)]
mod except_tests {
    use super::tests::{instance, partner, settings};
    use super::*;

    #[test]
    fn a_named_exception_is_kept_and_a_stale_one_is_an_error() {
        // Part 24's `chainmailm1.nif`: WAC provides it, so the relationship
        // selects it, but Steven keeps it deliberately -- a loose copy in
        // another mod wins that path regardless.
        let dir = tempfile::tempdir().unwrap();
        let (mods_dir, subject) = instance(
            dir.path(),
            &["meshes/a.nif", "meshes/keep.nif"],
            &["meshes/a.nif", "meshes/keep.nif"],
        );
        let settings = settings(mods_dir);
        let plan = [partner()];
        let cx = ActionCx {
            owner: "Subject",
            settings: &settings,
            mod_target: Some(&subject),
            plan_mods: &plan,
            active_plugins: &Default::default(),
        };

        let files = conflicting_files(
            &cx,
            &subject,
            &["Partner".to_string()],
            None,
            &["meshes/keep.nif".to_string()],
        )
        .unwrap();
        assert_eq!(files, ["meshes/a.nif"], "the exception survives the selection");

        // An exception the selection never picks is stale: the archives moved,
        // or the relationship changed, and the recorded reason now describes
        // nothing.
        let err = conflicting_files(
            &cx,
            &subject,
            &["Partner".to_string()],
            None,
            &["meshes/not-in-either.nif".to_string()],
        )
        .expect_err("a carve-out that carves nothing is stale");
        assert!(err.to_string().contains("not-in-either.nif"), "{err}");
    }
}
