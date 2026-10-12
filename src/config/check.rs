//! `check`: everything about a plan that can be known before installing it,
//! with every archive in the downloads folder.
//!
//! `download` leaves every archive in the downloads folder -- found in a search
//! path, it is linked in -- so that is the one place looked in. Each archive is
//! opened and its file list read, which catches a download cut short. Then the
//! layout `install` would use is planned from that list, which catches an
//! installer option or subpackage the archive does not have, and the plugins
//! it would install are read for their masters, which are checked against the
//! load order. Nothing is unpacked but plugin headers, and nothing is changed.

use crate::archive::{self, ArchiveFilters};
use crate::config::archive_store::{self, ArchiveRef};
use crate::config::filter::ModFilter;
use crate::config::install::layout::{self, EntryReader};
use crate::config::schema::{CompiledArchive, ModType, PersonalizedMod, PersonalizedPlan};
use globset::{GlobBuilder, GlobMatcher};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct CheckSettings {
    pub cache_dir: PathBuf,
    /// Which mods to check. Empty means the whole plan.
    pub filter: ModFilter,
}

pub struct CheckReport {
    pub mods_checked: usize,
    pub archives_checked: usize,
    pub plugins_checked: usize,
}

/// The plugins shipped by the game itself. A master among these is present
/// unless the list's load order says otherwise.
const GAME_PLUGINS: &[&str] = &[
    "oblivion.esm",
    "dlcshiveringisles.esp",
    "knights.esp",
    "dlcbattlehorncastle.esp",
    "dlcfrostcrag.esp",
    "dlchorsearmor.esp",
    "dlcmehrunesrazor.esp",
    "dlcorrery.esp",
    "dlcspelltomes.esp",
    "dlcthievesden.esp",
    "dlcvilelair.esp",
];

/// A plugin a mod's archives would install, and its masters.
struct Shipped {
    mod_id: String,
    masters: Vec<String>,
}

