//! M5/M7 gate: reproduce each of zMerge's six merges and compare semantically.
//!
//! Gated on `MUDCRAB_MOFAM_ROOT`:
//!
//!   MUDCRAB_MOFAM_ROOT=~/Games/Wabbajack/Oblivion/MOFAM-03.25 \
//!     cargo test --test merge_oracle -- --nocapture
//!
//! The merge definitions come from a committed distillation of zEdit's
//! `merges.json` (`tests/fixtures/merge/zmerge-definitions.json`), so the
//! oracle's own source list and load order drive the comparison rather than
//! whatever the modlist happens to say today. That distinction matters: the
//! installed Prebash merge contains `ORC.esp` from a mod version that has
//! since been upgraded, and comparing against the oracle means merging what
//! the oracle merged.
//!
//! Each source names its **mod folder**, not just a filename. Two mod folders
//! can hold the same plugin name -- `ORC.esp` exists in both the v180 and v194
//! folders -- so resolving by filename alone would silently merge the wrong
//! file and report a spurious difference. The fixture pins ORC to **v180**
//! even though merges.json names v194: the definition was updated when the ORC
//! upgrade began, but the installed Prebash merge was built from v180 and never
//! rebuilt. See MOFAM-test/notes/zmerge-dangling-refs.md.

use mudcrab::merge::{self, MergeRequest, MergeSource};
use mudcrab::plugin::{FormId, MasterTable, Origin, Plugin, PluginName, Record};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DEFINITIONS: &str = include_str!("fixtures/merge/zmerge-definitions.json");

#[derive(Debug, Deserialize)]
struct MergeDefinition {
    name: String,
    filename: String,
    method: String,
    plugins: Vec<SourceRef>,
    load_order: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SourceRef {
    #[serde(rename = "mod")]
    mod_folder: String,
    plugin: String,
}

fn definitions() -> Vec<MergeDefinition> {
    serde_json::from_str(DEFINITIONS).expect("merge definitions fixture should parse")
}

fn definition(name: &str) -> MergeDefinition {
    definitions()
        .into_iter()
        .find(|def| def.name == name)
        .unwrap_or_else(|| panic!("no merge definition named {name}"))
}

fn mods_dir() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("MUDCRAB_MOFAM_ROOT")?);
    let mods = root.join("mods");
    mods.is_dir().then_some(mods)
}

/// Resolve `<mods>/<mod folder>/<plugin>`, accepting the `.mohidden` form.
///
/// Never uses glob: plugin names contain glob metacharacters -- `Harvest
/// [Flora] - DLCFrostcrag.esp` reads as a character class and matches nothing.
fn find_plugin(mods: &Path, source: &SourceRef) -> Option<PathBuf> {
    let dir = mods.join(&source.mod_folder);
    let wanted = [
        source.plugin.to_ascii_lowercase(),
        format!("{}.mohidden", source.plugin.to_ascii_lowercase()),
    ];
    for file in std::fs::read_dir(&dir).ok()?.flatten() {
        let name = file.file_name().to_string_lossy().to_ascii_lowercase();
        if wanted.contains(&name) {
            return Some(file.path());
        }
    }
    None
}

/// Canonical identity of a FormID: which plugin originally defined it, and as
/// what object index. Independent of any particular numbering, so two valid
/// merges that number differently still compare equal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Canon {
    Master { plugin: String, object: u32 },
    Origin { plugin: String, object: u32 },
    Null,
    Unresolved(u32),
}

/// Invert an allocation so a merged object index can be traced back to its source.
struct Inverse {
    by_new: BTreeMap<u32, (String, u32)>,
    kept: BTreeMap<u32, (String, u32)>,
}

fn canonicalize(form_id: FormId, masters: &MasterTable, inverse: &Inverse) -> Canon {
    if form_id.is_null() {
        return Canon::Null;
    }
    match masters.resolve(form_id) {
        Some(Origin::Master { plugin, object_index }) => Canon::Master {
            plugin: plugin.as_str().to_ascii_lowercase(),
            object: object_index,
        },
        Some(Origin::Own { object_index }) => {
            let source = inverse
                .by_new
                .get(&object_index)
                .or_else(|| inverse.kept.get(&object_index));
            match source {
                Some((plugin, object)) => Canon::Origin {
                    plugin: plugin.clone(),
                    object: *object,
                },
                None => Canon::Unresolved(object_index),
            }
        }
        None => Canon::Unresolved(form_id.0),
    }
}

