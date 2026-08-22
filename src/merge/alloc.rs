//! FormID allocation: decide each source record's object index in the merge.
//!
//! The algorithm below reproduces zEdit/zMerge's `map.json` **exactly** for all
//! six MOFAM merges, including the two that actually renumber (Unique Forts,
//! 2004 remaps; TACE, 1170). It was derived by measurement, not from
//! documentation -- see `docs/design/merge-engine.md`.

use crate::plugin::PluginName;
use indexmap::IndexMap;
use std::collections::{BTreeMap, BTreeSet};

/// The first object index a plugin may allocate. Below this is reserved.
pub const FIRST_FREE_OBJECT_INDEX: u32 = 0x0000_0801;

/// Which source object indices moved, and where to.
///
/// Only records that actually moved appear. A plugin whose indices were all
/// free keeps them and contributes an empty map -- matching zMerge, whose
/// `map.json` has `{}` for such plugins.
#[derive(Debug, Clone, Default)]
pub struct Allocation {
    remaps: IndexMap<PluginName, BTreeMap<u32, u32>>,
}

impl Allocation {
    /// The object index `old` ends up at for `plugin`; unchanged if it did not move.
    pub fn map(&self, plugin: &PluginName, old: u32) -> u32 {
        self.remaps
            .get(plugin)
            .and_then(|m| m.get(&old))
            .copied()
            .unwrap_or(old)
    }

    pub fn remaps_for(&self, plugin: &PluginName) -> Option<&BTreeMap<u32, u32>> {
        self.remaps.get(plugin)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PluginName, &BTreeMap<u32, u32>)> {
        self.remaps.iter()
    }

    pub fn total_remapped(&self) -> usize {
        self.remaps.values().map(BTreeMap::len).sum()
    }

    /// Highest object index in use, for the merged header's `nextObjectID`.
    pub fn highest_object_index(&self) -> Option<u32> {
        self.remaps.values().flat_map(BTreeMap::values).max().copied()
    }
}

/// Assign object indices for a merge.
///
/// `sources` must be in merge order, each carrying that plugin's own object
/// indices. Those must be **deduplicated and ascending**, which is what
/// `Plugin::own_object_indices` returns:
///
/// - ascending, because iterating in file order produces a different (wrong)
///   assignment than zMerge's;
/// - deduplicated, because real plugins contain duplicate FormIDs -- Fort
///   Vlastarus has three -- and each duplicate would otherwise consume an extra
///   index and shift everything after it.
pub fn allocate(sources: &[(PluginName, BTreeSet<u32>)]) -> Allocation {
    let mut used: BTreeSet<u32> = BTreeSet::new();
    let mut cursor = FIRST_FREE_OBJECT_INDEX;
    let mut remaps = IndexMap::new();

    for (plugin, own_indices) in sources {
        let mut moved = BTreeMap::new();

        for &old in own_indices {
            // Free: keep the original index.
            if used.insert(old) {
                continue;
            }
            // Taken: take the next free index. The cursor is monotonic across
            // plugins and never reset, which is what produces the exact gaps
            // zMerge leaves.
            while !used.insert(cursor) {
                cursor += 1;
            }
            moved.insert(old, cursor);
            cursor += 1;
        }

        remaps.insert(plugin.clone(), moved);
    }

    Allocation { remaps }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(values: &[u32]) -> BTreeSet<u32> {
        values.iter().copied().collect()
    }

    #[test]
    fn first_plugin_keeps_its_indices() {
        let alloc = allocate(&[("a.esp".into(), set(&[0x801, 0x802, 0x900]))]);
        assert_eq!(alloc.total_remapped(), 0);
        assert_eq!(alloc.map(&"a.esp".into(), 0x900), 0x900);
    }

    #[test]
    fn colliding_indices_move_to_the_next_free_slot() {
        let alloc = allocate(&[
            ("a.esp".into(), set(&[0x801, 0x802])),
            ("b.esp".into(), set(&[0x801, 0x802])),
        ]);
        let b: PluginName = "b.esp".into();
        // 0x801/0x802 are taken, so b's records land immediately after.
        assert_eq!(alloc.map(&b, 0x801), 0x803);
        assert_eq!(alloc.map(&b, 0x802), 0x804);
        assert_eq!(alloc.total_remapped(), 2);
    }

    #[test]
    fn non_colliding_indices_are_kept_even_in_later_plugins() {
        let alloc = allocate(&[
            ("a.esp".into(), set(&[0x801])),
            ("b.esp".into(), set(&[0x801, 0x999])),
        ]);
        let b: PluginName = "b.esp".into();
        assert_eq!(alloc.map(&b, 0x801), 0x802, "collision moves");
        assert_eq!(alloc.map(&b, 0x999), 0x999, "no collision, index kept");
    }

    #[test]
    fn cursor_skips_indices_already_claimed_by_later_plugins() {
        // b keeps 0x803, so when c collides the cursor must step over it.
        let alloc = allocate(&[
            ("a.esp".into(), set(&[0x801])),
            ("b.esp".into(), set(&[0x803])),
            ("c.esp".into(), set(&[0x801, 0x803])),
        ]);
        let c: PluginName = "c.esp".into();
        assert_eq!(alloc.map(&c, 0x801), 0x802);
        assert_eq!(alloc.map(&c, 0x803), 0x804, "0x803 was taken by b");
    }

    #[test]
    fn allocation_is_stable_regardless_of_input_ordering_within_a_plugin() {
        // BTreeSet guarantees ascending iteration, so a caller cannot
        // accidentally feed file order and get a different answer.
        let ascending = allocate(&[
            ("a.esp".into(), set(&[0x801])),
            ("b.esp".into(), set(&[0x801, 0x802, 0x803])),
        ]);
        let shuffled = allocate(&[
            ("a.esp".into(), set(&[0x801])),
            ("b.esp".into(), set(&[0x803, 0x801, 0x802])),
        ]);
        let b: PluginName = "b.esp".into();
        for old in [0x801, 0x802, 0x803] {
            assert_eq!(ascending.map(&b, old), shuffled.map(&b, old));
        }
    }

    #[test]
    fn plugins_with_no_moves_are_recorded_with_an_empty_map() {
        // zMerge's map.json contains `"Fort Aurus.esp": {}`; matching that
        // shape keeps the oracle comparison exact.
        let alloc = allocate(&[("a.esp".into(), set(&[0x801]))]);
        assert_eq!(alloc.remaps_for(&"a.esp".into()), Some(&BTreeMap::new()));
    }
}
