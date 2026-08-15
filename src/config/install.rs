use crate::archive::{extract_with_builtins, ArchiveFilters};
use crate::config::download;
use crate::config::schema::
    {CompiledArchive, FomodSelection, PersonalizedMod, PersonalizedPlan, PostInstallAction};
use crate::config::mo2::{
    export_mo2_instance, mo2_profile_dir, prepare_mo2_profile, MO2_HIDDEN_SUFFIX,
};
use crate::config::tools::loot::run_loot_sort;
use crate::config::tools::ToolsConfig;
use crate::util::fs::{
    copy_filtered_tree, eq_ci, find_child_case_insensitive, normalize_relative_path,
    path_exists_case_insensitive, resolve_existing_path_case_insensitive, staging_dir_for,
};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
}

#[derive(Debug, Serialize, Deserialize)]
struct InstallManifest {
    name: String,
    #[serde(default)]
    installed_mods: Vec<InstalledMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledMod {
    id: String,
    #[serde(default)]
    definition_hash: String,
    extracted_files: usize,
    #[serde(default)]
    actions_applied: bool,
    /// Relative path (within mods/) where this mod was actually installed.
    /// If empty, defaults to mods/<id>. Populated when a mod is renamed due to conflicts.
    #[serde(default)]
    installed_path: String,
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
    let previous_by_id: HashMap<String, InstalledMod> = previous_manifest
        .map(|manifest| {
            manifest
                .installed_mods
                .into_iter()
                .map(|entry| (entry.id.clone(), entry))
                .collect()
        })
        .unwrap_or_default();

    if settings.execute_actions {
        crate::config::actions::apply_all(
            &plan.actions,
            &crate::config::actions::ActionCx { owner: "plan", settings, mod_target: None },
        )?;
    }

    for mod_entry in &plan.mods {
        let mut mod_target = settings.mods_dir.join(&mod_entry.id);
        let definition_hash = hash_personalized_mod(mod_entry)?;
        let previous = previous_by_id.get(&mod_entry.id);


        // If this mod was previously installed, use its stored installed_path (in case it was renamed).
        if let Some(prev) = previous {
            if !prev.installed_path.is_empty() {
                mod_target = settings.mods_dir.join(&prev.installed_path);
            }
        }
        // Handle mod conflicts: if another mod target exists but belongs to a different profile
        // with a different version, rename current profile's target to avoid collision.
        if !settings.dry_run {
            mod_target = handle_mod_conflict(&mod_target, &mod_entry.id, &definition_hash, settings)?;
        }

        if should_skip_mod_install(&mod_target, &definition_hash, previous, settings.dry_run) {
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

            if settings.execute_actions && !actions_applied {
                crate::config::actions::apply_all(
                &mod_entry.actions,
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
            clear_install_target(&mod_target)?;
        }

        let extracted_files = install_mod_archives(mod_entry, settings, &mod_target, &active_plugins)?;
        let mut actions_applied = false;
        if settings.execute_actions {
            crate::config::actions::apply_all(
                &mod_entry.actions,
                &crate::config::actions::ActionCx {
                    owner: &mod_entry.id,
                    settings,
                    mod_target: Some(&mod_target),
                },
            )?;
            actions_applied = true;
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
    }

    if !settings.dry_run {
        let manifest = InstallManifest {
            name: plan.name.clone(),
            installed_mods,
        };
        let payload = serde_json::to_string_pretty(&manifest)
            .map_err(|err| anyhow::anyhow!("failed to serialize install manifest: {err}"))?;

        let manifest_path = get_install_manifest_path(settings);
        std::fs::write(&manifest_path, payload).map_err(|err| {
            anyhow::anyhow!("failed to write {}: {err}", manifest_path.display())
        })?;

        if settings.mo2_instance_dir.is_some() {
            export_mo2_instance(plan, settings)?;
        }

        if settings.execute_actions {
            run_post_install_actions(plan, settings)?;
        }
    }

    Ok(())
}

fn get_install_manifest_path(settings: &InstallSettings) -> PathBuf {
    if let Some(profile_dir) = mo2_profile_dir(settings) {
        profile_dir.join("install_manifest.json")
    } else {
        settings.mods_dir.join("install_manifest.json")
    }
}

fn load_install_manifest(path: &Path) -> anyhow::Result<Option<InstallManifest>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    let parsed: InstallManifest = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            tracing::warn!(
                manifest = %path.display(),
                error = %err,
                "install: ignoring unreadable manifest and performing full reinstall"
            );
            return Ok(None);
        }
    };

    Ok(Some(parsed))
}

fn should_skip_mod_install(
    mod_target: &Path,
    definition_hash: &str,
    previous: Option<&InstalledMod>,
    dry_run: bool,
) -> bool {
    if dry_run {
        return false;
    }

    let Some(previous) = previous else {
        return false;
    };

    mod_target.exists() && previous.definition_hash == definition_hash
}

fn hash_personalized_mod(mod_entry: &PersonalizedMod) -> anyhow::Result<String> {
    let payload = serde_json::to_vec(mod_entry)
        .map_err(|err| anyhow::anyhow!("failed to serialize mod {} for hashing: {err}", mod_entry.id))?;
    let digest = Sha256::digest(&payload);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)

}

fn relative_path_to_mod(mod_path: &Path, mods_dir: &Path) -> String {
    mod_path
        .strip_prefix(mods_dir)
        .ok()
        .and_then(|rel| rel.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            mod_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        })
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

/// Is this a plugin the game should load?
///
/// Explicitly excludes MO2's hidden suffix. An extension check alone already
/// happens to reject `Foo.esp.mohidden` (its extension is `mohidden`), but
/// relying on that accident is fragile -- once merges start hiding their source
/// plugins, "hidden plugins are not loaded" becomes load-bearing behaviour and
/// should be stated, not inferred.
pub(crate) fn is_plugin_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if name.to_ascii_lowercase().ends_with(MO2_HIDDEN_SUFFIX) {
        return false;
    }

    let lower = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    lower == "esp" || lower == "esm"
}

