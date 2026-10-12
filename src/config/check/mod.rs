//! `check`: everything about a plan that can be known before installing it,
//! with every archive in the downloads folder.
//!
//! `download` leaves every archive in the downloads folder -- found in a search
//! path, it is linked in -- so that is the one place looked in. Each archive is
//! opened and its file list read, which catches a download cut short, and an
//! archive that records its MD5 is hashed. Then the layout `install` would use
//! is planned from that list, which catches an installer option or subpackage
//! the archive does not have; the mod's file actions are rehearsed on a
//! stand-in of its folder (see `rehearse`); and what is left is checked as a
//! whole: every plugin's masters against the load order, every load-order
//! plugin against what the mods install, every BSA against the plugins that
//! load it. Nothing is unpacked but plugin headers (and a BSA an action
//! unpacks), and nothing the user owns is changed.

mod rehearse;

use crate::archive::{self, ArchiveFilters};
use crate::config::archive_store::{self, ArchiveRef};
use crate::config::filter::ModFilter;
use crate::config::install::InstallSettings;
use crate::config::install::layout::{self, EntryReader};
use crate::config::schema::{CompiledArchive, ModType, PersonalizedMod, PersonalizedPlan};
use globset::{GlobBuilder, GlobMatcher};
use rehearse::{PartnerCache, Rehearsal};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
    pub warnings: usize,
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

/// A plugin Wrye Bash writes after the install: `Bashed Patch, 0.esp`. A list
/// declares and orders it ahead of time, so it lands in place when it appears;
/// no archive ships it.
fn made_by_wrye_bash(plugin: &str) -> bool {
    let lower = plugin.to_ascii_lowercase();
    lower.starts_with("bashed patch, ") && lower.ends_with(".esp")
}

/// A plugin a mod would install, and its masters.
struct Shipped {
    mod_id: String,
    masters: Vec<String>,
}

/// A mod's finished folder, as far as it can be known.
#[derive(Default)]
struct ModFolder {
    /// Every file, lowercased and `/`-separated.
    files: BTreeSet<String>,
    /// Masters of each plugin at the top of the folder, by lowercased name,
    /// with the name as spelled.
    plugins: BTreeMap<String, (String, Vec<String>)>,
    /// False when some of the folder is only made at install (a build, an
    /// archive inside an archive, an MO2 plugin, a merge) or an action could
    /// not be rehearsed: the folder is then not complete, so nothing is
    /// concluded from what it lacks.
    complete: bool,
}

