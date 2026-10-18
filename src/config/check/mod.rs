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
use crate::config::schema::{
    CompiledArchive, Mo2ModlistEntry, ModType, PersonalizedMod, PersonalizedPlan,
};
use crate::events::{Item, Text, Tone};
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
    /// Findings that would make the install fail or go wrong.
    pub problems: usize,
    pub warnings: usize,
    /// Everything found, in report order.
    pub findings: Vec<Finding>,
    /// Each plugin read from an archive: the mod installing it and its
    /// masters, for showing the load order with them.
    pub plugins: Vec<PluginMasters>,
}

/// A plugin a mod installs, and the masters its header names.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PluginMasters {
    pub plugin: String,
    pub mod_id: String,
    pub masters: Vec<String>,
}

/// The plugins shipped by the game itself. A master among these is present
/// unless the list's load order says otherwise.
pub const GAME_PLUGINS: &[&str] = &[
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

/// What a finding is about, which decides its heading and whether it fails
/// the check. Reported grouped, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    MissingArchive,
    DamagedArchive,
    WouldFail,
    Masters,
    LoadOrder,
    Bsa,
    Overwritten,
    Unordered,
    Filters,
    NotChecked,
}

impl Kind {
    pub fn heading(self) -> &'static str {
        match self {
            Kind::MissingArchive => "Not in the downloads folder",
            Kind::DamagedArchive => "Archives that are not what the list expects",
            Kind::WouldFail => "Would fail at install",
            Kind::Masters => "Plugins whose masters will not be loaded before them",
            Kind::LoadOrder => "Load order and installed plugins disagree",
            Kind::Bsa => "BSAs the game will not load",
            Kind::Overwritten => "Plugins installed by more than one mod (winner > overwritten)",
            Kind::Unordered => {
                "Installed but not in the load order (MO2 shows them unticked: exclude them, or \
                 add them to `plugins`)"
            }
            Kind::Filters => "Filters that do nothing",
            Kind::NotChecked => "Not checked",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            Kind::Overwritten | Kind::Unordered | Kind::Filters => Tone::Warning,
            Kind::NotChecked => Tone::Note,
            _ => Tone::Problem,
        }
    }
}

/// One thing `check` found: its kind, what it says, and the mod it is about.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Finding {
    pub kind: Kind,
    #[serde(flatten)]
    pub item: Item,
}

/// Everything `check` found, to be reported grouped by kind.
#[derive(Default)]
struct Findings(Vec<Finding>);

impl Findings {
    /// A finding about the plan as a whole, or several mods.
    fn push(&mut self, kind: Kind, message: impl Into<Text>) {
        self.0.push(Finding {
            kind,
            item: Item::new(message),
        });
    }

    /// A finding about one mod; the message starts with its id.
    fn about(&mut self, kind: Kind, mod_id: &str, message: impl Into<Text>) {
        self.0.push(Finding {
            kind,
            item: Item::new(
                Text::new()
                    .plain(format!("{mod_id}: "))
                    .append(message.into()),
            )
            .about(mod_id),
        });
    }

    fn count(&self, tone: Tone) -> usize {
        self.0
            .iter()
            .filter(|finding| finding.kind.tone() == tone)
            .count()
    }

    /// Each kind under its heading, in order; the log gets every line too.
    fn report(&self) {
        let mut by_kind: BTreeMap<Kind, Vec<Item>> = BTreeMap::new();
        for finding in &self.0 {
            let message = finding.item.text.to_plain();
            tracing::info!(kind = ?finding.kind, message, "check finding");
            by_kind
                .entry(finding.kind)
                .or_default()
                .push(finding.item.clone());
        }
        for (kind, items) in by_kind {
            crate::events::section(kind.tone(), kind.heading(), items);
        }
    }
}

