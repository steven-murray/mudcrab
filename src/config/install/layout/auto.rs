//! Automatic layout detection for archives that declare no explicit layout.

use crate::archive::{extract_with_builtins, ArchiveFilters};
use crate::util::fs::{
    copy_filtered_tree, eq_ci, find_child_case_insensitive, path_exists_case_insensitive,
    staging_dir_for,
};
use std::path::{Path, PathBuf};

use super::super::is_plugin_file;

pub(crate) fn extract_archive_with_auto_layout(
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
pub(crate) enum AutoLayoutKind {
    Root,
    Data,
    Mod,
    ModData,
}

pub(crate) fn detect_auto_source_root(staging_dir: &Path, mod_id: &str, source: &Path) -> anyhow::Result<PathBuf> {
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

pub(crate) fn detect_expected_content_wrapper_root(
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

pub(crate) fn dir_has_expected_top_level_content(dir: &Path) -> anyhow::Result<bool> {
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

pub(crate) fn is_expected_game_content_dir_name(name: &str) -> bool {
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

pub(crate) struct TopLevelEntries {
    pub(crate) dirs: Vec<String>,
    pub(crate) files: Vec<String>,
}

pub(crate) fn read_top_level(root: &Path) -> anyhow::Result<TopLevelEntries> {
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

pub(crate) fn collect_plugin_paths(root: &Path) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    collect_plugin_paths_recursive(root, root, &mut out)?;
    Ok(out)
}

pub(crate) fn collect_plugin_paths_recursive(root: &Path, current: &Path, out: &mut Vec<String>) -> anyhow::Result<()> {
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

pub(crate) fn classify_plugin_layout(plugin_rel: &str, mod_id: &str) -> Option<AutoLayoutKind> {
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

pub(crate) fn format_layout(layout: AutoLayoutKind, mod_id: &str) -> String {
    match layout {
        AutoLayoutKind::Root => "/plugin.esp".to_string(),
        AutoLayoutKind::Data => "/Data/plugin.esp".to_string(),
        AutoLayoutKind::Mod => format!("/{mod_id}/plugin.esp"),
        AutoLayoutKind::ModData => format!("/{mod_id}/Data/plugin.esp"),
    }
}

