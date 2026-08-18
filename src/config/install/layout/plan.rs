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
