//! Multi-archive `build` layers: overlay several archives into one staging tree.

use crate::archive::{extract_with_builtins, ArchiveFilters};
use crate::config::download;
use crate::config::schema::CompiledArchive;
use super::bain::apply_bain_from_staging;
use super::fomod::apply_fomod_from_staging;
use crate::util::fs::{copy_filtered_tree, normalize_relative_path, staging_dir_for};
use std::collections::HashSet;
use std::path::Path;

use super::super::InstallSettings;

pub(crate) fn extract_build_archive(
    mod_id: &str,
    archive_index: usize,
    archive: &CompiledArchive,
    target_root: &Path,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
    settings: &InstallSettings,
) -> anyhow::Result<usize> {
    let staging_dir = staging_dir_for(target_root)?;
    std::fs::create_dir_all(&staging_dir)
        .map_err(|err| anyhow::anyhow!("failed to create staging dir {}: {err}", staging_dir.display()))?;

    let result = (|| -> anyhow::Result<usize> {
        let passthrough = ArchiveFilters::new(&[], &[])?;

        for (layer_index, layer) in archive.build.iter().enumerate() {
            let cache_name = download::build_layer_cache_file_name(mod_id, archive_index, layer_index, &layer.path);
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

            merge_layer_into_staging(&source, &staging_dir, layer.dest_prefix.as_deref(), &passthrough)?;
        }

        let destination_root = if let Some(target_subdir) = archive.target_subdir.as_deref() {
            target_root.join(normalize_relative_path(target_subdir)?)
        } else {
            target_root.to_path_buf()
        };
        std::fs::create_dir_all(&destination_root)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", destination_root.display()))?;

        match archive.layout.as_deref() {
            Some("fomod") => {
                apply_fomod_from_staging(
                    &staging_dir,
                    &destination_root,
                    archive,
                    filters,
                    active_plugins,
                )
            }
            Some("bain") => apply_bain_from_staging(&staging_dir, &destination_root, archive, filters),
            _ => copy_filtered_tree(&staging_dir, &destination_root, filters),
        }
    })();

    let _ = std::fs::remove_dir_all(&staging_dir);
    result
}

pub(crate) fn merge_layer_into_staging(
    source: &Path,
    staging_dir: &Path,
    dest_prefix: Option<&str>,
    filters: &ArchiveFilters,
) -> anyhow::Result<()> {
    let layer_temp = staging_dir_for(staging_dir)?;
    std::fs::create_dir_all(&layer_temp)
        .map_err(|err| anyhow::anyhow!("failed to create layer temp dir {}: {err}", layer_temp.display()))?;

    let result = (|| -> anyhow::Result<()> {
        extract_with_builtins(source, &layer_temp, filters)?;

        let dest = if let Some(prefix) = dest_prefix {
            let rel = normalize_relative_path(prefix)
                .map_err(|err| anyhow::anyhow!("invalid build layer dest_prefix '{prefix}': {err}"))?;
            staging_dir.join(rel)
        } else {
            staging_dir.to_path_buf()
        };
        std::fs::create_dir_all(&dest)
            .map_err(|err| anyhow::anyhow!("failed to create staging dest {}: {err}", dest.display()))?;

        copy_filtered_tree(&layer_temp, &dest, filters)?;
        Ok(())
    })();

    let _ = std::fs::remove_dir_all(&layer_temp);
    result
}

