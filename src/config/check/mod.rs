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
//!
//! Asked for, it also reads each plugin whole, for the files it names, and
//! each BSA a mod ships, for the files in it: what a mod needs of another for
//! its meshes and sounds (see `resource_needs`). Both are kept, as masters
//! are, so only a changed archive is read again.

mod masters_cache;
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
    /// Only read each plugin's masters and check them: no MD5s, no filters,
    /// no actions rehearsed, nothing about the load order or BSAs. What the
    /// author's load order wants, quickly.
    pub masters_only: bool,
    /// Also find the mods each mod's plugins need for the files they name,
    /// with this game folder's own files as what no mod has to provide.
    /// Reading every plugin whole costs more than its header, so it is
    /// asked for.
    pub resources: Option<PathBuf>,
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
    /// What each mod needs of another for the files its plugins name, when
    /// asked for (see [`CheckSettings::resources`]); else `None`.
    pub resources: Option<Vec<FoundNeed>>,
    /// The mods whose archives were all read: what was found for them is
    /// all they need, so a found dependency they no longer have can go.
    pub read: Vec<String>,
    /// What each mod installs, as a list view shows it: in plan order.
    pub facts: Vec<ModFacts>,
}

/// What a mod installs, for showing and filtering a list by it: its INIs
/// and BSAs, and the files it shares with other mods.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ModFacts {
    pub mod_id: String,
    /// INIs and BSAs in its folder, lowercased.
    pub inis: Vec<String>,
    pub bsas: Vec<String>,
    /// The mods earlier in the plan whose files its own replace, with how
    /// many; and those later, whose replace its own.
    pub wins: Vec<(String, usize)>,
    pub losses: Vec<(String, usize)>,
    /// Its files are as its actions leave them; else as its archives lay
    /// them out, the actions not rehearsed.
    pub finished: bool,
    /// Every archive was read: else what it installs is partly unknown.
    pub read: bool,
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
    OptionalMasters,
    LoadOrder,
    Bsa,
    Overwritten,
    Unordered,
    Undeclared,
    OptionalResources,
    UndeclaredResources,
    Stale,
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
            Kind::OptionalMasters => {
                "Plugins whose masters come from a mod the player may untick, in a mod that \
                 stays"
            }
            Kind::LoadOrder => "Load order and installed plugins disagree",
            Kind::Bsa => "BSAs the game will not load",
            Kind::Overwritten => "Plugins installed by more than one mod (winner > overwritten)",
            Kind::Unordered => {
                "Installed but not in the load order (MO2 shows them unticked: exclude them, or \
                 add them to `plugins`)"
            }
            Kind::Undeclared => {
                "Mods whose plugins need another mod, which their `dependencies` do not list"
            }
            Kind::OptionalResources => {
                "Plugins that use files only a mod the player may untick provides, in a mod \
                 that stays"
            }
            Kind::UndeclaredResources => {
                "Mods whose plugins use files only another mod provides, which their \
                 `dependencies` do not list"
            }
            Kind::Stale => {
                "Found dependencies that no longer hold (adding found dependencies again \
                 takes them out)"
            }
            Kind::Filters => "Filters that do nothing",
            Kind::NotChecked => "Not checked",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            Kind::Overwritten
            | Kind::Unordered
            | Kind::Undeclared
            | Kind::OptionalResources
            | Kind::UndeclaredResources
            | Kind::Filters => Tone::Warning,
            Kind::NotChecked | Kind::Stale => Tone::Note,
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
    /// Every archive's contents were read: what was found of it is all
    /// there is, even where its actions leave the folder unknown.
    read: bool,
    /// When resources are asked for: the files each plugin at the top of the
    /// folder names, by lowercased name.
    references: BTreeMap<String, Vec<String>>,
    /// When resources are asked for: the files it provides, loose or in its
    /// BSAs. Kept apart from `files`, as it is filled in even when the
    /// folder is not complete -- what is known is still worth knowing.
    assets: BTreeSet<String>,
    /// Every file the archives lay out, lowercased, before any action: what
    /// is known of the folder when it is not complete.
    laid_out: BTreeSet<String>,
}

