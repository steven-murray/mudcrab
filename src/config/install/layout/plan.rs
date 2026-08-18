//! Deciding what an archive contributes, separately from doing it.
//!
//! Every layout handler used to answer "where do these files go?" by extracting
//! the whole archive and walking the result. That made two things impossible:
//! extracting less than everything, and answering the question at all without
//! performing the install -- which is what deriving conflict file lists needs.
//!
//! A [`LayoutPlan`] is that answer, computed from an archive's *entry list*.
//! `install` applies it; the file index keeps it and discards the rest. One
//! implementation, so the index cannot drift from what install really does.

use crate::archive::ArchiveFilters;
use std::collections::BTreeMap;

/// One archive entry and where it lands, relative to the mod folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    /// Path as the archive spells it, for handing back to the extractor.
    pub source: String,
    /// Path relative to the mod's staged folder, with directories folded to
    /// lowercase the way `copy_filtered_tree_folded` writes them.
    pub destination: String,
}

/// What an archive contributes to a mod, before anything is extracted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutPlan {
    /// Sorted by destination, so a plan is comparable and diffable.
    pub files: Vec<PlannedFile>,
}

impl LayoutPlan {
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        // A BTreeMap keyed on destination both sorts and settles collisions the
        // way an overlay copy would: the last writer to a destination wins,
        // which is what layering two subpackages onto one folder means.
        let settled: BTreeMap<String, String> = pairs
            .into_iter()
            .map(|(source, destination)| (destination, source))
            .collect();

        Self {
            files: settled
                .into_iter()
                .map(|(destination, source)| PlannedFile { source, destination })
                .collect(),
        }
    }

    /// Destination paths, which is what the file index and `conflicts_with`
    /// compare against.
    pub fn destinations(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(|file| file.destination.as_str())
    }

    /// Archive paths worth extracting. Anything absent from a plan is dead
    /// weight, and both `bsdtar` and `7z` accept a list of what to pull out.
    pub fn sources(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(|file| file.source.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }
}

/// The layout that does nothing: the tree root *is* the mod root.
///
/// `layout = "simple"` says so on the author's authority, and a `build` whose
/// layers already assemble the finished folder has nothing left to rebase.
pub fn plan_simple(paths: &[String], filters: &ArchiveFilters) -> LayoutPlan {
    LayoutPlan::from_pairs(paths.iter().filter_map(|path| {
        let normalized = path.replace('\\', "/");
        filters
            .should_extract(&normalized)
            .then(|| (path.clone(), folded_destination(&normalized)))
    }))
}

/// Fold directory components to lowercase, leaving the file name alone.
///
/// Mirrors `copy_filtered_tree_folded`, which is what actually writes these
/// paths. Two archives contributing `Sound/` and `sound/` have to land in one
/// folder on a case-sensitive filesystem.
pub fn folded_destination(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let Some((dirs, name)) = normalized.rsplit_once('/') else {
        return normalized;
    };
    format!("{}/{}", dirs.to_lowercase(), name)
}

/// Strip a leading directory from an archive path, case-insensitively.
///
/// Returns `None` when `path` is not under `prefix`. Used by every handler that
/// descends into a subfolder -- BAIN's subpackages, `auto`'s `Data/`, a
/// wrapper folder -- since descending *is* removing that prefix.
pub fn strip_dir_prefix(path: &str, prefix: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let prefix = prefix.replace('\\', "/");
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        return Some(path);
    }

    let (head, rest) = path.split_at_checked(prefix.len())?;
    if !head.eq_ignore_ascii_case(prefix) {
        return None;
    }
    // Must be a directory boundary: `Data/x` is under `Data`, `Database/x` is
    // not, and the prefix on its own names no file.
    rest.strip_prefix('/').map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destinations_fold_directories_but_not_file_names() {
        assert_eq!(folded_destination("Textures/Architecture/A.dds"), "textures/architecture/A.dds");
        assert_eq!(folded_destination("Readme.txt"), "Readme.txt");
        assert_eq!(folded_destination(r"Meshes\Weapons\Sword.nif"), "meshes/weapons/Sword.nif");
    }

    #[test]
    fn prefix_stripping_respects_directory_boundaries() {
        assert_eq!(strip_dir_prefix("Data/textures/a.dds", "Data").as_deref(), Some("textures/a.dds"));
        // Case-insensitive, as Windows-authored archives require.
        assert_eq!(strip_dir_prefix("data/textures/a.dds", "Data").as_deref(), Some("textures/a.dds"));
        assert_eq!(strip_dir_prefix("Data/a", "data/").as_deref(), Some("a"));
        // `Database` is not under `Data`.
        assert_eq!(strip_dir_prefix("Database/a.dds", "Data"), None);
        // The prefix alone is a directory, not a file.
        assert_eq!(strip_dir_prefix("Data", "Data"), None);
        assert_eq!(strip_dir_prefix("00 Core/x", "01 Other"), None);
    }

    #[test]
    fn a_plan_is_sorted_and_lets_the_last_writer_to_a_destination_win() {
        let plan = LayoutPlan::from_pairs([
            ("01 Second/textures/a.dds".to_string(), "textures/a.dds".to_string()),
            ("00 First/textures/a.dds".to_string(), "textures/a.dds".to_string()),
            ("00 First/meshes/b.nif".to_string(), "meshes/b.nif".to_string()),
        ]);

        assert_eq!(plan.len(), 2, "one destination, one winner");
        assert_eq!(
            plan.destinations().collect::<Vec<_>>(),
            ["meshes/b.nif", "textures/a.dds"],
            "sorted by destination"
        );
        // BTreeMap insertion order decides: the later pair overwrote the earlier.
        assert_eq!(
            plan.files.iter().find(|f| f.destination == "textures/a.dds").map(|f| f.source.as_str()),
            Some("00 First/textures/a.dds")
        );
    }
}

