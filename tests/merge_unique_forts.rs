//! M5 gate: merge "Unique Forts Merged" and compare against zMerge's output.
//!
//! Gated on `MUDCRAB_MOFAM_ROOT`:
//!
//!   MUDCRAB_MOFAM_ROOT=~/Games/Wabbajack/Oblivion/MOFAM-03.25 \
//!     cargo test --test merge_unique_forts -- --nocapture

use mudcrab::merge::{self, MergeRequest, MergeSource};
use mudcrab::plugin::{FormId, MasterTable, Origin, Plugin, PluginName, Record};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MERGE_NAME: &str = "Unique Forts Merged";

/// Source plugins in merge order, from zEdit's merges.json.
const SOURCES: &[&str] = &[
    "Unique Forts Fort Aurus.esp",
    "Unique Forts Fort Doublecross.esp",
    "Unique Forts Fort Facian.esp",
    "Unique Forts Fort Hastrel.esp",
    "Unique Forts Fort Irony.esp",
    "Unique Forts Fort Naso.esp",
    "Unique Forts Fort Rayles.esp",
    "Unique Forts Fort Redman.esp",
    "Unique Forts Fort Teleman.esp",
    "Unique Forts Fort Vlastarus.esp",
    "UFM Consistency Patch.esp",
];

/// The load order zMerge used, from merges.json.
const LOAD_ORDER: &[&str] = &[
    "Oblivion.esm",
    "xulCloudtopMountains.esp",
    "Unique Forts Fort Aurus.esp",
    "Unique Forts Fort Doublecross.esp",
    "Unique Forts Fort Facian.esp",
    "Unique Forts Fort Hastrel.esp",
    "Unique Forts Fort Irony.esp",
    "Unique Forts Fort Naso.esp",
    "Unique Forts Fort Rayles.esp",
    "Unique Forts Fort Redman.esp",
    "Unique Forts Fort Teleman.esp",
    "Unique Forts Fort Vlastarus.esp",
    "UFM Consistency Patch.esp",
];

fn mods_dir() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("MUDCRAB_MOFAM_ROOT")?);
    let mods = root.join("mods");
    mods.is_dir().then_some(mods)
}

/// Locate `<mods>/<any mod>/<filename>`, or its `.mohidden` form.
///
/// Never uses glob: plugin names contain glob metacharacters.
fn find_plugin(mods: &Path, filename: &str) -> Option<PathBuf> {
    let wanted = [
        filename.to_ascii_lowercase(),
        format!("{}.mohidden", filename.to_ascii_lowercase()),
    ];
    for entry in std::fs::read_dir(mods).ok()?.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        for file in std::fs::read_dir(entry.path()).ok()?.flatten() {
            let name = file.file_name().to_string_lossy().to_ascii_lowercase();
            if wanted.contains(&name) {
                return Some(file.path());
            }
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

#[test]
fn merges_unique_forts_equivalently_to_zmerge() {
    let Some(mods) = mods_dir() else {
        eprintln!("skipping: set MUDCRAB_MOFAM_ROOT to run against the real install");
        return;
    };

    // --- run our merge -------------------------------------------------
    let sources: Vec<MergeSource> = SOURCES
        .iter()
        .map(|name| MergeSource {
            plugin: PluginName::new(*name),
            path: find_plugin(&mods, name)
                .unwrap_or_else(|| panic!("source plugin not found: {name}")),
        })
        .collect();

    let request = MergeRequest {
        name: MERGE_NAME.to_string(),
        output: format!("{MERGE_NAME}.esp"),
        sources,
        load_order: LOAD_ORDER.iter().map(|n| PluginName::new(*n)).collect(),
    };

    let output = merge::run(&request).expect("merge should succeed");
    let ours = &output.plugin;
    let report = &output.report;

    eprintln!(
        "merged {} sources -> {} records, {} groups, {} remapped, {} clobbered",
        report.source_count,
        report.record_count,
        report.group_count,
        report.remapped,
        report.clobbered
    );

    // --- load zMerge's output ------------------------------------------
    let theirs_path = mods.join(MERGE_NAME).join(format!("{MERGE_NAME}.esp"));
    let theirs = Plugin::read(&theirs_path).expect("zMerge output should parse");

    // --- masters must match exactly ------------------------------------
    let our_masters: Vec<String> = ours
        .masters
        .masters()
        .iter()
        .map(|m| m.as_str().to_ascii_lowercase())
        .collect();
    let their_masters: Vec<String> = theirs
        .masters
        .masters()
        .iter()
        .map(|m| m.as_str().to_ascii_lowercase())
        .collect();
    assert_eq!(our_masters, their_masters, "master list differs");

    // --- build the inverse allocation maps ------------------------------
    let mut inverse = Inverse {
        by_new: BTreeMap::new(),
        kept: BTreeMap::new(),
    };
    for source in SOURCES {
        let plugin = Plugin::read(&find_plugin(&mods, source).unwrap()).unwrap();
        let name = PluginName::new(*source);
        let moved: BTreeMap<u32, u32> = output
            .allocation
            .remaps_for(&name)
            .map(|m| m.iter().map(|(k, v)| (*k, *v)).collect())
            .unwrap_or_default();
        for (old, new) in &moved {
            inverse
                .by_new
                .insert(*new, (source.to_ascii_lowercase(), *old));
        }
        for old in plugin.own_object_indices() {
            if !moved.contains_key(&old) {
                inverse
                    .kept
                    .entry(old)
                    .or_insert((source.to_ascii_lowercase(), old));
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

    let ours_only: Vec<&Canon> = our_records
        .keys()
        .filter(|k| !their_records.contains_key(k))
        .take(10)
        .collect();
    let theirs_only: Vec<&Canon> = their_records
        .keys()
        .filter(|k| !our_records.contains_key(k))
        .take(10)
        .collect();

    assert!(
        ours_only.is_empty() && theirs_only.is_empty(),
        "record sets differ.\n  only ours ({}): {ours_only:?}\n  only theirs ({}): {theirs_only:?}",
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
        "merged output defines the same FormID more than once: {dupes:?}"
    );

    // --- our output must contain no dangling references -----------------
    //
    // zMerge's own output fails this: 718 of its references carry the source
    // plugin's *load order position* as their mod index instead of the merged
    // plugin's own index, so they point past the master list at nothing. Those
    // are excluded from the comparison below rather than allowed to mask a
    // real difference, and this assertion keeps us honest about our own side.
    // See MOFAM-test/notes/zmerge-dangling-refs.md -- this must be re-checked
    // per merge in M7 before concluding zMerge is wrong in general.
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
        "our merged output has {} dangling reference(s): {:?}",
        our_dangling.len(),
        &our_dangling[..our_dangling.len().min(5)]
    );

    // --- tier 2: the reference graphs must match ------------------------
    let mut mismatched = Vec::new();
    let mut zmerge_defects = 0usize;
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
            zmerge_defects += broken;
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
        "{} record(s) have differing reference graphs:\n  {}",
        mismatched.len(),
        mismatched.join("\n  ")
    );

    eprintln!(
        "tier 2 passed: {} records, reference graphs identical\n\
         note: {zmerge_defects} reference(s) in zMerge's output point past its master \
         list and are repaired in ours",
        our_records.len()
    );
}
