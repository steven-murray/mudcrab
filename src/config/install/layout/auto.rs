//! Automatic layout detection for archives that declare no explicit layout.

use super::with_staged_archive;
use crate::archive::ArchiveFilters;
use crate::util::fs::{
    copy_filtered_tree, eq_ci, find_child_case_insensitive, path_exists_case_insensitive,
};
use std::path::{Path, PathBuf};

use super::super::is_plugin_file;

pub(crate) fn extract_archive_with_auto_layout(
    source: &Path,
    target_root: &Path,
    mod_id: &str,
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    with_staged_archive(source, target_root, |staging_dir| {
        let source_root = detect_auto_source_root(staging_dir, mod_id, source)?;
        copy_filtered_tree(&source_root, target_root, filters)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoLayoutKind {
    Root,
    Data,
    Mod,
    /// A single top-level folder holding a `Data/`, whatever that folder is
    /// called. Authors name it after the archive, the mod, the version, or
    /// nothing in particular, so the name carries no information -- the `Data/`
    /// inside it is what says the folder is a wrapper.
    WrapperData,
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

        if top.files.is_empty()
            && top.dirs.len() == 1
            && path_exists_case_insensitive(&staging_dir.join(&top.dirs[0]), "Data")
        {
            // A lone folder containing `Data/` is a wrapper regardless of its
            // name. Requiring the name to match the mod id used to leave these
            // installing one level too deep -- silently, because the files are
            // all present, just unreachable.
            AutoLayoutKind::WrapperData
        } else if top.files.is_empty() && top.dirs.len() == 1 && eq_ci(&top.dirs[0], mod_id) {
            // Named after the mod with no `Data/` inside. Still name-gated: a
            // lone folder with no `Data/` is every bit as likely to *be* the
            // content (an archive that is just `textures/`) as to wrap it.
            AutoLayoutKind::Mod
        } else if path_exists_case_insensitive(staging_dir, "Data") {
            AutoLayoutKind::Data
        } else {
            AutoLayoutKind::Root
        }
    };

    if inferred_layout == AutoLayoutKind::Mod
        && !(top.files.is_empty() && top.dirs.len() == 1 && eq_ci(&top.dirs[0], mod_id))
    {
        anyhow::bail!(
            "unsupported archive layout for {}: /{}/... auto-detection requires that the only top-level entry is a folder named '{}'",
            source.display(),
            mod_id,
            mod_id
        );
    }

    if inferred_layout == AutoLayoutKind::WrapperData
        && !(top.files.is_empty() && top.dirs.len() == 1)
    {
        anyhow::bail!(
            "unsupported archive layout for {}: /<folder>/Data/... auto-detection requires that \
             the only top-level entry is that one folder, but found {} director{} and {} file{}",
            source.display(),
            top.dirs.len(),
            if top.dirs.len() == 1 { "y" } else { "ies" },
            top.files.len(),
            if top.files.len() == 1 { "" } else { "s" },
        );
    }

    let root = match inferred_layout {
        AutoLayoutKind::Root => staging_dir.to_path_buf(),
        AutoLayoutKind::Data => find_child_case_insensitive(staging_dir, "Data")
            .ok_or_else(|| anyhow::anyhow!("internal error: expected top-level Data in {}", source.display()))?,
        AutoLayoutKind::Mod => find_child_case_insensitive(staging_dir, mod_id)
            .ok_or_else(|| anyhow::anyhow!("internal error: expected top-level mod folder '{}' in {}", mod_id, source.display()))?,
        AutoLayoutKind::WrapperData => {
            let wrapper = staging_dir.join(&top.dirs[0]);
            find_child_case_insensitive(&wrapper, "Data").ok_or_else(|| {
                anyhow::anyhow!(
                    "internal error: expected Data under wrapper folder '{}' in {}",
                    top.dirs[0],
                    source.display()
                )
            })?
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
    // Any single wrapper, not just one named after the mod: the mod id is our
    // name for the mod, not a property of the archive.
    if parts.len() == 3 && eq_ci(parts[1], "Data") {
        return Some(AutoLayoutKind::WrapperData);
    }

    None
}

pub(crate) fn format_layout(layout: AutoLayoutKind, mod_id: &str) -> String {
    match layout {
        AutoLayoutKind::Root => "/plugin.esp".to_string(),
        AutoLayoutKind::Data => "/Data/plugin.esp".to_string(),
        AutoLayoutKind::Mod => format!("/{mod_id}/plugin.esp"),
        AutoLayoutKind::WrapperData => "/<folder>/Data/plugin.esp".to_string(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Build a staging tree from a list of relative file paths.
    fn staged(files: &[&str]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        for path in files {
            let full = dir.path().join(path);
            std::fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
            std::fs::write(full, b"x").expect("write");
        }
        let root = dir.path().to_path_buf();
        (dir, root)
    }

    fn resolve(files: &[&str], mod_id: &str) -> anyhow::Result<String> {
        let (dir, root) = staged(files);
        let found = detect_auto_source_root(&root, mod_id, Path::new("archive.7z"))?;
        let relative = found
            .strip_prefix(&root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        drop(dir);
        Ok(if relative.is_empty() { ".".to_string() } else { relative })
    }

    #[test]
    fn a_lone_wrapper_holding_data_is_unwrapped_whatever_it_is_called() {
        // Six real archives in the MOFAM list look like this, and the wrapper is
        // named after the archive, never after our id for the mod. Requiring the
        // name to match left them installing one level too deep -- silently,
        // since every file is present, just where nothing will look for it.
        for (files, mod_id) in [
            (
                &["Unofficial Oblivion Tree Patch/Data/trees/shrubboxwood.spt"][..],
                "Unofficial Oblivion Tree Patch - UOTP",
            ),
            (
                &["MOO - Hill Giant Eye Fix/Data/Meshes/moo/hillgiant/giant.nif"][..],
                "Hill Giant Eye Fix - Loreless Creatures - MOO",
            ),
            (&["patch/Data/Textures/characters/x.dds"][..], "Warpaints patch"),
        ] {
            let root = resolve(files, mod_id).expect("should resolve");
            let expected = format!("{}/Data", files[0].split('/').next().unwrap());
            assert_eq!(root, expected, "for {files:?}");
        }
    }

    #[test]
    fn a_wrapper_holding_data_works_for_plugins_too() {
        let root = resolve(&["Some Mod v3/Data/Thing.esp"], "Thing").expect("resolve");
        assert_eq!(root, "Some Mod v3/Data");
    }

    #[test]
    fn a_lone_content_folder_is_not_mistaken_for_a_wrapper() {
        // The reason the no-Data case stays name-gated: an archive that is just
        // `textures/` is content, not a wrapper, and unwrapping it would install
        // the textures' *contents* at the mod root.
        let root = resolve(&["textures/menus/x.dds"], "Some Mod").expect("resolve");
        assert_eq!(root, ".");
    }

    #[test]
    fn a_top_level_data_folder_still_wins() {
        let root = resolve(&["Data/meshes/x.nif"], "Some Mod").expect("resolve");
        assert_eq!(root, "Data");
    }

    #[test]
    fn a_folder_named_after_the_mod_without_data_is_still_unwrapped() {
        let root = resolve(&["Some Mod/meshes/x.nif"], "Some Mod").expect("resolve");
        assert_eq!(root, "Some Mod");
    }

    #[test]
    fn two_top_level_folders_are_not_unwrapped_even_if_one_holds_data() {
        // Ambiguous: which one is the mod? Better to fail the guess and let the
        // entry say, than to silently pick one.
        let root = resolve(
            &["Option A/Data/meshes/x.nif", "Option B/readme.txt"],
            "Some Mod",
        )
        .expect("resolve");
        assert_eq!(root, ".", "no unwrap when there is more than one candidate");
    }
}
