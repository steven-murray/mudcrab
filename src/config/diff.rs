//! Comparing an install we produced against a reference ("Oracle") MO2 instance.
//!
//! Reproducing a 700-mod list is only tractable if each section can be checked
//! the moment it is built, while the handful of decisions that produced it are
//! still fresh. `check` validates the archives we are about to install from;
//! this validates what actually landed on disk, against a known-good instance.
//!
//! Two properties shape the whole module:
//!
//! * The trees come from Windows-authored archives, so path comparison is
//!   case-insensitive and `\` is folded to `/`. Two files that Windows would
//!   consider the same file must compare as the same file, or every mod would
//!   look like it had renamed half its textures.
//! * The Oracle is 40GB. Content is therefore established lazily -- size first,
//!   bytes only when the sizes agree, and nothing at all is read for a mod that
//!   exists on only one side.

use crate::config::add::parse_meta_ini;
use crate::config::filter::ModFilter;
use crate::config::schema::PersonalizedPlan;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// MO2's own bookkeeping file. Ours will never have one, so comparing it would
/// report a difference on every single mod and drown out the real ones.
const MO2_META_FILE: &str = "meta.ini";

/// MO2 renames a file to `<name>.mohidden` to drop it out of the virtual file
/// system. A plugin hidden on behalf of a merge is the same file under a
/// different name, not a missing file and not an extra one.
const MO2_HIDDEN_SUFFIX: &str = ".mohidden";

/// Publication date of the MOFAM guide, 2025-03-01T00:00:00Z.
///
/// The guide frequently says only "use the top file on the page", so an Oracle
/// archive stamped after this date is a file the guide never actually named --
/// drift the author has to consciously accept rather than assume.
const GUIDE_CUTOFF: i64 = 1_740_787_200;

/// Plausible range for a Unix timestamp embedded in a Nexus filename: 2001-09
/// to 2100. Nexus also puts bare mod ids in those filenames (`...-19039.7z`),
/// and a five-digit mod id read as a timestamp would date the mod to 1970.
const TIMESTAMP_MIN: i64 = 1_000_000_000;
const TIMESTAMP_MAX: i64 = 4_102_444_800;

/// Read buffer for byte comparison and hashing.
const CHUNK: usize = 128 * 1024;

/// Guard against a symlink loop turning the walk into an infinite descent.
const MAX_DEPTH: usize = 64;

/// Leftovers from OBMM's installer that some archives carry and that neither
/// the game nor MO2 reads. Whether a hand-built instance kept them is a record
/// of how it was clicked through, not of what the mod is -- the Oracle keeps
/// them for one manual install and dropped them for another. Comparing them
/// reports a difference that means nothing.
const OMOD_CONVERSION_DIR: &str = "omod conversion data";

/// Whether a path is documentation rather than game content.
///
/// These are installed exactly as before -- this only decides whether a
/// difference in them is worth *reporting*. Whether a hand-built instance kept
/// a readme records which checkboxes were ticked during a manual install, not
/// what the mod is: the Oracle keeps them for some mods and drops them for
/// others, so no consistent rule can match it, and chasing that produced four
/// "differences" in Part 16 alone that nobody would act on.
///
/// Deliberately narrow. Matching all `.txt` would be wrong -- mods do ship
/// readable data files -- so this keys on names that only ever mean
/// documentation, plus two artefacts of the tools rather than the mod:
/// OBMM's settings screenshot and the `.url` shortcut Russian mirrors bundle.
fn is_documentation(relative_path: &str) -> bool {
    let name = relative_path
        .rsplit('/')
        .next()
        .unwrap_or(relative_path)
        .to_lowercase();

    if name == "obmm_bsa_settings.jpg" || name.ends_with(".url") {
        return true;
    }

    let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(&name);
    ["readme", "readme_", "read me", "credits", "licence", "license", "changelog"]
        .iter()
        .any(|marker| stem.contains(marker))
}

/// MO2 keeps its list separators as empty mod folders. They carry no files, so
/// diffing them says nothing about whether a section was built correctly -- and
/// an install that has not exported to MO2 yet has none of them at all.
const SEPARATOR_SUFFIX: &str = "_separator";

pub struct DiffSettings {
    /// The install we produced.
    pub mods_dir: PathBuf,
    /// The reference MO2 instance's mods directory. Only ever read from.
    pub oracle_dir: PathBuf,
    /// Which mods to compare. Empty means every directory in either tree.
    pub filter: ModFilter,
    /// Section paths and declared archive names, when a plan was supplied.
    pub plan: Option<PlanIndex>,
}

/// What a plan tells us about a mod that the directories themselves cannot.
///
/// A mod folder on disk knows its name and its files. It does not know which
/// section of the list it belongs to, which archive we intended to install it
/// from, nor what the Oracle calls it when we deliberately named it something
/// else -- so `--section` filtering, archive comparison and `oracle_name`
/// matching all need the plan.
#[derive(Debug, Default)]
pub struct PlanIndex {
    /// Keyed by lowercased id, because the directory on disk may not match the
    /// plan's spelling even though MO2 and Windows consider them the same.
    entries: HashMap<String, PlanEntry>,
    /// Distinct section paths in plan order, so the report is grouped the way
    /// the modlist is written rather than alphabetically.
    order: Vec<Vec<String>>,
    /// Lowercased `oracle_name` to lowercased id, for the mods whose Oracle
    /// folder is deliberately named differently from ours. Without this, the
    /// Oracle's folder would be an unclaimed key and report as its own mod.
    aliases: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
struct PlanEntry {
    /// Our id in the plan's own spelling, used when the folder is missing from
    /// our tree and there is no on-disk name to report it under.
    id: String,
    /// The Oracle's folder name, only when the plan states one that differs.
    oracle_name: Option<String>,
    section: Vec<String>,
    file_names: Vec<String>,
}

impl PlanIndex {
    pub fn from_plan(plan: &PersonalizedPlan) -> Self {
        let mut entries = HashMap::new();
        let mut order: Vec<Vec<String>> = Vec::new();
        let mut aliases = HashMap::new();

        for mod_entry in &plan.mods {
            if !order.iter().any(|seen| seen == &mod_entry.section) {
                order.push(mod_entry.section.clone());
            }
            // An `oracle_name` that only restates the id is not an alias: it
            // would map a key onto itself and claim the mod's own folder.
            let oracle_name = mod_entry
                .oracle_name
                .clone()
                .filter(|name| !name.eq_ignore_ascii_case(&mod_entry.id));
            if let Some(name) = &oracle_name {
                aliases.insert(name.to_lowercase(), mod_entry.id.to_lowercase());
            }
            entries.insert(
                mod_entry.id.to_lowercase(),
                PlanEntry {
                    id: mod_entry.id.clone(),
                    oracle_name,
                    section: mod_entry.section.clone(),
                    file_names: mod_entry
                        .archives
                        .iter()
                        .filter_map(|archive| archive.file_name.clone())
                        .collect(),
                },
            );
        }

        Self {
            entries,
            order,
            aliases,
        }
    }

    fn get(&self, id: &str) -> Option<&PlanEntry> {
        self.entries.get(&id.to_lowercase())
    }

