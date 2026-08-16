//! Run the modlist's merges, then hide the plugins they consumed.
//!
//! Sequenced after every mod is installed and before LOOT sorts, so LOOT sees
//! the merged plugin and not its sources.

use crate::config::mo2::{hide_plugin, unhide_plugin, HideOutcome, MO2_HIDDEN_SUFFIX};
use crate::config::schema::{MergeSpec, PersonalizedPlan};
use crate::merge::{self, MergeRequest, MergeSource};
use crate::plugin::PluginName;
use crate::util::fs::{find_child_case_insensitive, write_text_file};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::manifest::{hash_merge_inputs, should_skip_merge, BuiltMerge};
use super::InstallSettings;

/// A plugin hidden on behalf of a merge, recorded so it can be restored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HiddenPlugin {
    /// The merge that consumed it.
    pub merge: String,
    /// Mod that owns the plugin, by id.
    pub mod_id: String,
    pub plugin: String,
    /// Path relative to mods/, so the record survives the instance moving.
    pub installed_path: String,
}

/// Where each installed mod actually landed, by id.
///
/// Not simply `mods_dir.join(id)`: a mod may have been renamed to
/// `<id> - <profile>` to avoid clobbering another profile's copy.
pub(crate) type InstalledPaths = HashMap<String, PathBuf>;

/// What one `install` run's merge phase produced.
pub(crate) struct MergeRunOutcome {
    pub(crate) hidden: Vec<HiddenPlugin>,
    /// Every merge the manifest should now claim as built, including the ones
    /// this run skipped because they were already current.
    pub(crate) built: Vec<BuiltMerge>,
}

pub(crate) fn run_merges(
    plan: &PersonalizedPlan,
    settings: &InstallSettings,
    installed: &InstalledPaths,
    previous_built: &HashMap<String, BuiltMerge>,
) -> anyhow::Result<MergeRunOutcome> {
    let mut hidden = Vec::new();
    let mut built = Vec::new();

    for (mod_entry, spec) in plan.merges() {
        // A merge is a mod like any other, so the filter decides whether this
        // run builds it. Its *sources* are looked up in `installed`, so a merge
        // in scope can still consume plugins from mods an earlier run put on
        // disk; only a source that was never installed is an error.
        if !settings.filter.matches(&mod_entry.section, &mod_entry.id) {
            // Carried forward for the same reason a filtered-out mod's entry
            // is: this run did not rebuild it, which is not the same as it
            // no longer being built.
            if let Some(previous) = previous_built.get(&mod_entry.id) {
                built.push(previous.clone());
            }
            tracing::debug!(
                merge = %mod_entry.id,
                "merge: skipped, excluded by --section/--only"
            );
            continue;
        }

        let target = installed
            .get(&mod_entry.id)
            .cloned()
            .unwrap_or_else(|| settings.mods_dir.join(&mod_entry.id));

        if settings.dry_run {
            tracing::info!(
                merge = %mod_entry.id,
                output = %spec.output,
                sources = spec.sources.len(),
                target = %target.display(),
                "merge: would build"
            );
            continue;
        }

        let sources = locate_sources(&mod_entry.id, spec, settings, installed)?;
        let input_hash = hash_merge_inputs(
            &mod_entry.id,
            spec,
            &plan.plugins,
            &sources.iter().map(|source| source.path.clone()).collect::<Vec<_>>(),
            &settings.mods_dir,
        )?;
        let output_path = target.join(&spec.output);

        // Rebuilding an 86-source merge is minutes of work to reproduce a file
        // that is already on disk byte for byte. The sources are still hidden
        // below either way, because hiding is what makes the merge take effect
        // and it is idempotent.
        if should_skip_merge(
            &output_path,
            &input_hash,
            previous_built.get(&mod_entry.id),
            settings.dry_run,
            settings.force_merges,
        ) {
            tracing::info!(
                merge = %mod_entry.id,
                output = %spec.output,
                sources = spec.sources.len(),
                status = "skipped",
                reason = "inputs unchanged since the last build",
                "merge: status"
            );
            if spec.hide_sources {
                hidden.extend(hide_sources(&mod_entry.id, spec, settings, installed)?);
            }
            built.push(BuiltMerge {
                id: mod_entry.id.clone(),
                input_hash,
                output: spec.output.clone(),
            });
            continue;
        }

        let report = build_merge(&mod_entry.id, spec, plan, &target, sources)?;

        if spec.hide_sources {
            hidden.extend(hide_sources(&mod_entry.id, spec, settings, installed)?);
        }

        built.push(BuiltMerge {
            id: mod_entry.id.clone(),
            input_hash,
            output: spec.output.clone(),
        });

        tracing::info!(
            merge = %mod_entry.id,
            output = %spec.output,
            sources = report.source_count,
            records = report.record_count,
            groups = report.group_count,
            remapped = report.remapped,
            clobbered = report.clobbered,
            hidden = hidden.len(),
            "merge: built"
        );
    }

    Ok(MergeRunOutcome { hidden, built })
}

