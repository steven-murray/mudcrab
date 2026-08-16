use crate::archive;
use crate::config::download;
use crate::config::filter::ModFilter;
use crate::config::schema::{CompiledArchive, PersonalizedPlan};
use globset::{GlobBuilder, GlobMatcher};
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct CheckSettings {
    pub cache_dir: PathBuf,
    /// Which mods to check. Empty means the whole plan.
    pub filter: ModFilter,
}

pub struct CheckReport {
    pub mods_checked: usize,
    pub archives_checked: usize,
    pub file_references_checked: usize,
}

pub fn check_all(plan: &PersonalizedPlan, settings: &CheckSettings) -> anyhow::Result<CheckReport> {
    let mut mods_checked = 0usize;
    let mut archives_checked = 0usize;
    let mut file_references_checked = 0usize;
    let mut errors = Vec::new();

    for mod_entry in &plan.mods {
        if !settings.filter.matches(&mod_entry.section, &mod_entry.id) {
            continue;
        }
        mods_checked += 1;

        for (archive_index, archive) in mod_entry.archives.iter().enumerate() {
            archives_checked += 1;

            let result = if archive.build.is_empty() {
                check_single_archive(mod_entry.id.as_str(), archive_index, archive, settings)
            } else {
                check_build_archive(mod_entry.id.as_str(), archive_index, archive, settings)
            };

            match result {
                Ok(reference_count) => {
                    file_references_checked += reference_count;
                }
                Err(err) => {
                    errors.push(format!(
                        "mod '{}' archive {} failed check: {err}",
                        mod_entry.id, archive_index
                    ));
                }
            }
        }
    }

    if !errors.is_empty() {
        let joined = errors.join("\n");
        anyhow::bail!("check failed with {} issue(s):\n{joined}", errors.len());
    }

    Ok(CheckReport {
        mods_checked,
        archives_checked,
        file_references_checked,
    })
}

fn check_single_archive(
    mod_id: &str,
    archive_index: usize,
    archive: &CompiledArchive,
    settings: &CheckSettings,
) -> anyhow::Result<usize> {
    let path = archive.path.as_deref().unwrap_or_default();
    let cache_name = download::cache_file_name(mod_id, archive_index, path);
    let source = download::resolve_cache_path(&settings.cache_dir, &cache_name)
        .unwrap_or_else(|| settings.cache_dir.join(&cache_name));

    if !source.exists() {
        anyhow::bail!("missing cached archive: {}", source.display());
    }
    if source.is_dir() {
        anyhow::bail!("cached archive path is a directory: {}", source.display());
    }

    // Listing the archive is what actually proves it is readable and not a
    // truncated or misnamed download. This previously claimed to "verify the
    // archive is openable" in a comment and then returned without opening it,
    // so `check` only stat'd files unless game_root_files was set.
    let file_paths = archive::list_archive_paths(&source)
        .map_err(|err| anyhow::anyhow!("cached archive is not readable ({}): {err}", source.display()))?;

    if archive.game_root_files.is_empty() {
        return Ok(0);
    }

    validate_file_references(&file_paths, &archive.game_root_files)
}

fn check_build_archive(
    mod_id: &str,
    archive_index: usize,
    archive: &CompiledArchive,
    settings: &CheckSettings,
) -> anyhow::Result<usize> {
    // Collect all paths from every layer, prefixing with dest_prefix where set.
    let mut merged_paths: Vec<String> = Vec::new();

    for (layer_index, layer) in archive.build.iter().enumerate() {
        let cache_name =
            download::build_layer_cache_file_name(mod_id, archive_index, layer_index, &layer.path);
        let source = download::resolve_cache_path(&settings.cache_dir, &cache_name)
            .unwrap_or_else(|| settings.cache_dir.join(&cache_name));

        if !source.exists() {
            anyhow::bail!(
                "missing cached archive for build layer {}: {}",
                layer_index,
                source.display()
            );
        }
        if source.is_dir() {
            anyhow::bail!(
                "cached archive path is a directory for build layer {}: {}",
                layer_index,
                source.display()
            );
        }

        if archive.game_root_files.is_empty() {
            // Only existence was needed; no need to list.
            continue;
        }

        let layer_paths = archive::list_archive_paths(&source)
            .map_err(|err| anyhow::anyhow!("unable to list build layer {} archive {}: {err}", layer_index, source.display()))?;

        if let Some(dest_prefix) = layer.dest_prefix.as_deref() {
            let prefix = normalize_prefix(dest_prefix)?;
            for p in layer_paths {
                merged_paths.push(format!("{prefix}/{p}"));
            }
        } else {
            merged_paths.extend(layer_paths);
        }
    }

    if archive.game_root_files.is_empty() {
        return Ok(0);
    }

    validate_file_references(&merged_paths, &archive.game_root_files)
}

fn validate_file_references(paths: &[String], refs: &[String]) -> anyhow::Result<usize> {
    let mut checked = 0usize;

    for reference in refs {
        let matcher = compile_case_insensitive_glob(reference)?;
        let matched = paths.iter().any(|p| matcher.is_match(p));
        checked += 1;
        if !matched {
            anyhow::bail!("file reference matched no files in archive contents: {}", reference);
        }
    }

    Ok(checked)
}

fn compile_case_insensitive_glob(pattern: &str) -> anyhow::Result<GlobMatcher> {
    GlobBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .map_err(|err| anyhow::anyhow!("invalid glob pattern '{}': {err}", pattern))
}

fn normalize_prefix(value: &str) -> anyhow::Result<String> {
    let normalized = archive::normalize_archive_path(std::path::Path::new(value))?;
    if normalized.is_empty() {
        anyhow::bail!("destination prefix cannot be empty");
    }
    Ok(normalized)
}