fn install_mod_archives(
    mod_entry: &PersonalizedMod,
    settings: &InstallSettings,
    target_root: &Path,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    if mod_entry.mod_type.as_deref() == Some("build-from-files") {
        return install_mod_from_files(mod_entry, settings, target_root);
    }

    let mut extracted_count = 0usize;

    for (archive_index, archive) in mod_entry.archives.iter().enumerate() {
        if !archive.build.is_empty() {
            let effective_exclude: Vec<String> = archive
                .exclude
                .iter()
                .chain(archive.game_root_files.iter())
                .cloned()
                .collect();
            let filters = ArchiveFilters::new(&archive.include, &effective_exclude)?;
            extracted_count += extract_build_archive(
                &mod_entry.id,
                archive_index,
                archive,
                target_root,
                &filters,
                active_plugins,
                settings,
            )?;
            continue;
        }

        let path = archive.path.as_deref().unwrap_or_default();
        let cache_name = download::cache_file_name(&mod_entry.id, archive_index, path);
        let source = download::resolve_cache_path(&settings.cache_dir, &cache_name)
            .unwrap_or_else(|| settings.cache_dir.join(&cache_name));

        if !source.exists() {
            anyhow::bail!(
                "missing cached archive for mod {}: {}",
                mod_entry.id,
                source.display()
            );
        }

        // Game-root extraction pass: extract matching files to the game-root output folder.
        // These files are also auto-excluded from the normal mod installation below.
        if !archive.game_root_files.is_empty() {
            if let Some(game_root_dir) = &settings.game_root_dir {
                let grf_filters = ArchiveFilters::new(&archive.game_root_files, &[])?;
                if settings.dry_run {
                    tracing::info!(
                        mod_id = %mod_entry.id,
                        source = %source.display(),
                        game_root = %game_root_dir.display(),
                        patterns = ?archive.game_root_files,
                        "install dry-run game-root extract"
                    );
                } else {
                    std::fs::create_dir_all(game_root_dir).map_err(|err| {
                        anyhow::anyhow!(
                            "failed to create game-root dir {}: {err}",
                            game_root_dir.display()
                        )
                    })?;
                    let extracted = extract_with_builtins(&source, game_root_dir, &grf_filters)?;
                    tracing::info!(
                        mod_id = %mod_entry.id,
                        game_root = %game_root_dir.display(),
                        extracted,
                        "game-root files extracted"
                    );
                }
            } else {
                tracing::warn!(
                    mod_id = %mod_entry.id,
                    patterns = ?archive.game_root_files,
                    "archive has game_root_files but no game-root-dir is configured; game-root files will not be extracted"
                );
            }
        }

        // Normal extraction pass; game_root_files are added to the effective exclude list so
        // they are not duplicated into the mod's staging folder.
        let effective_exclude: Vec<String> = archive
            .exclude
            .iter()
            .chain(archive.game_root_files.iter())
            .cloned()
            .collect();
        let filters = ArchiveFilters::new(&archive.include, &effective_exclude)?;

        if settings.dry_run {
            tracing::info!(
                mod_id = %mod_entry.id,
                source = %source.display(),
                destination = %target_root.display(),
                data_folder = ?archive.data_folder,
                target_subdir = ?archive.target_subdir,
                "install dry-run extract"
            );
        } else {
            std::fs::create_dir_all(&target_root).map_err(|err| {
                anyhow::anyhow!("failed to create {}: {err}", target_root.display())
            })?;

            extracted_count += extract_archive(&source, target_root, &mod_entry.id, archive, &filters, active_plugins)?;
        }
    }

    Ok(extracted_count)
}

fn install_mod_from_files(
    mod_entry: &PersonalizedMod,
    settings: &InstallSettings,
    target_root: &Path,
) -> anyhow::Result<usize> {
    if mod_entry.files.is_empty() {
        anyhow::bail!(
            "mod {} has type=build-from-files but no files were specified",
            mod_entry.id
        );
    }

    let Some(game_dir) = &settings.game_dir else {
        anyhow::bail!(
            "mod {} uses type=build-from-files and requires --game-dir for %GAME_DIR% expansion",
            mod_entry.id
        );
    };

    let mut copied = 0usize;
    let mut seen_targets: HashSet<String> = HashSet::new();

    if !settings.dry_run {
        std::fs::create_dir_all(target_root).map_err(|err| {
            anyhow::anyhow!("failed to create {}: {err}", target_root.display())
        })?;
    }

    for pattern in &mod_entry.files {
        let expanded = pattern.replace("%GAME_DIR%", &game_dir.to_string_lossy());
        let matches = resolve_file_pattern(&expanded)?;
        if matches.is_empty() {
            anyhow::bail!(
                "mod {} build-from-files pattern matched no files: {}",
                mod_entry.id,
                pattern
            );
        }

        for source in matches {
            let file_name = source
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or_else(|| anyhow::anyhow!(
                    "mod {} build-from-files source has invalid filename: {}",
                    mod_entry.id,
                    source.display()
                ))?
                .to_string();
            let key = file_name.to_ascii_lowercase();
            if !seen_targets.insert(key) {
                continue;
            }

            let destination = target_root.join(&file_name);
            if settings.dry_run {
                tracing::info!(
                    mod_id = %mod_entry.id,
                    source = %source.display(),
                    destination = %destination.display(),
                    "install dry-run build-from-files copy"
                );
            } else {
                std::fs::copy(&source, &destination).map_err(|err| {
                    anyhow::anyhow!(
                        "failed to copy {} to {}: {err}",
                        source.display(),
                        destination.display()
                    )
                })?;
            }

            copied += 1;
        }
    }

    Ok(copied)
}

fn resolve_file_pattern(pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
    let has_glob = pattern.contains('*') || pattern.contains('?') || pattern.contains('[');
    if !has_glob {
        let path = PathBuf::from(pattern);
        if path.exists() && path.is_file() {
            return Ok(vec![path]);
        }
        return Ok(Vec::new());
    }

    let normalized = pattern.replace('\\', "/");
    let split_idx = normalized.rfind('/').ok_or_else(|| {
        anyhow::anyhow!(
            "glob pattern must include a parent directory: {}",
            pattern
        )
    })?;
    let (parent_str, file_pat) = normalized.split_at(split_idx);
    let file_pat = &file_pat[1..];
    let parent = PathBuf::from(parent_str);

    if !parent.exists() {
        return Ok(Vec::new());
    }

    let matcher = globset::Glob::new(file_pat)
        .map_err(|err| anyhow::anyhow!("invalid file glob '{}': {err}", file_pat))?
        .compile_matcher();

    let mut matches = Vec::new();
    for entry in std::fs::read_dir(&parent)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", parent.display()))?
    {
        let entry = entry
            .map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", parent.display()))?;
        if !entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read type for {}: {err}", entry.path().display()))?
            .is_file()
        {
            continue;
        }

        let name = entry.file_name();
        if matcher.is_match(name.to_string_lossy().as_ref()) {
            matches.push(entry.path());
        }
    }

    matches.sort();
    Ok(matches)
}