pub fn check_all(plan: &PersonalizedPlan, settings: &CheckSettings) -> anyhow::Result<CheckReport> {
    let mut mods_checked = 0usize;
    let mut archives_checked = 0usize;
    let mut errors = Vec::new();
    // Lowercased plugin name -> what ships it.
    let mut shipped: HashMap<String, Shipped> = HashMap::new();
    let active_plugins: HashSet<String> = plan
        .plugins
        .iter()
        .map(|plugin| plugin.to_ascii_lowercase())
        .collect();

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
            let result = if archive.build.is_empty() {
                check_single_archive(mod_entry, archive, settings, &active_plugins)
            } else {
                check_build_archive(archive, settings).map(|()| Vec::new())
            };
            match result {
                Ok(plugins) => {
                    for (name, masters) in plugins {
                        shipped.insert(
                            name.to_ascii_lowercase(),
                            Shipped {
                                mod_id: mod_entry.id.clone(),
                                masters,
                            },
                        );
                    }
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

    let (master_errors, plugins_checked) = master_problems(plan, &shipped);
    errors.extend(master_errors);
    let unread: Vec<&String> = plan
        .plugins
        .iter()
        .filter(|plugin| {
            let key = plugin.to_ascii_lowercase();
            !shipped.contains_key(&key) && !GAME_PLUGINS.contains(&key.as_str())
        })
        .collect();
    tracing::info!(plugins = ?unread, "check: masters not read");
    let unread = unread.len();

    // Emitted before the bail: the run that fails is exactly the run whose
    // summary the author needs.
    crate::ui::say(format!(
        "Checked {archives_checked} archive(s) in {mods_checked} mod(s), and the masters of \
         {plugins_checked} plugin(s){}{}",
        if unread > 0 && settings.filter.is_empty() {
            format!(
                " ({unread} more are made at install -- merges, dummy plugins -- or come \
                 from an archive inside an archive)"
            )
        } else {
            String::new()
        },
        if errors.is_empty() {
            String::new()
        } else {
            format!("; {} problem(s)", errors.len())
        }
    ));
    tracing::info!(
        mods_checked,
        archives_checked,
        plugins_checked,
        "check summary"
    );

    if !errors.is_empty() {
        let joined = errors.join("\n");
        anyhow::bail!("check failed with {} issue(s):\n{joined}", errors.len());
    }

    Ok(CheckReport {
        mods_checked,
        archives_checked,
        plugins_checked,
    })
}

/// Every active plugin whose masters are known: each master must load before
/// it, and be active. Returns the problems and how many plugins were checked.
fn master_problems(
    plan: &PersonalizedPlan,
    shipped: &HashMap<String, Shipped>,
) -> (Vec<String>, usize) {
    let inactive: HashSet<String> = plan
        .inactive_plugins
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    let position: HashMap<String, usize> = plan
        .plugins
        .iter()
        .enumerate()
        .map(|(index, p)| (p.to_ascii_lowercase(), index))
        .collect();
    // A merge that hides its sources takes them out of the load order.
    let merged_into: HashMap<String, &str> = plan
        .merges()
        .filter(|(_, spec)| spec.hide_sources)
        .flat_map(|(_, spec)| {
            spec.sources
                .iter()
                .map(|source| (source.plugin.to_ascii_lowercase(), spec.output.as_str()))
        })
        .collect();

    let mut errors = Vec::new();
    let mut checked = 0usize;
    for (index, plugin) in plan.plugins.iter().enumerate() {
        let key = plugin.to_ascii_lowercase();
        if inactive.contains(&key) {
            continue;
        }
        let Some(shipped) = shipped.get(&key) else {
            continue;
        };
        checked += 1;
        for master in &shipped.masters {
            let wanted = master.to_ascii_lowercase();
            let problem = match position.get(&wanted) {
                Some(_) if inactive.contains(&wanted) => Some(format!(
                    "needs {master}, which the list orders but leaves inactive"
                )),
                Some(&at) if at > index => Some(format!(
                    "needs {master}, which loads after it (move {plugin} below {master} in \
                     `plugins`)"
                )),
                Some(_) => None,
                None => match merged_into.get(&wanted) {
                    Some(output) => Some(format!(
                        "needs {master}, which is merged into {output} and so not loaded; merge \
                         {plugin} too, or leave {master} out of the merge"
                    )),
                    None if GAME_PLUGINS.contains(&wanted.as_str()) => None,
                    None => Some(format!("needs {master}, which is not in the list")),
                },
            };
            if let Some(problem) = problem {
                errors.push(format!("{} ({plugin}): {problem}", shipped.mod_id));
            }
        }
    }
    (errors, checked)
}

/// The archive is in the downloads folder and opens; its game-root files are
/// there; `install` could plan its layout. Returns the plugins it would
/// install, with their masters.
fn check_single_archive(
    mod_entry: &PersonalizedMod,
    archive: &CompiledArchive,
    settings: &CheckSettings,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<Vec<(String, Vec<String>)>> {
    let source = present(archive, &ArchiveRef::for_archive(archive), settings)?;

    // Listing the archive is what proves it is readable and not a truncated
    // or misnamed download.
    let paths = archive::list_archive_paths(&source)
        .map_err(|err| anyhow::anyhow!("archive is not readable ({}): {err}", source.display()))?;
    if !archive.game_root_files.is_empty() {
        validate_file_references(&paths, &archive.game_root_files)?;
    }

    // Installed some other way than by laying out an archive: an MO2 plugin
    // goes into the instance, and a container's content is a second archive
    // only unpacking reaches.
    if matches!(
        mod_entry.mod_type,
        Some(ModType::Mo2Plugin | ModType::BuildFromFiles)
    ) || archive.inner_archive.is_some()
    {
        return Ok(Vec::new());
    }

    let effective_exclude: Vec<String> = archive
        .exclude
        .iter()
        .chain(archive.game_root_files.iter())
        .cloned()
        .collect();
    let filters = ArchiveFilters::new(&archive.include, &effective_exclude)?;
    let scratch = crate::util::fs::cache_scratch_dir("check");
    let label = source.display().to_string();
    let reader = EntryReader::new(&source, scratch.join("entry"));
    let planned = layout::plan_archive(
        &label,
        &paths,
        &reader,
        &mod_entry.id,
        archive,
        &filters,
        active_plugins,
    );
    let result = (|| {
        let planned = planned.map_err(|err| anyhow::anyhow!("install would fail: {err:#}"))?;
        // Plugins land at the top of the mod folder; a `target_subdir` puts
        // everything below it, so nothing there is a plugin the game loads.
        if archive
            .target_subdir
            .as_deref()
            .is_some_and(|dir| !dir.trim().is_empty())
        {
            return Ok(Vec::new());
        }
        let plugins: Vec<(String, String)> = planned
            .files
            .iter()
            .filter(|file| !file.destination.contains('/'))
            .filter(|file| layout::plan::is_plugin_name(&file.destination))
            .map(|file| (file.source.clone(), file.destination.clone()))
            .collect();
        if plugins.is_empty() {
            return Ok(Vec::new());
        }
        let out = scratch.join("plugins");
        std::fs::create_dir_all(&out)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", out.display()))?;
        let wanted: BTreeSet<String> = plugins.iter().map(|(from, _)| from.clone()).collect();
        archive::extract_entries(&source, &out, &wanted)?;
        plugins
            .into_iter()
            .map(|(from, name)| {
                let bytes = std::fs::read(out.join(&from))
                    .map_err(|err| anyhow::anyhow!("failed to read {from}: {err}"))?;
                let masters = crate::plugin::Plugin::masters_of(&bytes)
                    .map_err(|err| anyhow::anyhow!("{name} is not a readable plugin: {err}"))?;
                Ok((name, masters))
            })
            .collect()
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// Where an archive is in the downloads folder, or why it is not there.
fn present(
    archive: &CompiledArchive,
    reference: &ArchiveRef<'_>,
    settings: &CheckSettings,
) -> anyhow::Result<PathBuf> {
    if let Some(found) = archive_store::find(&settings.cache_dir, &[], reference) {
        if found.is_dir() {
            anyhow::bail!("archive path is a directory: {}", found.display());
        }
        return Ok(found);
    }
    let path = archive.path.as_deref().unwrap_or_default();
    let how = if path.starts_with("manual:") {
        "it is downloaded by hand (`manual:`): put it there, or run `mudcrab download`, which \
         opens its page"
    } else {
        "run `mudcrab download`"
    };
    let similar = crate::config::download::similar_archives(
        reference.describe(),
        std::slice::from_ref(&settings.cache_dir),
    );
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
    let by_hand = crate::config::download::how_to_get(
        archive.download_page.as_deref(),
        archive.download_note.as_deref(),
    )
    .map(|how| format!("\n{how}"))
    .unwrap_or_default();
    anyhow::bail!(
        "{} is not in the downloads folder ({}); {how}{hint}{by_hand}",
        reference.describe(),
        settings.cache_dir.display(),
    );
}

fn check_build_archive(archive: &CompiledArchive, settings: &CheckSettings) -> anyhow::Result<()> {
    // Collect all paths from every layer, prefixing with dest_prefix where set.
    let mut merged_paths: Vec<String> = Vec::new();

    for (layer_index, layer) in archive.build.iter().enumerate() {
        let reference = ArchiveRef::new(&layer.path, layer.file_name.as_deref());
        let source = present(archive, &reference, settings)
            .map_err(|err| anyhow::anyhow!("build layer {}: {err}", layer_index + 1))?;
        let layer_paths = archive::list_archive_paths(&source).map_err(|err| {
            anyhow::anyhow!(
                "build layer {} is not readable ({}): {err}",
                layer_index + 1,
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

    if !archive.game_root_files.is_empty() {
        validate_file_references(&merged_paths, &archive.game_root_files)?;
    }
    Ok(())
}

fn validate_file_references(paths: &[String], refs: &[String]) -> anyhow::Result<()> {
    for reference in refs {
        let matcher = compile_case_insensitive_glob(reference)?;
        if !paths.iter().any(|p| matcher.is_match(p)) {
            anyhow::bail!(
                "file reference matched no files in archive contents: {}",
                reference
            );
        }
    }
    Ok(())
}

fn compile_case_insensitive_glob(pattern: &str) -> anyhow::Result<GlobMatcher> {
    GlobBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .map_err(|err| anyhow::anyhow!("invalid glob pattern '{}': {err}", pattern))
}

fn normalize_prefix(value: &str) -> anyhow::Result<String> {
    let normalized = archive::normalize_archive_path(Path::new(value))?;
    if normalized.is_empty() {
        anyhow::bail!("destination prefix cannot be empty");
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(plugins: &[&str], inactive: &[&str], mods: serde_json::Value) -> PersonalizedPlan {
        serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "name": "t",
            "responses": {},
            "mod_order": [],
            "selected_mods": [],
            "mods": mods,
            "plugins": plugins,
            "inactive_plugins": inactive,
        }))
        .unwrap()
    }

    fn ships(entries: &[(&str, &[&str])]) -> HashMap<String, Shipped> {
        entries
            .iter()
            .map(|(name, masters)| {
                (
                    name.to_ascii_lowercase(),
                    Shipped {
                        mod_id: format!("mod of {name}"),
                        masters: masters.iter().map(|m| m.to_string()).collect(),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn a_master_must_be_in_the_list_and_active() {
        let plan = plan(
            &["A.esm", "B.esp", "Late.esp", "Off.esp", "C.esp"],
            &["Off.esp"],
            serde_json::json!([]),
        );
        let shipped = ships(&[
            ("A.esm", &["Oblivion.esm"]),
            ("B.esp", &["Oblivion.esm", "a.esm"]),
            (
                "C.esp",
                &["Late.esp", "Off.esp", "Missing.esp", "Knights.esp"],
            ),
            ("Late.esp", &[]),
            ("Off.esp", &[]),
        ]);
        let (errors, checked) = master_problems(&plan, &shipped);
        assert_eq!(checked, 4, "the inactive plugin's masters do not matter");
        assert_eq!(errors.len(), 2, "{errors:#?}");
        assert!(errors[0].contains("Off.esp") && errors[0].contains("inactive"));
        assert!(errors[1].contains("Missing.esp") && errors[1].contains("not in the list"));
    }

    #[test]
    fn a_master_must_load_first() {
        let plan = plan(&["C.esp", "Late.esp"], &[], serde_json::json!([]));
        let (errors, _) = master_problems(&plan, &ships(&[("C.esp", &["Late.esp"])]));
        assert!(errors[0].contains("loads after it"), "{errors:#?}");
    }

    #[test]
    fn a_master_merged_away_is_named_with_its_merge() {
        let plan = plan(
            &["Merged.esp", "Patch.esp"],
            &[],
            serde_json::json!([{
                "id": "Merge",
                "mod_type": "merge",
                "archives": [],
                "merge": {
                    "output": "Merged.esp",
                    "sources": [{"mod": "Src", "plugin": "Src.esp"}]
                }
            }]),
        );
        let (errors, _) = master_problems(&plan, &ships(&[("Patch.esp", &["Src.esp"])]));
        assert!(errors[0].contains("merged into Merged.esp"), "{errors:#?}");
    }
}
