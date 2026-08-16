//! Archive extraction and layout resolution.

pub mod auto;
pub mod bain;
pub mod build;
pub mod fomod;

use crate::archive::{extract_with_builtins, ArchiveFilters};
use crate::config::download;
use crate::config::schema::{ArchiveLayout, CompiledArchive, ModType, PersonalizedMod};
use auto::extract_archive_with_auto_layout;
use bain::extract_archive_with_bain_layout;
use build::extract_build_archive;
use fomod::extract_archive_with_fomod_layout;

use crate::util::fs::{
    copy_filtered_tree, normalize_relative_path, resolve_existing_path_case_insensitive,
    staging_dir_for,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::InstallSettings;

/// Extract `source` into a fresh staging directory, hand it to `f`, then clean up.
///
/// Every layout handler needs the same dance -- make a unique temp dir, extract
/// with passthrough filters, run the layout logic, remove the temp dir even on
/// failure. It was copy-pasted five times with subtly different cleanup.
pub(crate) fn with_staged_archive<T>(
    source: &Path,
    target_root: &Path,
    f: impl FnOnce(&Path) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let staging_dir = staging_dir_for(target_root)?;
    std::fs::create_dir_all(&staging_dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create staging dir {}: {err}",
            staging_dir.display()
        )
    })?;

    let no_patterns: Vec<String> = Vec::new();
    let result = ArchiveFilters::new(&no_patterns, &no_patterns)
        .and_then(|passthrough| extract_with_builtins(source, &staging_dir, &passthrough))
        .and_then(|_| f(&staging_dir));

    let _ = std::fs::remove_dir_all(&staging_dir);
    result
}

/// Append `target_subdir` to the mod root when the archive declares one.
pub(crate) fn destination_for(
    target_root: &Path,
    target_subdir: Option<&str>,
) -> anyhow::Result<PathBuf> {
    match target_subdir {
        Some(subdir) => Ok(target_root.join(normalize_relative_path(subdir)?)),
        None => Ok(target_root.to_path_buf()),
    }
}

pub(crate) fn install_mod_archives(
    mod_entry: &PersonalizedMod,
    settings: &InstallSettings,
    target_root: &Path,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    if mod_entry.mod_type == Some(ModType::BuildFromFiles) {
        return install_mod_from_files(mod_entry, settings, target_root);
    }

    let mut extracted_count = 0usize;

    for (archive_index, archive) in mod_entry.archives.iter().enumerate() {
        if !archive.build.is_empty() {
            // game_root_files has no extraction pass on the build path. Previously
            // these patterns were merged into the exclude list, so matching files
            // were dropped from the mod folder and never written to the game root
            // either -- silently lost. Reject rather than lose files.
            if !archive.game_root_files.is_empty() {
                anyhow::bail!(
                    "mod '{}' archive {}: game_root_files is not supported together with \
                     build layers. Split the game-root files into their own archive entry.",
                    mod_entry.id,
                    archive_index
                );
            }

            let filters = ArchiveFilters::new(&archive.include, &archive.exclude)?;

            if settings.dry_run {
                tracing::info!(
                    mod_id = %mod_entry.id,
                    destination = %target_root.display(),
                    layers = archive.build.len(),
                    "install dry-run build-layer extract"
                );
                continue;
            }

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
            std::fs::create_dir_all(target_root).map_err(|err| {
                anyhow::anyhow!("failed to create {}: {err}", target_root.display())
            })?;

            extracted_count += extract_archive(&source, target_root, &mod_entry.id, archive, &filters, active_plugins)?;
        }
    }

    Ok(extracted_count)
}

pub(crate) fn install_mod_from_files(
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

pub(crate) fn resolve_file_pattern(pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
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

pub(crate) fn extract_archive(
    source: &Path,
    target_root: &Path,
    mod_id: &str,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    if archive.layout == Some(ArchiveLayout::Fomod) {
        return extract_archive_with_fomod_layout(source, target_root, archive, filters, active_plugins);
    }

    if archive.layout == Some(ArchiveLayout::Bain) {
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
