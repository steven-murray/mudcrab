use crate::archive;
use crate::config::archive_store::{self, ArchiveRef, Found};
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
    /// Already in the downloads folder.
    Cached,
    /// Not in the downloads folder, but sitting in one of the search paths.
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

    let in_scope = || {
        plan.mods
            .iter()
            .filter(|m| settings.filter.matches(&m.section, &m.id))
    };
    let bar = crate::ui::progress(
        in_scope().map(|m| m.archives.len() as u64).sum(),
        "checking",
    );
    for mod_entry in in_scope() {
        mods_checked += 1;
        bar.set_message(mod_entry.id.clone());

        for (archive_index, archive) in mod_entry.archives.iter().enumerate() {
            archives_checked += 1;
            bar.inc(1);

            let availability =
                archive_availability(mod_entry.id.as_str(), archive_index, archive, settings);
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
                check_single_archive(archive, settings)
            } else {
                check_build_archive(archive, settings)
            };

            match result {
                Ok(reference_count) => {
                    file_references_checked += reference_count;
                }
                Err(err) => {
                    // Counted from 1, as a person counts a mod's archives.
                    let which = if mod_entry.archives.len() > 1 {
                        format!(
                            " (archive {} of {})",
                            archive_index + 1,
                            mod_entry.archives.len()
                        )
                    } else {
                        String::new()
                    };
                    errors.push(format!("{}{which}: {err}", mod_entry.id));
                }
            }
        }
    }

    bar.finish_and_clear();
    // Emitted before the bail: the run that fails is exactly the run whose
    // summary the author needs, since it is the one that names a dead link.
    crate::ui::say(format!(
        "Checked {archives_checked} archive(s) in {mods_checked} mod(s): {archives_cached} in the \
         downloads folder, {archives_local} in search paths, {archives_must_download} to download{}",
        if errors.is_empty() {
            String::new()
        } else {
            format!("; {} problem(s)", errors.len())
        }
    ));
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
    _mod_id: &str,
    _archive_index: usize,
    archive: &CompiledArchive,
    settings: &CheckSettings,
) -> ArchiveAvailability {
    let locate = |source: &str, file_name: Option<&str>| {
        archive_store::locate(
            &settings.cache_dir,
            &settings.archive_search_paths,
            &ArchiveRef::new(source, file_name),
        )
    };

    if !archive.build.is_empty() {
        let found: Vec<_> = archive
            .build
            .iter()
            .map(|layer| locate(&layer.path, None))
            .collect();
        return if found.iter().all(|f| matches!(f, Some(Found::Downloads(_)))) {
            ArchiveAvailability::Cached
        } else if found.iter().all(Option::is_some) {
            ArchiveAvailability::Local
        } else {
            ArchiveAvailability::MustDownload
        };
    }

    match locate(
        archive.path.as_deref().unwrap_or_default(),
        archive.file_name.as_deref(),
    ) {
        Some(Found::Downloads(_)) => ArchiveAvailability::Cached,
        Some(Found::SearchPath(_)) => ArchiveAvailability::Local,
        None => ArchiveAvailability::MustDownload,
    }
}

/// Where an archive is, in the downloads folder or a search path. A search
/// path copy is validated in place: `check` only ever reads.
fn located(settings: &CheckSettings, source: &str, file_name: Option<&str>) -> Option<PathBuf> {
    archive_store::locate(
        &settings.cache_dir,
        &settings.archive_search_paths,
        &ArchiveRef::new(source, file_name),
    )
    .map(|found| match found {
        Found::Downloads(path) | Found::SearchPath(path) => path,
    })
}

fn check_single_archive(
    archive: &CompiledArchive,
    settings: &CheckSettings,
) -> anyhow::Result<usize> {
    let path = archive.path.as_deref().unwrap_or_default();
    let reference = ArchiveRef::new(path, archive.file_name.as_deref());
    let Some(source) = located(settings, path, archive.file_name.as_deref()) else {
        let how = if path.starts_with("manual:") {
            "it is downloaded by hand (`manual:`), so put it there yourself"
        } else {
            "`mudcrab download` fetches it"
        };
        let mut searched = vec![settings.cache_dir.clone()];
        searched.extend(settings.archive_search_paths.iter().cloned());
        let similar = crate::config::download::similar_archives(reference.describe(), &searched);
        let hint = if similar.is_empty() {
            String::new()
        } else {
            format!(
                "; similarly named, if one is this file renamed: {}",
                similar
                    .iter()
                    .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        anyhow::bail!(
            "{} is not in the downloads folder ({}) or any search path; {how}{hint}",
            reference.describe(),
            settings.cache_dir.display(),
        );
    };
    if source.is_dir() {
        anyhow::bail!("archive path is a directory: {}", source.display());
    }

    // Listing the archive is what actually proves it is readable and not a
    // truncated or misnamed download. This previously claimed to "verify the
    // archive is openable" in a comment and then returned without opening it,
    // so `check` only stat'd files unless game_root_files was set.
    let file_paths = archive::list_archive_paths(&source)
        .map_err(|err| anyhow::anyhow!("archive is not readable ({}): {err}", source.display()))?;

    if archive.game_root_files.is_empty() {
        return Ok(0);
    }

    validate_file_references(&file_paths, &archive.game_root_files)
}

fn check_build_archive(
    archive: &CompiledArchive,
    settings: &CheckSettings,
) -> anyhow::Result<usize> {
    // Collect all paths from every layer, prefixing with dest_prefix where set.
    let mut merged_paths: Vec<String> = Vec::new();

    for (layer_index, layer) in archive.build.iter().enumerate() {
        let Some(source) = located(settings, &layer.path, None) else {
            anyhow::bail!(
                "build layer {} ({}) is not in \
                 {} or any search path",
                layer_index + 1,
                layer.path,
                settings.cache_dir.display()
            );
        };
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

        let layer_paths = archive::list_archive_paths(&source).map_err(|err| {
            anyhow::anyhow!(
                "unable to list build layer {} archive {}: {err}",
                layer_index,
                source.display()
            )
        })?;

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
            anyhow::bail!(
                "file reference matched no files in archive contents: {}",
                reference
            );
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