pub fn check_all(plan: &PersonalizedPlan, settings: &CheckSettings) -> anyhow::Result<CheckReport> {
    let scratch = crate::util::fs::cache_scratch_dir("check");
    let result = check_in(plan, settings, &scratch);
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

fn check_in(
    plan: &PersonalizedPlan,
    settings: &CheckSettings,
    scratch: &Path,
) -> anyhow::Result<CheckReport> {
    let mut mods_checked = 0usize;
    let mut archives_checked = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    // Mods whose actions could not be rehearsed, and why.
    let mut unrehearsed: Vec<(String, String)> = Vec::new();
    let mut folders: BTreeMap<String, ModFolder> = BTreeMap::new();
    let mut partners = PartnerCache::new();
    let active_plugins: HashSet<String> = plan
        .plugins
        .iter()
        .map(|plugin| plugin.to_ascii_lowercase())
        .collect();
    // What the actions run with: the downloads folder, and a mods folder that
    // does not exist, so another mod's files come from its archives.
    let install_settings = InstallSettings {
        cache_dir: settings.cache_dir.clone(),
        mods_dir: scratch.join("mods"),
        mo2_instance_dir: None,
        profile_name: "Default".to_string(),
        game_dir: None,
        execute_actions: true,
        dry_run: false,
        tools: Default::default(),
        filter: ModFilter::default(),
        force_merges: false,
        force: false,
    };

    let in_scope = || {
        plan.mods
            .iter()
            .filter(|m| settings.filter.matches(&m.section, &m.id))
    };
    let bar = crate::ui::progress(in_scope().count() as u64, "checking");
    for mod_entry in in_scope() {
        mods_checked += 1;
        bar.inc(1);
        bar.set_message(mod_entry.id.clone());
        archives_checked += mod_entry.archives.len();
        let folder = check_mod(
            mod_entry,
            plan,
            settings,
            &install_settings,
            &active_plugins,
            &mut partners,
            scratch,
            &mut errors,
            &mut warnings,
            &mut unrehearsed,
        );
        folders.insert(mod_entry.id.clone(), folder);
    }
    bar.finish_and_clear();

    let shipped: HashMap<String, Shipped> = folders
        .iter()
        .flat_map(|(id, folder)| {
            folder.plugins.values().map(move |(name, masters)| {
                (
                    name.to_ascii_lowercase(),
                    Shipped {
                        mod_id: id.clone(),
                        masters: masters.clone(),
                    },
                )
            })
        })
        .collect();
    let (master_errors, plugins_checked) = master_problems(plan, &shipped);
    errors.extend(master_errors);
    let whole_plan = settings.filter.is_empty();
    load_order_problems(plan, &folders, whole_plan, &mut errors, &mut warnings);
    bsa_problems(plan, &folders, &mut errors);
    duplicate_plugins(&folders, &mut warnings);

    let unread = plan
        .plugins
        .iter()
        .filter(|plugin| {
            let key = plugin.to_ascii_lowercase();
            !shipped.contains_key(&key)
                && !GAME_PLUGINS.contains(&key.as_str())
                && !made_by_wrye_bash(plugin)
        })
        .count();
    // Emitted before the bail: the run that fails is exactly the run whose
    // summary the author needs.
    crate::ui::say(format!(
        "Checked {archives_checked} archive(s) in {mods_checked} mod(s), and the masters of \
         {plugins_checked} plugin(s){}{}{}",
        if unread > 0 && whole_plan {
            format!(
                " ({unread} more are made at install, or come from an archive inside an archive)"
            )
        } else {
            String::new()
        },
        if errors.is_empty() {
            String::new()
        } else {
            format!("; {} problem(s)", errors.len())
        },
        if warnings.is_empty() {
            String::new()
        } else {
            format!("; {} warning(s)", warnings.len())
        },
    ));
    for warning in &warnings {
        crate::ui::say(format!("warning: {warning}"));
    }
    for (id, why) in &unrehearsed {
        crate::ui::say(format!("note: {id}'s actions were not checked: {why}"));
    }
    tracing::info!(
        mods_checked,
        archives_checked,
        plugins_checked,
        warnings = warnings.len(),
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
        warnings: warnings.len(),
    })
}

