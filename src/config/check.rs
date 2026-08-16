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
    /// Read-only directories holding archives already on this machine. An
    /// archive found here needs no download, so `check` must count it as
    /// satisfied or it would send the author hunting for links they already have.
    pub archive_search_paths: Vec<PathBuf>,
}

/// Where an archive can be had from, best case first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveAvailability {
    /// Already in the cache.
    Cached,
    /// Not cached, but sitting in one of the search paths.
    Local,
    /// Neither -- the network is the only way to get it.
    MustDownload,
}

impl ArchiveAvailability {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cached => "cached",
            Self::Local => "resolvable locally",
            Self::MustDownload => "MUST BE DOWNLOADED",
        }
    }
}

pub struct CheckReport {
    pub mods_checked: usize,
    pub archives_checked: usize,
    pub file_references_checked: usize,
    pub archives_cached: usize,
    pub archives_local: usize,
    pub archives_must_download: usize,
}

pub fn check_all(plan: &PersonalizedPlan, settings: &CheckSettings) -> anyhow::Result<CheckReport> {
    let mut mods_checked = 0usize;
    let mut archives_checked = 0usize;
    let mut file_references_checked = 0usize;
    let mut archives_cached = 0usize;
    let mut archives_local = 0usize;
    let mut archives_must_download = 0usize;
    let mut errors = Vec::new();

    for mod_entry in &plan.mods {
        if !settings.filter.matches(&mod_entry.section, &mod_entry.id) {
            continue;
        }
        mods_checked += 1;

        for (archive_index, archive) in mod_entry.archives.iter().enumerate() {
            archives_checked += 1;

            let availability = archive_availability(
                mod_entry.id.as_str(),
                archive_index,
                archive,
                settings,
            );
            match availability {
                ArchiveAvailability::Cached => archives_cached += 1,
                ArchiveAvailability::Local => archives_local += 1,
                ArchiveAvailability::MustDownload => archives_must_download += 1,
            }

            tracing::info!(
                mod_id = %mod_entry.id,
                archive_index,
                source = %archive.path.as_deref().unwrap_or("<build>"),
                file_name = %archive.file_name.as_deref().unwrap_or("<unset>"),
                availability = availability.label(),
                "check: archive availability"
            );

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

    // Emitted before the bail: the run that fails is exactly the run whose
    // summary the author needs, since it is the one that names a dead link.
    tracing::info!(
        mods_checked,
        archives_checked,
        cached = archives_cached,
        resolvable_locally = archives_local,
        must_be_downloaded = archives_must_download,
        "check summary"
    );

    if !errors.is_empty() {
        let joined = errors.join("\n");
        anyhow::bail!("check failed with {} issue(s):\n{joined}", errors.len());
    }

    Ok(CheckReport {
        mods_checked,
        archives_checked,
        file_references_checked,
        archives_cached,
        archives_local,
        archives_must_download,
    })
}

/// Classify one archive without fetching or adopting anything.
///
/// `check` is a read-only report, so a locally resolvable archive is *not*
/// linked into the cache here -- doing so would make a diagnostic command
/// silently mutate the cache it is reporting on.
fn archive_availability(
    mod_id: &str,
    archive_index: usize,
    archive: &CompiledArchive,
    settings: &CheckSettings,
) -> ArchiveAvailability {
    if !archive.build.is_empty() {
        // Build layers have no declared filename to match a local copy against,
        // so they are cached or they are not.
        let all_cached = archive.build.iter().enumerate().all(|(layer_index, layer)| {
            let cache_name = download::build_layer_cache_file_name(
                mod_id,
                archive_index,
                layer_index,
                &layer.path,
            );
            download::resolve_cache_path(&settings.cache_dir, &cache_name)
                .is_some_and(|path| path.is_file())
        });
        return if all_cached {
            ArchiveAvailability::Cached
        } else {
            ArchiveAvailability::MustDownload
        };
    }

    let path = archive.path.as_deref().unwrap_or_default();
    let cache_name = download::cache_file_name(mod_id, archive_index, path);
    if download::resolve_cache_path(&settings.cache_dir, &cache_name)
        .is_some_and(|path| path.is_file())
    {
        return ArchiveAvailability::Cached;
    }

    if download::find_local_archive(archive.file_name.as_deref(), &settings.archive_search_paths)
        .is_some()
    {
        return ArchiveAvailability::Local;
    }

    ArchiveAvailability::MustDownload
}

fn check_single_archive(
    mod_id: &str,
    archive_index: usize,
    archive: &CompiledArchive,
    settings: &CheckSettings,
) -> anyhow::Result<usize> {
    let path = archive.path.as_deref().unwrap_or_default();
    let cache_name = download::cache_file_name(mod_id, archive_index, path);
    // Falling back to the search-path copy lets the contents be validated
    // in place, so a not-yet-adopted archive is checked as thoroughly as a
    // cached one -- and the search path is only ever read.
    let source = download::resolve_cache_path(&settings.cache_dir, &cache_name)
        .or_else(|| {
            download::find_local_archive(
                archive.file_name.as_deref(),
                &settings.archive_search_paths,
            )
        })
        .unwrap_or_else(|| settings.cache_dir.join(&cache_name));

    if !source.exists() {
        anyhow::bail!(
            "archive must be downloaded; no cached copy at {} and no search path holds {}",
            source.display(),
            archive.file_name.as_deref().unwrap_or("it (no file_name declared)")
        );
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