    /// Whether this Oracle folder is claimed by some *other* mod's
    /// `oracle_name` and so must not be compared as a mod in its own right.
    ///
    /// A key that is also one of our own ids stays a mod: two entries could
    /// legitimately collide, and dropping ours would hide it entirely.
    fn is_claimed_alias(&self, key: &str) -> bool {
        self.aliases.contains_key(key) && !self.entries.contains_key(key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    /// The mod folder exists in both trees.
    Both,
    /// We installed something the Oracle does not have.
    OursOnly,
    /// The Oracle has it and we have not built it yet.
    OracleOnly,
}

/// One file that exists on both sides but is not the same file.
#[derive(Debug, Clone, Serialize)]
pub struct ContentDiff {
    /// Folder-relative path, as spelled in our tree.
    pub path: String,
    pub ours_size: u64,
    pub oracle_size: u64,
    /// Digests are filled in only when the sizes matched and the bytes were
    /// then found to differ: hashing is how a difference gets *described*, not
    /// how it gets detected, so identical files are never hashed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ours_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oracle_sha256: Option<String>,
}

/// How old the Oracle's archive is relative to the guide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GuideAge {
    /// The archive postdates the guide, so "the top file on the page" in March
    /// 2025 was not this file.
    PostGuide { timestamp: i64, date: String },
    /// The archive predates the guide and is what the guide would have named.
    PreGuide { timestamp: i64, date: String },
    /// No timestamp could be read. Reported rather than assumed to be fine.
    Unknown { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    /// `[General] version` from the Oracle's meta.ini, for the author's eyes:
    /// MO2 records it as a free-form string, so it is shown, not compared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oracle_version: Option<String>,
    /// `[General] installationFile` -- the archive the Oracle was built from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oracle_installation_file: Option<String>,
    /// The archive our plan declares, when a plan was supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_file_name: Option<String>,
    /// True when the plan names an archive and it is not the one the Oracle
    /// recorded installing. This is a real difference and gates the exit code.
    pub archive_mismatch: bool,
    pub guide_age: GuideAge,
}

/// A file both trees hold, which only one of them hides from the game.
#[derive(Debug, Clone, Serialize)]
pub struct HiddenDiff {
    /// Folder-relative path, with `.mohidden` stripped so both sides read the
    /// same.
    pub path: String,
    /// True when we hide it and the Oracle does not; false for the reverse.
    pub hidden_in_ours: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModDiff {
    pub id: String,
    /// The Oracle folder this was compared against, when the plan aliased it to
    /// a different id of ours. Always reported under our id; this names the
    /// folder the other side of the comparison actually came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oracle_name: Option<String>,
    pub section: Vec<String>,
    pub presence: Presence,
    /// Folder-relative paths present only in our tree.
    pub only_in_ours: Vec<String>,
    /// Folder-relative paths present only in the Oracle's tree.
    pub only_in_oracle: Vec<String>,
    pub content_differs: Vec<ContentDiff>,
    /// Files present on both sides but hidden on only one. The game sees a
    /// different set of files even though the folders hold the same bytes.
    pub hidden_differs: Vec<HiddenDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<VersionInfo>,
    /// Anything that went wrong reading either tree. Recorded per mod rather
    /// than aborting: one unreadable file should not cost the whole report.
    pub errors: Vec<String>,
}

impl ModDiff {
    /// Whether this mod reproduces the Oracle.
    ///
    /// A `POST-GUIDE` flag deliberately does *not* count: it describes the
    /// Oracle's own archive being newer than the guide, which is a fact about
    /// the reference, not a fault in our copy of it. Gating on it would fail
    /// every run for something no install could fix.
    pub fn is_identical(&self) -> bool {
        self.presence == Presence::Both
            && self.only_in_ours.is_empty()
            && self.only_in_oracle.is_empty()
            && self.content_differs.is_empty()
            && self.hidden_differs.is_empty()
            && self.errors.is_empty()
            && !self.version.as_ref().is_some_and(|v| v.archive_mismatch)
    }

    /// Whether this mod has something to say under "version notes", even when
    /// its files reproduce the Oracle exactly.
    fn has_version_note(&self) -> bool {
        self.version.as_ref().is_some_and(|version| {
            version.archive_mismatch || !matches!(version.guide_age, GuideAge::PreGuide { .. })
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SectionReport {
    /// Section path joined for display, or a placeholder when a mod has none.
    pub name: String,
    pub path: Vec<String>,
    pub compared: usize,
    pub identical: usize,
    pub mods: Vec<ModDiff>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffSummary {
    pub mods_compared: usize,
    pub identical: usize,
    pub differing: usize,
    /// In the Oracle but not in ours -- sections still to build.
    pub missing: usize,
    /// In ours but not in the Oracle.
    pub extra: usize,
    /// Mods whose Oracle archive postdates the guide.
    pub post_guide: usize,
    /// Mods whose Oracle archive carries no readable timestamp.
    pub unknown_archive_age: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffReport {
    pub ours: String,
    pub oracle: String,
    pub scope: String,
    pub summary: DiffSummary,
    pub sections: Vec<SectionReport>,
}

impl DiffReport {
    /// Whether the run found anything that should gate a section.
    pub fn has_differences(&self) -> bool {
        self.summary.differing > 0 || self.summary.missing > 0 || self.summary.extra > 0
    }
}

/// Compare every in-scope mod folder, in parallel across mods.
pub fn diff_all(settings: &DiffSettings) -> anyhow::Result<DiffReport> {
    let ours = list_mod_dirs(&settings.mods_dir)?;
    let oracle = list_mod_dirs(&settings.oracle_dir)?;

    let candidates = union_candidates(&ours, &oracle, settings);
    let diffs = compare_in_parallel(&candidates);

    Ok(assemble_report(diffs, settings))
}

/// A mod folder to compare, resolved on both sides before any IO is done.
struct Candidate {
    /// Display id: our spelling where we have it, the Oracle's otherwise.
    id: String,
    /// The Oracle's folder name, only when the plan aliased it to a different
    /// id of ours. Carried so the report can say which folder it compared.
    oracle_name: Option<String>,
    section: Vec<String>,
    ours: Option<PathBuf>,
    oracle: Option<PathBuf>,
    plan_file_names: Vec<String>,
}

/// Directory names in a mods folder, keyed by lowercased name.
///
/// A missing directory is not an error: comparing against a section that has
/// not been built yet is exactly what this command is for.
fn list_mod_dirs(root: &Path) -> anyhow::Result<BTreeMap<String, PathBuf>> {
    let mut found = BTreeMap::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(found),
        Err(err) => anyhow::bail!("failed to read mods directory {}: {err}", root.display()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_lowercase().ends_with(SEPARATOR_SUFFIX) {
            continue;
        }
        found.insert(name.to_lowercase(), path);
    }

    Ok(found)
}

fn union_candidates(
    ours: &BTreeMap<String, PathBuf>,
    oracle: &BTreeMap<String, PathBuf>,
    settings: &DiffSettings,
) -> Vec<Candidate> {
    let mut keys: Vec<String> = ours.keys().chain(oracle.keys()).cloned().collect();
    // An aliased mod we have not built yet has no key on either side under our
    // id -- only the Oracle's differently-named folder, which is skipped below.
    // Adding our id back is what makes it report as missing rather than vanish.
    if let Some(plan) = &settings.plan {
        for (oracle_key, id_key) in &plan.aliases {
            if oracle.contains_key(oracle_key) {
                keys.push(id_key.clone());
            }
        }
    }
    keys.sort_unstable();
    keys.dedup();

    let mut candidates = Vec::new();
    for key in keys {
        if settings
            .plan
            .as_ref()
            .is_some_and(|plan| plan.is_claimed_alias(&key))
        {
            continue;
        }

        let entry = settings.plan.as_ref().and_then(|plan| plan.get(&key));
        // Our side is always keyed by our own id; only the Oracle side follows
        // `oracle_name`, and only when the plan set one.
        let ours_path = ours.get(&key);
        let oracle_name = entry.and_then(|entry| entry.oracle_name.clone());
        let oracle_path = match &oracle_name {
            Some(name) => oracle.get(&name.to_lowercase()),
            None => oracle.get(&key),
        };

        let id = match ours_path {
            Some(path) => file_name_of(path, &key),
            // The Oracle's folder name is not ours to report an aliased mod
            // under, so the plan's spelling stands in when our tree has none.
            None if oracle_name.is_some() => {
                entry.map(|entry| entry.id.clone()).unwrap_or_else(|| key.clone())
            }
            None => oracle_path
                .map(|path| file_name_of(path, &key))
                .unwrap_or_else(|| key.clone()),
        };

        let section = entry.map(|entry| entry.section.clone()).unwrap_or_default();

        if !settings.filter.matches(&section, &id) {
            continue;
        }

        candidates.push(Candidate {
            id,
            oracle_name,
            section,
            ours: ours_path.cloned(),
            oracle: oracle_path.cloned(),
            plan_file_names: entry.map(|entry| entry.file_names.clone()).unwrap_or_default(),
        });
    }

    candidates
}

fn file_name_of(path: &Path, fallback: &str) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| fallback.to_string())
}

/// Spread the mods across a small worker pool.
///
/// Work per mod varies by three orders of magnitude -- an empty tweak folder
/// against a 2GB texture pack -- so the cursor hands out mods one at a time
/// rather than slicing the list up front and leaving one thread with all the
/// texture packs. Results are written back by index so the report is
/// deterministic regardless of who finished first.
fn compare_in_parallel(candidates: &[Candidate]) -> Vec<ModDiff> {
    if candidates.is_empty() {
        return Vec::new();
    }

    // Capped: past a handful of readers this is bound by the disk, and more
    // threads just multiply seeking between two 40GB trees.
    let workers = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .clamp(1, 8)
        .min(candidates.len());

    let cursor = AtomicUsize::new(0);
    let collected = Mutex::new(Vec::with_capacity(candidates.len()));

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let index = cursor.fetch_add(1, Ordering::Relaxed);
                    let Some(candidate) = candidates.get(index) else {
                        break;
                    };
                    local.push((index, compare_mod(candidate)));
                }
                collected
                    .lock()
                    .expect("diff collector should not be poisoned")
                    .extend(local);
            });
        }
    });

    let mut collected = collected
        .into_inner()
        .expect("diff collector should not be poisoned");
    // Sorted back into candidate order: the report must not depend on which
    // worker happened to finish first.
    collected.sort_by_key(|(index, _)| *index);
    collected.into_iter().map(|(_, diff)| diff).collect()
}

fn compare_mod(candidate: &Candidate) -> ModDiff {
    let mut diff = ModDiff {
        id: candidate.id.clone(),
        oracle_name: candidate.oracle_name.clone(),
        section: candidate.section.clone(),
        presence: match (&candidate.ours, &candidate.oracle) {
            (Some(_), Some(_)) => Presence::Both,
            (Some(_), None) => Presence::OursOnly,
            _ => Presence::OracleOnly,
        },
        only_in_ours: Vec::new(),
        only_in_oracle: Vec::new(),
        content_differs: Vec::new(),
        hidden_differs: Vec::new(),
        version: None,
        errors: Vec::new(),
    };

    if let Some(oracle_dir) = &candidate.oracle {
        diff.version = Some(read_version_info(oracle_dir, &candidate.plan_file_names));
    }

    // Nothing is read for a mod that exists on only one side: there is no
    // comparison to make, and the whole point of a section-by-section diff is
    // that the sections you have not built yet cost nothing to skip.
    let (Some(ours_dir), Some(oracle_dir)) = (&candidate.ours, &candidate.oracle) else {
        return diff;
    };

    let ours_tree = match walk_mod_tree(ours_dir) {
        Ok(tree) => tree,
        Err(err) => {
            diff.errors.push(err.to_string());
            return diff;
        }
    };
    let oracle_tree = match walk_mod_tree(oracle_dir) {
        Ok(tree) => tree,
        Err(err) => {
            diff.errors.push(err.to_string());
            return diff;
        }
    };

    for (key, ours_file) in &ours_tree {
        let Some(oracle_file) = oracle_tree.get(key) else {
            diff.only_in_ours.push(ours_file.display.clone());
            continue;
        };

        // Same file, but one side is out of the VFS and the other is not.
        // Content comparison cannot see this: the key deliberately ignores
        // `.mohidden` so a hidden file still matches its unhidden twin.
        if ours_file.hidden != oracle_file.hidden {
            diff.hidden_differs.push(HiddenDiff {
                path: strip_hidden_path(&ours_file.display),
                hidden_in_ours: ours_file.hidden,
            });
        }

        match compare_files(ours_file, oracle_file) {
            Ok(Some(content)) => diff.content_differs.push(content),
            Ok(None) => {}
            Err(err) => diff.errors.push(err),
        }
    }

    for (key, oracle_file) in &oracle_tree {
        if !ours_tree.contains_key(key) {
            diff.only_in_oracle.push(oracle_file.display.clone());
        }
    }

    diff
}

struct FileEntry {
    /// Folder-relative path with `/` separators, in its on-disk spelling.
    display: String,
    path: PathBuf,
    size: u64,
    /// Whether any segment of the path carries `.mohidden`, i.e. whether the
    /// game can see this file at all.
    hidden: bool,
}

/// Walk a mod folder into a map from comparison key to file.
///
/// The key is lowercased, `/`-separated and stripped of any `.mohidden`
/// suffix on *every* segment, which is what makes `Textures\Foo.DDS` and
/// `textures/foo.dds` the same file, `Fort Aurus.esp.mohidden` the same file as
/// `Fort Aurus.esp`, and `hair.mohidden/x.dds` the same file as `hair/x.dds`.
fn walk_mod_tree(root: &Path) -> anyhow::Result<BTreeMap<String, FileEntry>> {
    let mut files = BTreeMap::new();
    collect_files(root, root, 0, &mut files)?;
    Ok(files)
}

fn collect_files(
    current: &Path,
    root: &Path,
    depth: usize,
    files: &mut BTreeMap<String, FileEntry>,
) -> anyhow::Result<()> {
    if depth > MAX_DEPTH {
        anyhow::bail!("directory nesting exceeded {MAX_DEPTH} levels at {}", current.display());
    }

    let entries = std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?;

    // Sorted so a case-only collision resolves to the same winner every run
    // rather than to whatever order the filesystem happened to return.
    let mut children: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    children.sort();

    for path in children {
        let Some(name) = path.file_name().map(|name| name.to_string_lossy().into_owned()) else {
            continue;
        };

        // Follows symlinks deliberately: an install may hard- or symlink its
        // files in from a cache, and what matters is the content they resolve to.
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(err) => {
                anyhow::bail!("failed to stat {}: {err}", path.display());
            }
        };

        if metadata.is_dir() {
            if depth == 0 && name.eq_ignore_ascii_case(OMOD_CONVERSION_DIR) {
                continue;
            }
            collect_files(&path, root, depth + 1, files)?;
            continue;
        }

        if depth == 0 && name.eq_ignore_ascii_case(MO2_META_FILE) {
            continue;
        }

        let relative = path.strip_prefix(root).unwrap_or(&path);
        let display = relative.to_string_lossy().replace('\\', "/");

        if is_documentation(&display) {
            continue;
        }
        let key = comparison_key(&display);

        let hidden = display.split('/').any(|segment| strip_hidden_suffix(segment) != segment);

        files.entry(key).or_insert(FileEntry {
            display,
            path: path.clone(),
            size: metadata.len(),
            hidden,
        });
    }