/// Resolve every source plugin to a path on disk.
///
/// Split out of `build_merge` because the skip check needs the same paths --
/// they are what the input hash is taken over -- and resolving them twice, or
/// resolving them differently, would make the hash disagree with the build.
fn locate_sources(
    merge_id: &str,
    spec: &MergeSpec,
    settings: &InstallSettings,
    installed: &InstalledPaths,
) -> anyhow::Result<Vec<MergeSource>> {
    let mut sources = Vec::with_capacity(spec.sources.len());
    for source in &spec.sources {
        let mod_dir = installed
            .get(&source.mod_id)
            .cloned()
            .unwrap_or_else(|| settings.mods_dir.join(&source.mod_id));
        let path = locate_source_plugin(&mod_dir, &source.plugin).ok_or_else(|| {
            anyhow::anyhow!(
                "merge {merge_id}: {} is not in mod {} ({})",
                source.plugin,
                source.mod_id,
                mod_dir.display()
            )
        })?;
        sources.push(MergeSource {
            plugin: PluginName::new(&source.plugin),
            path,
        });
    }
    Ok(sources)
}

fn build_merge(
    merge_id: &str,
    spec: &MergeSpec,
    plan: &PersonalizedPlan,
    target: &Path,
    sources: Vec<MergeSource>,
) -> anyhow::Result<merge::MergeReport> {
    let request = MergeRequest {
        name: merge_id.to_string(),
        output: spec.output.clone(),
        sources,
        load_order: plan.plugins.iter().map(PluginName::new).collect(),
    };

    let output = merge::run(&request)?;

    std::fs::create_dir_all(target)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", target.display()))?;
    let plugin_path = target.join(&spec.output);
    std::fs::write(&plugin_path, output.plugin.to_bytes())
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", plugin_path.display()))?;

    write_merge_sidecar(target, merge_id, spec, &output)?;

    Ok(output.report)
}

/// Emit zMerge's `merge - <name>/` sidecar directory.
///
/// `map.json` is written in zEdit's exact shape so it can be diffed straight
/// against a real zMerge run; the report is ours and records what the merge
/// actually did, which is what the manual verification loop reads.
fn write_merge_sidecar(
    target: &Path,
    merge_id: &str,
    spec: &MergeSpec,
    output: &merge::MergeOutput,
) -> anyhow::Result<()> {
    let dir = target.join(format!("merge - {merge_id}"));
    std::fs::create_dir_all(&dir)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", dir.display()))?;

    write_text_file(&dir.join("map.json"), &merge::run::map_json(&output.allocation))?;

    let report = serde_json::json!({
        "name": output.report.name,
        "output": output.report.output,
        "method": spec.method,
        "masters": output.report.masters,
        "sources": spec.sources,
        "source_count": output.report.source_count,
        "record_count": output.report.record_count,
        "group_count": output.report.group_count,
        "remapped": output.report.remapped,
        "clobbered": output.report.clobbered,
        "next_object_id": format!("{:06X}", output.report.next_object_id),
    });
    let payload = serde_json::to_string_pretty(&report)
        .map_err(|err| anyhow::anyhow!("failed to serialize merge report: {err}"))?;
    write_text_file(&dir.join("mudcrab-merge.json"), &payload)?;

    Ok(())
}