fn extract_archive(
    source: &Path,
    target_root: &Path,
    mod_id: &str,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    if matches!(archive.layout.as_deref(), Some("fomod")) {
        return extract_archive_with_fomod_layout(source, target_root, archive, filters, active_plugins);
    }

    if matches!(archive.layout.as_deref(), Some("bain")) {
        return extract_archive_with_bain_layout(source, target_root, archive, filters);
    }

    let has_data_folder = archive
        .data_folder
        .as_ref()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let has_target_subdir = archive
        .target_subdir
        .as_ref()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    if !has_data_folder && !has_target_subdir {
        return extract_archive_with_auto_layout(source, target_root, mod_id, filters);
    }

    let staging_dir = staging_dir_for(target_root)?;
    std::fs::create_dir_all(&staging_dir)
        .map_err(|err| anyhow::anyhow!("failed to create staging dir {}: {err}", staging_dir.display()))?;

    let empty_patterns: Vec<String> = Vec::new();
    let passthrough_filters = ArchiveFilters::new(&empty_patterns, &empty_patterns)?;
    let extract_result = extract_with_builtins(source, &staging_dir, &passthrough_filters);
    if let Err(err) = extract_result {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(err);
    }

    let mut source_root = staging_dir.clone();
    if let Some(data_folder) = archive.data_folder.as_deref() {
        let rel = normalize_relative_path(data_folder)?;
        let wanted = source_root.join(rel);
        source_root = resolve_existing_path_case_insensitive(&wanted).unwrap_or(wanted);
        if !source_root.exists() {
            let _ = std::fs::remove_dir_all(&staging_dir);
            anyhow::bail!(
                "data_folder '{}' was not found in extracted archive {}",
                data_folder,
                source.display()
            );
        }
    }

    let mut destination_root = target_root.to_path_buf();
    if let Some(target_subdir) = archive.target_subdir.as_deref() {
        let rel = normalize_relative_path(target_subdir)?;
        destination_root = destination_root.join(rel);
    }

    let copy_result = copy_filtered_tree(&source_root, &destination_root, filters);
    let _ = std::fs::remove_dir_all(&staging_dir);
    copy_result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FomodOptionType {
    Required,
    Recommended,
    Optional,
    CouldBeUsable,
    NotUsable,
}

#[derive(Debug, Clone)]
enum FomodInstallEntry {
    File {
        source: String,
        destination: String,
        priority: i32,
    },
    Folder {
        source: String,
        destination: String,
        priority: i32,
    },
}

fn extract_archive_with_fomod_layout(
    source: &Path,
    target_root: &Path,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    if archive.data_folder.is_some() {
        anyhow::bail!(
            "FOMOD layout for {} cannot be combined with data_folder",
            source.display()
        );
    }
    if !archive.bain_subpackages.is_empty() {
        anyhow::bail!(
            "FOMOD layout for {} cannot be combined with bain_subpackages",
            source.display()
        );
    }

    let staging_dir = staging_dir_for(target_root)?;
    std::fs::create_dir_all(&staging_dir)
        .map_err(|err| anyhow::anyhow!("failed to create staging dir {}: {err}", staging_dir.display()))?;

    let empty_patterns: Vec<String> = Vec::new();
    let passthrough_filters = ArchiveFilters::new(&empty_patterns, &empty_patterns)?;
    let extract_result = extract_with_builtins(source, &staging_dir, &passthrough_filters);
    if let Err(err) = extract_result {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(err);
    }

    let result = (|| -> anyhow::Result<usize> {
        let module_config = find_fomod_config(&staging_dir)?;
        let xml = read_xml_text(&module_config)?;
        let doc = Document::parse(&xml).map_err(|err| {
            anyhow::anyhow!("failed to parse FOMOD config {}: {err}", module_config.display())
        })?;
        let content_root = module_config
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| anyhow::anyhow!("invalid FOMOD structure in {}", source.display()))?;

        let root = doc.root_element();
        let selection_map = build_fomod_selection_map(&archive.fomod_selections);
        let mut flags = HashMap::<String, String>::new();
        let mut entries = Vec::<FomodInstallEntry>::new();

        if let Some(required) = find_child_element(root, "requiredInstallFiles") {
            collect_fomod_entries(required, &mut entries)?;
        }

        if let Some(steps) = find_child_element(root, "installSteps") {
            for step in child_elements(steps, "installStep") {
                if let Some(visible) = find_child_element(step, "visible") {
                    if let Some(deps) = find_child_element(visible, "dependencies") {
                        if !fomod_dependencies_match(deps, active_plugins, &flags)? {
                            continue;
                        }
                    }
                }

                let step_name = step.attribute("name").unwrap_or("");
                for file_groups in child_elements(step, "optionalFileGroups") {
                    for group in child_elements(file_groups, "group") {
                        let group_name = group.attribute("name").unwrap_or("");
                        let group_type = group.attribute("type").unwrap_or("SelectAny");
                        let plugins = find_child_element(group, "plugins").ok_or_else(|| {
                            anyhow::anyhow!(
                                "FOMOD group '{}' in step '{}' is missing <plugins>",
                                group_name,
                                step_name
                            )
                        })?;

                        let plugin_nodes: Vec<Node<'_, '_>> = child_elements(plugins, "plugin").collect();
                        let option_types: Vec<FomodOptionType> = plugin_nodes
                            .iter()
                            .map(|plugin| fomod_option_type(*plugin, active_plugins, &flags))
                            .collect::<anyhow::Result<_>>()?;

                        let selected_indices = select_fomod_options(
                            step_name,
                            group_name,
                            group_type,
                            &plugin_nodes,
                            &option_types,
                            &selection_map,
                        )?;

                        for idx in selected_indices {
                            let plugin = plugin_nodes[idx];
                            if let Some(condition_flags) = find_child_element(plugin, "conditionFlags") {
                                for flag in child_elements(condition_flags, "flag") {
                                    let Some(name) = flag.attribute("name") else {
                                        continue;
                                    };
                                    let value = flag.text().unwrap_or("").trim().to_string();
                                    flags.insert(name.to_ascii_lowercase(), value);
                                }
                            }

                            if let Some(files) = find_child_element(plugin, "files") {
                                collect_fomod_entries(files, &mut entries)?;
                            }
                        }
                    }
                }
            }
        }

        let mut destination_root = target_root.to_path_buf();
        if let Some(target_subdir) = archive.target_subdir.as_deref() {
            let rel = normalize_relative_path(target_subdir)?;
            destination_root = destination_root.join(rel);
        }

        apply_fomod_entries(content_root, &destination_root, &entries, filters)
    })();

    let _ = std::fs::remove_dir_all(&staging_dir);
    result
}