    Ok(())
}

/// Fold a folder-relative path to the form both trees agree on.
fn comparison_key(relative: &str) -> String {
    let normalized = relative.replace('\\', "/");
    // Every segment, not just the last: MO2 hides a whole folder by renaming
    // the directory, so `textures/characters/nuska/hair.mohidden/x.dds` is the
    // same file as `textures/characters/nuska/hair/x.dds`.
    normalized
        .split('/')
        .map(strip_hidden_suffix)
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase()
}

/// A path with `.mohidden` removed from every segment, keeping its on-disk case.
fn strip_hidden_path(display: &str) -> String {
    display
        .split('/')
        .map(strip_hidden_suffix)
        .collect::<Vec<_>>()
        .join("/")
}

fn strip_hidden_suffix(name: &str) -> &str {
    if name.len() > MO2_HIDDEN_SUFFIX.len()
        && name[name.len() - MO2_HIDDEN_SUFFIX.len()..].eq_ignore_ascii_case(MO2_HIDDEN_SUFFIX)
    {
        &name[..name.len() - MO2_HIDDEN_SUFFIX.len()]
    } else {
        name
    }
}

/// Decide whether two files differ, doing as little IO as the answer allows.
///
/// Different sizes settle it outright, and no byte is read. Equal sizes are
/// compared chunk by chunk with an early exit on the first mismatch, so a file
/// that differs in its header costs one read rather than a full pass over a
/// multi-GB archive. Only once a difference is confirmed -- for the handful of
/// files that actually differ -- are digests computed, to name the difference
/// in the report. Identical files are therefore never hashed at all, which is
/// what keeps a whole-instance run bound by a single sequential read.
fn compare_files(ours: &FileEntry, oracle: &FileEntry) -> Result<Option<ContentDiff>, String> {
    if ours.size != oracle.size {
        return Ok(Some(ContentDiff {
            path: ours.display.clone(),
            ours_size: ours.size,
            oracle_size: oracle.size,
            ours_sha256: None,
            oracle_sha256: None,
        }));
    }

    let same = bytes_equal(&ours.path, &oracle.path).map_err(|err| {
        format!(
            "failed to compare {} with {}: {err}",
            ours.path.display(),
            oracle.path.display()
        )
    })?;
    if same {
        return Ok(None);
    }

    let ours_sha256 = sha256_file(&ours.path)
        .map_err(|err| format!("failed to hash {}: {err}", ours.path.display()))?;
    let oracle_sha256 = sha256_file(&oracle.path)
        .map_err(|err| format!("failed to hash {}: {err}", oracle.path.display()))?;

    Ok(Some(ContentDiff {
        path: ours.display.clone(),
        ours_size: ours.size,
        oracle_size: oracle.size,
        ours_sha256: Some(ours_sha256),
        oracle_sha256: Some(oracle_sha256),
    }))
}

fn bytes_equal(left: &Path, right: &Path) -> std::io::Result<bool> {
    let mut left = std::io::BufReader::with_capacity(CHUNK, std::fs::File::open(left)?);
    let mut right = std::io::BufReader::with_capacity(CHUNK, std::fs::File::open(right)?);

    let mut left_buf = vec![0u8; CHUNK];
    let mut right_buf = vec![0u8; CHUNK];

    loop {
        let read = read_full(&mut left, &mut left_buf)?;
        let other = read_full(&mut right, &mut right_buf)?;
        if read != other {
            return Ok(false);
        }
        if read == 0 {
            return Ok(true);
        }
        if left_buf[..read] != right_buf[..read] {
            return Ok(false);
        }
    }
}

/// Fill `buf` as far as the reader allows, so both sides are compared over the
/// same window even when one of them returns short reads.
fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..])? {
            0 => break,
            count => filled += count,
        }
    }
    Ok(filled)
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn read_version_info(oracle_dir: &Path, plan_file_names: &[String]) -> VersionInfo {
    let meta = std::fs::read_to_string(oracle_dir.join(MO2_META_FILE))
        .ok()
        .map(|text| parse_meta_ini(&text))
        .unwrap_or_default();

    let installation_file = meta.installation_file.clone();
    let plan_file_name = plan_file_names.first().cloned();

    // Only a plan that actually declares an archive can disagree with the
    // Oracle; a mod we build from loose files or a merge has nothing to compare.
    let archive_mismatch = match (&plan_file_name, &installation_file) {
        (Some(ours), Some(theirs)) => !names_the_same_archive(ours, theirs),
        _ => false,
    };

    VersionInfo {
        oracle_version: meta.version,
        guide_age: classify_guide_age(
            installation_file.as_deref(),
            meta.nexus_last_modified.as_deref(),
            meta.mod_id,
            meta.file_id,
        ),
        oracle_installation_file: installation_file,
        plan_file_name,
        archive_mismatch,
    }
}