/// Every canonical reference a record makes, as a sorted multiset.
///
/// Deliberately order-insensitive. Some array-valued fields -- CELL/XCLR's
/// region list is the clearest case -- are semantically sets, and zEdit emits
/// them ascending while we preserve source order. That is a serialization
/// choice, like the trailing NUL zEdit adds to SCTX, not a difference in what
/// the record means. Comparing as a multiset still catches a missing, extra or
/// wrongly-targeted reference, which is what this tier is for.
fn reference_edges(record: &Record, masters: &MasterTable, inverse: &Inverse) -> Vec<Canon> {
    let mut out = Vec::new();
    mudcrab::plugin::schema::visit_form_ids(record, |form_id| {
        out.push(canonicalize(form_id, masters, inverse));
    })
    .expect("schema must describe every field in the merged output");
    out.sort();
    out
}

/// What comparing one merge against zMerge's output established.
struct Verdict {
    records: usize,
    clobbered: usize,
    remapped: usize,
    /// References in *zMerge's* output whose mod index is greater than its own,
    /// so they address a master that does not exist.
    beyond_masters: usize,
    /// References in zMerge's output that claim to be its own records but name
    /// an object index it never allocated. A different defect from the above,
    /// and worth counting separately -- conflating them hides the pattern.
    unknown_own: usize,
}

/// Classify every reference in zMerge's output that resolves to nothing.
///
/// Returns `(beyond_masters, unknown_own, samples)`. The samples exist so the
/// claim "zMerge is wrong here" can be checked by hand in TES4Edit rather than
/// taken on trust -- being different from a suspect oracle is not the same as
/// being right.
fn zmerge_broken_references(
    theirs: &Plugin,
    inverse: &Inverse,
    def: &MergeDefinition,
) -> (usize, usize, Vec<String>) {
    let own_index = theirs.masters.own_mod_index();
    let (mut beyond, mut unknown) = (0usize, 0usize);
    let mut samples = Vec::new();

    for record in theirs.records() {
        mudcrab::plugin::schema::visit_form_ids(record, |form_id| {
            if form_id.is_null() {
                return;
            }
            if form_id.mod_index() > own_index {
                beyond += 1;
                if samples.len() < 4 {
                    // Is the bad mod index the source plugin's position in the
                    // load order? That was the pattern in Unique Forts.
                    let index = form_id.mod_index() as usize;
                    let at_position = def
                        .load_order
                        .get(index)
                        .map(String::as_str)
                        .unwrap_or("<past the load order>");
                    samples.push(format!(
                        "{} {} -> {form_id} (mod index {index}; load order[{index}] = {at_position})",
                        record.sig_str(),
                        record.form_id
                    ));
                }
            } else if form_id.mod_index() == own_index {
                let object = form_id.object_index();
                if !inverse.by_new.contains_key(&object) && !inverse.kept.contains_key(&object) {
                    unknown += 1;
                }
            }
        })
        .expect("schema coverage");
    }
    (beyond, unknown, samples)
}

