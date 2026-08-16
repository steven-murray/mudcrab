use crate::config::filter::ModFilter;
use crate::config::schema::
    {PersonalizedPlan, PostInstallAction};
use crate::config::mo2::{
    export_mo2_instance, prepare_mo2_profile,
};
use crate::config::tools::loot::run_loot_sort;
use crate::config::tools::ToolsConfig;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct InstallSettings {
    pub cache_dir: PathBuf,
    pub mods_dir: PathBuf,
    pub mo2_instance_dir: Option<PathBuf>,
    pub profile_name: String,
    pub game_dir: Option<PathBuf>,
    pub game_root_dir: Option<PathBuf>,
    pub execute_actions: bool,
    pub dry_run: bool,
    pub tools: ToolsConfig,
    /// Which mods this run installs. Empty means the whole plan.
    pub filter: ModFilter,
    /// Read-only directories holding archives already on this machine. An
    /// archive found here is adopted into the cache instead of downloaded, so a
    /// list can be installed with no network access at all.
    pub archive_search_paths: Vec<PathBuf>,
    /// Rebuild every merge in scope even when its recorded inputs still match.
    pub force_merges: bool,
    /// Reinstall every mod in scope even when its recorded definition matches.
    pub force: bool,
}

pub mod layout;
pub mod manifest;
pub mod merge;
pub mod stage;

use layout::install_mod_archives;
use manifest::{
    get_install_manifest_path, hash_personalized_mod, load_install_manifest,
    relative_path_to_mod, should_skip_mod_install, BuiltMerge, InstallManifest, InstalledMod,
};
use merge::HiddenPlugin;
pub(crate) use stage::is_plugin_file;

/// Validate a mod id before using it as a directory name.
///
/// Mod ids are arbitrary keys from user-authored TOML, but they are joined
/// straight onto the mods directory. Archive *members* were carefully
/// normalised against traversal while the mod id itself was not, so an id
/// containing a separator or `..` could escape the mods directory.
pub(crate) fn safe_mod_dir_name(mod_id: &str) -> anyhow::Result<&str> {
    if mod_id.is_empty() {
        anyhow::bail!("mod id must not be empty");
    }
    if mod_id.contains('/') || mod_id.contains('\\') {
        anyhow::bail!("mod id '{mod_id}' must not contain a path separator");
    }
    if mod_id == "." || mod_id == ".." {
        anyhow::bail!("mod id '{mod_id}' is not a valid directory name");
    }
    if Path::new(mod_id).is_absolute() {
        anyhow::bail!("mod id '{mod_id}' must not be an absolute path");
    }
    Ok(mod_id)
}