/// A read-only view of an archive's entry list, shaped like a directory tree.
///
/// The detection layout does -- "is there a `Data/` here?", "where are the
/// plugins?", "is the only top-level entry a wrapper folder?" -- is all
/// structural, and structure is exactly what a list of paths already carries.
/// Answering from the list instead of from an extracted tree is what lets a
/// plan be computed before anything is unpacked.
pub struct Listing<'a> {
    paths: &'a [String],
}

/// Immediate children of one directory.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Children {
    pub dirs: Vec<String>,
    pub files: Vec<String>,
}

impl<'a> Listing<'a> {
    pub fn new(paths: &'a [String]) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &'a [String] {
        self.paths
    }

    /// Paths under `prefix`, with the prefix removed. `""` means the root.
    pub fn under(&self, prefix: &str) -> impl Iterator<Item = String> + '_ {
        let prefix = prefix.to_string();
        self.paths.iter().filter_map(move |path| {
            if prefix.is_empty() {
                Some(path.replace('\\', "/"))
            } else {
                strip_dir_prefix(path, &prefix)
            }
        })
    }

    /// Immediate children of `prefix`, deduplicated and in listing order.
    pub fn children(&self, prefix: &str) -> Children {
        let mut children = Children::default();
        for rest in self.under(prefix) {
            match rest.split_once('/') {
                Some((dir, _)) => {
                    if !children.dirs.iter().any(|seen| seen.eq_ignore_ascii_case(dir)) {
                        children.dirs.push(dir.to_string());
                    }
                }
                None => {
                    if !children.files.iter().any(|seen| seen.eq_ignore_ascii_case(&rest)) {
                        children.files.push(rest);
                    }
                }
            }
        }
        children
    }

    pub fn has_dir(&self, prefix: &str, name: &str) -> bool {
        self.children(prefix)
            .dirs
            .iter()
            .any(|dir| dir.eq_ignore_ascii_case(name))
    }

    /// Plugin paths anywhere under `prefix`, relative to it.
    pub fn plugin_paths(&self, prefix: &str) -> Vec<String> {
        self.under(prefix)
            .filter(|rest| is_plugin_name(rest))
            .collect()
    }
}

/// Whether a path names a plugin the game would load.
///
/// Mirrors `stage::is_plugin_file`, which takes a `Path`; this works on the
/// archive's own strings, before anything exists on disk.
pub fn is_plugin_name(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    if name.ends_with(".mohidden") {
        return false;
    }
    name.ends_with(".esp") || name.ends_with(".esm")
}

#[cfg(test)]
mod listing_tests {
    use super::*;

    fn sample() -> Vec<String> {
        [
            "readme.txt",
            "Data/textures/rock.dds",
            "Data/meshes/a.nif",
            "Data/Plugin.esp",
            "Docs/manual.txt",
        ]
        .iter()
        .map(ToString::to_string)
        .collect()
    }

    #[test]
    fn children_separates_dirs_from_files_without_duplicates() {
        let paths = sample();
        let listing = Listing::new(&paths);

        let root = listing.children("");
        assert_eq!(root.dirs, ["Data", "Docs"]);
        assert_eq!(root.files, ["readme.txt"]);

        // `Data` has two subdirectories and one loose file, each listed once
        // even though `textures/` holds several entries.
        let data = listing.children("Data");
        assert_eq!(data.dirs, ["textures", "meshes"]);
        assert_eq!(data.files, ["Plugin.esp"]);
    }

    #[test]
    fn lookups_are_case_insensitive_like_the_archives_they_come_from() {
        let paths = sample();
        let listing = Listing::new(&paths);
        assert!(listing.has_dir("", "data"));
        assert!(listing.has_dir("", "DATA"));
        assert!(!listing.has_dir("", "Database"));
        assert_eq!(listing.plugin_paths("data"), ["Plugin.esp"]);
    }

    #[test]
    fn hidden_plugins_are_not_plugins() {
        assert!(is_plugin_name("Data/Foo.esp"));
        assert!(is_plugin_name("Foo.ESM"));
        assert!(!is_plugin_name("Foo.esp.mohidden"));
        assert!(!is_plugin_name("textures/foo.dds"));
    }
}