/// A plugin Wrye Bash writes after the install: `Bashed Patch, 0.esp`. A list
/// declares and orders it ahead of time, so it lands in place when it appears;
/// no archive ships it.
pub(crate) fn made_by_wrye_bash(plugin: &str) -> bool {
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
    let mut findings = Findings::default();
    let mut folders: BTreeMap<String, ModFolder> = BTreeMap::new();
    let partners = PartnerCache::default();
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
        redo_actions: Vec::new(),
    };

    let in_scope: Vec<&PersonalizedMod> = plan
        .mods
        .iter()
        .filter(|m| settings.filter.matches(&m.section, &m.id))
        .collect();
    let mods_checked = in_scope.len();
    let archives_checked: usize = in_scope.iter().map(|m| m.archives.len()).sum();
    let bar = crate::events::progress(mods_checked as u64, "checking");

    // Mods are checked side by side, a thread per CPU: most of the time goes
    // on `7z` and `bsdtar` listing and unpacking, and on hashing, one archive
    // at a time. Each mod has its own scratch folder and its own findings,
    // gathered back in list order so the report reads the same every run.
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(in_scope.len().max(1));
    let mut results: Vec<Option<(ModFolder, Findings)>> = Vec::new();
    results.resize_with(in_scope.len(), || None);
    let results = std::sync::Mutex::new(results);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(mod_entry) = in_scope.get(index) else {
                        break;
                    };
                    bar.set_message(&mod_entry.id);
                    let own = scratch.join(format!("mod-{index}"));
                    let mut found = Findings::default();
                    let folder = check_mod(
                        mod_entry,
                        plan,
                        settings,
                        &install_settings,
                        &active_plugins,
                        &partners,
                        &own,
                        &mut found,
                    );
                    let _ = std::fs::remove_dir_all(&own);
                    if let Ok(mut results) = results.lock() {
                        results[index] = Some((folder, found));
                    }
                    bar.inc(1);
                }
            });
        }
    });
    bar.finish();
    let results = results
        .into_inner()
        .map_err(|_| anyhow::anyhow!("a check thread panicked"))?;
    for (mod_entry, result) in in_scope.iter().zip(results) {
        let (folder, found) =
            result.ok_or_else(|| anyhow::anyhow!("{} was not checked", mod_entry.id))?;
        findings.0.extend(found.0);
        folders.insert(mod_entry.id.clone(), folder);
    }

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
    for (mod_id, error) in master_errors {
        findings.0.push(Finding {
            kind: Kind::Masters,
            item: Item::new(error).about(mod_id),
        });
    }
    let whole_plan = settings.filter.is_empty();
    load_order_problems(plan, &folders, whole_plan, &mut findings);
    bsa_problems(plan, &folders, &mut findings);
    duplicate_plugins(plan, &folders, &mut findings);

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
    if unread > 0 && whole_plan {
        findings.push(
            Kind::NotChecked,
            format!(
                "the masters of {unread} plugin(s) made at install (merges) or inside an \
                 archive in an archive"
            ),
        );
    }

    // Reported before the bail: the run that fails is exactly the run whose
    // findings the author needs.
    findings.report();
    let problems = findings.count(Tone::Problem);
    let warnings = findings.count(Tone::Warning);
    crate::events::status(
        if problems > 0 {
            Tone::Problem
        } else if warnings > 0 {
            Tone::Warning
        } else {
            Tone::Good
        },
        format!(
            "Checked {archives_checked} archive(s) in {mods_checked} mod(s), and the masters of \
             {plugins_checked} plugin(s): {problems} problem(s), {warnings} warning(s)"
        ),
    );
    tracing::info!(
        mods_checked,
        archives_checked,
        plugins_checked,
        problems,
        warnings,
        "check summary"
    );

    Ok(CheckReport {
        mods_checked,
        archives_checked,
        plugins_checked,
        problems,
        warnings,
        findings: findings.0,
        plugins: folders
            .iter()
            .flat_map(|(id, folder)| {
                folder
                    .plugins
                    .values()
                    .map(move |(name, masters)| PluginMasters {
                        plugin: name.clone(),
                        mod_id: id.clone(),
                        masters: masters.clone(),
                    })
            })
            .collect(),
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
    partners: &PartnerCache,
    scratch: &Path,
    findings: &mut Findings,
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
                findings,
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
                let message = format!("{err:#}");
                let (kind, message) = if message.contains("is not in the downloads folder") {
                    (Kind::MissingArchive, message)
                } else if let Some(rest) = message.strip_prefix("install would fail: ") {
                    (Kind::WouldFail, rest.to_string())
                } else {
                    (Kind::DamagedArchive, message)
                };
                findings.0.push(Finding {
                    kind,
                    item: Item::new(format!("{}{which}: {message}", mod_entry.id))
                        .about(&mod_entry.id),
                });
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
                findings.about(Kind::WouldFail, &mod_entry.id, format!("{err:#}"));
                folder.complete = false;
                return folder;
            }
            Ok(Rehearsal::Unknown(why)) => {
                tracing::info!(mod_id = %mod_entry.id, why, "check: actions not rehearsed");
                findings.about(Kind::NotChecked, &mod_entry.id, why);
                folder.complete = false;
                return folder;
            }
            Err(err) => {
                findings.about(Kind::WouldFail, &mod_entry.id, format!("{err:#}"));
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

/// An archive as `install` sees it: every entry, and the entries it takes.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ArchiveContents {
    /// Every file in the archive, as the archive names it.
    pub entries: Vec<String>,
    /// The entries `install` takes, by destination.
    pub taken: Vec<Taken>,
    /// What the layout would take with no `include` or `exclude`: `taken`,
    /// plus what the archive's filters leave out. An entry in neither is one
    /// the layout does not choose (a BAIN package or FOMOD option not picked).
    pub available: Vec<Taken>,
}

/// An archive entry `install` takes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Taken {
    /// As the archive names it.
    pub source: String,
    /// Its path in the mod's folder, before the mod's actions run.
    pub destination: String,
    /// The path the archive's `include` and `exclude` match it as: inside the
    /// data folder, BAIN subpackage or FOMOD folder it came from.
    pub filtered_as: String,
}

