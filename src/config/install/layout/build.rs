//! Multi-archive `build` layers: overlay several archives into one staging tree.

use super::bain::{apply_bain_from_staging, list_relative_paths};
use super::fomod::apply_fomod_from_staging;
use super::plan::plan_simple;
use crate::archive::{extract_with_builtins, list_archive_paths, ArchiveFilters};
use crate::config::download;
use crate::config::schema::{ArchiveLayout, CompiledArchive};
use crate::util::fs::{copy_filtered_tree, normalize_relative_path, staging_dir_for};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use super::super::InstallSettings;

/// Where one staged file came from, before the layers were overlaid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StagedSource {
    /// Index into `archive.build`.
    pub(crate) layer: usize,
    /// The path as that layer's archive spells it.
    pub(crate) path: String,
}

pub(crate) fn extract_build_archive(
    mod_id: &str,
    archive_index: usize,
    archive: &CompiledArchive,
    target_root: &Path,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
    settings: &InstallSettings,
) -> anyhow::Result<usize> {
    let sources = resolve_build_layers(mod_id, archive_index, archive, settings)?;

    // What the overlaid tree will contain, decided before anything is unpacked.
    // Everything downstream -- the FOMOD's `source` lookups, BAIN's subpackage
    // names, the plain fold -- is answered from this list rather than from the
    // tree, so a build mod's contribution is knowable without installing it.
    let layer_listings = sources
        .iter()
        .zip(&archive.build)
        .map(|(source, layer)| Ok((layer.dest_prefix.as_deref(), list_archive_paths(source)?)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let predicted = plan_build_staging(&layer_listings)?;

    let staging_dir = staging_dir_for(target_root)?;
    std::fs::create_dir_all(&staging_dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create staging dir {}: {err}",
            staging_dir.display()
        )
    })?;

    let result = (|| -> anyhow::Result<usize> {
        let passthrough = ArchiveFilters::new(&[], &[])?;
        for (source, layer) in sources.iter().zip(&archive.build) {
            merge_layer_into_staging(
                source,
                &staging_dir,
                layer.dest_prefix.as_deref(),
                &passthrough,
            )?;
        }

        let staged = list_relative_paths(&staging_dir)?;
        check_prediction(&predicted, &staged, mod_id)?;

        let destination_root = super::destination_for(target_root, archive.target_subdir.as_deref())?;
        std::fs::create_dir_all(&destination_root).map_err(|err| {
            anyhow::anyhow!("failed to create {}: {err}", destination_root.display())
        })?;

        match archive.layout {
            Some(ArchiveLayout::Fomod) => apply_fomod_from_staging(
                &staging_dir,
                &destination_root,
                archive,
                filters,
                active_plugins,
            ),
            Some(ArchiveLayout::Bain) => apply_bain_from_staging(
                &staging_dir,
                &destination_root,
                archive,
                filters,
                "build layer",
            ),
            _ => super::apply_plan(
                &staging_dir,
                &destination_root,
                &plan_simple(&staged, filters),
            ),
        }
    })();

    let _ = std::fs::remove_dir_all(&staging_dir);
    result
}

fn resolve_build_layers(
    mod_id: &str,
    archive_index: usize,
    archive: &CompiledArchive,
    settings: &InstallSettings,
) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::with_capacity(archive.build.len());
    for (layer_index, layer) in archive.build.iter().enumerate() {
        let cache_name =
            download::build_layer_cache_file_name(mod_id, archive_index, layer_index, &layer.path);
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
        out.push(source);
    }
    Ok(out)
}

/// The staged tree a set of `build` layers produces, from their entry lists.
///
/// Each layer is rebased onto its `dest_prefix` and copied over what came
/// before, so a later layer replacing an earlier one at the same path is the
/// point rather than a collision -- that is how "take the base mod, then
/// overlay the patch" is spelled.
pub(crate) fn plan_build_staging(
    layers: &[(Option<&str>, Vec<String>)],
) -> anyhow::Result<BTreeMap<String, StagedSource>> {
    let mut staged = BTreeMap::new();

    for (index, (dest_prefix, paths)) in layers.iter().enumerate() {
        let prefix = match dest_prefix {
            Some(prefix) => normalize_relative_path(prefix)
                .map_err(|err| {
                    anyhow::anyhow!("invalid build layer dest_prefix '{prefix}': {err}")
                })?
                .to_string_lossy()
                .replace('\\', "/"),
            None => String::new(),
        };

        for path in paths {
            let normalized = path.replace('\\', "/");
            let staged_path = if prefix.is_empty() {
                normalized.clone()
            } else {
                format!("{prefix}/{normalized}")
            };
            staged.insert(
                staged_path,
                StagedSource {
                    layer: index,
                    path: normalized,
                },
            );
        }
    }

    Ok(staged)
}