fn find_fomod_config(staging_dir: &Path) -> anyhow::Result<PathBuf> {
    let mut module_config = None;
    let mut script_xml = None;
    find_fomod_config_recursive(staging_dir, &mut module_config, &mut script_xml)?;

    if let Some(path) = module_config {
        return Ok(path);
    }
    if let Some(path) = script_xml {
        return Ok(path);
    }

    anyhow::bail!(
        "FOMOD archive {} does not contain fomod/ModuleConfig.xml or fomod/script.xml",
        staging_dir.display()
    )
}

fn apply_fomod_from_staging(
    staging_dir: &Path,
    destination_root: &Path,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    let module_config = find_fomod_config(staging_dir)?;
    let xml = read_xml_text(&module_config)?;
    let doc = Document::parse(&xml).map_err(|err| {
        anyhow::anyhow!("failed to parse FOMOD config {}: {err}", module_config.display())
    })?;
    let content_root = module_config
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow::anyhow!("invalid FOMOD structure in {}", staging_dir.display()))?;

    let root = doc.root_element();
    let selection_map = build_fomod_selection_map(&archive.fomod_selections);
    let mut flags = HashMap::<String, String>::new();
    let mut entries = Vec::<FomodInstallEntry>::new();

    if let Some(required) = find_child_element(root, "requiredInstallFiles") {
        collect_fomod_entries(required, &mut entries)?;
    }

    if let Some(steps) = find_child_element(root, "installSteps") {
        for step in child_elements(steps, "installStep") {
            if let Some(visible) = find_child_element(step, "visible") {
                if let Some(deps) = find_child_element(visible, "dependencies") {
                    if !fomod_dependencies_match(deps, active_plugins, &flags)? {
                        continue;
                    }
                }
            }

            let step_name = step.attribute("name").unwrap_or("");
            for file_groups in child_elements(step, "optionalFileGroups") {
                for group in child_elements(file_groups, "group") {
                    let group_name = group.attribute("name").unwrap_or("");
                    let group_type = group.attribute("type").unwrap_or("SelectAny");
                    let plugins = find_child_element(group, "plugins").ok_or_else(|| {
                        anyhow::anyhow!(
                            "FOMOD group '{}' in step '{}' is missing <plugins>",
                            group_name,
                            step_name
                        )
                    })?;

                    let plugin_nodes: Vec<Node<'_, '_>> = child_elements(plugins, "plugin").collect();
                    let option_types: Vec<FomodOptionType> = plugin_nodes
                        .iter()
                        .map(|plugin| fomod_option_type(*plugin, active_plugins, &flags))
                        .collect::<anyhow::Result<_>>()?;

                    let selected_indices = select_fomod_options(
                        step_name,
                        group_name,
                        group_type,
                        &plugin_nodes,
                        &option_types,
                        &selection_map,
                    )?;

                    for idx in selected_indices {
                        let plugin = plugin_nodes[idx];
                        if let Some(condition_flags) = find_child_element(plugin, "conditionFlags") {
                            for flag in child_elements(condition_flags, "flag") {
                                let Some(name) = flag.attribute("name") else {
                                    continue;
                                };
                                let value = flag.text().unwrap_or("").trim().to_string();
                                flags.insert(name.to_ascii_lowercase(), value);
                            }
                        }

                        if let Some(files) = find_child_element(plugin, "files") {
                            collect_fomod_entries(files, &mut entries)?;
                        }
                    }
                }
            }
        }
    }

    apply_fomod_entries(content_root, destination_root, &entries, filters)
}

fn apply_bain_from_staging(
    staging_dir: &Path,
    destination_root: &Path,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    if archive.bain_subpackages.is_empty() {
        anyhow::bail!("BAIN layout requires bain_subpackages to list the top-level package folders to install");
    }

    let top = read_top_level(staging_dir)?;
    let available_dirs = top.dirs;
    let mut copied = 0usize;
    for subpackage in &archive.bain_subpackages {
        let Some(package_root) = find_child_case_insensitive(staging_dir, subpackage) else {
            anyhow::bail!(
                "BAIN subpackage '{}' was not found in staging dir. Available top-level directories: {}",
                subpackage,
                available_dirs.join(", ")
            );
        };
        copied += copy_filtered_tree(&package_root, destination_root, filters)?;
    }
    Ok(copied)
}

fn find_fomod_config_recursive(
    current: &Path,
    module_config: &mut Option<PathBuf>,
    script_xml: &mut Option<PathBuf>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?
    {
        let entry = entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read file type for {}: {err}", path.display()))?;

        if file_type.is_dir() {
            find_fomod_config_recursive(&path, module_config, script_xml)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case("ModuleConfig.xml") {
            *module_config = Some(path);
        } else if name.eq_ignore_ascii_case("script.xml") {
            *script_xml = Some(path);
        }
    }

    Ok(())
}

fn read_xml_text(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;

    if bytes.starts_with(&[0xFF, 0xFE]) {
        let mut values = Vec::new();
        for chunk in bytes[2..].chunks_exact(2) {
            values.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        return String::from_utf16(&values)
            .map_err(|err| anyhow::anyhow!("failed to decode UTF-16LE XML {}: {err}", path.display()));
    }

    if bytes.starts_with(&[0xFE, 0xFF]) {
        let mut values = Vec::new();
        for chunk in bytes[2..].chunks_exact(2) {
            values.push(u16::from_be_bytes([chunk[0], chunk[1]]));
        }
        return String::from_utf16(&values)
            .map_err(|err| anyhow::anyhow!("failed to decode UTF-16BE XML {}: {err}", path.display()));
    }

    let payload = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        &bytes[..]
    };

    String::from_utf8(payload.to_vec())
        .map_err(|err| anyhow::anyhow!("failed to decode UTF-8 XML {}: {err}", path.display()))
}

fn build_fomod_selection_map(
    selections: &[FomodSelection],
) -> HashMap<(String, String), Vec<String>> {
    let mut out = HashMap::new();
    for selection in selections {
        out.insert(
            (
                selection.step.to_ascii_lowercase(),
                selection.group.to_ascii_lowercase(),
            ),
            selection.options.clone(),
        );
    }
    out
}

fn find_child_element<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
}