pub fn install_all(plan: &PersonalizedPlan, settings: &InstallSettings) -> anyhow::Result<()> {
    let active_plugins: HashSet<String> = plan
        .plugins
        .iter()
        .map(|plugin| plugin.to_ascii_lowercase())
        .collect();

    if !settings.dry_run {
        std::fs::create_dir_all(&settings.mods_dir).map_err(|err| {
            anyhow::anyhow!(
                "failed to create mods directory {}: {err}",
                settings.mods_dir.display()
            )
        })?;

        if settings.mo2_instance_dir.is_some() {
            prepare_mo2_profile(settings)?;
        }
    }

    let mut installed_mods = Vec::new();
    let manifest_path = get_install_manifest_path(settings);
    let previous_manifest = if settings.dry_run {
        None
    } else {
        load_install_manifest(&manifest_path)?
    };
    let (previous_installed, previous_hidden, previous_built) = previous_manifest
        .map(|manifest| {
            (
                manifest.installed_mods,
                manifest.hidden_plugins,
                manifest.built_merges,
            )
        })
        .unwrap_or_default();
    let previous_by_id: HashMap<String, InstalledMod> = previous_installed
        .iter()
        .map(|entry| (entry.id.clone(), entry.clone()))
        .collect();
    let previous_built_by_id: HashMap<String, BuiltMerge> = previous_built
        .iter()
        .map(|entry| (entry.id.clone(), entry.clone()))
        .collect();

    // What the previous run recorded and this run has not yet reached. An entry
    // leaves this list the moment its mod is touched, so a manifest written
    // part way through describes exactly what is on disk: the mods this run has
    // finished, plus the ones an earlier run left alone.
    let mut carried_forward = previous_installed;

    if !settings.filter.is_empty() {
        tracing::info!(
            scope = %settings.filter.describe(),
            "install: filtered run, only the selected mods will be installed"
        );
    }

    // Actions mutate things outside the mods directory -- ini_set rewrites
    // Oblivion.ini, qac shells out to xEdit and rewrites real plugins -- so a
    // dry run must not reach them. It previously did, which made --dry-run
    // considerably more destructive than installing.
    if settings.execute_actions && !settings.dry_run {
        crate::config::actions::apply_all(
            &plan.actions,
            &crate::config::actions::ActionCx { owner: "plan", settings, mod_target: None },
        )?;
    } else if settings.execute_actions && !plan.actions.is_empty() {
        tracing::info!(
            owner = "plan",
            actions = plan.actions.len(),
            "install dry-run: would apply actions"
        );
    }

    for mod_entry in &plan.mods {
        // Whatever happens to this mod below, this run now owns its record:
        // the copy the previous manifest holds must not also be carried
        // forward, or a mod that is cleared and then fails to extract would
        // stay recorded as installed and never be retried.
        carried_forward.retain(|entry| entry.id != mod_entry.id);

        if !settings.filter.matches(&mod_entry.section, &mod_entry.id) {
            // A filtered run narrows what is installed; it does not uninstall
            // the rest. Carrying the previous entry forward keeps the manifest
            // a record of everything on disk, so installing section B after
            // section A does not make A look uninstalled -- which would delete
            // it from the MO2 profile and reinstall it from scratch next time.
            if let Some(previous) = previous_by_id.get(&mod_entry.id) {
                installed_mods.push(previous.clone());
            }
            tracing::debug!(
                mod_id = %mod_entry.id,
                status = "skipped",
                reason = "excluded by --section/--only",
                "install: mod status"
            );
            continue;
        }

        let mut mod_target = settings.mods_dir.join(safe_mod_dir_name(&mod_entry.id)?);
        let definition_hash = hash_personalized_mod(mod_entry)?;
        let previous = previous_by_id.get(&mod_entry.id);


        // If this mod was previously installed, use its stored installed_path (in case it was renamed).
        if let Some(prev) = previous
            && !prev.installed_path.is_empty()
        {
            mod_target = settings.mods_dir.join(&prev.installed_path);
        }
        // Handle mod conflicts: if another mod target exists but belongs to a different profile
        // with a different version, rename current profile's target to avoid collision.
        if !settings.dry_run {
            mod_target = handle_mod_conflict(&mod_target, &mod_entry.id, &definition_hash, settings)?;
        }

        if should_skip_mod_install(
            &mod_target,
            &definition_hash,
            previous,
            settings.dry_run,
            settings.force,
        ) {
            let previous_extracted = previous.map(|entry| entry.extracted_files).unwrap_or(0);
            let mut actions_applied = previous.map(|entry| entry.actions_applied).unwrap_or(false);

            tracing::info!(
                mod_id = %mod_entry.id,
                target = %mod_target.display(),
                status = "skipped",
                reason = "already installed and unchanged",
                extracted_files = previous_extracted,
                "install: mod status"
            );

            // A skipped mod still re-runs any action that writes outside its
            // own folder, because "the mod is unchanged" says nothing about
            // whether that target still holds the edit. Everything else is
            // applied once and latched.
            let pending: Vec<_> = if actions_applied {
                mod_entry
                    .actions
                    .iter()
                    .filter(|action| action.writes_outside_mod_folder())
                    .cloned()
                    .collect()
            } else {
                mod_entry.actions.clone()
            };

            if settings.execute_actions && !pending.is_empty() && !settings.dry_run {
                crate::config::actions::apply_all(
                &pending,
                &crate::config::actions::ActionCx {
                    owner: &mod_entry.id,
                    settings,
                    mod_target: Some(&mod_target),
                },
            )?;
                actions_applied = true;
            }

            installed_mods.push(InstalledMod {
                id: mod_entry.id.clone(),
                definition_hash,
                extracted_files: previous_extracted,
                actions_applied,
                            installed_path: relative_path_to_mod(&mod_target, &settings.mods_dir),
            });
            continue;
        }

        let status = if previous.is_some() { "updated" } else { "installed" };
        tracing::info!(
            mod_id = %mod_entry.id,
            target = %mod_target.display(),
            status,
            "install: mod status"
        );

        if !settings.dry_run {
            // Recorded as gone *before* it is removed. Between the clear and
            // the extraction the mod is not on disk, and a manifest still
            // claiming it would make the next run skip a mod that is not there.
            if previous.is_some() {
                write_install_manifest(
                    &manifest_path,
                    plan,
                    snapshot(&installed_mods, &carried_forward),
                    previous_hidden.clone(),
                    previous_built.clone(),
                )?;
            }
            clear_install_target(&mod_target)?;
        }

        let extracted_files = install_mod_archives(mod_entry, settings, &mod_target, &active_plugins)?;
        let mut actions_applied = false;
        if settings.execute_actions && !settings.dry_run {
            crate::config::actions::apply_all(
                &mod_entry.actions,
                &crate::config::actions::ActionCx {
                    owner: &mod_entry.id,
                    settings,
                    mod_target: Some(&mod_target),
                },
            )?;
            actions_applied = true;
        } else if settings.execute_actions && !mod_entry.actions.is_empty() {
            tracing::info!(
                mod_id = %mod_entry.id,
                actions = mod_entry.actions.len(),
                "install dry-run: would apply actions"
            );
        }

        tracing::info!(
            mod_id = %mod_entry.id,
            target = %mod_target.display(),
            status,
            extracted_files,
            actions_applied,
            "install: mod completed"
        );

        installed_mods.push(InstalledMod {
            id: mod_entry.id.clone(),
            definition_hash,
            extracted_files,
                        installed_path: relative_path_to_mod(&mod_target, &settings.mods_dir),
            actions_applied,
        });

        // Persisted per mod rather than once at the end. A run that fails on
        // mod 250 of 300 used to return Err with nothing written, so the 249
        // mods it had just extracted were absent from the manifest and the next
        // run unpacked every one of them again.
        if !settings.dry_run {
            write_install_manifest(
                &manifest_path,
                plan,
                snapshot(&installed_mods, &carried_forward),
                previous_hidden.clone(),
                previous_built.clone(),
            )?;
        }
    }

    // Merges run after every mod is on disk (they read other mods' plugins)
    // and before LOOT sorts, so LOOT sees the merged plugin rather than the
    // sources it just hid.
    let installed_paths: merge::InstalledPaths = installed_mods
        .iter()
        .map(|entry| {
            (
                entry.id.clone(),
                settings.mods_dir.join(&entry.installed_path),
            )
        })
        .collect();
    let merge_outcome = merge::run_merges(plan, settings, &installed_paths, &previous_built_by_id)?;
    let mut hidden_plugins = merge_outcome.hidden;

    // A merge the filter skipped was not rebuilt, so it also did not re-record
    // what it hid. Without carrying those records forward the sources it hid on
    // an earlier run would stay renamed with nothing left saying so, and
    // `unhide-merges` could no longer restore them.
    let skipped_merges: HashSet<&str> = plan
        .merges()
        .filter(|(entry, _)| !settings.filter.matches(&entry.section, &entry.id))
        .map(|(entry, _)| entry.id.as_str())
        .collect();
    hidden_plugins.extend(
        previous_hidden
            .into_iter()
            .filter(|entry| skipped_merges.contains(entry.merge.as_str())),
    );

    if !settings.dry_run {
        // With a filter active the MO2 profile must describe exactly what is on
        // disk, which is what the manifest now records: this run's mods plus
        // everything an earlier run installed.
        let export_scope: Option<HashSet<String>> = if settings.filter.is_empty() {
            None
        } else {
            Some(installed_mods.iter().map(|entry| entry.id.clone()).collect())
        };

        write_install_manifest(
            &manifest_path,
            plan,
            snapshot(&installed_mods, &carried_forward),
            hidden_plugins,
            merge_outcome.built,
        )?;

        if settings.mo2_instance_dir.is_some() {
            export_mo2_instance(plan, settings, export_scope.as_ref())?;
        }

        if settings.execute_actions {
            run_post_install_actions(plan, settings)?;
        }
    }

    Ok(())
}