/// Confirm the predicted staging tree is the one that actually appeared.
///
/// The prediction is what makes a build mod's file list knowable without
/// installing it, and a wrong prediction is a silently dropped file rather
/// than a failure -- so it is checked while the tree is still there to check
/// against. This goes away when the extraction itself is driven by the plan.
fn check_prediction(
    predicted: &BTreeMap<String, StagedSource>,
    staged: &[String],
    mod_id: &str,
) -> anyhow::Result<()> {
    let actual: BTreeMap<&str, ()> = staged.iter().map(|path| (path.as_str(), ())).collect();
    let missing: Vec<&str> = predicted
        .keys()
        .filter(|path| !actual.contains_key(path.as_str()))
        .map(String::as_str)
        .collect();
    let unexpected: Vec<&str> = actual
        .keys()
        .filter(|path| !predicted.contains_key(**path))
        .copied()
        .collect();

    if missing.is_empty() && unexpected.is_empty() {
        return Ok(());
    }

    anyhow::bail!(
        "build layers for mod {mod_id} did not stage what their entry lists predicted: \
         {} predicted but absent (e.g. {}), {} present but unpredicted (e.g. {})",
        missing.len(),
        missing.first().copied().unwrap_or("-"),
        unexpected.len(),
        unexpected.first().copied().unwrap_or("-"),
    );
}

pub(crate) fn merge_layer_into_staging(
    source: &Path,
    staging_dir: &Path,
    dest_prefix: Option<&str>,
    filters: &ArchiveFilters,
) -> anyhow::Result<()> {
    let layer_temp = staging_dir_for(staging_dir)?;
    std::fs::create_dir_all(&layer_temp).map_err(|err| {
        anyhow::anyhow!(
            "failed to create layer temp dir {}: {err}",
            layer_temp.display()
        )
    })?;

    let result = (|| -> anyhow::Result<()> {
        extract_with_builtins(source, &layer_temp, filters)?;

        let dest = if let Some(prefix) = dest_prefix {
            let rel = normalize_relative_path(prefix).map_err(|err| {
                anyhow::anyhow!("invalid build layer dest_prefix '{prefix}': {err}")
            })?;
            staging_dir.join(rel)
        } else {
            staging_dir.to_path_buf()
        };
        std::fs::create_dir_all(&dest)
            .map_err(|err| anyhow::anyhow!("failed to create staging dest {}: {err}", dest.display()))?;

        // Preserve case: this assembles the *staging* tree, which a FOMOD or
        // BAIN script then reads, naming its sources in the archive's own
        // casing. Folding here makes those sources unfindable. The fold happens
        // on the way out of staging, into the mod's own folder.
        copy_filtered_tree(&layer_temp, &dest, filters)?;
        Ok(())
    })();

    let _ = std::fs::remove_dir_all(&layer_temp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(entries: &[&str]) -> Vec<String> {
        entries.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_dest_prefix_rebases_a_whole_layer() {
        let staged = plan_build_staging(&[
            (None, paths(&["fomod/ModuleConfig.xml"])),
            (Some("DarNified UI 132 FOMOD"), paths(&["menus/main.xml"])),
        ])
        .expect("plan");

        assert_eq!(
            staged.keys().collect::<Vec<_>>(),
            [
                "DarNified UI 132 FOMOD/menus/main.xml",
                "fomod/ModuleConfig.xml"
            ]
        );
        assert_eq!(
            staged["DarNified UI 132 FOMOD/menus/main.xml"],
            StagedSource {
                layer: 1,
                path: "menus/main.xml".to_string()
            },
            "the layer keeps its own spelling of the path, for extracting from"
        );
    }

    #[test]
    fn a_later_layer_overlays_an_earlier_one() {
        let staged = plan_build_staging(&[
            (None, paths(&["textures/a.dds", "textures/b.dds"])),
            (None, paths(&["textures/a.dds"])),
        ])
        .expect("plan");

        assert_eq!(staged.len(), 2);
        assert_eq!(
            staged["textures/a.dds"].layer, 1,
            "the patch wins, which is what overlaying it means"
        );
        assert_eq!(staged["textures/b.dds"].layer, 0);
    }

    #[test]
    fn a_prediction_that_misses_a_staged_file_is_an_error() {
        let predicted = plan_build_staging(&[(None, paths(&["a.txt"]))]).expect("plan");
        assert!(check_prediction(&predicted, &paths(&["a.txt"]), "Mod").is_ok());

        let err = check_prediction(&predicted, &paths(&["a.txt", "b.txt"]), "Mod")
            .expect_err("should reject");
        assert!(err.to_string().contains("b.txt"), "{err}");

        let err = check_prediction(&predicted, &[], "Mod").expect_err("should reject");
        assert!(err.to_string().contains("a.txt"), "{err}");
    }
}