/// One mod: each archive present, readable and installable; its actions
/// rehearsed; its finished folder.
#[allow(clippy::too_many_arguments)]
fn check_mod(
    mod_entry: &PersonalizedMod,
    plan: &PersonalizedPlan,
    settings: &CheckSettings,
    install_settings: &InstallSettings,
    active_plugins: &HashSet<String>,
    partners: &mut PartnerCache,
    scratch: &Path,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
    unrehearsed: &mut Vec<(String, String)>,
) -> ModFolder {
    let mut folder = ModFolder {
        complete: !matches!(
            mod_entry.mod_type,
            Some(ModType::Merge | ModType::Mo2Plugin | ModType::BuildFromFiles)
        ),
        ..ModFolder::default()
    };
    // Destination -> where its real bytes were put, when they matter.
    let mut planned: Vec<(String, Option<PathBuf>)> = Vec::new();
    // Masters of every plugin the archives hold, at any depth, by destination.
    let mut masters_at: HashMap<String, Vec<String>> = HashMap::new();
    let extract_bsas = rehearse::bsas_to_extract(mod_entry);
    let real = scratch.join("real");

    for (archive_index, archive) in mod_entry.archives.iter().enumerate() {
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
        let result = if archive.build.is_empty() {
            check_single_archive(
                mod_entry,
                archive,
                settings,
                active_plugins,
                scratch,
                &real.join(archive_index.to_string()),
                &extract_bsas,
                warnings,
            )
        } else {
            check_build_archive(archive, settings).map(|()| None)
        };
        match result {
            Ok(Some(contents)) => {
                planned.extend(contents.files);
                masters_at.extend(contents.masters);
            }
            Ok(None) => folder.complete = false,
            Err(err) => {
                errors.push(format!("{}{which}: {err:#}", mod_entry.id));
                folder.complete = false;
            }
        }
    }
    if !folder.complete {
        // Still worth reading what is known, for the masters check.
        for (destination, _) in &planned {
            add_plugin(&mut folder, destination, masters_at.get(destination));
        }
        return folder;
    }

    let finished: Vec<rehearse::Finished> = if mod_entry
        .actions
        .iter()
        .any(crate::config::actions::changes_files)
    {
        match rehearse::rehearse(
            mod_entry,
            &planned,
            &scratch.join("rehearsal"),
            install_settings,
            &plan.mods,
            active_plugins,
            partners,
        ) {
            Ok(Rehearsal::Done(files)) => files,
            Ok(Rehearsal::Fails(err)) => {
                errors.push(format!("{}: install would fail: {err:#}", mod_entry.id));
                folder.complete = false;
                return folder;
            }
            Ok(Rehearsal::Unknown(why)) => {
                tracing::info!(mod_id = %mod_entry.id, why, "check: actions not rehearsed");
                unrehearsed.push((mod_entry.id.clone(), why));
                folder.complete = false;
                return folder;
            }
            Err(err) => {
                errors.push(format!("{}: {err:#}", mod_entry.id));
                folder.complete = false;
                return folder;
            }
        }
    } else {
        planned
            .iter()
            .map(|(destination, _)| rehearse::Finished {
                path: destination.to_lowercase(),
                origin: Some(destination.clone()),
                masters: None,
            })
            .collect()
    };
    for file in finished {
        if !file.path.contains('/') && layout::plan::is_plugin_name(&file.path) {
            // Planned from an archive, or made by an action (a dummy plugin).
            let masters = match &file.origin {
                Some(from) => masters_at.get(from).cloned(),
                None => file.masters,
            };
            folder.plugins.insert(
                file.path.clone(),
                (file.path.clone(), masters.unwrap_or_default()),
            );
        }
        folder.files.insert(file.path);
    }
    // The name as the archive spells it reads better in messages.
    for (key, (name, _)) in folder.plugins.iter_mut() {
        if let Some(spelled) = mod_entry
            .plugins
            .iter()
            .chain(plan.plugins.iter())
            .find(|p| p.eq_ignore_ascii_case(key))
        {
            *name = spelled.clone();
        }
    }
    folder
}

fn add_plugin(folder: &mut ModFolder, destination: &str, masters: Option<&Vec<String>>) {
    if destination.contains('/') || !layout::plan::is_plugin_name(destination) {
        return;
    }
    if let Some(masters) = masters {
        folder.plugins.insert(
            destination.to_lowercase(),
            (destination.to_string(), masters.clone()),
        );
    }
}

/// What one archive contributes: each planned file (with the real bytes for
/// those that need them), and the masters of each plugin among them.
struct Contents {
    files: Vec<(String, Option<PathBuf>)>,
    masters: HashMap<String, Vec<String>>,
}