/// Everything the manifest should claim right now: what this run has settled,
/// then what an earlier run installed and this run has not reached.
fn snapshot(settled: &[InstalledMod], carried_forward: &[InstalledMod]) -> Vec<InstalledMod> {
    let mut out = Vec::with_capacity(settled.len() + carried_forward.len());
    out.extend_from_slice(settled);
    out.extend_from_slice(carried_forward);
    out
}

fn write_install_manifest(
    manifest_path: &Path,
    plan: &PersonalizedPlan,
    installed_mods: Vec<InstalledMod>,
    hidden_plugins: Vec<HiddenPlugin>,
    built_merges: Vec<BuiltMerge>,
) -> anyhow::Result<()> {
    let manifest = InstallManifest {
        name: plan.name.clone(),
        installed_mods,
        hidden_plugins,
        built_merges,
    };
    let payload = serde_json::to_string_pretty(&manifest)
        .map_err(|err| anyhow::anyhow!("failed to serialize install manifest: {err}"))?;
    std::fs::write(manifest_path, payload)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", manifest_path.display()))
}

fn clear_install_target(mod_target: &Path) -> anyhow::Result<()> {
    if !mod_target.exists() {
        return Ok(());
    }

    if mod_target.is_dir() {
        std::fs::remove_dir_all(mod_target)
            .map_err(|err| anyhow::anyhow!("failed to clear {}: {err}", mod_target.display()))?;
    } else {
        std::fs::remove_file(mod_target)
            .map_err(|err| anyhow::anyhow!("failed to clear {}: {err}", mod_target.display()))?;
    }

    Ok(())
}

