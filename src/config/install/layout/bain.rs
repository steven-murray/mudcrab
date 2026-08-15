//! BAIN archive layout: merge selected top-level subpackages into the mod root.

use super::auto::read_top_level;
use super::{destination_for, with_staged_archive};
use crate::archive::ArchiveFilters;
use crate::config::schema::CompiledArchive;
use crate::util::fs::{copy_filtered_tree, find_child_case_insensitive};
use std::path::Path;

pub(crate) fn extract_archive_with_bain_layout(
    source: &Path,
    target_root: &Path,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    if archive.data_folder.is_some() {
        anyhow::bail!(
            "BAIN layout for {} cannot be combined with data_folder",
            source.display()
        );
    }

    let destination_root = destination_for(target_root, archive.target_subdir.as_deref())?;
    with_staged_archive(source, target_root, |staging_dir| {
        apply_bain_from_staging(
            staging_dir,
            &destination_root,
            archive,
            filters,
            &source.display().to_string(),
        )
    })
}

/// Copy the selected subpackages out of an already-extracted staging tree.
///
/// Shared by the single-archive path above and the multi-archive `build` path,
/// which previously each had their own near-identical copy.
pub(crate) fn apply_bain_from_staging(
    staging_dir: &Path,
    destination_root: &Path,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    source_label: &str,
) -> anyhow::Result<usize> {
    if archive.bain_subpackages.is_empty() {
        anyhow::bail!(
            "BAIN layout for {source_label} requires bain_subpackages to list the top-level \
             package folders to install"
        );
    }

    let available_dirs = read_top_level(staging_dir)?.dirs;
    let mut copied = 0usize;
    for subpackage in &archive.bain_subpackages {
        let Some(package_root) = find_child_case_insensitive(staging_dir, subpackage) else {
            anyhow::bail!(
                "BAIN subpackage '{subpackage}' was not found in {source_label}. \
                 Available top-level directories: {}",
                available_dirs.join(", ")
            );
        };
        copied += copy_filtered_tree(&package_root, destination_root, filters)?;
    }
    Ok(copied)
}