/// What `install` would do with `archive` for `mod_entry`. `None` when that
/// is only known at install (an archive inside an archive, an MO2 plugin, a
/// build from files).
pub fn archive_contents(
    mod_entry: &PersonalizedMod,
    archive: &CompiledArchive,
    cache_dir: &Path,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<Option<ArchiveContents>> {
    let settings = CheckSettings {
        cache_dir: cache_dir.to_path_buf(),
        filter: ModFilter::default(),
    };
    let source = present(archive, &ArchiveRef::for_archive(archive), &settings)?;
    if matches!(
        mod_entry.mod_type,
        Some(ModType::Mo2Plugin | ModType::BuildFromFiles)
    ) || archive.inner_archive.is_some()
    {
        return Ok(None);
    }
    let paths = archive::list_archive_paths(&source)
        .map_err(|err| anyhow::anyhow!("archive is not readable ({}): {err}", source.display()))?;
    let scratch = crate::util::fs::cache_scratch_dir("files");
    let reader = EntryReader::new(&source, scratch.join("entry"));
    let exclude: Vec<String> = archive
        .exclude
        .iter()
        .chain(archive.game_root_files.iter())
        .cloned()
        .collect();
    let plan = |include: &[String], exclude: &[String]| {
        ArchiveFilters::new(include, exclude).and_then(|filters| {
            layout::plan_archive(
                &source.display().to_string(),
                &paths,
                &reader,
                &mod_entry.id,
                archive,
                &filters,
                active_plugins,
            )
        })
    };
    let planned = plan(&archive.include, &exclude);
    let unfiltered = plan(&[], &archive.game_root_files);
    let _ = std::fs::remove_dir_all(&scratch);
    let prefix = archive
        .target_subdir
        .as_deref()
        .map(|value| value.replace('\\', "/").trim_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .map(|value| format!("{value}/"))
        .unwrap_or_default();
    let taken = |plan: layout::plan::LayoutPlan| -> Vec<Taken> {
        plan.files
            .into_iter()
            .map(|file| Taken {
                source: file.source,
                destination: format!("{prefix}{}", file.destination),
                filtered_as: file.filtered_as,
            })
            .collect()
    };
    Ok(Some(ArchiveContents {
        entries: paths,
        taken: taken(planned?),
        available: taken(unfiltered?),
    }))
}

/// The files `install` would take from `archive`: each one's path in the
/// mod's folder, sorted. `None` as for [`archive_contents`].
pub fn planned_files(
    mod_entry: &PersonalizedMod,
    archive: &CompiledArchive,
    cache_dir: &Path,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<Option<Vec<String>>> {
    Ok(
        archive_contents(mod_entry, archive, cache_dir, active_plugins)?.map(|contents| {
            let mut files: Vec<String> = contents
                .taken
                .into_iter()
                .map(|taken| taken.destination)
                .collect();
            files.sort();
            files
        }),
    )
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
    findings: &mut Findings,
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
            findings.about(
                Kind::Filters,
                &mod_entry.id,
                format!("include '{pattern}' matches nothing in {file_name}"),
            );
        }
    }
    if !archive.exclude.is_empty()
        && let Ok(unfiltered) = plan_with(&archive.include, &[])
    {
        for pattern in &archive.exclude {
            if plan_with(&archive.include, std::slice::from_ref(pattern))
                .is_ok_and(|plan| plan.len() == unfiltered.len())
            {
                findings.about(
                    Kind::Filters,
                    &mod_entry.id,
                    format!("exclude '{pattern}' leaves nothing out of {file_name}"),
                );
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
    findings: &mut Findings,
) {
    // A plan made before mods' declared plugins were carried into it says
    // nothing about who declares what; comparing against that would report
    // every plugin of a mod not fully known here.
    let declarations_known =
        plan.plugins.is_empty() || plan.mods.iter().any(|m| !m.plugins.is_empty());
    if !declarations_known {
        findings.push(
            Kind::NotChecked,
            "the load order against what each mod declares: this plan was made by an older \
             mudcrab; run `compile` and `query` again"
                .to_string(),
        );
    }
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
                findings.about(
                    Kind::LoadOrder,
                    &mod_entry.id,
                    Text::new().plain("declares ").name(declared).plain(
                        ", which it does not install (renamed in a mod update, or left out by \
                         its layout)",
                    ),
                );
            }
        }
        for (key, (name, _)) in &folder.plugins {
            // An unticked mod's plugins are unordered on purpose: MO2 cannot
            // see them until the player switches the mod on.
            if mod_entry.enabled && !ordered.contains(key) && !merged_away.contains(key) {
                findings.0.push(Finding {
                    kind: Kind::Unordered,
                    item: Item::new(Text::new().name(name).plain(format!(" ({})", mod_entry.id)))
                        .about(&mod_entry.id),
                });
            }
        }
    }

    if !whole_plan || !declarations_known {
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
            findings.push(
                Kind::LoadOrder,
                Text::new()
                    .name(plugin)
                    .plain(" is in the load order, but no mod installs it"),
            );
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
    findings: &mut Findings,
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
            findings.about(
                Kind::Bsa,
                id,
                Text::new().name(bsa).plain(
                    " -- no active plugin's name begins it, so its contents never reach the \
                     game; add a plugin of that name (`create_dummy_plugin`), or rename it",
                ),
            );
        }
    }
}

