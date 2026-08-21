//! Archive extraction and layout resolution.

pub mod auto;
pub mod bain;
pub mod build;
pub mod fomod;
pub mod plan;

use crate::archive::{extract_entries, extract_with_builtins, list_archive_paths, ArchiveFilters};
use crate::config::download;
use crate::config::schema::{ArchiveLayout, CompiledArchive, ModType, PersonalizedMod};
use build::extract_build_archive;

use crate::util::fs::{lowercase_path, normalize_relative_path, staging_dir_for};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use super::InstallSettings;

/// List an archive, decide what it contributes, then unpack only that.
///
/// The old shape was the other way round -- unpack everything, walk the result,
/// copy what was wanted -- which meant a 3854-file texture pack was written out
/// in full so that 48 files could be copied from it. Deciding first is the
/// point of the planner; this is where the decision starts paying.
///
/// The scratch reader is for the one layout that cannot decide from paths
/// alone: FOMOD needs its `ModuleConfig.xml`, so that single entry comes out
/// first and the rest still waits for the plan.
pub(crate) fn with_planned_archive(
    source: &Path,
    target_root: &Path,
    destination_root: &Path,
    plan_from: impl FnOnce(&[String], &EntryReader<'_>) -> anyhow::Result<plan::LayoutPlan>,
) -> anyhow::Result<usize> {
    let paths = list_archive_paths(source)?;

    let staging_dir = staging_dir_for(target_root)?;
    std::fs::create_dir_all(&staging_dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create staging dir {}: {err}",
            staging_dir.display()
        )
    })?;

    let result = (|| -> anyhow::Result<usize> {
        let reader = EntryReader {
            source,
            scratch: staging_dir.join(".mudcrab-entry"),
        };
        let plan = plan_from(&paths, &reader)?;
        let _ = std::fs::remove_dir_all(&reader.scratch);

        let wanted: BTreeSet<String> = plan.sources().map(ToString::to_string).collect();
        extract_entries(source, &staging_dir, &wanted)?;
        apply_plan(&staging_dir, destination_root, &plan)
    })();

    let _ = std::fs::remove_dir_all(&staging_dir);
    result
}

/// Reads a single entry out of an archive, before the plan exists.
pub(crate) struct EntryReader<'a> {
    source: &'a Path,
    scratch: PathBuf,
}

impl<'a> EntryReader<'a> {
    pub(crate) fn new(source: &'a Path, scratch: PathBuf) -> Self {
        Self { source, scratch }
    }

    pub(crate) fn read(&self, entry: &str) -> anyhow::Result<Vec<u8>> {
        std::fs::create_dir_all(&self.scratch).map_err(|err| {
            anyhow::anyhow!("failed to create {}: {err}", self.scratch.display())
        })?;
        let wanted = BTreeSet::from([entry.to_string()]);
        extract_entries(self.source, &self.scratch, &wanted)?;

        let path = self.scratch.join(entry);
        std::fs::read(&path).map_err(|err| {
            anyhow::anyhow!(
                "failed to read '{entry}' from {}: {err}",
                self.source.display()
            )
        })
    }
}

/// Append `target_subdir` to the mod root when the archive declares one.
pub(crate) fn destination_for(
    target_root: &Path,
    target_subdir: Option<&str>,
) -> anyhow::Result<PathBuf> {
    match target_subdir {
        // Every component here is a directory, so all of them fold. See
        // `lowercase_dir_components`.
        Some(subdir) => Ok(target_root.join(lowercase_path(&normalize_relative_path(subdir)?))),
        None => Ok(target_root.to_path_buf()),
    }
}

/// Copy a planned set of files out of a staged tree.
///
/// The plan already decided every source and destination, so this is only the
/// doing: create parents, copy, count.
pub(crate) fn apply_plan(
    staging_dir: &Path,
    destination_root: &Path,
    plan: &plan::LayoutPlan,
) -> anyhow::Result<usize> {
    for file in &plan.files {
        let from = staging_dir.join(&file.source);
        let to = destination_root.join(&file.destination);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
        }
        std::fs::copy(&from, &to).map_err(|err| {
            anyhow::anyhow!("failed to copy {} to {}: {err}", from.display(), to.display())
        })?;
    }
    Ok(plan.files.len())
}