fn child_elements<'a, 'input>(
    node: Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    node.children()
        .filter(move |child| child.is_element() && child.tag_name().name() == name)
}

fn collect_fomod_entries(node: Node<'_, '_>, out: &mut Vec<FomodInstallEntry>) -> anyhow::Result<()> {
    for child in node.children().filter(|child| child.is_element()) {
        let priority = child
            .attribute("priority")
            .unwrap_or("0")
            .parse::<i32>()
            .map_err(|err| anyhow::anyhow!("invalid FOMOD file priority: {err}"))?;
        let source = child
            .attribute("source")
            .ok_or_else(|| anyhow::anyhow!("FOMOD install entry missing source attribute"))?
            .to_string();
        let destination = child.attribute("destination").unwrap_or(&source).to_string();

        match child.tag_name().name() {
            "file" => out.push(FomodInstallEntry::File {
                source,
                destination,
                priority,
            }),
            "folder" => out.push(FomodInstallEntry::Folder {
                source,
                destination,
                priority,
            }),
            other => anyhow::bail!("unsupported FOMOD install entry '{}'; only file/folder are supported", other),
        }
    }
    Ok(())
}

fn fomod_option_type(
    plugin: Node<'_, '_>,
    active_plugins: &HashSet<String>,
    flags: &HashMap<String, String>,
) -> anyhow::Result<FomodOptionType> {
    let Some(type_descriptor) = find_child_element(plugin, "typeDescriptor") else {
        return Ok(FomodOptionType::Optional);
    };

    if let Some(type_node) = find_child_element(type_descriptor, "type") {
        return parse_fomod_option_type(type_node.attribute("name").unwrap_or("Optional"));
    }

    if let Some(dep_type) = find_child_element(type_descriptor, "dependencyType") {
        let default_type = find_child_element(dep_type, "defaultType")
            .and_then(|node| node.attribute("name"))
            .unwrap_or("Optional");
        if let Some(patterns) = find_child_element(dep_type, "patterns") {
            for pattern in child_elements(patterns, "pattern") {
                let matches = if let Some(deps) = find_child_element(pattern, "dependencies") {
                    fomod_dependencies_match(deps, active_plugins, flags)?
                } else {
                    true
                };
                if !matches {
                    continue;
                }

                if let Some(type_node) = find_child_element(pattern, "type") {
                    return parse_fomod_option_type(type_node.attribute("name").unwrap_or(default_type));
                }
            }
        }

        return parse_fomod_option_type(default_type);
    }

    Ok(FomodOptionType::Optional)
}

fn parse_fomod_option_type(name: &str) -> anyhow::Result<FomodOptionType> {
    match name {
        "Required" => Ok(FomodOptionType::Required),
        "Recommended" => Ok(FomodOptionType::Recommended),
        "Optional" => Ok(FomodOptionType::Optional),
        "CouldBeUsable" => Ok(FomodOptionType::CouldBeUsable),
        "NotUsable" => Ok(FomodOptionType::NotUsable),
        other => anyhow::bail!("unsupported FOMOD option type '{other}'"),
    }
}

fn fomod_dependencies_match(
    dependencies: Node<'_, '_>,
    active_plugins: &HashSet<String>,
    flags: &HashMap<String, String>,
) -> anyhow::Result<bool> {
    match dependencies.tag_name().name() {
        "dependencies" => {
            let operator = dependencies.attribute("operator").unwrap_or("And");
            let mut matched = Vec::new();
            for child in dependencies.children().filter(|child| child.is_element()) {
                matched.push(fomod_dependencies_match(child, active_plugins, flags)?);
            }
            Ok(if operator.eq_ignore_ascii_case("Or") {
                matched.into_iter().any(|v| v)
            } else {
                matched.into_iter().all(|v| v)
            })
        }
        "fileDependency" => {
            let file = dependencies
                .attribute("file")
                .ok_or_else(|| anyhow::anyhow!("FOMOD fileDependency missing file attribute"))?
                .to_ascii_lowercase();
            let state = dependencies.attribute("state").unwrap_or("Active");
            let is_active = active_plugins.contains(&file);
            Ok(match state {
                "Active" => is_active,
                "Inactive" => !is_active,
                "Missing" => !is_active,
                other => anyhow::bail!("unsupported FOMOD fileDependency state '{other}'"),
            })
        }
        "flagDependency" => {
            let flag = dependencies
                .attribute("flag")
                .ok_or_else(|| anyhow::anyhow!("FOMOD flagDependency missing flag attribute"))?
                .to_ascii_lowercase();
            let value = dependencies.attribute("value").unwrap_or("On");
            Ok(flags
                .get(&flag)
                .map(|found| found.eq_ignore_ascii_case(value))
                .unwrap_or(false))
        }
        other => anyhow::bail!("unsupported FOMOD dependency node '{other}'"),
    }
}

fn select_fomod_options(
    step_name: &str,
    group_name: &str,
    group_type: &str,
    plugins: &[Node<'_, '_>],
    option_types: &[FomodOptionType],
    selections: &HashMap<(String, String), Vec<String>>,
) -> anyhow::Result<Vec<usize>> {
    let selection_key = (step_name.to_ascii_lowercase(), group_name.to_ascii_lowercase());
    if let Some(wanted) = selections.get(&selection_key) {
        let mut out = Vec::new();
        for desired in wanted {
            let Some((idx, _)) = plugins.iter().enumerate().find(|(_, plugin)| {
                plugin
                    .attribute("name")
                    .map(|name| name.eq_ignore_ascii_case(desired))
                    .unwrap_or(false)
            }) else {
                let available = plugins
                    .iter()
                    .filter_map(|plugin| plugin.attribute("name"))
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::bail!(
                    "FOMOD selection for step '{}' group '{}' references unknown option '{}'. Available options: {}",
                    step_name,
                    group_name,
                    desired,
                    available
                );
            };
            if option_types[idx] == FomodOptionType::NotUsable {
                anyhow::bail!(
                    "FOMOD selection for step '{}' group '{}' chose unusable option '{}'",
                    step_name,
                    group_name,
                    desired
                );
            }
            out.push(idx);
        }

        if group_type == "SelectExactlyOne" && out.len() != 1 {
            anyhow::bail!(
                "FOMOD step '{}' group '{}' requires exactly one selected option",
                step_name,
                group_name
            );
        }
        if group_type == "SelectAtLeastOne" && out.is_empty() {
            anyhow::bail!(
                "FOMOD step '{}' group '{}' requires at least one selected option",
                step_name,
                group_name
            );
        }

        return Ok(out);
    }

    let usable: Vec<usize> = option_types
        .iter()
        .enumerate()
        .filter_map(|(idx, ty)| (*ty != FomodOptionType::NotUsable).then_some(idx))
        .collect();
    let required: Vec<usize> = option_types
        .iter()
        .enumerate()
        .filter_map(|(idx, ty)| (*ty == FomodOptionType::Required).then_some(idx))
        .collect();
    let recommended: Vec<usize> = option_types
        .iter()
        .enumerate()
        .filter_map(|(idx, ty)| (*ty == FomodOptionType::Recommended).then_some(idx))
        .collect();
    let could_be_usable: Vec<usize> = option_types
        .iter()
        .enumerate()
        .filter_map(|(idx, ty)| (*ty == FomodOptionType::CouldBeUsable).then_some(idx))
        .collect();

    let selected = match group_type {
        "SelectAll" => usable,
        "SelectAny" => {
            let mut out = required;
            out.extend(recommended);
            out
        }
        "SelectExactlyOne" | "SelectAtLeastOne" | "SelectAtMostOne" => {
            if let Some(idx) = required
                .first()
                .or_else(|| recommended.first())
                .or_else(|| could_be_usable.first())
                .or_else(|| usable.first())
            {
                vec![*idx]
            } else {
                Vec::new()
            }
        }
        other => anyhow::bail!(
            "unsupported FOMOD option group type '{}' in step '{}' group '{}'",
            other,
            step_name,
            group_name
        ),
    };

    Ok(selected)
}