/// Whether two `installationFile` spellings name the same archive.
///
/// Usually a plain comparison, but not when the Oracle was built by installing
/// from mudcrab's own cache. The cache renames on the way in --
/// `{mod id}_{archive index}_{sanitised source}`, so `Ogorod 1.1.rar` arrives as
/// `Ogorod_0_manual_Ogorod_1.1.rar` -- and MO2 records whatever name it was
/// handed. Reporting that as "the Oracle installed a different archive" is
/// false: it is the same bytes under the name we gave them.
///
/// The prefix has to *look like* a cache prefix, not merely end in `_`. A first
/// version required only the underscore, which made `Base_Metal.7z` "the same
/// archive" as `Metal.7z` -- and archive names on Nexus and tesall.ru use
/// underscores as word separators constantly, so that was a live way to swallow
/// a real mismatch rather than a theoretical one. The marker is the archive
/// index: an all-digit component, which a filename does not otherwise produce
/// in that position.
fn names_the_same_archive(plan: &str, oracle: &str) -> bool {
    if plan.eq_ignore_ascii_case(oracle) {
        return true;
    }

    // The cache sanitises every character outside `[A-Za-z0-9._-]`, so the plan
    // name has to be sanitised the same way before it can be looked for.
    let sanitise = |name: &str| -> String {
        name.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .to_ascii_lowercase()
    };

    let Some(prefix) = sanitise(oracle).strip_suffix(&sanitise(plan)).map(str::to_string) else {
        return false;
    };
    let Some(prefix) = prefix.strip_suffix('_') else {
        return false;
    };

    prefix
        .split('_')
        .any(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// The Nexus file id separating files published before the guide from files
/// published after it.
///
/// Nexus appears to allocate file ids in ascending order, so an id doubles as an
/// upload date. Calibrated against the archives in this list carrying both a
/// file id and a Unix timestamp in their filename: 406 of them, of which 405
/// were usable, and across those 405 the two orderings agree exactly. The
/// boundary is clean -- largest pre-guide id 1_000_040_927 (2025-02-23),
/// smallest post-guide id 1_000_040_999 (2025-03-01), nothing in between.
/// Legacy ids, five or six digits, fall far below either.
///
/// The 406th is Part 20's VGR patch, whose two versions' MO2 sidecars both claim
/// the same file id -- which is itself why that row is written up as unresolved.
/// Excluded rather than allowed to widen the boundary.
///
/// The value is baked in, so a user who has never seen a reference instance gets
/// the same answers -- which is the real gain over `nexusLastModified`, and the
/// reason this constant is worth having. It is *not* an independently verified
/// property of Nexus site-wide: it is one game's corpus over one year, and
/// re-calibrating means finding the boundary again from filename timestamps.
const GUIDE_FILE_ID: u64 = 1_000_040_999;

/// Date the Oracle's archive, to decide whether the March 2025 guide could
/// have meant this file.
///
/// Two sources, in order of how specific they are:
///
/// 1. The timestamp Nexus embeds in a download's filename
///    (`<title>-<modid>-<version parts>-<unix timestamp>.7z`). This names the
///    exact file, so it is preferred -- but Nexus only started doing it at some
///    point, and older archives end in the bare mod id or nothing useful.
/// 2. The Nexus file id, against [`GUIDE_FILE_ID`].
///
/// `nexusLastModified` used to be the second source and is no longer consulted
/// at all: it is frequently the date MO2 fetched rather than the date the file
/// was published, and the file id answers the same question correctly. It is
/// still read from `meta.ini`, because its *presence* is what distinguishes
/// "no date recorded" from "a date we decline to trust" in the message below.
///
/// With neither source, the age is reported as unknown rather than guessed: "no
/// timestamp" and "old enough" are very different answers.
pub fn classify_guide_age(
    installation_file: Option<&str>,
    nexus_last_modified: Option<&str>,
    mod_id: Option<u64>,
    file_id: Option<u64>,
) -> GuideAge {
    let name = installation_file.map(str::trim).filter(|name| !name.is_empty());

    // `nexusLastModified` on a mod that did not come from Nexus is not a date
    // for the file -- it is when MO2 wrote the entry. Part 10's WAC is the case
    // that exposed it: a v1 beta from around 2010, hosted on TES Alliance,
    // stamped 2026-01-25 because that is when it was installed here, and duly
    // reported as POST-GUIDE. The archive filename is still fair evidence when
    // it carries a Unix timestamp, so only the Nexus-derived date is dropped.
    // The date is evidence only if MO2 actually recorded a Nexus *file*. A
    // missing file id means it did not, whatever the mod id says -- Part 13's
    // `Oblivion Landskape` is a tesall.ru download whose meta.ini claims
    // modid=7, which is MO2 bookkeeping rather than provenance, and which slid
    // straight past a mod-id-only check to be flagged POST-GUIDE.
    let from_nexus = !matches!(mod_id, Some(0) | None) && !matches!(file_id, Some(0) | None);

    let Some(name) = name else {
        // Without an `installationFile` there is no archive to date. MO2 still
        // writes a `nexusLastModified`, but for a hand-installed mod that is
        // simply when the folder was created -- dating a 2019 file to today.
        // A confident wrong answer is worse here than admitting ignorance.
        return GuideAge::Unknown {
            reason: "the Oracle's meta.ini records no installationFile".to_string(),
        };
    };

    if let Some(timestamp) = parse_trailing_timestamp(name) {
        return classify_timestamp(timestamp);
    }

    // The file id dates the file, and far more reliably than
    // `nexusLastModified` does: ids are allocated in ascending order, while the
    // meta.ini date is frequently just when the archive was downloaded here.
    // `TIBs Compact Quivers` is the shape -- three 2018 uploads stamped
    // 2026-01-26, all three reported as newer than the guide.
    //
    // Gated on `from_nexus` for the same reason the date below is: a `meta.ini`
    // field can be MO2 bookkeeping rather than provenance, and a stray number
    // would otherwise become a confident answer with nothing behind it.
    if let Some(file_id) = file_id.filter(|_| from_nexus).filter(|id| *id > 0) {
        return if file_id < GUIDE_FILE_ID {
            GuideAge::PreGuide {
                timestamp: 0,
                date: format!("Nexus file id {file_id}"),
            }
        } else {
            GuideAge::PostGuide {
                timestamp: 0,
                date: format!("Nexus file id {file_id}"),
            }
        };
    }

    // Only worth saying when a date was actually discarded. With no
    // `nexusLastModified` at all, the filename is the whole story and the
    // message below says so more usefully.
    if !from_nexus && nexus_last_modified.is_some() {
        return GuideAge::Unknown {
            reason: format!(
                "'{name}' has no recorded Nexus file, so its meta.ini date is not \
                 evidence about the archive"
            ),
        };
    }

    GuideAge::Unknown {
        reason: format!("no Unix timestamp in '{name}', and no nexusLastModified"),
    }
}

fn classify_timestamp(timestamp: i64) -> GuideAge {
    let date = format_date(timestamp);
    if timestamp >= GUIDE_CUTOFF {
        GuideAge::PostGuide { timestamp, date }
    } else {
        GuideAge::PreGuide { timestamp, date }
    }
}


fn parse_trailing_timestamp(file_name: &str) -> Option<i64> {
    // The last `-` separated field, cut at its first non-digit. This survives
    // multi-part extensions (`...-1647873144.7z.001`) without special-casing
    // every archive suffix Nexus serves.
    let tail = file_name.rsplit('-').next()?;
    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }

    let value: i64 = digits.parse().ok()?;
    (TIMESTAMP_MIN..=TIMESTAMP_MAX).contains(&value).then_some(value)
}

/// Render a Unix timestamp as `YYYY-MM-DD` (UTC).
///
/// Hand-rolled rather than pulling in a date crate for one format string.
fn format_date(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };

    format!("{year:04}-{month:02}-{day:02}")
}