pub fn check_all(plan: &PersonalizedPlan, settings: &CheckSettings) -> anyhow::Result<CheckReport> {
    // Its own: two checks at once in one process -- a front end reading
    // masters while a check runs -- would otherwise clear each other's.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let scratch = crate::util::fs::cache_scratch_dir(&format!(
        "check-{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
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
                    if crate::util::cancel::requested() {
                        break;
                    }
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
    // Some mods unchecked: a report on the rest would say the list is fine.
    crate::util::cancel::check()?;
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
    dependency_problems(plan, &folders, &mut findings);
    let whole_plan = settings.filter.is_empty();
    // Which mod alone provides a file is only known from every mod's files.
    let resources = match &settings.resources {
        Some(game_dir) if whole_plan => {
            let game = crate::config::assets::game_files(game_dir)?;
            let needs = resource_needs(plan, &folders, &game);
            resource_problems(&needs, &mut findings);
            Some(needs)
        }
        _ => None,
    };
    stale_dependencies(plan, &folders, resources.as_deref(), &mut findings);
    if !settings.masters_only {
        load_order_problems(plan, &folders, whole_plan, &mut findings);
        bsa_problems(plan, &folders, &mut findings);
        duplicate_plugins(plan, &folders, &mut findings);
    }

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
        plugins: plugin_masters(&folders),
        resources,
        read: folders
            .iter()
            .filter(|(_, folder)| folder.read)
            .map(|(id, _)| id.clone())
            .collect(),
        facts: mod_facts(plan, &folders),
    })
}

/// A folder's files as finished, where its actions were rehearsed; else as
/// its archives lay them out.
fn facts_files(folder: &ModFolder) -> &BTreeSet<String> {
    if folder.complete && !folder.files.is_empty() {
        &folder.files
    } else {
        &folder.laid_out
    }
}

