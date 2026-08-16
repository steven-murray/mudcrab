//! M4 gate: reproduce zMerge's FormID allocation exactly.
//!
//! zEdit writes a `map.json` beside each merge it builds, recording the old ->
//! new object index for every record it renumbered. Those files are committed
//! here verbatim as the expected output, alongside the allocator's inputs
//! extracted from the same install, so this runs without the game present.
//!
//! Regenerate the fixtures from a real install with the snippet in
//! `MOFAM-test/notes/merge-recon.md`.

use mudcrab::merge::alloc::allocate;
use mudcrab::plugin::PluginName;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/merge")
}

/// Parse an allocator input file: `@plugin.esp` lines followed by 6-hex indices.
fn read_input(path: &Path) -> Vec<(PluginName, BTreeSet<u32>)> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("reading {}: {err}", path.display()));

    let mut sources: Vec<(PluginName, BTreeSet<u32>)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('@') {
            sources.push((PluginName::new(name), BTreeSet::new()));
            continue;
        }
        let index = u32::from_str_radix(line, 16)
            .unwrap_or_else(|err| panic!("{}: bad index {line:?}: {err}", path.display()));
        sources
            .last_mut()
            .unwrap_or_else(|| panic!("{}: index before any @plugin line", path.display()))
            .1
            .insert(index);
    }
    sources
}

/// zMerge's map.json: `{ "<plugin>.esp": { "OLDHEX": "NEWHEX", ... }, ... }`.
fn read_expected(path: &Path) -> BTreeMap<String, BTreeMap<u32, u32>> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("reading {}: {err}", path.display()));
    let raw: BTreeMap<String, BTreeMap<String, String>> =
        serde_json::from_str(&text).unwrap_or_else(|err| panic!("{}: {err}", path.display()));

    raw.into_iter()
        .map(|(plugin, entries)| {
            let parsed = entries
                .into_iter()
                .map(|(old, new)| {
                    (
                        u32::from_str_radix(&old, 16).expect("hex old index"),
                        u32::from_str_radix(&new, 16).expect("hex new index"),
                    )
                })
                .collect();
            (plugin.to_ascii_lowercase(), parsed)
        })
        .collect()
}

fn check_merge(slug: &str) {
    let dir = fixture_dir();
    let sources = read_input(&dir.join(format!("{slug}.input.txt")));
    let expected = read_expected(&dir.join(format!("{slug}.expected.json")));

    assert!(!sources.is_empty(), "{slug}: no source plugins in fixture");

    let allocation = allocate(&sources);

    for (plugin, _) in &sources {
        let key = plugin.as_str().to_ascii_lowercase();
        let want = expected
            .get(&key)
            .unwrap_or_else(|| panic!("{slug}: {plugin} missing from zMerge's map.json"));
        let got: BTreeMap<u32, u32> = allocation
            .remaps_for(plugin)
            .map(|m| m.iter().map(|(k, v)| (*k, *v)).collect())
            .unwrap_or_default();

        if &got != want {
            let mismatches: Vec<String> = want
                .iter()
                .filter(|(old, new)| got.get(old) != Some(new))
                .take(5)
                .map(|(old, new)| {
                    format!("{old:06X}: expected {new:06X}, got {:06X}", allocation.map(plugin, *old))
                })
                .collect();
            panic!(
                "{slug}: {plugin} differs ({} expected vs {} produced)\n  {}",
                want.len(),
                got.len(),
                mismatches.join("\n  ")
            );
        }
    }

    // and nothing was invented for a plugin zMerge did not list
    let listed: BTreeSet<String> = sources
        .iter()
        .map(|(p, _)| p.as_str().to_ascii_lowercase())
        .collect();
    for plugin in expected.keys() {
        assert!(listed.contains(plugin), "{slug}: unexpected plugin {plugin}");
    }
}

#[test]
fn reproduces_zmerge_allocation_for_unique_forts() {
    // 2004 remaps -- the heaviest renumbering of the six.
    check_merge("unique-forts-merged");
}

#[test]
fn reproduces_zmerge_allocation_for_tace() {
    // 1170 remaps.
    check_merge("tace-merge");
}

#[test]
fn reproduces_zmerge_allocation_for_ooo_patches() {
    check_merge("ooo-patches-merged");
}

#[test]
fn reproduces_zmerge_allocation_for_late_loaders() {
    check_merge("late-loaders-merged");
}

#[test]
fn reproduces_zmerge_allocation_for_npc_merge() {
    check_merge("npc-merge");
}

#[test]
fn reproduces_zmerge_allocation_for_prebash() {
    // 86 source plugins, the largest merge in MOFAM.
    check_merge("prebash-merge");
}

#[test]
fn every_committed_merge_fixture_is_covered_by_a_test() {
    let mut slugs: Vec<String> = std::fs::read_dir(fixture_dir())
        .expect("fixture dir should exist")
        .flatten()
        .filter_map(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .strip_suffix(".input.txt")
                .map(str::to_string)
        })
        .collect();
    slugs.sort();

    assert_eq!(
        slugs,
        vec![
            "late-loaders-merged",
            "npc-merge",
            "ooo-patches-merged",
            "prebash-merge",
            "tace-merge",
            "unique-forts-merged",
        ],
        "a fixture was added or removed without updating the tests above"
    );
}