fn compare_against_zmerge(name: &str) -> Option<Verdict> {
    let mods = mods_dir()?;
    let def = definition(name);
    assert_eq!(def.method, "Clobber", "{name}: only Clobber is implemented");

    // --- run our merge -------------------------------------------------
    let sources: Vec<MergeSource> = def
        .plugins
        .iter()
        .map(|source| MergeSource {
            plugin: PluginName::new(&source.plugin),
            path: find_plugin(&mods, source).unwrap_or_else(|| {
                panic!(
                    "{name}: source plugin not found: {}/{}",
                    source.mod_folder, source.plugin
                )
            }),
        })
        .collect();

    let request = MergeRequest {
        name: def.name.clone(),
        output: def.filename.clone(),
        sources,
        load_order: def.load_order.iter().map(PluginName::new).collect(),
    };

    let output = merge::run(&request).unwrap_or_else(|err| panic!("{name}: merge failed: {err}"));
    let ours = &output.plugin;
    let report = &output.report;

    eprintln!(
        "\n{name}: merged {} sources -> {} records, {} groups, {} remapped, {} clobbered",
        report.source_count,
        report.record_count,
        report.group_count,
        report.remapped,
        report.clobbered
    );

    // --- load zMerge's output ------------------------------------------
    let theirs_path = mods.join(&def.name).join(&def.filename);
    let theirs = Plugin::read(&theirs_path)
        .unwrap_or_else(|err| panic!("{name}: zMerge output should parse: {err}"));

    // --- masters: ours must be a subset, ordered by the load order ------
    //
    // Not equality. For Late Loaders and NPC Merge, zMerge's master list is a
    // strict superset and is *not* monotonic in the load order merges.json
    // records -- NPC Merge's even names a plugin absent from that load order
    // entirely. The recorded load order is therefore stale relative to when
    // those merges were built, so their exact master list is not reproducible
    // from what we have. An unused extra master is harmless: it shifts mod
    // indices, and tier 2 is immune to that by construction.
    //
    // What must still hold, and does catch real bugs: we never require a
    // master zMerge did not, and our own ordering follows the load order.
    let lowercase = |plugin: &Plugin| -> Vec<String> {
        plugin
            .masters
            .masters()
            .iter()
            .map(|m| m.as_str().to_ascii_lowercase())
            .collect()
    };
    let ours_masters = lowercase(ours);
    let theirs_masters = lowercase(&theirs);

    let invented: Vec<&String> = ours_masters
        .iter()
        .filter(|m| !theirs_masters.contains(m))
        .collect();
    assert!(
        invented.is_empty(),
        "{name}: we require masters zMerge did not: {invented:?}"
    );

    let load_order_position = |plugin: &str| {
        def.load_order
            .iter()
            .position(|entry| entry.eq_ignore_ascii_case(plugin))
    };
    let positions: Vec<Option<usize>> = ours_masters.iter().map(|m| load_order_position(m)).collect();
    assert!(
        positions.windows(2).all(|w| w[0] <= w[1]),
        "{name}: our master list is not in load order: {ours_masters:?}"
    );

    let extra: Vec<&String> = theirs_masters
        .iter()
        .filter(|m| !ours_masters.contains(m))
        .collect();
    if !extra.is_empty() {
        eprintln!(
            "{name}: zMerge carries {} master(s) no source requires: {extra:?}",
            extra.len()
        );
    }

    // --- build the inverse allocation maps ------------------------------
    let mut inverse = Inverse {
        by_new: BTreeMap::new(),
        kept: BTreeMap::new(),
    };
    for source in &def.plugins {
        let plugin = Plugin::read(&find_plugin(&mods, source).unwrap()).unwrap();
        let plugin_name = PluginName::new(&source.plugin);
        let key = source.plugin.to_ascii_lowercase();
        let moved: BTreeMap<u32, u32> = output
            .allocation
            .remaps_for(&plugin_name)
            .map(|m| m.iter().map(|(k, v)| (*k, *v)).collect())
            .unwrap_or_default();
        for (old, new) in &moved {
            inverse.by_new.insert(*new, (key.clone(), *old));
        }
        for old in plugin.own_object_indices() {
            if !moved.contains_key(&old) {
                inverse.kept.entry(old).or_insert((key.clone(), old));
            }
        }
    }

    // --- tier 2: record sets must match canonically ---------------------
    let our_records: BTreeMap<Canon, &Record> = ours
        .records()
        .map(|r| (canonicalize(r.form_id, &ours.masters, &inverse), r))
        .collect();
    let their_records: BTreeMap<Canon, &Record> = theirs
        .records()
        .map(|r| (canonicalize(r.form_id, &theirs.masters, &inverse), r))
        .collect();

    // Aggregate the symmetric difference by originating plugin. A raw list of
    // FormIDs says nothing useful at this scale; "all 29 came from orc.esp"
    // immediately distinguishes a merge bug from a source that changed on disk
    // since zMerge ran.
    let origin_of = |canon: &Canon| -> String {
        match canon {
            Canon::Master { plugin, .. } => format!("master:{plugin}"),
            Canon::Origin { plugin, .. } => plugin.clone(),
            Canon::Null => "null".to_string(),
            Canon::Unresolved(_) => "unresolved".to_string(),
        }
    };
    let tally = |keys: Vec<(&Canon, &&Record)>| -> Vec<(String, usize)> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for (key, record) in keys {
            *counts
                .entry(format!("{} {}", origin_of(key), record.sig_str()))
                .or_default() += 1;
        }
        let mut rows: Vec<(String, usize)> = counts.into_iter().collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1));
        rows
    };

    let ours_only = tally(
        our_records
            .iter()
            .filter(|(k, _)| !their_records.contains_key(*k))
            .collect(),
    );
    let theirs_only = tally(
        their_records
            .iter()
            .filter(|(k, _)| !our_records.contains_key(*k))
            .collect(),
    );

    assert!(
        ours_only.is_empty() && theirs_only.is_empty(),
        "{name}: record sets differ (ours {}, theirs {}).\n  \
         only ours, by origin:   {ours_only:?}\n  only theirs, by origin: {theirs_only:?}",
        our_records.len(),
        their_records.len()
    );

    // --- no FormID may appear twice -------------------------------------
    let mut seen: BTreeMap<FormId, usize> = BTreeMap::new();
    for record in ours.records() {
        *seen.entry(record.form_id).or_default() += 1;
    }
    let dupes: Vec<(FormId, usize)> = seen
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .take(5)
        .collect();
    assert!(
        dupes.is_empty(),
        "{name}: merged output defines the same FormID more than once: {dupes:?}"
    );

    // --- our output must contain no dangling references -----------------
    //
    // zMerge's Unique Forts output fails this: 718 of its references carry the
    // source plugin's *load order position* as their mod index instead of the
    // merged plugin's own index, so they point past the master list at
    // nothing. Those are excluded from the comparison below by count rather
    // than allowed to mask a real difference, and this assertion keeps us
    // honest about our own side.
    // See MOFAM-test/notes/zmerge-dangling-refs.md.
    let our_own_index = ours.masters.own_mod_index();
    let mut our_dangling = Vec::new();
    for record in ours.records() {
        mudcrab::plugin::schema::visit_form_ids(record, |form_id| {
            if !form_id.is_null() && form_id.mod_index() > our_own_index {
                our_dangling.push(format!("{} -> {form_id}", record.form_id));
            }
        })
        .expect("schema coverage");
    }
    assert!(
        our_dangling.is_empty(),
        "{name}: our merged output has {} dangling reference(s): {:?}",
        our_dangling.len(),
        &our_dangling[..our_dangling.len().min(5)]
    );

    // --- tier 2: the reference graphs must match ------------------------
    let mut mismatched = Vec::new();
    let mut zmerge_dangling = 0usize;
    for (canon, our_record) in &our_records {
        let their_record = &their_records[canon];
        if our_record.signature != their_record.signature {
            mismatched.push(format!("{canon:?}: signature differs"));
            continue;
        }
        let ours_edges = reference_edges(our_record, &ours.masters, &inverse);
        let mut theirs_edges = reference_edges(their_record, &theirs.masters, &inverse);

        // Drop zMerge's broken references, and the matching count of ours, so
        // its defect cannot hide a real disagreement in the rest.
        let broken = theirs_edges
            .iter()
            .filter(|e| matches!(e, Canon::Unresolved(_)))
            .count();
        if broken > 0 {
            zmerge_dangling += broken;
            theirs_edges.retain(|e| !matches!(e, Canon::Unresolved(_)));
            let mut kept = ours_edges.clone();
            for edge in &theirs_edges {
                if let Some(pos) = kept.iter().position(|e| e == edge) {
                    kept.remove(pos);
                }
            }
            // whatever ours has beyond theirs should be exactly the repaired refs
            if kept.len() != broken {
                mismatched.push(format!(
                    "{canon:?} ({}): zMerge has {broken} broken ref(s) but ours differs by {}",
                    our_record.sig_str(),
                    kept.len()
                ));
            }
            continue;
        }

        if ours_edges != theirs_edges {
            let only_ours: Vec<_> = ours_edges
                .iter()
                .filter(|e| !theirs_edges.contains(e))
                .take(3)
                .collect();
            let only_theirs: Vec<_> = theirs_edges
                .iter()
                .filter(|e| !ours_edges.contains(e))
                .take(3)
                .collect();
            mismatched.push(format!(
                "{canon:?} ({}):\n      ours only:   {only_ours:?}\n      theirs only: {only_theirs:?}",
                our_record.sig_str(),
            ));
        }
        if mismatched.len() >= 3 {
            break;
        }
    }

    assert!(
        mismatched.is_empty(),
        "{name}: {} record(s) have differing reference graphs:\n  {}",
        mismatched.len(),
        mismatched.join("\n  ")
    );

    let (beyond_masters, unknown_own, samples) = zmerge_broken_references(&theirs, &inverse, &def);
    assert_eq!(
        beyond_masters + unknown_own,
        zmerge_dangling,
        "{name}: the two ways of counting zMerge's broken references disagree"
    );
    eprintln!(
        "{name}: tier 2 passed -- {} records, reference graphs identical; \
         zMerge broken refs: {beyond_masters} beyond masters, {unknown_own} unknown own",
        our_records.len()
    );

    for sample in &samples {
        eprintln!("{name}:   broken in zMerge: {sample}");
    }

    Some(Verdict {
        records: our_records.len(),
        clobbered: report.clobbered,
        remapped: report.remapped,
        beyond_masters,
        unknown_own,
    })
}