fn hide_sources(
    merge_id: &str,
    spec: &MergeSpec,
    settings: &InstallSettings,
    installed: &InstalledPaths,
) -> anyhow::Result<Vec<HiddenPlugin>> {
    let mut hidden = Vec::with_capacity(spec.sources.len());

    for source in &spec.sources {
        let mod_dir = installed
            .get(&source.mod_id)
            .cloned()
            .unwrap_or_else(|| settings.mods_dir.join(&source.mod_id));

        let outcome = hide_plugin(&mod_dir, &source.plugin)
            .map_err(|err| anyhow::anyhow!("merge {merge_id}: {err}"))?;
        if outcome == HideOutcome::Hidden {
            tracing::debug!(
                merge = merge_id,
                mod_id = %source.mod_id,
                plugin = %source.plugin,
                "merge: hid source plugin"
            );
        }

        hidden.push(HiddenPlugin {
            merge: merge_id.to_string(),
            mod_id: source.mod_id.clone(),
            plugin: source.plugin.clone(),
            installed_path: relative_to_mods_dir(&mod_dir, &settings.mods_dir),
        });
    }

    Ok(hidden)
}

/// Restore every plugin this install's merges hid, and forget the records.
///
/// Reads the manifest rather than the modlist, so it undoes what was actually
/// done even if the modlist has since changed. Clearing the records afterwards
/// keeps the manifest honest -- a second run then correctly reports nothing to
/// do rather than re-reporting the same restores.
pub fn unhide_merges(
    mods_dir: &Path,
    mo2_instance_dir: Option<&Path>,
    profile_name: &str,
) -> anyhow::Result<(usize, usize)> {
    let path = super::manifest::manifest_path(mods_dir, mo2_instance_dir, profile_name);
    let Some(mut manifest) = super::manifest::load_install_manifest(&path)? else {
        anyhow::bail!(
            "no install manifest at {}; nothing is recorded as hidden",
            path.display()
        );
    };

    let recorded = manifest.hidden_plugins.len();
    let restored = unhide_all(mods_dir, &manifest.hidden_plugins)?;

    manifest.hidden_plugins.clear();
    let payload = serde_json::to_string_pretty(&manifest)
        .map_err(|err| anyhow::anyhow!("failed to serialize install manifest: {err}"))?;
    std::fs::write(&path, payload)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))?;

    Ok((restored, recorded))
}

fn unhide_all(mods_dir: &Path, hidden: &[HiddenPlugin]) -> anyhow::Result<usize> {
    let mut restored = 0usize;
    for entry in hidden {
        let mod_dir = mods_dir.join(&entry.installed_path);
        if !mod_dir.exists() {
            tracing::warn!(
                mod_id = %entry.mod_id,
                path = %mod_dir.display(),
                "unhide: mod folder is gone, skipping"
            );
            continue;
        }
        if find_child_case_insensitive(&mod_dir, &format!("{}{MO2_HIDDEN_SUFFIX}", entry.plugin))
            .is_some()
        {
            restored += 1;
        }
        unhide_plugin(&mod_dir, &entry.plugin)?;
    }
    Ok(restored)
}

/// Find a source plugin whether or not a previous run already hid it.
///
/// Without the hidden fallback a merge would succeed once and then fail on
/// every re-install, because its own hiding step renamed its inputs away.
fn locate_source_plugin(mod_dir: &Path, plugin: &str) -> Option<PathBuf> {
    find_child_case_insensitive(mod_dir, plugin).or_else(|| {
        find_child_case_insensitive(mod_dir, &format!("{plugin}{MO2_HIDDEN_SUFFIX}"))
    })
}

