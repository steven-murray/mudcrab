//! BAIN archive layout: merge selected top-level subpackages into the mod root.

use crate::archive::{extract_with_builtins, ArchiveFilters};
use crate::config::schema::CompiledArchive;
use super::auto::read_top_level;
use crate::util::fs::{
    copy_filtered_tree, find_child_case_insensitive, normalize_relative_path, staging_dir_for,
};
use std::path::Path;

pub(crate) fn extract_archive_with_bain_layout(
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

pub(crate) fn apply_bain_from_staging(
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