fn apply_fomod_entries(
    content_root: &Path,
    destination_root: &Path,
    entries: &[FomodInstallEntry],
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|entry| match entry {
        FomodInstallEntry::File { priority, .. } => *priority,
        FomodInstallEntry::Folder { priority, .. } => *priority,
    });

    let mut copied = 0usize;
    for entry in &sorted {
        match entry {
            FomodInstallEntry::File {
                source,
                destination,
                ..
            } => {
                let rel_source = normalize_fomod_relative_path(source)?;
                let rel_source_norm = rel_source.to_string_lossy().replace('\\', "/");
                if !filters.should_extract(&rel_source_norm) {
                    continue;
                }

                let source_path = content_root.join(&rel_source);
                let rel_dest = normalize_fomod_relative_path(destination)?;
                let destination_path = destination_root.join(rel_dest);
                copy_fomod_file(&source_path, &destination_path)?;
                copied += 1;
            }
            FomodInstallEntry::Folder {
                source,
                destination,
                ..
            } => {
                let source_rel = normalize_fomod_relative_path(source)?;
                let source_root = content_root.join(&source_rel);
                let destination_rel = normalize_fomod_relative_path_allow_empty(destination)?;
                let destination = destination_root.join(destination_rel);
                copied += copy_filtered_tree(&source_root, &destination, filters)?;
            }
        }
    }

    Ok(copied)
}

fn copy_fomod_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
    }

    std::fs::copy(source, destination).map_err(|err| {
        anyhow::anyhow!(
            "failed to copy FOMOD file {} to {}: {err}",
            source.display(),
            destination.display()
        )
    })?;

    Ok(())
}

fn normalize_fomod_relative_path(value: &str) -> anyhow::Result<PathBuf> {
    let normalised = value.replace('\\', "/");
    normalize_relative_path(&normalised)
}

fn normalize_fomod_relative_path_allow_empty(value: &str) -> anyhow::Result<PathBuf> {
    if value.trim().is_empty() {
        return Ok(PathBuf::new());
    }
    normalize_fomod_relative_path(value)
}

fn extract_build_archive(
    mod_id: &str,
    archive_index: usize,
    archive: &CompiledArchive,
    target_root: &Path,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
    settings: &InstallSettings,
) -> anyhow::Result<usize> {
    let staging_dir = staging_dir_for(target_root)?;
    std::fs::create_dir_all(&staging_dir)
        .map_err(|err| anyhow::anyhow!("failed to create staging dir {}: {err}", staging_dir.display()))?;

    let result = (|| -> anyhow::Result<usize> {
        let passthrough = ArchiveFilters::new(&[], &[])?;

        for (layer_index, layer) in archive.build.iter().enumerate() {
            let cache_name = download::build_layer_cache_file_name(mod_id, archive_index, layer_index, &layer.path);
            let source = download::resolve_cache_path(&settings.cache_dir, &cache_name)
                .unwrap_or_else(|| settings.cache_dir.join(&cache_name));

            if !source.exists() {
                anyhow::bail!(
                    "missing cached archive for build layer {} of mod {}: {}",
                    layer_index,
                    mod_id,
                    source.display()
                );
            }

            merge_layer_into_staging(&source, &staging_dir, layer.dest_prefix.as_deref(), &passthrough)?;
        }

        let destination_root = if let Some(target_subdir) = archive.target_subdir.as_deref() {
            target_root.join(normalize_relative_path(target_subdir)?)
        } else {
            target_root.to_path_buf()
        };
        std::fs::create_dir_all(&destination_root)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", destination_root.display()))?;

        match archive.layout.as_deref() {
            Some("fomod") => {
                apply_fomod_from_staging(
                    &staging_dir,
                    &destination_root,
                    archive,
                    filters,
                    active_plugins,
                )
            }
            Some("bain") => apply_bain_from_staging(&staging_dir, &destination_root, archive, filters),
            _ => copy_filtered_tree(&staging_dir, &destination_root, filters),
        }
    })();

    let _ = std::fs::remove_dir_all(&staging_dir);
    result
}

fn merge_layer_into_staging(
    source: &Path,
    staging_dir: &Path,
    dest_prefix: Option<&str>,
    filters: &ArchiveFilters,
) -> anyhow::Result<()> {
    let layer_temp = staging_dir_for(staging_dir)?;
    std::fs::create_dir_all(&layer_temp)
        .map_err(|err| anyhow::anyhow!("failed to create layer temp dir {}: {err}", layer_temp.display()))?;

    let result = (|| -> anyhow::Result<()> {
        extract_with_builtins(source, &layer_temp, filters)?;

        let dest = if let Some(prefix) = dest_prefix {
            let rel = normalize_relative_path(prefix)
                .map_err(|err| anyhow::anyhow!("invalid build layer dest_prefix '{prefix}': {err}"))?;
            staging_dir.join(rel)
        } else {
            staging_dir.to_path_buf()
        };
        std::fs::create_dir_all(&dest)
            .map_err(|err| anyhow::anyhow!("failed to create staging dest {}: {err}", dest.display()))?;

        copy_filtered_tree(&layer_temp, &dest, filters)?;
        Ok(())
    })();

    let _ = std::fs::remove_dir_all(&layer_temp);
    result
}