/// The archive is in the downloads folder, opens, and matches its recorded
/// MD5; its game-root files are there; `install` could plan its layout; its
/// filters each do something. `None` when its content is only known at
/// install.
#[allow(clippy::too_many_arguments)]
fn check_single_archive(
    mod_entry: &PersonalizedMod,
    archive: &CompiledArchive,
    settings: &CheckSettings,
    active_plugins: &HashSet<String>,
    scratch: &Path,
    real: &Path,
    extract_bsas: &BTreeSet<String>,
    warnings: &mut Vec<String>,
) -> anyhow::Result<Option<Contents>> {
    let source = present(archive, &ArchiveRef::for_archive(archive), settings)?;

    // Listing the archive is what proves it is readable and not a truncated
    // or misnamed download.
    let paths = archive::list_archive_paths(&source)
        .map_err(|err| anyhow::anyhow!("archive is not readable ({}): {err}", source.display()))?;
    if let Some(md5) = &archive.md5 {
        let actual = crate::config::identify::md5_of_file(&source)?;
        anyhow::ensure!(
            actual.eq_ignore_ascii_case(md5),
            "{} is not the file the list records: its MD5 is {actual}, not {md5}. Download \
             it again, or (as the list's author) record this one with `downloads record`",
            source.display()
        );
    }
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
        return Ok(None);
    }

    let reader = EntryReader::new(&source, scratch.join("entry"));
    let label = source.display().to_string();
    let plan_with = |include: &[String], exclude: &[String]| {
        let exclude: Vec<String> = exclude
            .iter()
            .chain(archive.game_root_files.iter())
            .cloned()
            .collect();
        let filters = ArchiveFilters::new(include, &exclude)?;
        layout::plan_archive(
            &label,
            &paths,
            &reader,
            &mod_entry.id,
            archive,
            &filters,
            active_plugins,
        )
    };
    let planned = plan_with(&archive.include, &archive.exclude)
        .map_err(|err| anyhow::anyhow!("install would fail: {err:#}"))?;

    // A filter that does nothing has usually outlived a mod update.
    let file_name = archive.file_name.as_deref().unwrap_or(&label);
    for pattern in &archive.include {
        if plan_with(std::slice::from_ref(pattern), &archive.exclude)
            .is_ok_and(|plan| plan.is_empty())
        {
            warnings.push(format!(
                "{}: include '{pattern}' matches nothing in {file_name}",
                mod_entry.id
            ));
        }
    }
    if !archive.exclude.is_empty()
        && let Ok(unfiltered) = plan_with(&archive.include, &[])
    {
        for pattern in &archive.exclude {
            if plan_with(&archive.include, std::slice::from_ref(pattern))
                .is_ok_and(|plan| plan.len() == unfiltered.len())
            {
                warnings.push(format!(
                    "{}: exclude '{pattern}' leaves nothing out of {file_name}",
                    mod_entry.id
                ));
            }
        }
    }

    let prefix = archive
        .target_subdir
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            format!(
                "{}/",
                value.replace('\\', "/").trim_matches('/').to_lowercase()
            )
        })
        .unwrap_or_default();
    let mut contents = Contents {
        files: Vec::new(),
        masters: HashMap::new(),
    };

    // Plugins (at any depth: a `file_move` may bring one up) and BSAs an
    // action unpacks are the files whose bytes matter.
    let wanted: BTreeMap<String, String> = planned
        .files
        .iter()
        .map(|file| (format!("{prefix}{}", file.destination), file.source.clone()))
        .filter(|(destination, _)| {
            layout::plan::is_plugin_name(destination)
                || extract_bsas.contains(&destination.to_lowercase())
        })
        .collect();
    if !wanted.is_empty() {
        std::fs::create_dir_all(real)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", real.display()))?;
        let sources: BTreeSet<String> = wanted.values().cloned().collect();
        archive::extract_entries(&source, real, &sources)?;
    }
    for file in &planned.files {
        let destination = format!("{prefix}{}", file.destination);
        let bytes_at = wanted.get(&destination).map(|from| real.join(from));
        if let Some(path) = &bytes_at
            && layout::plan::is_plugin_name(&destination)
        {
            let bytes = std::fs::read(path)
                .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", file.source))?;
            let masters = crate::plugin::Plugin::masters_of(&bytes)
                .map_err(|err| anyhow::anyhow!("{destination} is not a readable plugin: {err}"))?;
            contents.masters.insert(destination.clone(), masters);
        }
        let keep_bytes = extract_bsas.contains(&destination.to_lowercase());
        contents
            .files
            .push((destination, bytes_at.filter(|_| keep_bytes)));
    }
    Ok(Some(contents))
}