pub(crate) fn install_mod_archives(
    mod_entry: &PersonalizedMod,
    settings: &InstallSettings,
    target_root: &Path,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    if mod_entry.mod_type == Some(ModType::BuildFromFiles) {
        return install_mod_from_files(mod_entry, settings, target_root);
    }

    let mut extracted_count = 0usize;

    for (archive_index, archive) in mod_entry.archives.iter().enumerate() {
        if !archive.build.is_empty() {
            // game_root_files has no extraction pass on the build path. Previously
            // these patterns were merged into the exclude list, so matching files
            // were dropped from the mod folder and never written to the game root
            // either -- silently lost. Reject rather than lose files.
            if !archive.game_root_files.is_empty() {
                anyhow::bail!(
                    "mod '{}' archive {}: game_root_files is not supported together with \
                     build layers. Split the game-root files into their own archive entry.",
                    mod_entry.id,
                    archive_index
                );
            }

            let filters = ArchiveFilters::new(&archive.include, &archive.exclude)?;

            if settings.dry_run {
                tracing::info!(
                    mod_id = %mod_entry.id,
                    destination = %target_root.display(),
                    layers = archive.build.len(),
                    "install dry-run build-layer extract"
                );
                continue;
            }

            extracted_count += extract_build_archive(
                &mod_entry.id,
                archive_index,
                archive,
                target_root,
                &filters,
                active_plugins,
                settings,
            )?;
            continue;
        }

        let path = archive.path.as_deref().unwrap_or_default();
        let cache_name = download::cache_file_name(&mod_entry.id, archive_index, path);
        let mut source = download::resolve_cache_path(&settings.cache_dir, &cache_name)
            .unwrap_or_else(|| settings.cache_dir.join(&cache_name));

        if !source.exists() {
            // A dry run exists to show the whole plan, so an uncached archive
            // is something to report and carry on from -- aborting on the
            // first one hides everything after it. `check` is the command that
            // verifies the cache; this one previews.
            if settings.dry_run {
                // A dry run must not write, so it reports what adoption *would*
                // find rather than linking it into the cache.
                match download::find_local_archive(
                    archive.file_name.as_deref(),
                    &settings.archive_search_paths,
                ) {
                    Some(local) => tracing::info!(
                        mod_id = %mod_entry.id,
                        local = %local.display(),
                        "install dry-run: archive would be adopted from a local search path"
                    ),
                    None => tracing::warn!(
                        mod_id = %mod_entry.id,
                        archive = %source.display(),
                        "install dry-run: archive is not cached, would need downloading"
                    ),
                }
                continue;
            }

            // Installing straight from archives already on disk is the point of
            // --archive-search-path: no `download` run has to have happened.
            match download::link_local_archive(
                archive.file_name.as_deref(),
                &settings.cache_dir,
                &cache_name,
                &settings.archive_search_paths,
            )? {
                Some(linked) => source = linked,
                None => anyhow::bail!(
                    "missing cached archive for mod {}: {}",
                    mod_entry.id,
                    source.display()
                ),
            }
        }

        // Game-root extraction pass: extract matching files to the game-root output folder.
        // These files are also auto-excluded from the normal mod installation below.
        if !archive.game_root_files.is_empty() {
            if let Some(game_root_dir) = &settings.game_root_dir {
                let grf_filters = ArchiveFilters::new(&archive.game_root_files, &[])?;
                if settings.dry_run {
                    tracing::info!(
                        mod_id = %mod_entry.id,
                        source = %source.display(),
                        game_root = %game_root_dir.display(),
                        patterns = ?archive.game_root_files,
                        "install dry-run game-root extract"
                    );
                } else {
                    std::fs::create_dir_all(game_root_dir).map_err(|err| {
                        anyhow::anyhow!(
                            "failed to create game-root dir {}: {err}",
                            game_root_dir.display()
                        )
                    })?;
                    let extracted = extract_with_builtins(&source, game_root_dir, &grf_filters)?;
                    tracing::info!(
                        mod_id = %mod_entry.id,
                        game_root = %game_root_dir.display(),
                        extracted,
                        "game-root files extracted"
                    );
                }
            } else {
                tracing::warn!(
                    mod_id = %mod_entry.id,
                    patterns = ?archive.game_root_files,
                    "archive has game_root_files but no game-root-dir is configured; game-root files will not be extracted"
                );
            }
        }

        // Normal extraction pass; game_root_files are added to the effective exclude list so
        // they are not duplicated into the mod's staging folder.
        let effective_exclude: Vec<String> = archive
            .exclude
            .iter()
            .chain(archive.game_root_files.iter())
            .cloned()
            .collect();
        let filters = ArchiveFilters::new(&archive.include, &effective_exclude)?;

        if settings.dry_run {
            tracing::info!(
                mod_id = %mod_entry.id,
                source = %source.display(),
                destination = %target_root.display(),
                data_folder = ?archive.data_folder,
                target_subdir = ?archive.target_subdir,
                "install dry-run extract"
            );
        } else {
            std::fs::create_dir_all(target_root).map_err(|err| {
                anyhow::anyhow!("failed to create {}: {err}", target_root.display())
            })?;

            extracted_count += extract_archive(&source, target_root, &mod_entry.id, archive, &filters, active_plugins)?;
        }
    }

    Ok(extracted_count)
}