/// Handle mod conflicts when installing to a shared mods/ directory used by multiple profiles.
/// 
/// When a mod name conflicts with an existing mod that (1) exists on disk and (2) is owned by
/// another profile and (3) has a different definition hash, we rename the current profile's mod
/// by appending ` - <profilename>` to its folder name. This preserves all other profiles' state
/// and only modifies the current profile's manifest, which stores the renamed path.
/// 
/// Returns the actual target path that should be used for this installation.
fn handle_mod_conflict(
    intended_target: &Path,
    mod_id: &str,
    new_hash: &str,
    settings: &InstallSettings,
) -> anyhow::Result<PathBuf> {
    if !intended_target.exists() {
        return Ok(intended_target.to_path_buf());
    }

    // Try to find which profile owns this existing mod by scanning all manifests in profiles/
    let owning_profile = find_mod_owner_profile(&settings.mods_dir, mod_id)?;

    // If we can't determine ownership or it's owned by the current profile, leave it alone
    let Some((old_profile_name, old_hash)) = owning_profile else {
        return Ok(intended_target.to_path_buf());
    };

    // If same profile owns it, let the normal logic handle it
    if old_profile_name == settings.profile_name {
        return Ok(intended_target.to_path_buf());
    }

    // If same version (hash matches), both profiles can share it
    if old_hash == new_hash {
        return Ok(intended_target.to_path_buf());
    }

    // Conflict detected: different profile owns it with different version.
    // Rename *this* profile's target folder to avoid overwriting the other profile's mod.
    let parent = intended_target.parent().ok_or_else(|| {
        anyhow::anyhow!("mod target has no parent directory: {}", intended_target.display())
    })?;
    let renamed_target = parent.join(format!("{} - {}", mod_id, settings.profile_name));

    // If the renamed target already exists, remove it to avoid conflicts
    if renamed_target.exists() {
        tracing::warn!(
            mod_id = mod_id,
            renamed_path = %renamed_target.display(),
            "install: removing existing {} folder from previous conflict resolution",
            renamed_target.file_name().unwrap_or_default().to_string_lossy()
        );
        clear_install_target(&renamed_target)?;
    }

    tracing::info!(
        mod_id = mod_id,
        other_profile = old_profile_name,
        renamed_path = %renamed_target.display(),
        "install: resolved mod conflict; installing current profile's mod as {} to avoid overwriting {}",
        renamed_target.file_name().unwrap_or_default().to_string_lossy(),
        old_profile_name
    );

    Ok(renamed_target)
}