/// The load order against what the mods install: a plugin ordered that
/// nothing installs, one installed that is not ordered, one a mod declares but
/// does not ship.
fn load_order_problems(
    plan: &PersonalizedPlan,
    folders: &BTreeMap<String, ModFolder>,
    whole_plan: bool,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let ordered: HashSet<String> = plan
        .plugins
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    let merged_away = merged_away(plan);
    let merge_outputs: HashSet<String> = plan
        .merges()
        .map(|(_, spec)| spec.output.to_ascii_lowercase())
        .collect();

    for mod_entry in &plan.mods {
        let Some(folder) = folders.get(&mod_entry.id).filter(|f| f.complete) else {
            continue;
        };
        for declared in &mod_entry.plugins {
            if !folder.plugins.contains_key(&declared.to_ascii_lowercase())
                && !made_by_wrye_bash(declared)
            {
                errors.push(format!(
                    "{}: declares {declared}, which it does not install (a renamed plugin \
                     after a mod update, or a layout that leaves it out)",
                    mod_entry.id
                ));
            }
        }
        for (key, (name, _)) in &folder.plugins {
            if !ordered.contains(key) && !merged_away.contains(key) {
                warnings.push(format!(
                    "{}: installs {name}, which the load order does not list, so MO2 shows it \
                     unticked; exclude it, or add it to `plugins`",
                    mod_entry.id
                ));
            }
        }
    }

    if !whole_plan {
        return;
    }
    // A plugin may still come from a mod whose folder is not fully known;
    // its declaration is taken at its word.
    let mut provided: HashSet<String> = folders
        .values()
        .flat_map(|folder| folder.plugins.keys().cloned())
        .collect();
    for mod_entry in &plan.mods {
        if folders.get(&mod_entry.id).is_none_or(|f| !f.complete) {
            provided.extend(mod_entry.plugins.iter().map(|p| p.to_ascii_lowercase()));
        }
    }
    for plugin in &plan.plugins {
        let key = plugin.to_ascii_lowercase();
        if !provided.contains(&key)
            && !made_by_wrye_bash(plugin)
            && !merge_outputs.contains(&key)
            && !GAME_PLUGINS.contains(&key.as_str())
        {
            errors.push(format!(
                "{plugin} is in the load order, but no mod installs it"
            ));
        }
    }
}

/// Plugins a merge with `hide_sources` takes out of the load order.
fn merged_away(plan: &PersonalizedPlan) -> HashSet<String> {
    plan.merges()
        .filter(|(_, spec)| spec.hide_sources)
        .flat_map(|(_, spec)| spec.sources.iter().map(|s| s.plugin.to_ascii_lowercase()))
        .collect()
}

/// Oblivion loads a mod's BSA only when an active plugin's name begins it
/// (`Foo.esp` loads `Foo.bsa` and `Foo - Textures.bsa`), or an INI lists it.
/// Any other BSA is silently ignored, and its contents with it.
fn bsa_problems(
    plan: &PersonalizedPlan,
    folders: &BTreeMap<String, ModFolder>,
    errors: &mut Vec<String>,
) {
    let inactive: HashSet<String> = plan
        .inactive_plugins
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    let stems: Vec<String> = plan
        .plugins
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .filter(|p| !inactive.contains(p))
        .filter_map(|p| p.rsplit_once('.').map(|(stem, _)| stem.to_string()))
        .collect();
    // Anything an INI action says, in case it adds the archive to
    // `sArchiveList`.
    let ini_text = serde_json::to_string(&plan.actions)
        .unwrap_or_default()
        .to_lowercase()
        + &serde_json::to_string(&plan.mods.iter().map(|m| &m.actions).collect::<Vec<_>>())
            .unwrap_or_default()
            .to_lowercase();

    for (id, folder) in folders.iter().filter(|(_, f)| f.complete) {
        for bsa in folder
            .files
            .iter()
            .filter(|f| !f.contains('/') && f.ends_with(".bsa"))
        {
            let stem = bsa.trim_end_matches(".bsa");
            if stems.iter().any(|plugin| stem.starts_with(plugin.as_str()))
                || ini_text.contains(bsa.as_str())
            {
                continue;
            }
            errors.push(format!(
                "{id}: installs {bsa}, which no active plugin loads (Oblivion loads a BSA only \
                 when an active plugin's name begins its own), so its contents never reach \
                 the game; add a plugin of that name (`create_dummy_plugin`), or rename it"
            ));
        }
    }
}

/// The same plugin from two mods: allowed (the later mod's wins), but worth
/// knowing about.
fn duplicate_plugins(folders: &BTreeMap<String, ModFolder>, warnings: &mut Vec<String>) {
    let mut by_plugin: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, folder) in folders {
        for key in folder.plugins.keys() {
            by_plugin.entry(key.as_str()).or_default().push(id.as_str());
        }
    }
    for (key, mods) in by_plugin.into_iter().filter(|(_, mods)| mods.len() > 1) {
        let name = &folders[mods[0]].plugins[key].0;
        warnings.push(format!(
            "{name} is installed by {} mods ({}); MO2 uses the copy from the one lowest in its \
             list",
            mods.len(),
            mods.join(", ")
        ));
    }
}

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
