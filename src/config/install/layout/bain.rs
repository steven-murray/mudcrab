//! BAIN archive layout: merge selected top-level subpackages into the mod root.

use crate::archive::ArchiveFilters;
use crate::config::schema::CompiledArchive;
use super::plan::{folded_destination, strip_dir_prefix, LayoutPlan, Listing};
use std::path::Path;

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

    let listing = list_relative_paths(staging_dir)?;
    let plan = plan_bain(&listing, archive, filters, source_label)?;
    super::apply_plan(staging_dir, destination_root, &plan)
}

/// Decide what a BAIN archive contributes, from its entry list alone.
///
/// The layout is a rebase: a selected subpackage's name is stripped and what
/// remains lands in the mod root. Selections are applied in the order the
/// modlist lists them, so a later subpackage overlays an earlier one -- which
/// is what selecting both `00 Core` and `01 Patch` is asking for.
pub(crate) fn plan_bain(
    paths: &[String],
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    source_label: &str,
) -> anyhow::Result<LayoutPlan> {
    let mut pairs: Vec<(String, String)> = Vec::new();

    for subpackage in &archive.bain_subpackages {
        let mut matched = 0usize;
        for path in paths {
            let Some(rest) = strip_dir_prefix(path, subpackage) else {
                continue;
            };
            matched += 1;
            if !filters.should_extract(&rest) {
                continue;
            }
            pairs.push((path.clone(), folded_destination(&rest)));
        }

        // A subpackage naming nothing is a typo in the modlist, and silence
        // here means installing a mod that is quietly missing a third of its
        // files. Reported against the archive's real top-level names.
        if matched == 0 {
            anyhow::bail!(
                "BAIN subpackage '{subpackage}' was not found in {source_label}. \
                 Available top-level directories: {}",
                Listing::new(paths).children("").dirs.join(", ")
            );
        }
    }

    Ok(LayoutPlan::from_pairs(pairs))
}

/// Every file under `root`, as `/`-separated paths relative to it.
pub(crate) fn list_relative_paths(root: &Path) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    collect(root, root, &mut out)?;
    out.sort();
    return Ok(out);

    fn collect(dir: &Path, root: &Path, out: &mut Vec<String>) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(dir)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", dir.display()))?
        {
            let entry =
                entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", dir.display()))?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|err| {
                anyhow::anyhow!("failed to read type of {}: {err}", path.display())
            })?;
            if file_type.is_dir() {
                collect(&path, root, out)?;
            } else if let Ok(relative) = path.strip_prefix(root) {
                out.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
        Ok(())
    }
}