fn extract_archive_with_bain_layout(
    source: &Path,
    target_root: &Path,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    if archive.bain_subpackages.is_empty() {
        anyhow::bail!(
            "BAIN layout for {} requires bain_subpackages to list the top-level package folders to install",
            source.display()
        );
    }
    if archive.data_folder.is_some() {
        anyhow::bail!(
            "BAIN layout for {} cannot be combined with data_folder",
            source.display()
        );
    }

    let staging_dir = staging_dir_for(target_root)?;
    std::fs::create_dir_all(&staging_dir)
        .map_err(|err| anyhow::anyhow!("failed to create staging dir {}: {err}", staging_dir.display()))?;

    let empty_patterns: Vec<String> = Vec::new();
    let passthrough_filters = ArchiveFilters::new(&empty_patterns, &empty_patterns)?;
    let extract_result = extract_with_builtins(source, &staging_dir, &passthrough_filters);
    if let Err(err) = extract_result {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(err);
    }

    let mut destination_root = target_root.to_path_buf();
    if let Some(target_subdir) = archive.target_subdir.as_deref() {
        let rel = normalize_relative_path(target_subdir)?;
        destination_root = destination_root.join(rel);
    }

    let top = read_top_level(&staging_dir)?;
    let available_dirs = top.dirs;
    let mut copied = 0usize;
    for subpackage in &archive.bain_subpackages {
        let Some(package_root) = find_child_case_insensitive(&staging_dir, subpackage) else {
            let _ = std::fs::remove_dir_all(&staging_dir);
            anyhow::bail!(
                "BAIN subpackage '{}' was not found in {}. Available top-level directories: {}",
                subpackage,
                source.display(),
                available_dirs.join(", ")
            );
        };
        copied += copy_filtered_tree(&package_root, &destination_root, filters)?;
    }

    let _ = std::fs::remove_dir_all(&staging_dir);
    Ok(copied)
}

fn extract_archive_with_auto_layout(
    source: &Path,
    target_root: &Path,
    mod_id: &str,
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    let staging_dir = staging_dir_for(target_root)?;
    std::fs::create_dir_all(&staging_dir)
        .map_err(|err| anyhow::anyhow!("failed to create staging dir {}: {err}", staging_dir.display()))?;

    let empty_patterns: Vec<String> = Vec::new();
    let passthrough_filters = ArchiveFilters::new(&empty_patterns, &empty_patterns)?;
    let extract_result = extract_with_builtins(source, &staging_dir, &passthrough_filters);
    if let Err(err) = extract_result {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(err);
    }

    let source_root = detect_auto_source_root(&staging_dir, mod_id, source)?;
    let copy_result = copy_filtered_tree(&source_root, target_root, filters);
    let _ = std::fs::remove_dir_all(&staging_dir);
    copy_result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoLayoutKind {
    Root,
    Data,
    Mod,
    ModData,
}

fn detect_auto_source_root(staging_dir: &Path, mod_id: &str, source: &Path) -> anyhow::Result<PathBuf> {
    let plugin_paths = collect_plugin_paths(staging_dir)?;

    let mut inferred_layout: Option<AutoLayoutKind> = None;
    for plugin in &plugin_paths {
        let layout = classify_plugin_layout(plugin, mod_id).ok_or_else(|| {
            anyhow::anyhow!(
                "unsupported archive layout for {}: plugin '{}' is not in one of the supported roots (/plugin.esp, /Data/plugin.esp, /{}/plugin.esp, /{}/Data/plugin.esp)",
                source.display(),
                plugin,
                mod_id,
                mod_id
            )
        })?;

        if let Some(existing) = inferred_layout {
            if existing != layout {
                anyhow::bail!(
                    "unsupported archive layout for {}: plugins are split across multiple roots (at least '{}' and '{}')",
                    source.display(),
                    format_layout(existing, mod_id),
                    format_layout(layout, mod_id)
                );
            }
        } else {
            inferred_layout = Some(layout);
        }
    }

    let top = read_top_level(staging_dir)?;
    let inferred_layout = if let Some(layout) = inferred_layout {
        layout
    } else {
        if let Some(resolved_root) = detect_expected_content_wrapper_root(staging_dir, source, &top)? {
            return Ok(resolved_root);
        }

        if top.files.is_empty() && top.dirs.len() == 1 && eq_ci(&top.dirs[0], mod_id) {
            if path_exists_case_insensitive(&staging_dir.join(&top.dirs[0]), "Data") {
                AutoLayoutKind::ModData
            } else {
                AutoLayoutKind::Mod
            }
        } else if path_exists_case_insensitive(staging_dir, "Data") {
            AutoLayoutKind::Data
        } else {
            AutoLayoutKind::Root
        }
    };

    if matches!(inferred_layout, AutoLayoutKind::Mod | AutoLayoutKind::ModData)
        && !(top.files.is_empty() && top.dirs.len() == 1 && eq_ci(&top.dirs[0], mod_id))
    {
        anyhow::bail!(
            "unsupported archive layout for {}: /{}/... auto-detection requires that the only top-level entry is a folder named '{}'",
            source.display(),
            mod_id,
            mod_id
        );
    }

    let root = match inferred_layout {
        AutoLayoutKind::Root => staging_dir.to_path_buf(),
        AutoLayoutKind::Data => find_child_case_insensitive(staging_dir, "Data")
            .ok_or_else(|| anyhow::anyhow!("internal error: expected top-level Data in {}", source.display()))?,
        AutoLayoutKind::Mod => find_child_case_insensitive(staging_dir, mod_id)
            .ok_or_else(|| anyhow::anyhow!("internal error: expected top-level mod folder '{}' in {}", mod_id, source.display()))?,
        AutoLayoutKind::ModData => {
            let mod_root = find_child_case_insensitive(staging_dir, mod_id)
                .ok_or_else(|| anyhow::anyhow!("internal error: expected top-level mod folder '{}' in {}", mod_id, source.display()))?;
            find_child_case_insensitive(&mod_root, "Data")
                .ok_or_else(|| anyhow::anyhow!("internal error: expected Data under mod folder '{}' in {}", mod_id, source.display()))?
        }
    };

    Ok(root)
}

fn detect_expected_content_wrapper_root(
    staging_dir: &Path,
    source: &Path,
    top: &TopLevelEntries,
) -> anyhow::Result<Option<PathBuf>> {
    let root_has_expected = dir_has_expected_top_level_content(staging_dir)?;
    let mut child_hits: Vec<PathBuf> = Vec::new();

    for child_name in &top.dirs {
        // A top-level expected content folder (e.g. Textures) is not a wrapper candidate.
        // Scanning inside these can produce false positives like Textures/Menus/...
        if is_expected_game_content_dir_name(child_name) {
            continue;
        }
        let child_path = staging_dir.join(child_name);
        if dir_has_expected_top_level_content(&child_path)? {
            child_hits.push(child_path);
        }
    }

    if root_has_expected && !child_hits.is_empty() {
        anyhow::bail!(
            "unsupported archive layout for {}: expected game-content roots were found at both archive root and nested directory level",
            source.display()
        );
    }

    if child_hits.len() > 1 {
        let names = child_hits
            .iter()
            .filter_map(|p| p.file_name().and_then(|v| v.to_str()))
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!(
            "unsupported archive layout for {}: expected game-content roots were found in multiple sibling directories: {}",
            source.display(),
            names
        );
    }

    if !root_has_expected && child_hits.len() == 1 && top.files.is_empty() && top.dirs.len() == 1 {
        let only = &top.dirs[0];
        if !is_expected_game_content_dir_name(only) {
            return Ok(child_hits.into_iter().next());
        }
    }

    Ok(None)
}

fn dir_has_expected_top_level_content(dir: &Path) -> anyhow::Result<bool> {
    for entry in std::fs::read_dir(dir)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", dir.display()))?
    {
        let entry = entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", dir.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read file type for {}: {err}", entry.path().display()))?;

        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if is_expected_game_content_dir_name(&name) {
                return Ok(true);
            }
            continue;
        }

        if file_type.is_file() && is_plugin_file(&entry.path()) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_expected_game_content_dir_name(name: &str) -> bool {
    // Only include actual game-content folders, not container folders like Data.
    // Data/ wrappers are handled separately by detect_expected_content_wrapper_root.
    const EXPECTED_DIRS: &[&str] = &[
        "meshes",
        "textures",
        "sound",
        "menus",
        "ini",
        "video",
        "obse",
    ];

    EXPECTED_DIRS.iter().any(|expected| eq_ci(name, expected))
}