fn assemble_report(diffs: Vec<ModDiff>, settings: &DiffSettings) -> DiffReport {
    let mut summary = DiffSummary {
        mods_compared: diffs.len(),
        identical: 0,
        differing: 0,
        missing: 0,
        extra: 0,
        post_guide: 0,
        unknown_archive_age: 0,
    };

    for diff in &diffs {
        match diff.presence {
            Presence::OursOnly => summary.extra += 1,
            Presence::OracleOnly => summary.missing += 1,
            Presence::Both => {
                if diff.is_identical() {
                    summary.identical += 1;
                } else {
                    summary.differing += 1;
                }
            }
        }
        match diff.version.as_ref().map(|version| &version.guide_age) {
            Some(GuideAge::PostGuide { .. }) => summary.post_guide += 1,
            Some(GuideAge::Unknown { .. }) => summary.unknown_archive_age += 1,
            _ => {}
        }
    }

    let mut grouped: BTreeMap<Vec<String>, Vec<ModDiff>> = BTreeMap::new();
    for diff in diffs {
        grouped.entry(diff.section.clone()).or_default().push(diff);
    }

    // Plan order first, so the report reads in modlist order; anything the plan
    // does not mention follows, sorted, rather than being dropped.
    let mut ordered: Vec<Vec<String>> = Vec::new();
    if let Some(plan) = &settings.plan {
        for section in &plan.order {
            if grouped.contains_key(section) && !ordered.contains(section) {
                ordered.push(section.clone());
            }
        }
    }
    let seen: HashSet<&Vec<String>> = ordered.iter().collect();
    let mut rest: Vec<Vec<String>> = grouped
        .keys()
        .filter(|section| !seen.contains(section))
        .cloned()
        .collect();
    rest.sort();
    ordered.extend(rest);

    let sections = ordered
        .into_iter()
        .filter_map(|path| {
            let mods = grouped.remove(&path)?;
            let identical = mods.iter().filter(|diff| diff.is_identical()).count();
            Some(SectionReport {
                name: describe_section(&path),
                path,
                compared: mods.len(),
                identical,
                mods,
            })
        })
        .collect();

    DiffReport {
        ours: settings.mods_dir.display().to_string(),
        oracle: settings.oracle_dir.display().to_string(),
        scope: settings.filter.describe(),
        summary,
        sections,
    }
}

fn describe_section(path: &[String]) -> String {
    if path.is_empty() {
        "(no section)".to_string()
    } else {
        path.join(" / ")
    }
}

/// How many entries of a list to print before summarising the rest.
///
/// A mod installed with the wrong layout differs in every one of its files, and
/// a report that prints five thousand paths is one nobody reads. The JSON
/// output carries the full list for anything that needs it.
const LIST_LIMIT: usize = 20;