/// The same plugin from two mods: allowed -- the copy from the mod highest in
/// MO2's list wins -- but worth knowing about. Listed winner first.
fn duplicate_plugins(
    plan: &PersonalizedPlan,
    folders: &BTreeMap<String, ModFolder>,
    findings: &mut Findings,
) {
    // Later in the plan is higher in MO2's list.
    // MO2's own list where the plan has one, else the plan's mods.
    let mut priority: HashMap<&str, usize> = HashMap::new();
    let ids = plan
        .mo2_modlist_entries
        .iter()
        .filter_map(|entry| match entry {
            Mo2ModlistEntry::Mod { id } => Some(id.as_str()),
            _ => None,
        })
        .chain(plan.mods.iter().map(|m| m.id.as_str()));
    for (index, id) in ids.enumerate() {
        priority.entry(id).or_insert(index);
    }
    let mut by_plugin: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, folder) in folders {
        for key in folder.plugins.keys() {
            by_plugin.entry(key.as_str()).or_default().push(id.as_str());
        }
    }
    for (key, mut mods) in by_plugin.into_iter().filter(|(_, mods)| mods.len() > 1) {
        mods.sort_by_key(|id| std::cmp::Reverse(priority.get(id).copied().unwrap_or(0)));
        let name = &folders[mods[0]].plugins[key].0;
        findings.push(
            Kind::Overwritten,
            Text::new()
                .name(name)
                .plain(format!(": {}", mods.join(" > "))),
        );
    }
}

fn master_problems(
    plan: &PersonalizedPlan,
    shipped: &HashMap<String, Shipped>,
) -> (Vec<(String, String)>, usize) {
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
                errors.push((
                    shipped.mod_id.clone(),
                    format!("{} ({plugin}): {problem}", shipped.mod_id),
                ));
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
        assert!(errors[0].1.contains("Off.esp") && errors[0].1.contains("inactive"));
        assert!(errors[1].1.contains("Missing.esp") && errors[1].1.contains("not in the list"));
    }

    #[test]
    fn a_master_must_load_first() {
        let plan = plan(&["C.esp", "Late.esp"], &[], serde_json::json!([]));
        let (errors, _) = master_problems(&plan, &ships(&[("C.esp", &["Late.esp"])]));
        assert!(errors[0].1.contains("loads after it"), "{errors:#?}");
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
        assert!(
            errors[0].1.contains("merged into Merged.esp"),
            "{errors:#?}"
        );
    }
}
