//! Automatic layout detection for archives that declare no explicit layout.

use crate::archive::ArchiveFilters;
use crate::util::fs::eq_ci;
use super::plan::{folded_destination, is_plugin_name, strip_dir_prefix, Children, LayoutPlan, Listing};

// ---------------------------------------------------------------------------
// Listing-based detection.
//
// The same decisions as the staged-tree code below, taken from an archive's
// entry list instead of from an extracted tree. This is the version install
// uses; the staged tree is listed and fed through here, so there is one
// implementation and a file index built from it cannot disagree with what
// install does.
// ---------------------------------------------------------------------------

/// The directory prefix to descend into, `""` for the archive root.
pub(crate) fn detect_auto_root(
    listing: &Listing<'_>,
    mod_id: &str,
    source_label: &str,
) -> anyhow::Result<String> {
    let mut inferred: Option<AutoLayoutKind> = None;
    for plugin in listing.plugin_paths("") {
        let layout = classify_plugin_layout(&plugin, mod_id).ok_or_else(|| {
            anyhow::anyhow!(
                "unsupported archive layout for {source_label}: plugin '{plugin}' is not in one of \
                 the supported roots (/plugin.esp, /Data/plugin.esp, /{mod_id}/plugin.esp, \
                 /{mod_id}/Data/plugin.esp)"
            )
        })?;

        if let Some(existing) = inferred
            && existing != layout
        {
            anyhow::bail!(
                "unsupported archive layout for {source_label}: plugins are split across multiple \
                 roots (at least '{}' and '{}')",
                format_layout(existing, mod_id),
                format_layout(layout, mod_id)
            );
        }
        inferred = Some(layout);
    }

    let top = listing.children("");
    let kind = match inferred {
        Some(kind) => kind,
        None => {
            if let Some(wrapper) = detect_content_wrapper(listing, source_label, &top)? {
                return Ok(wrapper);
            }
            if top.files.is_empty() && top.dirs.len() == 1 && listing.has_dir(&top.dirs[0], "Data") {
                AutoLayoutKind::WrapperData
            } else if top.files.is_empty() && top.dirs.len() == 1 && eq_ci(&top.dirs[0], mod_id) {
                AutoLayoutKind::Mod
            } else if listing.has_dir("", "Data") {
                AutoLayoutKind::Data
            } else {
                AutoLayoutKind::Root
            }
        }
    };

    if kind == AutoLayoutKind::Mod
        && !(top.files.is_empty() && top.dirs.len() == 1 && eq_ci(&top.dirs[0], mod_id))
    {
        anyhow::bail!(
            "unsupported archive layout for {source_label}: /{mod_id}/... auto-detection requires \
             that the only top-level entry is a folder named '{mod_id}'"
        );
    }

    if kind == AutoLayoutKind::WrapperData && !(top.files.is_empty() && top.dirs.len() == 1) {
        anyhow::bail!(
            "unsupported archive layout for {source_label}: /<folder>/Data/... auto-detection \
             requires that the only top-level entry is that one folder, but found {} director{} \
             and {} file{}",
            top.dirs.len(),
            if top.dirs.len() == 1 { "y" } else { "ies" },
            top.files.len(),
            if top.files.len() == 1 { "" } else { "s" },
        );
    }

    Ok(match kind {
        AutoLayoutKind::Root => String::new(),
        AutoLayoutKind::Data => find_child_name(listing, "", "Data"),
        AutoLayoutKind::Mod => find_child_name(listing, "", mod_id),
        AutoLayoutKind::WrapperData => {
            let wrapper = top.dirs[0].clone();
            let data = find_child_name(listing, &wrapper, "Data");
            format!("{wrapper}/{data}")
        }
    })
}

/// The archive's own spelling of a child directory, so the prefix we strip
/// matches the paths we strip it from.
fn find_child_name(listing: &Listing<'_>, prefix: &str, name: &str) -> String {
    listing
        .children(prefix)
        .dirs
        .into_iter()
        .find(|dir| eq_ci(dir, name))
        .unwrap_or_else(|| name.to_string())
}

fn has_expected_content(listing: &Listing<'_>, prefix: &str) -> bool {
    let children = listing.children(prefix);
    children
        .dirs
        .iter()
        .any(|dir| is_expected_game_content_dir_name(dir))
        || children.files.iter().any(|file| is_plugin_name(file))
}

fn detect_content_wrapper(
    listing: &Listing<'_>,
    source_label: &str,
    top: &Children,
) -> anyhow::Result<Option<String>> {
    let root_has_expected = has_expected_content(listing, "");
    let hits: Vec<String> = top
        .dirs
        .iter()
        .filter(|dir| !is_expected_game_content_dir_name(dir))
        .filter(|dir| has_expected_content(listing, dir))
        .cloned()
        .collect();

    if root_has_expected && !hits.is_empty() {
        anyhow::bail!(
            "unsupported archive layout for {source_label}: expected game-content roots were found \
             at both archive root and nested directory level"
        );
    }
    if hits.len() > 1 {
        anyhow::bail!(
            "unsupported archive layout for {source_label}: expected game-content roots were found \
             in multiple sibling directories: {}",
            hits.join(", ")
        );
    }

    if !root_has_expected
        && hits.len() == 1
        && top.files.is_empty()
        && top.dirs.len() == 1
        && !is_expected_game_content_dir_name(&top.dirs[0])
    {
        return Ok(hits.into_iter().next());
    }
    Ok(None)
}

/// What an auto-detected archive contributes, from its entry list alone.
pub(crate) fn plan_auto(
    paths: &[String],
    mod_id: &str,
    source_label: &str,
    filters: &ArchiveFilters,
) -> anyhow::Result<LayoutPlan> {
    let listing = Listing::new(paths);
    let root = detect_auto_root(&listing, mod_id, source_label)?;

    let mut pairs: Vec<(String, String)> = Vec::new();

    // The content itself.
    for path in paths {
        let Some(rest) = (if root.is_empty() {
            Some(path.replace('\\', "/"))
        } else {
            strip_dir_prefix(path, &root)
        }) else {
            continue;
        };
        if !filters.should_extract(&rest) {
            continue;
        }
        pairs.push((path.clone(), folded_destination(&rest)));
    }

    // Files that sat beside the folder we descended into. MO2's installer keeps
    // them; unwrapping would otherwise drop them. Only files, and only from the
    // levels actually skipped -- a sibling *directory* is an alternative the
    // author separated on purpose, not extra content.
    if !root.is_empty() {
        let mut level = String::new();
        for step in root.split('/') {
            for file in listing.children(&level).files {
                let full = if level.is_empty() {
                    file.clone()
                } else {
                    format!("{level}/{file}")
                };
                if filters.should_extract(&file) {
                    pairs.push((full, file));
                }
            }
            level = if level.is_empty() {
                step.to_string()
            } else {
                format!("{level}/{step}")
            };
        }
    }

    Ok(LayoutPlan::from_pairs(pairs))
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

    /// Detection now works from an archive's entry list, so a test fixture is
    /// just that list -- no staging tree needed.
    fn resolve(files: &[&str], mod_id: &str) -> anyhow::Result<String> {
        let paths: Vec<String> = files.iter().map(ToString::to_string).collect();
        let root = detect_auto_root(&Listing::new(&paths), mod_id, "archive.7z")?;
        Ok(if root.is_empty() { ".".to_string() } else { root })
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