/// Render the report as the compact per-section text an agent can paste back.
pub fn render_text(report: &DiffReport) -> String {
    let mut out = String::new();
    out.push_str("mudcrab diff\n");
    out.push_str(&format!("  ours:   {}\n", report.ours));
    out.push_str(&format!("  oracle: {}\n", report.oracle));
    out.push_str(&format!("  scope:  {}\n\n", report.scope));

    let summary = &report.summary;
    out.push_str(&format!(
        "summary: {} compared, {} identical, {} differing, {} missing from ours, {} extra in ours\n",
        summary.mods_compared, summary.identical, summary.differing, summary.missing, summary.extra
    ));

    if report.summary.mods_compared == 0 {
        out.push_str("\nnothing in scope: no mod folder matched the filter in either tree\n");
        return out;
    }

    for section in &report.sections {
        out.push_str(&format!("\n[{}]\n", section.name));
        out.push_str(&format!(
            "  {} of {} identical\n",
            section.identical, section.compared
        ));

        for diff in &section.mods {
            if diff.is_identical() {
                continue;
            }
            render_mod(&mut out, diff);
        }
    }

    render_version_notes(&mut out, report);
    out
}

fn render_mod(out: &mut String, diff: &ModDiff) {
    // Reported under our id, with the Oracle's own folder named alongside it so
    // the line can still be found in the reference instance.
    let alias = diff
        .oracle_name
        .as_deref()
        .map(|name| format!("  (oracle: {name})"))
        .unwrap_or_default();

    match diff.presence {
        Presence::OracleOnly => {
            out.push_str(&format!("  - {}{alias}  (missing from ours)\n", diff.id));
            return;
        }
        Presence::OursOnly => {
            out.push_str(&format!("  + {}{alias}  (not in the Oracle)\n", diff.id));
            return;
        }
        Presence::Both => out.push_str(&format!("  ~ {}{alias}\n", diff.id)),
    }

    if !diff.content_differs.is_empty() {
        out.push_str(&format!(
            "      content differs ({}):\n",
            diff.content_differs.len()
        ));
        for entry in diff.content_differs.iter().take(LIST_LIMIT) {
            if entry.ours_size == entry.oracle_size {
                out.push_str(&format!(
                    "        {}  (same size {} B, sha256 {} vs {})\n",
                    entry.path,
                    entry.ours_size,
                    short_hash(entry.ours_sha256.as_deref()),
                    short_hash(entry.oracle_sha256.as_deref()),
                ));
            } else {
                out.push_str(&format!(
                    "        {}  (ours {} B, oracle {} B)\n",
                    entry.path, entry.ours_size, entry.oracle_size
                ));
            }
        }
        render_overflow(out, diff.content_differs.len());
    }

    if !diff.hidden_differs.is_empty() {
        out.push_str(&format!(
            "      hidden on one side only ({}):\n",
            diff.hidden_differs.len()
        ));
        for entry in diff.hidden_differs.iter().take(LIST_LIMIT) {
            out.push_str(&format!(
                "        {}  (hidden in {})\n",
                entry.path,
                if entry.hidden_in_ours { "ours" } else { "the Oracle" }
            ));
        }
        render_overflow(out, diff.hidden_differs.len());
    }

    render_paths(out, "only in ours", &diff.only_in_ours);
    render_paths(out, "only in the Oracle", &diff.only_in_oracle);

    if let Some(version) = &diff.version
        && version.archive_mismatch
    {
        out.push_str(&format!(
            "      archive mismatch: plan has '{}', Oracle installed '{}'\n",
            version.plan_file_name.as_deref().unwrap_or("<unset>"),
            version.oracle_installation_file.as_deref().unwrap_or("<unset>"),
        ));
    }

    for error in &diff.errors {
        out.push_str(&format!("      error: {error}\n"));
    }
}

fn render_paths(out: &mut String, label: &str, paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    out.push_str(&format!("      {label} ({}):\n", paths.len()));
    for path in paths.iter().take(LIST_LIMIT) {
        out.push_str(&format!("        {path}\n"));
    }
    render_overflow(out, paths.len());
}

fn render_overflow(out: &mut String, total: usize) {
    if total > LIST_LIMIT {
        out.push_str(&format!("        ... and {} more\n", total - LIST_LIMIT));
    }
}

fn short_hash(hash: Option<&str>) -> String {
    hash.map(|value| value.chars().take(12).collect())
        .unwrap_or_else(|| "?".to_string())
}