/// Each checked mod's INIs, BSAs and shared files, in plan order: a later
/// mod's file replaces an earlier one's, as MO2 lays them over each other.
/// Files at the top of a folder other than plugins, BSAs and INIs (readmes)
/// are left out of what is shared: every mod replacing them is noise.
fn mod_facts(plan: &PersonalizedPlan, folders: &BTreeMap<String, ModFolder>) -> Vec<ModFacts> {
    let checked: Vec<(&str, &ModFolder)> = plan
        .mods
        .iter()
        .filter_map(|m| folders.get(&m.id).map(|f| (m.id.as_str(), f)))
        .collect();
    let ends = |path: &str, ext: &str| path.ends_with(ext);
    let counts = |path: &str| {
        path.contains('/')
            || layout::plan::is_plugin_name(path)
            || ends(path, ".bsa")
            || ends(path, ".ini")
    };
    let mut owners: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, (_, folder)) in checked.iter().enumerate() {
        for path in facts_files(folder).iter().filter(|p| counts(p)) {
            owners.entry(path.as_str()).or_default().push(index);
        }
    }
    let mut wins: Vec<BTreeMap<usize, usize>> = vec![BTreeMap::new(); checked.len()];
    let mut losses: Vec<BTreeMap<usize, usize>> = vec![BTreeMap::new(); checked.len()];
    for indices in owners.values().filter(|o| o.len() > 1) {
        for (n, &earlier) in indices.iter().enumerate() {
            for &later in &indices[n + 1..] {
                *wins[later].entry(earlier).or_default() += 1;
                *losses[earlier].entry(later).or_default() += 1;
            }
        }
    }
    let named = |counts: &BTreeMap<usize, usize>| {
        counts
            .iter()
            .map(|(&other, &n)| (checked[other].0.to_string(), n))
            .collect()
    };
    checked
        .iter()
        .enumerate()
        .map(|(index, (id, folder))| {
            let all = facts_files(folder);
            ModFacts {
                mod_id: id.to_string(),
                inis: all.iter().filter(|p| ends(p, ".ini")).cloned().collect(),
                bsas: all.iter().filter(|p| ends(p, ".bsa")).cloned().collect(),
                wins: named(&wins[index]),
                losses: named(&losses[index]),
                finished: folder.complete,
                read: folder.read,
            }
        })
        .collect()
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
        read: true,
        ..ModFolder::default()
    };
    // Destination -> where its real bytes were put, when they matter.
    let mut planned: Vec<(String, Option<PathBuf>)> = Vec::new();
    // Masters of every plugin the archives hold, at any depth, by destination.
    let mut masters_at: HashMap<String, Vec<String>> = HashMap::new();
    // When resources are asked for, likewise: the files each plugin names,
    // and the files in each BSA.
    let mut references_at: HashMap<String, Vec<String>> = HashMap::new();
    let mut bsa_files_at: HashMap<String, Vec<String>> = HashMap::new();
    // BSAs are unpacked only for a rehearsal.
    let extract_bsas = if settings.masters_only {
        BTreeSet::new()
    } else {
        rehearse::bsas_to_extract(mod_entry)
    };
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
                references_at.extend(contents.references);
                bsa_files_at.extend(contents.bsa_files);
            }
            Ok(None) => {
                folder.complete = false;
                folder.read = false;
            }
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
                folder.read = false;
            }
        }
    }
    folder.laid_out = planned
        .iter()
        .map(|(destination, _)| destination.replace('\\', "/").to_lowercase())
        .collect();
    // Rehearsing the actions is the slow part; without it, the plugins are
    // those the archives lay out (an action's renames and dummies unknown).
    if settings.masters_only
        && mod_entry
            .actions
            .iter()
            .any(crate::config::actions::changes_files)
    {
        folder.complete = false;
    }
    if !folder.complete {
        // Still worth reading what is known, for the masters check.
        for (destination, _) in &planned {
            add_plugin(&mut folder, destination, masters_at.get(destination));
            add_resources(
                &mut folder,
                destination,
                destination,
                &references_at,
                &bsa_files_at,
            );
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
        if let Some(from) = &file.origin {
            add_resources(&mut folder, &file.path, from, &references_at, &bsa_files_at);
        }
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

/// What a file at `path` in the folder, planned at `from`, gives the resource
/// check: itself, the files a plugin names, the files in a BSA.
fn add_resources(
    folder: &mut ModFolder,
    path: &str,
    from: &str,
    references_at: &HashMap<String, Vec<String>>,
    bsa_files_at: &HashMap<String, Vec<String>>,
) {
    let path = path.to_lowercase();
    if let Some(named) = references_at.get(from) {
        folder.references.insert(path.clone(), named.clone());
    }
    if let Some(files) = bsa_files_at.get(from) {
        folder.assets.extend(files.iter().cloned());
    }
    folder.assets.insert(path);
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
        masters_only: false,
        cache_dir: cache_dir.to_path_buf(),
        filter: ModFilter::default(),
        resources: None,
    };
    let source = present(archive, &ArchiveRef::for_archive(archive), &settings)?;
    if matches!(
        mod_entry.mod_type,
        Some(ModType::Mo2Plugin | ModType::BuildFromFiles)
    ) || archive.inner_archive.is_some()
    {
        return Ok(None);
    }
    let paths = masters_cache::listing(&source)
        .map_err(|err| anyhow::anyhow!("archive is not readable ({}): {err}", source.display()))?;
    // Its own: a front end reads a mod's archives side by side.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let scratch = crate::util::fs::cache_scratch_dir(&format!(
        "files-{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let reader = EntryReader::kept(
        &source,
        scratch.join("entry"),
        masters_cache::entries_dir(&source),
    );
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
    /// When resources are asked for: the files each plugin names, and the
    /// files in each BSA at the top of the folder, by destination.
    references: HashMap<String, Vec<String>>,
    bsa_files: HashMap<String, Vec<String>>,
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
    let paths = masters_cache::listing(&source)
        .map_err(|err| anyhow::anyhow!("archive is not readable ({}): {err}", source.display()))?;
    if let Some(md5) = archive.md5.as_ref().filter(|_| !settings.masters_only) {
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

    let reader = EntryReader::kept(
        &source,
        scratch.join("entry"),
        masters_cache::entries_dir(&source),
    );
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
    let filters_checked: &[String] = if settings.masters_only {
        &[]
    } else {
        &archive.include
    };
    for pattern in filters_checked {
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
        && !settings.masters_only
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
        references: HashMap::new(),
        bsa_files: HashMap::new(),
    };

    // Plugins (at any depth: a `file_move` may bring one up) and BSAs an
    // action unpacks are the files whose bytes matter -- a plugin's only
    // until its masters are known from an earlier check.
    let mut known = masters_cache::ArchiveMasters::load(&source);
    // A plugin's named files, and a BSA's: kept as its masters are, under
    // `plugin:` and `bsa:` and the entry.
    let mut resources = settings
        .resources
        .as_ref()
        .map(|_| masters_cache::ArchiveMasters::load_kind(&source, "resources"));
    let is_bsa = |destination: &str| {
        !destination.contains('/') && destination.to_lowercase().ends_with(".bsa")
    };
    let unread = |kind: &str, source: &str| {
        resources
            .as_ref()
            .is_some_and(|known| known.get(&format!("{kind}:{source}")).is_none())
    };
    let wanted: BTreeMap<String, String> = planned
        .files
        .iter()
        .map(|file| (format!("{prefix}{}", file.destination), file.source.clone()))
        .filter(|(destination, source)| {
            (layout::plan::is_plugin_name(destination)
                && (known.get(source).is_none() || unread("plugin", source)))
                || (is_bsa(destination) && unread("bsa", source))
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
        if let Some(resources) = resources.as_mut() {
            let read = |kind: &str,
                        resources: &mut masters_cache::ArchiveMasters,
                        list: fn(&Path) -> anyhow::Result<Vec<String>>| {
                let key = format!("{kind}:{}", file.source);
                if let Some(found) = resources.get(&key) {
                    return Some(found.clone());
                }
                let path = bytes_at.as_ref()?;
                // One that cannot be read is said in the log and left out:
                // the resource check is a help, not a gate.
                match list(path) {
                    Ok(found) => {
                        resources.put(&key, &found);
                        Some(found)
                    }
                    Err(err) => {
                        tracing::warn!(entry = %file.source, "check: not read for resources: {err:#}");
                        None
                    }
                }
            };
            if layout::plan::is_plugin_name(&destination) {
                if let Some(named) = read("plugin", resources, named_files) {
                    contents.references.insert(destination.clone(), named);
                }
            } else if is_bsa(&destination)
                && let Some(files) = read("bsa", resources, bsa_files)
            {
                contents.bsa_files.insert(destination.clone(), files);
            }
        }
        if layout::plan::is_plugin_name(&destination) {
            if let Some(masters) = known.get(&file.source) {
                contents
                    .masters
                    .insert(destination.clone(), masters.clone());
            } else if let Some(path) = &bytes_at {
                let bytes = std::fs::read(path)
                    .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", file.source))?;
                let masters = crate::plugin::Plugin::masters_of(&bytes).map_err(|err| {
                    anyhow::anyhow!("{destination} is not a readable plugin: {err}")
                })?;
                known.put(&file.source, &masters);
                contents.masters.insert(destination.clone(), masters);
            }
        }
        let keep_bytes = extract_bsas.contains(&destination.to_lowercase());
        contents
            .files
            .push((destination, bytes_at.filter(|_| keep_bytes)));
    }
    known.save();
    if let Some(resources) = &resources {
        resources.save();
    }
    Ok(Some(contents))
}

/// The files a plugin on disk names.
fn named_files(path: &Path) -> anyhow::Result<Vec<String>> {
    let plugin = crate::plugin::Plugin::read(path)?;
    Ok(crate::config::assets::named_paths(&plugin))
}

/// The files in a BSA on disk, lowercased and `/`-separated.
fn bsa_files(path: &Path) -> anyhow::Result<Vec<String>> {
    Ok(crate::bsa::reader::list_paths(path)?
        .into_iter()
        .map(|p| p.replace('\\', "/").to_lowercase())
        .collect())
}

/// A mod one of a mod's plugins needs: for a master, or for a file only
/// that mod provides.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FoundNeed {
    pub mod_id: String,
    pub needs: String,
    /// The first of its plugins, and what it names that `needs` provides
    /// (a master, or a file), to say so.
    pub plugin: String,
    pub named: String,
    /// How many files of `needs` its plugins name; 1 for a master.
    pub count: usize,
    /// `dependencies` already lists it, in any group.
    pub listed: bool,
    /// `needs` is optional, while `mod_id` is not and does not follow it:
    /// listing it is refused, as the player may untick it.
    pub optional: bool,
}

impl FoundNeed {
    fn new(
        plan: &HashMap<&str, &PersonalizedMod>,
        needing: &PersonalizedMod,
        needs: &str,
        (plugin, named, count): (&str, &str, usize),
    ) -> Self {
        FoundNeed {
            mod_id: needing.id.clone(),
            needs: needs.to_string(),
            plugin: plugin.to_string(),
            named: named.to_string(),
            count,
            listed: needing.dependencies.iter().any(|dep| dep == needs),
            optional: plan.get(needs).is_some_and(|m| m.optional_in.is_some())
                && needing.optional_in.is_none()
                && !needing.follows.iter().any(|f| f == needs),
        }
    }
}

/// What each mod's plugins need of other mods, by their masters (as `check`
/// read them): in plan order, each mod's by the mod it needs.
pub fn master_needs(plan: &PersonalizedPlan, plugins: &[PluginMasters]) -> Vec<FoundNeed> {
    let by_id: HashMap<&str, &PersonalizedMod> =
        plan.mods.iter().map(|m| (m.id.as_str(), m)).collect();
    // The mod installing each plugin: as read, else as a mod declares it.
    let mut provider: HashMap<String, &str> = HashMap::new();
    for mod_entry in &plan.mods {
        for plugin in &mod_entry.plugins {
            provider
                .entry(plugin.to_ascii_lowercase())
                .or_insert(&mod_entry.id);
        }
    }
    for read in plugins {
        provider.insert(read.plugin.to_ascii_lowercase(), &read.mod_id);
    }
    let mut out = Vec::new();
    for needing in &plan.mods {
        let mut needs: BTreeMap<&str, (&str, &str, usize)> = BTreeMap::new();
        for read in plugins.iter().filter(|p| p.mod_id == needing.id) {
            for master in &read.masters {
                if let Some(&from) = provider.get(&master.to_ascii_lowercase())
                    && from != needing.id
                {
                    needs.entry(from).or_insert((&read.plugin, master, 1));
                }
            }
        }
        for (from, why) in needs {
            out.push(FoundNeed::new(&by_id, needing, from, why));
        }
    }
    out
}

/// What each mod's plugins need of other mods for the files they name: a
/// file the mod has itself, or the game has, needs nothing; one that exactly
/// one other mod provides needs that mod. One several mods provide needs
/// none of them in particular -- any would do, and which one wins is the
/// order's business, not a dependency -- and one nothing provides is
/// `assets`'s business. So the order never changes what is found here; only
/// which mods, and which files, there are.
fn resource_needs(
    plan: &PersonalizedPlan,
    folders: &BTreeMap<String, ModFolder>,
    game: &BTreeSet<String>,
) -> Vec<FoundNeed> {
    use crate::config::assets::provides;
    let by_id: HashMap<&str, &PersonalizedMod> =
        plan.mods.iter().map(|m| (m.id.as_str(), m)).collect();
    // Each file, with the mods providing it. A sound may name a folder, so
    // a folder under `sound/` counts as provided by each mod with a file in
    // it.
    let mut providers: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut sound_folders: HashMap<String, Vec<&str>> = HashMap::new();
    for (id, folder) in folders {
        for file in &folder.assets {
            providers.entry(file.as_str()).or_default().push(id);
            if file.starts_with("sound/") {
                let mut at = file.as_str();
                while let Some((parent, _)) = at.rsplit_once('/') {
                    let mods = sound_folders.entry(parent.to_string()).or_default();
                    if mods.last() != Some(&id.as_str()) {
                        mods.push(id);
                    }
                    at = parent;
                }
            }
        }
    }
    let mut out = Vec::new();
    for needing in &plan.mods {
        let Some(folder) = folders.get(&needing.id) else {
            continue;
        };
        let mut needs: BTreeMap<&str, (&str, &str, usize)> = BTreeMap::new();
        for (plugin, named) in &folder.references {
            let plugin = folder
                .plugins
                .get(plugin)
                .map(|(name, _)| name.as_str())
                .unwrap_or(plugin);
            for path in named {
                if provides(&folder.assets, path) || provides(game, path) {
                    continue;
                }
                let from = providers
                    .get(path.as_str())
                    .or_else(|| sound_folders.get(path.as_str()));
                if let Some([only]) = from.map(Vec::as_slice) {
                    needs
                        .entry(only)
                        .and_modify(|(_, _, count)| *count += 1)
                        .or_insert((plugin, path, 1));
                }
            }
        }
        for (from, why) in needs {
            out.push(FoundNeed::new(&by_id, needing, from, why));
        }
    }
    out
}

/// What each mod's plugins need of other mods, by their masters, against what
/// it says it needs. A master from a mod its `dependencies` do not list is a
/// dependency left unsaid: listed, `validate` and the GUI know of it too. One
/// from an optional mod, in a mod that is not optional and does not follow
/// it, goes missing whenever the player unticks that mod -- whatever this
/// plan's answers are.
fn dependency_problems(
    plan: &PersonalizedPlan,
    folders: &BTreeMap<String, ModFolder>,
    findings: &mut Findings,
) {
    for need in master_needs(plan, &plugin_masters(folders)) {
        if need.listed {
            continue;
        }
        let shown = Text::new()
            .plain("its ")
            .name(&need.plugin)
            .plain(" has ")
            .name(&need.named)
            .plain(format!(" as a master, from {}, ", need.needs));
        if need.optional {
            findings.about(
                Kind::OptionalMasters,
                &need.mod_id,
                shown.plain(format!(
                    "which is optional; make {} optional too, or give it `if = '{}'`",
                    need.mod_id,
                    crate::config::condition::mod_term(&need.needs)
                )),
            );
        } else {
            findings.about(
                Kind::Undeclared,
                &need.mod_id,
                shown.plain("which its `dependencies` do not list"),
            );
        }
    }
}

/// As [`dependency_problems`], for the files plugins name. Only warnings: a
/// mesh the game cannot find shows as a marker, where a missing master stops
/// the game.
fn resource_problems(needs: &[FoundNeed], findings: &mut Findings) {
    for need in needs.iter().filter(|need| !need.listed) {
        let more = match need.count {
            1 => String::new(),
            n => format!(" (and {} more of its files)", n - 1),
        };
        let shown = Text::new()
            .plain("its ")
            .name(&need.plugin)
            .plain(" uses ")
            .name(&need.named)
            .plain(format!(", which only {} provides{more}, ", need.needs));
        if need.optional {
            findings.about(
                Kind::OptionalResources,
                &need.mod_id,
                shown.plain(format!(
                    "and is optional; make {} optional too, or give it `if = '{}'`",
                    need.mod_id,
                    crate::config::condition::mod_term(&need.needs)
                )),
            );
        } else {
            findings.about(
                Kind::UndeclaredResources,
                &need.mod_id,
                shown.plain("which its `dependencies` do not list"),
            );
        }
    }
}

/// Found dependencies (`masters`, and `resources` when they were looked
/// for) that were not found this time, for the mods whose plugins were all
/// read. Not wrong to keep -- only no longer the reason given.
fn stale_dependencies(
    plan: &PersonalizedPlan,
    folders: &BTreeMap<String, ModFolder>,
    resources: Option<&[FoundNeed]>,
    findings: &mut Findings,
) {
    let masters = master_needs(plan, &plugin_masters(folders));
    for mod_entry in &plan.mods {
        if !folders.get(&mod_entry.id).is_some_and(|f| f.read) {
            continue;
        }
        let mut groups = vec![("masters", &mod_entry.found.masters, masters.as_slice())];
        if let Some(resources) = resources {
            groups.push(("resources", &mod_entry.found.resources, resources));
        }
        for (group, listed, needs) in groups {
            for dep in listed {
                if !needs
                    .iter()
                    .any(|n| n.mod_id == mod_entry.id && &n.needs == dep)
                {
                    findings.about(
                        Kind::Stale,
                        &mod_entry.id,
                        format!("`dependencies.{group}` lists {dep}, which it no longer needs"),
                    );
                }
            }
        }
    }
}

/// Each plugin read, with its mod and masters.
fn plugin_masters(folders: &BTreeMap<String, ModFolder>) -> Vec<PluginMasters> {
    folders
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
        .collect()
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
    let (merge_errors, merge_checked) = merge_source_problems(plan, shipped, &position);
    errors.extend(merge_errors);
    (errors, checked + merge_checked)
}

/// A merge's sources are out of the load order (the merge hides them), so
/// their masters are not checked above, and a missing one used to surface
/// only when `install` built the merge. The merge needs each source's masters
/// to be its other sources, or in the load order ahead of its output -- as
/// `merge::masters::build` does.
fn merge_source_problems(
    plan: &PersonalizedPlan,
    shipped: &HashMap<String, Shipped>,
    position: &HashMap<String, usize>,
) -> (Vec<(String, String)>, usize) {
    let mut errors = Vec::new();
    let mut checked = 0usize;
    for (merge_mod, spec) in plan.merges() {
        let sources: HashSet<String> = spec
            .sources
            .iter()
            .map(|source| source.plugin.to_ascii_lowercase())
            .collect();
        let output_at = position.get(&spec.output.to_ascii_lowercase()).copied();
        for source in &spec.sources {
            let Some(found) = shipped.get(&source.plugin.to_ascii_lowercase()) else {
                continue;
            };
            checked += 1;
            for master in &found.masters {
                let wanted = master.to_ascii_lowercase();
                if sources.contains(&wanted) {
                    continue;
                }
                let problem = match (position.get(&wanted), output_at) {
                    (None, _) => format!(
                        "needs {master}, which is not in the list: add the mod that has it, and \
                         {master} to `plugins`, or leave {} out of the merge",
                        source.plugin
                    ),
                    (Some(&at), Some(out)) if at == out => format!(
                        "needs {master}, the merge's own output: a plugin cannot be merged into \
                         one it needs"
                    ),
                    (Some(&at), Some(out)) if at > out => format!(
                        "needs {master}, which loads after the merge's output {} (move {master} \
                         above it in `plugins`)",
                        spec.output
                    ),
                    _ => continue,
                };
                errors.push((
                    merge_mod.id.clone(),
                    format!(
                        "{} (merge source {} of {}): {problem}",
                        source.mod_id, source.plugin, merge_mod.id
                    ),
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
        let layer_paths = masters_cache::listing(&source).map_err(|err| {
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
    fn a_merge_sources_masters_are_checked_though_it_is_out_of_the_load_order() {
        let merge = serde_json::json!([{
            "id": "Forts Merged",
            "mod_type": "merge",
            "dependencies": [],
            "archives": [],
            "merge": {
                "output": "Forts Merged.esp",
                "sources": [
                    { "mod": "Fort A", "plugin": "Fort A.esp" },
                    { "mod": "Patch", "plugin": "Patch.esp" },
                ],
            },
        }]);
        let plan = plan(
            &["Oblivion.esm", "Forts Merged.esp", "Late.esp"],
            &[],
            merge,
        );
        let shipped = ships(&[
            ("Fort A.esp", &["Oblivion.esm"]),
            // Another source of the same merge is fine; a master out of the
            // list, or loading after the output, is not.
            // Nor is the merge's own output.
            (
                "Patch.esp",
                &["Fort A.esp", "Cloudtop.esp", "Late.esp", "Forts Merged.esp"],
            ),
        ]);
        let (errors, checked) = master_problems(&plan, &shipped);
        assert_eq!(checked, 2, "{errors:#?}");
        assert_eq!(errors.len(), 3, "{errors:#?}");
        assert!(
            errors[2].1.contains("the merge's own output"),
            "{errors:#?}"
        );
        assert!(
            errors[0]
                .1
                .contains("merge source Patch.esp of Forts Merged")
                && errors[0]
                    .1
                    .contains("Cloudtop.esp, which is not in the list"),
            "{errors:#?}"
        );
        assert!(
            errors[1].1.contains("Late.esp, which loads after"),
            "{errors:#?}"
        );
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