pub(crate) fn install_mod_from_files(
    mod_entry: &PersonalizedMod,
    settings: &InstallSettings,
    target_root: &Path,
) -> anyhow::Result<usize> {
    if mod_entry.files.is_empty() {
        anyhow::bail!(
            "mod {} has type=build-from-files but no files were specified",
            mod_entry.id
        );
    }

    let Some(game_dir) = &settings.game_dir else {
        anyhow::bail!(
            "mod {} uses type=build-from-files and requires --game-dir for %GAME_DIR% expansion",
            mod_entry.id
        );
    };

    let mut copied = 0usize;
    let mut seen_targets: HashSet<String> = HashSet::new();

    if !settings.dry_run {
        std::fs::create_dir_all(target_root).map_err(|err| {
            anyhow::anyhow!("failed to create {}: {err}", target_root.display())
        })?;
    }

    for pattern in &mod_entry.files {
        let expanded = pattern.replace("%GAME_DIR%", &game_dir.to_string_lossy());
        let matches = resolve_file_pattern(&expanded)?;
        if matches.is_empty() {
            anyhow::bail!(
                "mod {} build-from-files pattern matched no files: {}",
                mod_entry.id,
                pattern
            );
        }

        for source in matches {
            let file_name = source
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or_else(|| anyhow::anyhow!(
                    "mod {} build-from-files source has invalid filename: {}",
                    mod_entry.id,
                    source.display()
                ))?
                .to_string();
            let key = file_name.to_ascii_lowercase();
            if !seen_targets.insert(key) {
                continue;
            }

            let destination = target_root.join(&file_name);
            if settings.dry_run {
                tracing::info!(
                    mod_id = %mod_entry.id,
                    source = %source.display(),
                    destination = %destination.display(),
                    "install dry-run build-from-files copy"
                );
            } else {
                std::fs::copy(&source, &destination).map_err(|err| {
                    anyhow::anyhow!(
                        "failed to copy {} to {}: {err}",
                        source.display(),
                        destination.display()
                    )
                })?;
            }

            copied += 1;
        }
    }

    Ok(copied)
}

pub(crate) fn resolve_file_pattern(pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
    let has_glob = pattern.contains('*') || pattern.contains('?') || pattern.contains('[');
    if !has_glob {
        let path = PathBuf::from(pattern);
        if path.exists() && path.is_file() {
            return Ok(vec![path]);
        }
        return Ok(Vec::new());
    }

    let normalized = pattern.replace('\\', "/");
    let split_idx = normalized.rfind('/').ok_or_else(|| {
        anyhow::anyhow!(
            "glob pattern must include a parent directory: {}",
            pattern
        )
    })?;
    let (parent_str, file_pat) = normalized.split_at(split_idx);
    let file_pat = &file_pat[1..];
    let parent = PathBuf::from(parent_str);

    if !parent.exists() {
        return Ok(Vec::new());
    }

    let matcher = globset::Glob::new(file_pat)
        .map_err(|err| anyhow::anyhow!("invalid file glob '{}': {err}", file_pat))?
        .compile_matcher();

    let mut matches = Vec::new();
    for entry in std::fs::read_dir(&parent)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", parent.display()))?
    {
        let entry = entry
            .map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", parent.display()))?;
        if !entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read type for {}: {err}", entry.path().display()))?
            .is_file()
        {
            continue;
        }

        let name = entry.file_name();
        if matcher.is_match(name.to_string_lossy().as_ref()) {
            matches.push(entry.path());
        }
    }

    matches.sort();
    Ok(matches)
}

pub(crate) fn extract_archive(
    source: &Path,
    target_root: &Path,
    mod_id: &str,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    if let Some(inner) = archive.inner_archive.as_deref() {
        return extract_nested_archive(
            source,
            target_root,
            mod_id,
            archive,
            filters,
            active_plugins,
            inner,
        );
    }

    plan_and_extract(source, target_root, mod_id, archive, filters, active_plugins)
}

fn plan_and_extract(
    source: &Path,
    target_root: &Path,
    mod_id: &str,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    let destination_root = destination_for(target_root, archive.target_subdir.as_deref())?;
    with_planned_archive(source, target_root, &destination_root, |paths, reader| {
        plan_archive(
            &source.display().to_string(),
            paths,
            reader,
            mod_id,
            archive,
            filters,
            active_plugins,
        )
    })
}