/// Scan the profiles/ directory to find which profile currently claims ownership of a mod.
/// 
/// Returns `Some((profile_name, definition_hash))` if found, `None` if not found or if
/// the current profile already owns it or if ownership cannot be determined.
fn find_mod_owner_profile(mods_dir: &Path, mod_id: &str) -> anyhow::Result<Option<(String, String)>> {
    let mo2_root = mods_dir.parent().and_then(|p| p.parent());
    let Some(mo2_root) = mo2_root else {
        // Not an MO2 setup; no other profiles to check
        return Ok(None);
    };

    let profiles_dir = mo2_root.join("profiles");
    if !profiles_dir.exists() || !profiles_dir.is_dir() {
        return Ok(None);
    }

    for profile_entry in std::fs::read_dir(&profiles_dir)
        .map_err(|err| anyhow::anyhow!("failed to scan profiles dir {}: {err}", profiles_dir.display()))?
    {
        let entry = profile_entry
            .map_err(|err| anyhow::anyhow!("failed to read profile entry: {err}"))?;
        let profile_path = entry.path();
        if !profile_path.is_dir() {
            continue;
        }

        let manifest_path = profile_path.join("install_manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        // Try to read and parse this profile's manifest
        let raw = match std::fs::read_to_string(&manifest_path) {
            Ok(content) => content,
            Err(_) => continue, // Skip if unreadable
        };

        let manifest: InstallManifest = match serde_json::from_str(&raw) {
            Ok(parsed) => parsed,
            Err(_) => continue, // Skip if unparseable
        };

        // Check if this profile has the mod and return its hash
        for installed in &manifest.installed_mods {
            if installed.id == mod_id {
                let profile_name = profile_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                return Ok(Some((profile_name, installed.definition_hash.clone())));
            }
        }
    }

    Ok(None)
}

fn run_post_install_actions(plan: &PersonalizedPlan, settings: &InstallSettings) -> anyhow::Result<()> {
    for action in &plan.post_install_actions {
        match action {
            PostInstallAction::LootSort => run_loot_sort(plan, settings)?,
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn rejects_mod_ids_that_would_escape_the_mods_directory() {
        use super::safe_mod_dir_name;
        for bad in ["", "..", ".", "a/b", "a\\b", "/etc/passwd", "../../etc"] {
            assert!(
                safe_mod_dir_name(bad).is_err(),
                "mod id {bad:?} should be rejected"
            );
        }
        // Real MOFAM ids contain spaces, brackets, apostrophes and dashes.
        for ok in [
            "Harvest [Flora]",
            "Oscuro's_Oblivion_Overhaul",
            "36 - zMERGED PLUGINS",
            "OOO - KotN Patch",
        ] {
            assert!(safe_mod_dir_name(ok).is_ok(), "mod id {ok:?} should be accepted");
        }
    }
    use super::{
        hash_personalized_mod, should_skip_mod_install, InstalledMod,
    };
    use super::layout::extract_archive;
    use crate::config::actions::ini_set::apply_ini_set;
    use crate::config::tools::loot::find_unlisted_plugins;
    use crate::config::schema::{
        ArchiveLayout, CompiledArchive, FomodSelection, IniSetFormat, ModAction, PersonalizedMod,
        QacAction,
    };
    use std::collections::HashSet;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    fn sample_mod(id: &str, archive_path: &str) -> PersonalizedMod {
        PersonalizedMod {
            id: id.to_string(),
            oracle_name: None,
            section: Vec::new(),
            mod_type: None,
            merge: None,
            archives: vec![CompiledArchive {
                path: Some(archive_path.to_string()),
                file_name: None,
                download_handler: None,
                layout: None,
                data_folder: None,
                target_subdir: None,
                bain_subpackages: Vec::new(),
                fomod_selections: Vec::new(),
                build: Vec::new(),
                include: Vec::new(),
                exclude: Vec::new(),
                game_root_files: Vec::new(),
            }],
            files: Vec::new(),
            actions: vec![ModAction::Qac(QacAction {
                plugins: vec!["*.esp".to_string()],
            })],
        }
    }

    #[test]
    fn finds_discovered_plugins_not_in_declared_list() {
        let declared = vec!["Unofficial Oblivion Patch.esp".to_string()];
        let discovered = vec![
            "Oblivion Citadel Door Fix.esp".to_string(),
            "Unofficial Oblivion Patch.esp".to_string(),
        ];

        let unlisted = find_unlisted_plugins(&declared, &discovered);

        assert_eq!(
            unlisted,
            vec!["Oblivion Citadel Door Fix.esp".to_string(),]
        );
    }

    #[test]
    fn unlisted_detection_is_case_insensitive_and_deduplicated() {
        let declared = vec!["MyPlugin.esp".to_string()];
        let discovered = vec![
            "myplugin.esp".to_string(),
            "Another.esp".to_string(),
            "another.esp".to_string(),
        ];

        let unlisted = find_unlisted_plugins(&declared, &discovered);

        assert_eq!(unlisted, vec!["Another.esp".to_string()]);
    }

    #[test]
    fn mod_hash_changes_when_definition_changes() {
        let original = sample_mod("Example", "foo.7z");
        let mut changed = original.clone();
        changed.archives[0].path = Some("bar.7z".to_string());

        let original_hash = hash_personalized_mod(&original).expect("hash should succeed");
        let changed_hash = hash_personalized_mod(&changed).expect("hash should succeed");

        assert_ne!(original_hash, changed_hash);
    }

    #[test]
    fn skip_requires_matching_hash_and_existing_target() {
        let temp = tempdir().expect("tempdir");
        let mod_target = temp.path().join("Example");
        std::fs::create_dir_all(&mod_target).expect("create mod target");

        let mod_entry = sample_mod("Example", "foo.7z");
        let hash = hash_personalized_mod(&mod_entry).expect("hash should succeed");
        let previous = InstalledMod {
            id: "Example".to_string(),
            definition_hash: hash.clone(),
            extracted_files: 1,
            actions_applied: true,
                    installed_path: String::new(),
        };

        assert!(should_skip_mod_install(&mod_target, &hash, Some(&previous), false, false));

        let different_hash = "deadbeef".to_string();
        assert!(!should_skip_mod_install(
            &mod_target,
            &different_hash,
            Some(&previous),
            false,
            false
        ));

        let missing_target = temp.path().join("Missing");
        assert!(!should_skip_mod_install(
            &missing_target,
            &hash,
            Some(&previous),
            false,
            false
        ));

        assert!(!should_skip_mod_install(&mod_target, &hash, Some(&previous), true, false));

        // --force reinstalls a mod the fingerprint says is settled. The
        // fingerprint covers the plan, not the installer, so it cannot see a
        // change in what an action does.
        assert!(!should_skip_mod_install(&mod_target, &hash, Some(&previous), false, true));
    }

    #[test]
    fn ini_set_updates_standard_assignment() {
        let temp = tempdir().expect("tempdir");
        let ini_path = temp.path().join("example.ini");
        std::fs::write(&ini_path, "foo = 1\n").expect("write ini");

        apply_ini_set(&ini_path, "foo", "2", IniSetFormat::Standard).expect("ini_set should succeed");

        let content = std::fs::read_to_string(&ini_path).expect("read ini");
        assert_eq!(content, "foo = 2\n", "a wholly spaced file stays spaced");
    }

    #[test]
    fn ini_set_keeps_bethesda_spacing_because_the_parser_is_literal() {
        // Oblivion takes everything after the `=` literally, so writing
        // `SFontFile_1 = Data\Fonts\x.fnt` into an INI that uses `Key=Value`
        // yields a path with a leading space. The font then fails to load and
        // the game falls back to vanilla -- a broken UI caused by two spaces.
        let temp = tempdir().expect("tempdir");
        let ini_path = temp.path().join("Oblivion.ini");
        std::fs::write(
            &ini_path,
            "[Fonts]\nSFontFile_1=Data\\Fonts\\Kingthings_Regular.fnt\nSFontFile_2=Data\\Fonts\\Vanilla.fnt\n",
        )
        .expect("write ini");

        apply_ini_set(
            &ini_path,
            "SFontFile_2",
            "Data\\Fonts\\DarN_Kingthings_Petrock_14.fnt",
            IniSetFormat::Standard,
        )
        .expect("ini_set should succeed");

        let content = std::fs::read_to_string(&ini_path).expect("read ini");
        assert!(
            content.contains("SFontFile_2=Data\\Fonts\\DarN_Kingthings_Petrock_14.fnt"),
            "no spaces should be introduced:\n{content}"
        );
        assert!(!content.contains("SFontFile_2 ="), "{content}");
    }

    #[test]
    fn ini_set_repairs_a_line_an_earlier_version_wrote_with_the_wrong_spacing() {
        // mudcrab used to write `Key = Value` unconditionally, which broke the
        // DarNified font paths in a real install. Re-running must fix those
        // lines, not preserve them, so the whole file's style decides.
        let temp = tempdir().expect("tempdir");
        let ini_path = temp.path().join("Oblivion.ini");
        std::fs::write(
            &ini_path,
            "[Fonts]\na=1\nb=2\nc=3\nSFontFile_2 = Data\\Fonts\\Old.fnt\n",
        )
        .expect("write ini");

        apply_ini_set(&ini_path, "SFontFile_2", "Data\\Fonts\\New.fnt", IniSetFormat::Standard)
            .expect("ini_set");

        let content = std::fs::read_to_string(&ini_path).expect("read ini");
        assert!(content.contains("SFontFile_2=Data\\Fonts\\New.fnt"), "{content}");
        assert!(!content.contains(" = "), "the spaced form should be gone:\n{content}");
    }

    #[test]
    fn ini_set_appends_a_new_key_in_the_files_dominant_style() {
        let temp = tempdir().expect("tempdir");

        let tight = temp.path().join("tight.ini");
        std::fs::write(&tight, "a=1\nb=2\nc=3\n").expect("write");
        apply_ini_set(&tight, "d", "4", IniSetFormat::Standard).expect("ini_set");
        assert!(
            std::fs::read_to_string(&tight).unwrap().contains("d=4"),
            "a file written Key=Value gets Key=Value"
        );

        let spaced = temp.path().join("spaced.ini");
        std::fs::write(&spaced, "a = 1\nb = 2\nc = 3\n").expect("write");
        apply_ini_set(&spaced, "d", "4", IniSetFormat::Standard).expect("ini_set");
        assert!(
            std::fs::read_to_string(&spaced).unwrap().contains("d = 4"),
            "a file written Key = Value keeps that"
        );

        // Nothing to learn from: use the Bethesda form.
        let empty = temp.path().join("empty.ini");
        std::fs::write(&empty, "[Section]\n").expect("write");
        apply_ini_set(&empty, "d", "4", IniSetFormat::Standard).expect("ini_set");
        assert!(std::fs::read_to_string(&empty).unwrap().contains("d=4"));
    }

    #[test]
    fn ini_set_updates_set_to_assignment_with_arbitrary_spacing() {
        let temp = tempdir().expect("tempdir");
        let ini_path = temp.path().join("example.ini");
        std::fs::write(&ini_path, "set   zzzMigckQ.bBetterSkillup    to    1\n")
            .expect("write ini");

        apply_ini_set(&ini_path, "zzzMigckQ.bBetterSkillup", "0", IniSetFormat::SetTo)
            .expect("ini_set should succeed");

        let content = std::fs::read_to_string(&ini_path).expect("read ini");
        assert_eq!(content, "set zzzMigckQ.bBetterSkillup to 0\n");
    }

    #[test]
    fn bain_layout_flattens_selected_subpackages_into_mod_root() {
        let temp = tempdir().expect("tempdir");
        let archive_path = temp.path().join("bain.zip");
        let file = std::fs::File::create(&archive_path).expect("create zip");
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        zip.add_directory("00 Option1/", options).expect("dir 00");
        zip.start_file("00 Option1/plugin1.esp", options)
            .expect("plugin1");
        std::io::Write::write_all(&mut zip, b"plugin1").expect("write plugin1");

        zip.add_directory("01 Option2/", options).expect("dir 01");
        zip.start_file("01 Option2/plugin2.esp", options)
            .expect("plugin2");
        std::io::Write::write_all(&mut zip, b"plugin2").expect("write plugin2");
        zip.start_file("01 Option2/Textures/foo.dds", options)
            .expect("texture");
        std::io::Write::write_all(&mut zip, b"texture").expect("write texture");

        zip.add_directory("02 Option3/", options).expect("dir 02");
        zip.start_file("02 Option3/plugin3.esp", options)
            .expect("plugin3");
        std::io::Write::write_all(&mut zip, b"plugin3").expect("write plugin3");

        zip.finish().expect("finish zip");

        let target_root = temp.path().join("out");
        let archive = CompiledArchive {
            path: Some("nexus:oblivion/1/1".to_string()),
            file_name: None,
            download_handler: None,
            layout: Some(ArchiveLayout::Bain),
            data_folder: None,
            target_subdir: None,
            bain_subpackages: vec!["00 Option1".to_string(), "01 Option2".to_string()],
            fomod_selections: Vec::new(),
            build: Vec::new(),
            include: Vec::new(),
            exclude: Vec::new(),
            game_root_files: Vec::new(),
        };
        let filters = crate::archive::ArchiveFilters::new(&[], &[]).expect("filters");
        let active_plugins = HashSet::new();

        let extracted = extract_archive(&archive_path, &target_root, "Example", &archive, &filters, &active_plugins)
            .expect("extract should succeed");

        assert_eq!(extracted, 3);
        assert!(target_root.join("plugin1.esp").exists());
        assert!(target_root.join("plugin2.esp").exists());
        assert!(target_root.join("Textures/foo.dds").exists());
        assert!(!target_root.join("plugin3.esp").exists());
    }

    #[test]
    fn data_folder_lookup_is_case_insensitive() {
        let temp = tempdir().expect("tempdir");
        let archive_path = temp.path().join("case.zip");
        let file = std::fs::File::create(&archive_path).expect("create zip");
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        zip.add_directory("mind your head - signs repositioned 1.3/", options)
            .expect("dir");
        zip.start_file(
            "mind your head - signs repositioned 1.3/MindYourHead.esp",
            options,
        )
        .expect("esp");
        std::io::Write::write_all(&mut zip, b"esp").expect("write esp");
        zip.finish().expect("finish zip");

        let target_root = temp.path().join("out");
        let archive = CompiledArchive {
            path: Some("nexus:oblivion/1/1".to_string()),
            file_name: None,
            download_handler: None,
            layout: None,
            data_folder: Some("Mind Your Head - Signs Repositioned 1.3".to_string()),
            target_subdir: None,
            bain_subpackages: Vec::new(),
            fomod_selections: Vec::new(),
            build: Vec::new(),
            include: Vec::new(),
            exclude: Vec::new(),
            game_root_files: Vec::new(),
        };
        let filters = crate::archive::ArchiveFilters::new(&[], &[]).expect("filters");
        let active_plugins = HashSet::new();

        let extracted = extract_archive(&archive_path, &target_root, "Example", &archive, &filters, &active_plugins)
            .expect("extract should succeed");

        assert_eq!(extracted, 1);
        assert!(target_root.join("MindYourHead.esp").exists());
    }

    #[test]
    fn fomod_layout_applies_required_files_and_explicit_selections() {
        let temp = tempdir().expect("tempdir");
        let archive_path = temp.path().join("fomod.zip");
        let file = std::fs::File::create(&archive_path).expect("create zip");
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        zip.add_directory("Example FOMOD/fomod/", options).expect("fomod dir");
        zip.start_file("Example FOMOD/fomod/ModuleConfig.xml", options)
            .expect("module config");
        std::io::Write::write_all(
            &mut zip,
            br#"<?xml version="1.0" encoding="utf-8"?>
<config>
  <requiredInstallFiles>
    <file source="base.txt" destination="base.txt" />
  </requiredInstallFiles>
  <installSteps order="Explicit">
    <installStep name="Fonts">
      <optionalFileGroups order="Explicit">
        <group name="Font Size" type="SelectExactlyOne">
          <plugins order="Explicit">
            <plugin name="Normal">
              <files>
                <file source="normal.txt" destination="font.txt" priority="0" />
              </files>
              <typeDescriptor><type name="Optional" /></typeDescriptor>
            </plugin>
            <plugin name="Large">
              <files>
                <file source="large.txt" destination="font.txt" priority="1" />
              </files>
              <typeDescriptor><type name="Optional" /></typeDescriptor>
            </plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
    <installStep name="Custom Options">
      <optionalFileGroups order="Explicit">
        <group name="Options" type="SelectAny">
          <plugins order="Explicit">
            <plugin name="Docs">
              <files>
                <folder source="Docs" destination="Docs" priority="0" />
              </files>
              <typeDescriptor><type name="Optional" /></typeDescriptor>
            </plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
  </installSteps>
</config>"#,
        )
        .expect("write module config");
        zip.start_file("Example FOMOD/base.txt", options).expect("base");
        std::io::Write::write_all(&mut zip, b"base").expect("write base");
        zip.start_file("Example FOMOD/normal.txt", options).expect("normal");
        std::io::Write::write_all(&mut zip, b"normal").expect("write normal");
        zip.start_file("Example FOMOD/large.txt", options).expect("large");
        std::io::Write::write_all(&mut zip, b"large").expect("write large");
        zip.start_file("Example FOMOD/Docs/readme.txt", options).expect("readme");
        std::io::Write::write_all(&mut zip, b"docs").expect("write docs");
        zip.finish().expect("finish zip");

        let target_root = temp.path().join("out");
        let archive = CompiledArchive {
            path: Some("local-fomod.zip".to_string()),
            file_name: None,
            download_handler: None,
            layout: Some(ArchiveLayout::Fomod),
            data_folder: None,
            target_subdir: None,
            bain_subpackages: Vec::new(),
            fomod_selections: vec![
                FomodSelection {
                    step: "Fonts".to_string(),
                    group: "Font Size".to_string(),
                    options: vec!["Large".to_string()],
                },
                FomodSelection {
                    step: "Custom Options".to_string(),
                    group: "Options".to_string(),
                    options: Vec::new(),
                },
            ],
            build: Vec::new(),
            include: Vec::new(),
            exclude: Vec::new(),
            game_root_files: Vec::new(),
        };
        let filters = crate::archive::ArchiveFilters::new(&[], &[]).expect("filters");
        let active_plugins = HashSet::new();

        let extracted = extract_archive(&archive_path, &target_root, "Example", &archive, &filters, &active_plugins)
            .expect("extract should succeed");

        assert_eq!(extracted, 2);
        assert_eq!(std::fs::read_to_string(target_root.join("base.txt")).expect("base read"), "base");
        assert_eq!(std::fs::read_to_string(target_root.join("font.txt")).expect("font read"), "large");
        assert!(!target_root.join("Docs/readme.txt").exists());
    }
}