fn relative_to_mods_dir(mod_dir: &Path, mods_dir: &Path) -> String {
    mod_dir
        .strip_prefix(mods_dir)
        .ok()
        .and_then(|rel| rel.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            mod_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn touch(path: &Path) {
        std::fs::write(path, b"TES4").expect("write");
    }

    #[test]
    fn hiding_renames_then_becomes_a_no_op() {
        let temp = tempdir().expect("tempdir");
        let mod_dir = temp.path();
        touch(&mod_dir.join("Fort Aurus.esp"));

        assert_eq!(
            hide_plugin(mod_dir, "Fort Aurus.esp").expect("hide"),
            HideOutcome::Hidden
        );
        assert!(mod_dir.join("Fort Aurus.esp.mohidden").exists());
        assert!(!mod_dir.join("Fort Aurus.esp").exists());

        // Second install pass over the same instance.
        assert_eq!(
            hide_plugin(mod_dir, "Fort Aurus.esp").expect("hide again"),
            HideOutcome::AlreadyHidden
        );
        assert!(mod_dir.join("Fort Aurus.esp.mohidden").exists());
    }

    #[test]
    fn hiding_is_reversible() {
        let temp = tempdir().expect("tempdir");
        let mod_dir = temp.path();
        touch(&mod_dir.join("Fort Aurus.esp"));

        hide_plugin(mod_dir, "Fort Aurus.esp").expect("hide");
        unhide_plugin(mod_dir, "Fort Aurus.esp").expect("unhide");
        assert!(mod_dir.join("Fort Aurus.esp").exists());
        assert!(!mod_dir.join("Fort Aurus.esp.mohidden").exists());

        // Unhiding something already visible is fine.
        unhide_plugin(mod_dir, "Fort Aurus.esp").expect("unhide again");
        assert!(mod_dir.join("Fort Aurus.esp").exists());
    }

    #[test]
    fn both_names_present_is_an_error_rather_than_a_guess() {
        let temp = tempdir().expect("tempdir");
        let mod_dir = temp.path();
        touch(&mod_dir.join("Fort Aurus.esp"));
        touch(&mod_dir.join("Fort Aurus.esp.mohidden"));

        let err = hide_plugin(mod_dir, "Fort Aurus.esp").unwrap_err().to_string();
        assert!(err.contains("ambiguous"), "unexpected error: {err}");
    }

    #[test]
    fn hiding_matches_the_casing_actually_on_disk() {
        // Mod archives are Windows-authored; the modlist's casing routinely
        // differs from the file's.
        let temp = tempdir().expect("tempdir");
        let mod_dir = temp.path();
        touch(&mod_dir.join("fort aurus.ESP"));

        assert_eq!(
            hide_plugin(mod_dir, "Fort Aurus.esp").expect("hide"),
            HideOutcome::Hidden
        );
        assert!(mod_dir.join("fort aurus.ESP.mohidden").exists());
    }

    #[test]
    fn a_missing_source_plugin_is_an_error() {
        let temp = tempdir().expect("tempdir");
        let err = hide_plugin(temp.path(), "Absent.esp").unwrap_err().to_string();
        assert!(err.contains("not in"), "unexpected error: {err}");
    }

    #[test]
    fn sources_are_still_found_after_a_previous_run_hid_them() {
        let temp = tempdir().expect("tempdir");
        let mod_dir = temp.path();
        touch(&mod_dir.join("Fort Aurus.esp"));
        hide_plugin(mod_dir, "Fort Aurus.esp").expect("hide");

        let found = locate_source_plugin(mod_dir, "Fort Aurus.esp").expect("still locatable");
        assert_eq!(found, mod_dir.join("Fort Aurus.esp.mohidden"));
    }
}