macro_rules! oracle_test {
    ($fn_name:ident, $merge:expr) => {
        #[test]
        fn $fn_name() {
            if compare_against_zmerge($merge).is_none() {
                eprintln!("skipping: set MUDCRAB_MOFAM_ROOT to run against the real install");
            }
        }
    };
}

// In the difficulty order the plan established: Unique Forts first (small
// enough to diff by hand but exercising CELL re-blocking, PGRD, worldspace
// children and compressed records), Prebash last (86 plugins, mostly
// DIAL/INFO).
oracle_test!(merges_unique_forts, "Unique Forts Merged");
oracle_test!(merges_tace, "TACE Merge");
oracle_test!(merges_npcs, "NPC Merge");
oracle_test!(merges_late_loaders, "Late Loaders Merged");
oracle_test!(merges_ooo_patches, "OOO Patches Merged");
oracle_test!(merges_prebash, "Prebash Merge");

/// Answer the open question in MOFAM-test/notes/zmerge-dangling-refs.md: is
/// zMerge's dangling-reference defect confined to Unique Forts, confined to
/// the two merges that renumber FormIDs, or systemic?
///
/// This is a report, not a pass/fail assertion -- it prints a table. The
/// per-merge tests above are what gate correctness.
#[test]
fn reports_zmerge_dangling_references_across_all_merges() {
    if mods_dir().is_none() {
        eprintln!("skipping: set MUDCRAB_MOFAM_ROOT to run against the real install");
        return;
    }

    let mut rows = Vec::new();
    for def in definitions() {
        if let Some(verdict) = compare_against_zmerge(&def.name) {
            rows.push((def.name, verdict));
        }
    }

    eprintln!("\n=== zMerge dangling references ===");
    eprintln!(
        "{:<22} {:>8} {:>9} {:>9} {:>9} {:>9}",
        "merge", "records", "remapped", "clobbered", "beyond", "unknown"
    );
    for (name, verdict) in &rows {
        eprintln!(
            "{:<22} {:>8} {:>9} {:>9} {:>9} {:>9}",
            name,
            verdict.records,
            verdict.remapped,
            verdict.clobbered,
            verdict.beyond_masters,
            verdict.unknown_own
        );
    }

    let affected: Vec<&str> = rows
        .iter()
        .filter(|(_, v)| v.beyond_masters + v.unknown_own > 0)
        .map(|(name, _)| name.as_str())
        .collect();
    let renumbering: Vec<&str> = rows
        .iter()
        .filter(|(_, v)| v.remapped > 0)
        .map(|(name, _)| name.as_str())
        .collect();

    eprintln!("\naffected:    {affected:?}");
    eprintln!("renumbering: {renumbering:?}");
    eprintln!(
        "verdict:     {}",
        if affected.is_empty() {
            "no defect reproduced".to_string()
        } else if affected == renumbering {
            "confined to the merges that renumber FormIDs -- points at zMerge's \
             renumbering path".to_string()
        } else if affected.len() == rows.len() {
            "systemic across all merges".to_string()
        } else {
            format!("confined to {affected:?} -- no clean pattern; investigate individually")
        }
    );
}