fn render_version_notes(out: &mut String, report: &DiffReport) {
    let notable: Vec<&ModDiff> = report
        .sections
        .iter()
        .flat_map(|section| section.mods.iter())
        .filter(|diff| diff.has_version_note())
        .collect();

    if notable.is_empty() {
        return;
    }

    let post_guide: Vec<&&ModDiff> = notable
        .iter()
        .filter(|diff| {
            matches!(
                diff.version.as_ref().map(|v| &v.guide_age),
                Some(GuideAge::PostGuide { .. })
            )
        })
        .collect();
    let unknown: Vec<&&ModDiff> = notable
        .iter()
        .filter(|diff| {
            matches!(
                diff.version.as_ref().map(|v| &v.guide_age),
                Some(GuideAge::Unknown { .. })
            )
        })
        .collect();

    out.push_str("\nversion notes:\n");

    if !post_guide.is_empty() {
        out.push_str(&format!(
            "  POST-GUIDE ({}): the Oracle's archive is newer than the March 2025 guide,\n  so \"the top file on the page\" is not what it installed.\n",
            post_guide.len()
        ));
        for diff in &post_guide {
            let version = diff.version.as_ref().expect("post-guide implies version info");
            let GuideAge::PostGuide { date, .. } = &version.guide_age else {
                continue;
            };
            out.push_str(&format!(
                "    ! {}  dated {}  ({}){}\n",
                diff.id,
                date,
                version.oracle_installation_file.as_deref().unwrap_or("<unset>"),
                version
                    .oracle_version
                    .as_deref()
                    .map(|value| format!("  version {value}"))
                    .unwrap_or_default(),
            ));
        }
    }

    if !unknown.is_empty() {
        out.push_str(&format!(
            "  UNKNOWN AGE ({}): no timestamp could be read, so whether the guide named\n  this file cannot be determined from the filename alone.\n",
            unknown.len()
        ));
        for diff in &unknown {
            let version = diff.version.as_ref().expect("unknown implies version info");
            let GuideAge::Unknown { reason } = &version.guide_age else {
                continue;
            };
            out.push_str(&format!("    ? {}  {}\n", diff.id, reason));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn comparison_key_folds_case_separators_and_hidden_suffix() {
        assert_eq!(comparison_key("Textures\\Foo.DDS"), "textures/foo.dds");
        assert_eq!(comparison_key("textures/foo.dds"), "textures/foo.dds");
        assert_eq!(comparison_key("Fort Aurus.esp.mohidden"), "fort aurus.esp");
        // MO2 hides a folder by renaming the directory, so the suffix can sit
        // on any segment, not only the last.
        assert_eq!(
            comparison_key("Textures/characters/nuska/hair.mohidden/nugrey.dds"),
            "textures/characters/nuska/hair/nugrey.dds"
        );
        assert_eq!(comparison_key("Fort Aurus.esp.MOHIDDEN"), "fort aurus.esp");
        // A file actually named ".mohidden" keeps its name: stripping it would
        // leave an empty path.
        assert_eq!(comparison_key(".mohidden"), ".mohidden");
    }

    #[test]
    fn trailing_timestamps_are_read_only_when_plausible() {
        assert_eq!(
            parse_trailing_timestamp("Better Fort Aurus-50682-1-1-1647873144.7z"),
            Some(1_647_873_144)
        );
        assert_eq!(
            parse_trailing_timestamp("Something-1-0-1647873144.7z.001"),
            Some(1_647_873_144)
        );
        // A bare Nexus mod id is not a timestamp.
        assert_eq!(parse_trailing_timestamp("Anvil Morning Glory-19039.7z"), None);
        assert_eq!(parse_trailing_timestamp("DarNified UI 132 FOMOD - Merged.7z"), None);
        assert_eq!(parse_trailing_timestamp(""), None);
    }

    /// A Nexus file id from just before the guide, for tests about something
    /// other than the id itself. `1` used to serve, but an id now dates the file
    /// on its own -- which is the point of
    /// `GUIDE_FILE_ID` and would decide these cases before they got to what they
    /// are testing.
    const MODERN_FILE_ID: u64 = 1_000_040_000;

    #[test]
    fn guide_age_splits_on_the_march_2025_cutoff() {
        assert_eq!(
            classify_guide_age(Some("Better Fort Aurus-50682-1-1-1647873144.7z"), None, Some(50682), Some(MODERN_FILE_ID)),
            GuideAge::PreGuide {
                timestamp: 1_647_873_144,
                date: "2022-03-21".to_string()
            }
        );
        assert_eq!(
            classify_guide_age(Some("Newer Mod-1234-2-0-1750000000.7z"), None, Some(1234), Some(MODERN_FILE_ID)),
            GuideAge::PostGuide {
                timestamp: 1_750_000_000,
                date: "2025-06-15".to_string()
            }
        );
        // A filename with no timestamp used to be unanswerable. The file id
        // answers it now, which is the point of the boundary.
        assert!(matches!(
            classify_guide_age(Some("Anvil Morning Glory-19039.7z"), None, Some(19039), Some(MODERN_FILE_ID)),
            GuideAge::PreGuide { .. }
        ));
        // With neither a timestamp nor a file id there is still nothing to go on.
        assert!(matches!(
            classify_guide_age(Some("Anvil Morning Glory.7z"), None, Some(19039), Some(0)),
            GuideAge::Unknown { .. }
        ));
        assert!(matches!(classify_guide_age(None, None, Some(1), Some(MODERN_FILE_ID)), GuideAge::Unknown { .. }));
    }

    fn compare_trees(ours: &Path, oracle: &Path) -> ModDiff {
        compare_mod(&Candidate {
            id: "Example".to_string(),
            oracle_name: None,
            section: vec!["7 - CHARACTER AND NPCS".to_string()],
            ours: Some(ours.to_path_buf()),
            oracle: Some(oracle.to_path_buf()),
            plan_file_names: Vec::new(),
        })
    }

    fn write_file(path: &Path, contents: &[u8]) {
        std::fs::create_dir_all(path.parent().expect("file has a parent")).expect("mkdir");
        std::fs::write(path, contents).expect("write");
    }

    #[test]
    fn a_file_hidden_on_only_one_side_is_reported() {
        // `comparison_key` deliberately ignores `.mohidden`, so a hidden file
        // still matches its unhidden twin and the content comparison sees
        // nothing. Without this check, hiding the wrong file is invisible.
        let temp = tempdir().expect("tempdir");
        let ours = temp.path().join("ours");
        let oracle = temp.path().join("oracle");

        write_file(&ours.join("textures/khajiit/head.dds.mohidden"), b"same");
        write_file(&oracle.join("textures/khajiit/head.dds"), b"same");
        write_file(&ours.join("textures/khajiit/ear.dds"), b"same");
        write_file(&oracle.join("textures/khajiit/ear.dds"), b"same");

        let diff = compare_trees(&ours, &oracle);

        assert!(diff.only_in_ours.is_empty(), "{:?}", diff.only_in_ours);
        assert!(diff.only_in_oracle.is_empty(), "{:?}", diff.only_in_oracle);
        assert!(diff.content_differs.is_empty(), "the bytes are identical");
        assert_eq!(diff.hidden_differs.len(), 1);
        assert_eq!(diff.hidden_differs[0].path, "textures/khajiit/head.dds");
        assert!(diff.hidden_differs[0].hidden_in_ours);
        assert!(!diff.is_identical(), "a hidden-state mismatch is a difference");
    }

    #[test]
    fn hiding_a_whole_folder_matches_hiding_it_the_same_way() {
        let temp = tempdir().expect("tempdir");
        let ours = temp.path().join("ours");
        let oracle = temp.path().join("oracle");

        write_file(&ours.join("textures/nuska/hair.mohidden/grey.dds"), b"same");
        write_file(&oracle.join("textures/nuska/hair.mohidden/grey.dds"), b"same");

        let diff = compare_trees(&ours, &oracle);

        assert!(diff.hidden_differs.is_empty(), "{:?}", diff.hidden_differs);
        assert!(diff.is_identical());
    }

    #[test]
    fn no_installation_file_means_unknown_even_with_a_meta_ini_date() {
        // A hand-installed mod has no `installationFile`, and MO2's
        // `nexusLastModified` is then just when the folder was written. Reading
        // it would date a 2019 archive to today and flag it POST-GUIDE.
        let age = classify_guide_age(None, Some("2026-08-16T20:43:11Z"), Some(1), Some(MODERN_FILE_ID));
        assert!(matches!(age, GuideAge::Unknown { .. }), "{age:?}");

        let age = classify_guide_age(Some("   "), Some("2026-08-16T20:43:11Z"), Some(1), Some(MODERN_FILE_ID));
        assert!(matches!(age, GuideAge::Unknown { .. }), "{age:?}");
    }

    #[test]
    fn a_non_nexus_mods_meta_ini_date_is_not_evidence_about_its_archive() {
        // Part 10's WAC: a v1 beta from around 2010, hosted on TES Alliance, so
        // modid=0. MO2 still wrote nexusLastModified=2026-01-25 -- the day it
        // was installed here, not the day the file was published -- and reading
        // it flagged a fifteen-year-old archive as newer than the guide.
        let age = classify_guide_age(Some("WACv_1beta.7z"), Some("2026-01-25T20:19:46Z"), Some(0), Some(0));
        assert!(matches!(age, GuideAge::Unknown { .. }), "{age:?}");
        match age {
            GuideAge::Unknown { reason } => assert!(reason.contains("no recorded Nexus file"), "{reason}"),
            other => panic!("expected Unknown, got {other:?}"),
        }

        // Part 13's `Oblivion Landskape`: a tesall.ru download whose meta.ini
        // claims modid=7 and records no file id. A mod-id-only check let this
        // through and flagged a hand-downloaded archive POST-GUIDE.
        let age = classify_guide_age(
            Some("Oblivion landskape.7z"),
            Some("2026-01-25T20:19:46Z"),
            Some(7),
            None,
        );
        assert!(matches!(age, GuideAge::Unknown { .. }), "{age:?}");

        // A real Nexus file is dated by its id instead, and correctly: WAC's
        // meta.ini date of 2026-01-25 is when it was installed here.
        let age = classify_guide_age(
            Some("WACv_1beta.7z"),
            Some("2026-01-25T20:19:46Z"),
            Some(1318),
            Some(52000),
        );
        assert!(matches!(age, GuideAge::PreGuide { .. }), "{age:?}");

        // A filename timestamp names *this file* whoever hosted it, so it
        // survives modid=0 -- only the Nexus-derived date is dropped.
        let age = classify_guide_age(Some("Mod-1-0-1647873144.7z"), None, Some(0), Some(0));
        assert!(matches!(age, GuideAge::PreGuide { .. }), "{age:?}");
    }

    #[test]
    fn a_filename_timestamp_wins_over_the_meta_ini_date() {
        // The filename names *this* file; nexusLastModified is about the mod
        // page and can have moved on since.
        let age = classify_guide_age(
            Some("Better Fort Aurus-50682-1-1-1647873144.7z"),
            Some("2026-01-01T00:00:00Z"),
            Some(50682),
            Some(MODERN_FILE_ID),
        );
        match age {
            GuideAge::PreGuide { date, .. } => assert_eq!(date, "2022-03-21"),
            other => panic!("expected the filename's 2022 date, got {other:?}"),
        }
    }

    #[test]
    fn documentation_is_recognised_narrowly() {
        for doc in [
            "readme.txt",
            "ReadMeEN.txt",
            "readme_JP.txt",
            "TD_aesthetics_readme.txt",
            "HD Cobwebs - Readme.txt",
            "Readme and Credits.txt",
            "docs/License.txt",
            "obmm_BSA_settings.jpg",
            "Файл скачан с сайта TESALL.RU.url",
        ] {
            assert!(is_documentation(doc), "should be documentation: {doc}");
        }

        // Game content that happens to be text, or merely lives near docs,
        // must still be compared -- this rule decides what gets *reported*, and
        // over-matching here would hide real differences.
        for content in [
            "textures/rock.dds",
            "meshes/plants/cattail.nif",
            "OBSE/Plugins/ORC.ini",
            "Data/notes.txt",
            "shaders/orc/fog/Fog.ini",
            "Maskar's Oblivion Overhaul for spawns.ini",
        ] {
            assert!(!is_documentation(content), "should not be documentation: {content}");
        }
    }

    #[test]
    fn the_cutoff_constant_is_the_guides_publication_date() {
        assert_eq!(format_date(GUIDE_CUTOFF), "2025-03-01");
        assert_eq!(format_date(GUIDE_CUTOFF - 1), "2025-02-28");
        assert_eq!(format_date(0), "1970-01-01");
    }
}

#[cfg(test)]
mod archive_name_tests {
    use super::names_the_same_archive;

    #[test]
    fn a_cache_renamed_archive_is_the_same_archive() {
        // Installing into the Oracle from mudcrab's cache leaves MO2 recording
        // the cache's name for it. That is the same bytes, not a different
        // download, and reporting it as a mismatch buries the real ones.
        assert!(names_the_same_archive(
            "Ogorod 1.1.rar",
            "Ogorod_0_manual_Ogorod_1.1.rar"
        ));
        assert!(names_the_same_archive(
            "well.zip",
            "KatKat's Well_0_manual_well.zip"
        ));
        assert!(names_the_same_archive("Bliss.7z", "Bliss.7z"));
    }

    #[test]
    fn a_genuinely_different_archive_still_reports() {
        // The case this check must never swallow: Part 16's BETA1 vs BETA2.
        assert!(!names_the_same_archive(
            "T4UTXL - Architecture_BETA1-54904-Architecture-BETA1-1742397074.7z",
            "T4UTXL - Architecture_BETA2 (Part 2)-54904-ARCHITECTURE-BETA2-1744730302.7z"
        ));
        // A suffix match that is not at a cache-prefix boundary is a coincidence.
        assert!(!names_the_same_archive("Metal.7z", "BaseMetal.7z"));
        assert!(!names_the_same_archive("Bliss.7z", "OtherBliss.7z"));
    }

    #[test]
    fn an_underscore_alone_does_not_make_it_a_cache_name() {
        // The gap in the first version. Archive names use underscores as word
        // separators all the time, so requiring only `<something>_<plan name>`
        // matched real, different archives. A cache name carries the archive
        // index, and an all-digit component is what says so.
        assert!(!names_the_same_archive("Metal.7z", "Base_Metal.7z"));
        assert!(!names_the_same_archive("Ships.rar", "Old_Ships.rar"));
        assert!(!names_the_same_archive("Core.7z", "T4UT_Core.7z"));
        // Still matched when the index really is there.
        assert!(names_the_same_archive("Metal.7z", "Ayleid Ruins_0_manual_Metal.7z"));
        // The cache sanitises apostrophes and spaces, so the plan name has to be
        // sanitised the same way to be found inside the cache name.
        assert!(names_the_same_archive(
            "well.zip",
            "KatKat_s Well_0_manual_well.zip"
        ));
    }
}

#[cfg(test)]
mod file_id_age_tests {
    use super::{classify_guide_age, GuideAge};

    #[test]
    fn a_file_id_outweighs_a_download_stamped_meta_ini() {
        // `Mehrunes Dagon Retex`: file id 54124, an old-scheme upload, with
        // `nexusLastModified=2026-01-26` -- which is when it was downloaded to
        // this machine, not when it was published. Reported as newer than the
        // guide, along with seven others in the list.
        let age = classify_guide_age(
            Some("Mehrunes Dagon Alt Textures - matching arms and legs-29314.7z"),
            Some("2026-01-26T00:06:25Z"),
            Some(29314),
            Some(54124),
        );
        match age {
            GuideAge::PreGuide { date, .. } => assert!(date.contains("54124"), "{date}"),
            other => panic!("expected PreGuide, got {other:?}"),
        }

        // `TIBs Compact Quivers`: a 2018 upload stamped 2026-01-26, which is
        // when it was downloaded here. Three of them were reported as newer
        // than the guide.
        let age = classify_guide_age(
            Some("TIBs Compact Quivers - Manual Install-45111-1-0.rar"),
            Some("2026-01-26T00:00:00Z"),
            Some(45111),
            Some(1000006810),
        );
        assert!(matches!(age, GuideAge::PreGuide { .. }), "{age:?}");
    }

    #[test]
    fn a_file_id_above_the_boundary_is_after_the_guide() {
        // The check must not swallow real drift. Part 19's Simple Horse
        // Utilities is a genuine post-guide file and stays one.
        let age = classify_guide_age(
            Some("Simple Horse Utilities-51197.7z"),
            None,
            Some(51197),
            Some(1000041917),
        );
        assert!(matches!(age, GuideAge::PostGuide { .. }), "{age:?}");

        // And the boundary itself: the smallest post-guide id observed.
        let age = classify_guide_age(Some("Sneak Vignette.7z"), None, Some(1), Some(1000040999));
        assert!(matches!(age, GuideAge::PostGuide { .. }), "{age:?}");
        // ...against the largest pre-guide id observed.
        let age = classify_guide_age(Some("Better dungeons.7z"), None, Some(1), Some(1000040927));
        assert!(matches!(age, GuideAge::PreGuide { .. }), "{age:?}");
    }

    #[test]
    fn a_timestamped_filename_still_wins_over_everything() {
        // The filename names the exact file, so it stays the first source even
        // when the id is legacy.
        let age = classify_guide_age(
            Some("Old Mod-123-1-0-1600000000.7z"),
            Some("2026-01-26T00:06:25Z"),
            Some(123),
            Some(4567),
        );
        match age {
            GuideAge::PreGuide { date, .. } => assert!(date.starts_with("2020"), "{date}"),
            other => panic!("expected the filename's own date, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod file_id_provenance_tests {
    use super::{classify_guide_age, GuideAge};

    #[test]
    fn a_file_id_without_a_mod_id_behind_it_proves_nothing() {
        // `meta.ini` fields are not provenance on their own -- Part 13's
        // `Oblivion Landskape` is a tesall.ru download whose meta.ini claims a
        // mod id. A stray small file id with no Nexus mod behind it must not
        // become "definitely older than the guide"; the honest answer is that
        // the age is unknown.
        let age = classify_guide_age(Some("Hand Named.7z"), None, Some(0), Some(4567));
        assert!(matches!(age, GuideAge::Unknown { .. }), "{age:?}");

        let age = classify_guide_age(Some("Hand Named.7z"), None, None, Some(4567));
        assert!(matches!(age, GuideAge::Unknown { .. }), "{age:?}");
    }
}