/// Install a mod whose content is packed inside a second archive.
///
/// Two extractions rather than one. The inner archive goes through the whole
/// layout pipeline, because that is where the game content is and so that is
/// what `layout`, `data_folder` and the rest are describing. The container's
/// own files -- readmes, screenshots -- are staged at the mod root, which is
/// where they sit in the container. The container entry itself is never
/// written: it is packaging, not content.
///
/// `filters` apply to both halves, so an `exclude` can drop a container readme
/// as readily as an inner texture. One rule for the pair is easier to predict
/// than a split, and the alternative -- filters that quietly stop applying
/// halfway through a mod -- is the kind of thing only noticed by a diff.
#[allow(clippy::too_many_arguments)]
fn extract_nested_archive(
    source: &Path,
    target_root: &Path,
    mod_id: &str,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
    inner: &str,
) -> anyhow::Result<usize> {
    let paths = list_archive_paths(source)?;
    let entry = paths
        .iter()
        .find(|path| path.eq_ignore_ascii_case(inner))
        .cloned()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "mod '{mod_id}': inner_archive '{inner}' is not in {} ({} entries). Run \
                 `mudcrab inspect` on the archive to see what it holds.",
                source.display(),
                paths.len()
            )
        })?;

    let staging_dir = staging_dir_for(target_root)?;
    std::fs::create_dir_all(&staging_dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create staging dir {}: {err}",
            staging_dir.display()
        )
    })?;

    let result = (|| -> anyhow::Result<usize> {
        extract_entries(source, &staging_dir, &BTreeSet::from([entry.clone()]))?;
        let nested = staging_dir.join(&entry);

        let destination_root = destination_for(target_root, archive.target_subdir.as_deref())?;
        let container_files =
            with_planned_archive(source, target_root, &destination_root, |paths, _| {
                let leftovers: Vec<String> = paths
                    .iter()
                    .filter(|path| **path != entry)
                    .cloned()
                    .collect();
                Ok(plan::plan_simple(&leftovers, filters))
            })?;

        let inner_files = plan_and_extract(
            &nested,
            target_root,
            mod_id,
            archive,
            filters,
            active_plugins,
        )?;

        Ok(container_files + inner_files)
    })();

    let _ = std::fs::remove_dir_all(&staging_dir);
    result
}

/// Which layout applies, and what it decides.
///
/// The single dispatch. `install` applies the plan; the file index keeps the
/// destinations and throws the rest away. Two callers, one answer -- which is
/// the point: an index that disagreed with what install does would report
/// conflicts against files that never arrive.
pub(crate) fn plan_archive(
    source_label: &str,
    paths: &[String],
    reader: &EntryReader<'_>,
    mod_id: &str,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<plan::LayoutPlan> {
    if archive.layout == Some(ArchiveLayout::Fomod) {
        if archive.data_folder.is_some() {
            anyhow::bail!("FOMOD layout for {source_label} cannot be combined with data_folder");
        }
        if !archive.bain_subpackages.is_empty() {
            anyhow::bail!(
                "FOMOD layout for {source_label} cannot be combined with bain_subpackages"
            );
        }
        return fomod::plan_fomod_archive(
            paths,
            reader,
            source_label,
            archive,
            filters,
            active_plugins,
        );
    }

    if archive.layout == Some(ArchiveLayout::Bain) {
        if archive.data_folder.is_some() {
            anyhow::bail!("BAIN layout for {source_label} cannot be combined with data_folder");
        }
        return bain::plan_bain(paths, archive, filters, source_label);
    }

    // `simple` says the archive root *is* the data folder, on the author's
    // authority. It used to fall through to auto-detection, which made the
    // setting silently inert -- and worse than inert for an archive auto
    // detection cannot classify, since declaring the answer explicitly still
    // got you the guess. Part 10's WAC archive is exactly that: its plugins sit
    // in a `WAC_Natural_Habitat_by_Max_Tael/` subfolder that matches none of
    // the layouts detection knows, so it is rejected outright even though the
    // two files this list wants are plainly at the root.
    if archive.layout == Some(ArchiveLayout::Simple) {
        return Ok(plan::plan_simple(paths, filters));
    }

    let declared = |value: &Option<String>| value.as_deref().is_some_and(|v| !v.trim().is_empty());
    if !declared(&archive.data_folder) && !declared(&archive.target_subdir) {
        return auto::plan_auto(paths, mod_id, source_label, filters);
    }

    plan::plan_data_folder(paths, archive.data_folder.as_deref(), filters, source_label)
}