struct TopLevelEntries {
    dirs: Vec<String>,
    files: Vec<String>,
}

fn read_top_level(root: &Path) -> anyhow::Result<TopLevelEntries> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in std::fs::read_dir(root)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", root.display()))?
    {
        let entry = entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", root.display()))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read file type for {}: {err}", entry.path().display()))?;

        if file_type.is_dir() {
            dirs.push(name);
        } else if file_type.is_file() {
            files.push(name);
        }
    }

    Ok(TopLevelEntries { dirs, files })
}

fn collect_plugin_paths(root: &Path) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    collect_plugin_paths_recursive(root, root, &mut out)?;
    Ok(out)
}

fn collect_plugin_paths_recursive(root: &Path, current: &Path, out: &mut Vec<String>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?
    {
        let entry = entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read file type for {}: {err}", path.display()))?;

        if file_type.is_dir() {
            collect_plugin_paths_recursive(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let lower = path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !(lower.ends_with(".esp") || lower.ends_with(".esm")) {
            continue;
        }

        let rel = path.strip_prefix(root).map_err(|err| {
            anyhow::anyhow!(
                "failed to compute relative path for {} from {}: {err}",
                path.display(),
                root.display()
            )
        })?;
        out.push(rel.to_string_lossy().replace('\\', "/"));
    }

    Ok(())
}

fn classify_plugin_layout(plugin_rel: &str, mod_id: &str) -> Option<AutoLayoutKind> {
    let parts: Vec<&str> = plugin_rel.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }

    if parts.len() == 1 {
        return Some(AutoLayoutKind::Root);
    }
    if parts.len() == 2 && eq_ci(parts[0], "Data") {
        return Some(AutoLayoutKind::Data);
    }
    if parts.len() == 2 && eq_ci(parts[0], mod_id) {
        return Some(AutoLayoutKind::Mod);
    }
    if parts.len() == 3 && eq_ci(parts[0], mod_id) && eq_ci(parts[1], "Data") {
        return Some(AutoLayoutKind::ModData);
    }

    None
}

fn format_layout(layout: AutoLayoutKind, mod_id: &str) -> String {
    match layout {
        AutoLayoutKind::Root => "/plugin.esp".to_string(),
        AutoLayoutKind::Data => "/Data/plugin.esp".to_string(),
        AutoLayoutKind::Mod => format!("/{mod_id}/plugin.esp"),
        AutoLayoutKind::ModData => format!("/{mod_id}/Data/plugin.esp"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_archive, hash_personalized_mod, should_skip_mod_install, InstalledMod,
    };
    use crate::config::actions::ini_set::apply_ini_set;
    use crate::config::tools::loot::find_unlisted_plugins;
    use crate::config::schema::{
        CompiledArchive, FomodSelection, IniSetFormat, ModAction, PersonalizedMod, QacAction,
    };
    use std::collections::HashSet;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    fn sample_mod(id: &str, archive_path: &str) -> PersonalizedMod {
        PersonalizedMod {
            id: id.to_string(),
            mod_type: None,
            archives: vec![CompiledArchive {
                path: Some(archive_path.to_string()),
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

        assert!(should_skip_mod_install(&mod_target, &hash, Some(&previous), false));

        let different_hash = "deadbeef".to_string();
        assert!(!should_skip_mod_install(
            &mod_target,
            &different_hash,
            Some(&previous),
            false
        ));

        let missing_target = temp.path().join("Missing");
        assert!(!should_skip_mod_install(
            &missing_target,
            &hash,
            Some(&previous),
            false
        ));

        assert!(!should_skip_mod_install(&mod_target, &hash, Some(&previous), true));
    }

    #[test]
    fn ini_set_updates_standard_assignment() {
        let temp = tempdir().expect("tempdir");
        let ini_path = temp.path().join("example.ini");
        std::fs::write(&ini_path, "foo = 1\n").expect("write ini");

        apply_ini_set(&ini_path, "foo", "2", IniSetFormat::Standard).expect("ini_set should succeed");

        let content = std::fs::read_to_string(&ini_path).expect("read ini");
        assert_eq!(content, "foo = 2\n");
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
            download_handler: None,
            layout: Some("bain".to_string()),
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
            download_handler: None,
            layout: Some("fomod".to_string()),
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
