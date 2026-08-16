//! Rebuild the GRUP tree for a merged plugin.
//!
//! Source groups cannot simply be concatenated. A cell's interior block and
//! sub-block are derived from its **object index**, so renumbering a CELL moves
//! it to a different block; copying the source structure would leave it in the
//! wrong one and the game would not find it.

use crate::plugin::record::FLAG_PERSISTENT;
use crate::plugin::{Entry, FormId, Group, GroupType, Record};
use indexmap::IndexMap;

/// Canonical order of top-level GRUPs.
///
/// Taken from the real merged outputs; the game and xEdit both expect this
/// order rather than the order records happened to arrive in.
pub const TOP_LEVEL_ORDER: &[[u8; 4]] = &[
    *b"GMST", *b"GLOB", *b"CLAS", *b"FACT", *b"HAIR", *b"EYES", *b"RACE", *b"SOUN", *b"SKIL",
    *b"MGEF", *b"SCPT", *b"LTEX", *b"ENCH", *b"SPEL", *b"BSGN", *b"ACTI", *b"APPA", *b"ARMO",
    *b"BOOK", *b"CLOT", *b"CONT", *b"DOOR", *b"INGR", *b"LIGH", *b"MISC", *b"STAT", *b"GRAS",
    *b"TREE", *b"FLOR", *b"FURN", *b"WEAP", *b"AMMO", *b"NPC_", *b"CREA", *b"LVLC", *b"SLGM",
    *b"KEYM", *b"ALCH", *b"SBSP", *b"SGST", *b"LVLI", *b"WTHR", *b"CLMT", *b"REGN", *b"CELL",
    *b"WRLD", *b"DIAL", *b"QUST", *b"IDLE", *b"PACK", *b"CSTY", *b"LSCR", *b"LVSP", *b"ANIO",
    *b"WATR", *b"EFSH",
];

/// Interior cells are bucketed by their object index, not their FormID.
///
/// Using the full FormID gives a different (wrong) block for every plugin the
/// cell has passed through.
pub fn interior_block(object_index: u32) -> (i32, i32) {
    let block = (object_index % 10) as i32;
    let sub_block = ((object_index / 10) % 10) as i32;
    (block, sub_block)
}

/// Exterior cells are bucketed by their grid coordinates.
///
/// Division must floor toward negative infinity: Cyrodiil has plenty of
/// negative grid coordinates and truncation would put them in the wrong block.
pub fn exterior_block(x: i32, y: i32) -> ((i16, i16), (i16, i16)) {
    let block = (y.div_euclid(32) as i16, x.div_euclid(32) as i16);
    let sub_block = (y.div_euclid(8) as i16, x.div_euclid(8) as i16);
    (block, sub_block)
}

/// A cell plus the children that travel with it.
struct CellBundle {
    cell: Record,
    persistent: Vec<Entry>,
    temporary: Vec<Entry>,
    visible_distant: Vec<Entry>,
}

impl CellBundle {
    fn new(cell: Record) -> Self {
        CellBundle {
            cell,
            persistent: Vec::new(),
            temporary: Vec::new(),
            visible_distant: Vec::new(),
        }
    }

    /// Place a child reference by its **persistent flag**, not by which group
    /// it arrived in. The flag is authoritative; a source file that disagrees
    /// is what we are trying to normalise.
    ///
    /// Children clobber by FormID like everything else: several sources
    /// commonly override the same vanilla reference in the same cell, and
    /// appending both would emit the record twice.
    fn push_child(&mut self, entry: Entry, from_group: i32) {
        let persistent = match &entry {
            Entry::Record(record) => record.flags & FLAG_PERSISTENT != 0,
            Entry::Group(_) => false,
        };
        let list = match (from_group, persistent) {
            (10, _) => &mut self.visible_distant,
            (_, true) => &mut self.persistent,
            (_, false) => &mut self.temporary,
        };

        if let Entry::Record(incoming) = &entry {
            let form_id = incoming.form_id;
            if let Some(slot) = list.iter_mut().find(
                |existing| matches!(existing, Entry::Record(r) if r.form_id == form_id),
            ) {
                *slot = entry;
                return;
            }
            // A reference may also have been filed under a different
            // persistence than this source claims; drop the stale copy.
            for other in [&mut self.persistent, &mut self.temporary, &mut self.visible_distant] {
                other.retain(|e| !matches!(e, Entry::Record(r) if r.form_id == form_id));
            }
            let list = match (from_group, persistent) {
                (10, _) => &mut self.visible_distant,
                (_, true) => &mut self.persistent,
                (_, false) => &mut self.temporary,
            };
            list.push(entry);
            return;
        }

        list.push(entry);
    }

    fn into_entries(self) -> Vec<Entry> {
        let cell_id = self.cell.form_id;
        let mut children = Vec::new();
        if !self.persistent.is_empty() {
            children.push(Entry::Group(Group::new(
                GroupType::CellPersistentChildren(cell_id),
                self.persistent,
            )));
        }
        if !self.temporary.is_empty() {
            children.push(Entry::Group(Group::new(
                GroupType::CellTemporaryChildren(cell_id),
                self.temporary,
            )));
        }
        if !self.visible_distant.is_empty() {
            children.push(Entry::Group(Group::new(
                GroupType::CellVisibleDistantChildren(cell_id),
                self.visible_distant,
            )));
        }

        let mut out = vec![Entry::Record(self.cell)];
        if !children.is_empty() {
            out.push(Entry::Group(Group::new(
                GroupType::CellChildren(cell_id),
                children,
            )));
        }
        out
    }
}

/// Everything gathered from the sources, keyed so later plugins clobber earlier.
#[derive(Default)]
pub struct Collected {
    /// Plain top-level records by signature, in insertion order.
    by_signature: IndexMap<[u8; 4], IndexMap<FormId, Record>>,
    /// Interior cells.
    interior_cells: IndexMap<FormId, CellBundle>,
    /// Exterior cells, grouped by their worldspace.
    exterior_cells: IndexMap<FormId, IndexMap<FormId, (CellBundle, (i32, i32))>>,
    /// Worldspace records themselves.
    worlds: IndexMap<FormId, Record>,
    /// Dialogue topics and their INFO children.
    dialogue: IndexMap<FormId, (Record, Vec<Entry>)>,
    /// Which cell currently holds each child reference.
    ///
    /// Clobbering has to be global, not per-cell: two sources can place the
    /// same override under *different* parent cells, and keeping both would
    /// emit the record twice.
    child_parent: IndexMap<FormId, FormId>,
    /// Which dialogue topic currently holds each INFO, for the same reason.
    info_parent: IndexMap<FormId, FormId>,
}

impl Collected {
    /// Absorb one rewritten source plugin.
    ///
    /// Records are keyed by post-rewrite FormID, so a later source silently
    /// replaces an earlier one -- which is exactly Clobber's last-writer-wins.
    pub fn absorb(&mut self, entries: Vec<Entry>) {
        for entry in entries {
            self.absorb_entry(entry, None);
        }
    }

    fn absorb_entry(&mut self, entry: Entry, world: Option<FormId>) {
        match entry {
            Entry::Record(record) => self.absorb_record(record, world),
            Entry::Group(group) => match group.group_type {
                GroupType::WorldChildren(parent) => {
                    for child in group.entries {
                        self.absorb_entry(child, Some(parent));
                    }
                }
                // Type 6 is a wrapper holding groups 8/9/10; recurse into it.
                GroupType::CellChildren(_) => {
                    for child in group.entries {
                        self.absorb_entry(child, world);
                    }
                }
                GroupType::CellPersistentChildren(parent)
                | GroupType::CellTemporaryChildren(parent)
                | GroupType::CellVisibleDistantChildren(parent) => {
                    let raw = group.group_type.raw_type();
                    for child in group.entries {
                        self.attach_child(parent, child, raw);
                    }
                }
                GroupType::TopicChildren(parent) => {
                    for child in group.entries {
                        self.attach_info(parent, child);
                    }
                }
                _ => {
                    for child in group.entries {
                        self.absorb_entry(child, world);
                    }
                }
            },
        }
    }

    fn absorb_record(&mut self, record: Record, world: Option<FormId>) {
        match &record.signature {
            b"CELL" => {
                let id = record.form_id;
                let grid = cell_grid(&record);
                // Several sources commonly override the same vanilla cell,
                // each adding their own references. Clobber replaces the cell
                // *record*; the children accumulate.
                match (world, grid) {
                    (Some(world_id), Some(grid)) => {
                        let cells = self.exterior_cells.entry(world_id).or_default();
                        match cells.get_mut(&id) {
                            Some((bundle, existing_grid)) => {
                                bundle.cell = record;
                                *existing_grid = grid;
                            }
                            None => {
                                cells.insert(id, (CellBundle::new(record), grid));
                            }
                        }
                    }
                    _ => match self.interior_cells.get_mut(&id) {
                        Some(bundle) => bundle.cell = record,
                        None => {
                            self.interior_cells.insert(id, CellBundle::new(record));
                        }
                    },
                }
            }
            b"WRLD" => {
                self.worlds.insert(record.form_id, record);
            }
            b"DIAL" => {
                // Same rule as cells: Clobber replaces the topic *record*, but
                // the INFO children accumulate across sources. Replacing the
                // whole entry discards every response an earlier source filed
                // under this topic.
                match self.dialogue.get_mut(&record.form_id) {
                    Some((existing, _)) => *existing = record,
                    None => {
                        self.dialogue.insert(record.form_id, (record, Vec::new()));
                    }
                }
            }
            signature => {
                self.by_signature
                    .entry(*signature)
                    .or_default()
                    .insert(record.form_id, record);
            }
        }
    }

    fn attach_child(&mut self, parent: FormId, child: Entry, from_group: i32) {
        // If this reference is already filed under a different cell, remove it
        // there first so the later source wins outright.
        if let Entry::Record(record) = &child {
            let form_id = record.form_id;
            if let Some(previous) = self.child_parent.get(&form_id).copied() {
                if previous != parent {
                    self.remove_child(previous, form_id);
                }
            }
            self.child_parent.insert(form_id, parent);
        }

        if let Some(bundle) = self.interior_cells.get_mut(&parent) {
            bundle.push_child(child, from_group);
            return;
        }
        for cells in self.exterior_cells.values_mut() {
            if let Some((bundle, _)) = cells.get_mut(&parent) {
                bundle.push_child(child, from_group);
                return;
            }
        }
        // A child whose cell is not part of the merge: keep it as a top-level
        // record so it is not silently dropped.
        if let Entry::Record(record) = child {
            self.absorb_record(record, None);
        }
    }

    /// File one INFO under its dialogue topic, last writer winning.
    ///
    /// Mirrors `attach_child`: the same response can be defined by several
    /// sources, and can even be attached to *different* topics by different
    /// sources, so clobbering has to be global rather than per-topic.
    fn attach_info(&mut self, parent: FormId, child: Entry) {
        if let Entry::Record(record) = &child {
            let form_id = record.form_id;
            if let Some(previous) = self.info_parent.get(&form_id).copied() {
                self.remove_info(previous, form_id);
            }
            self.info_parent.insert(form_id, parent);
        }

        if let Some((_, infos)) = self.dialogue.get_mut(&parent) {
            infos.push(child);
            return;
        }
        // A response whose topic no source defines. The file format puts the
        // DIAL immediately before its children group, so this should not
        // happen -- but keep the record rather than drop it silently.
        if let Entry::Record(record) = child {
            self.absorb_record(record, None);
        }
    }

    fn remove_info(&mut self, parent: FormId, form_id: FormId) {
        if let Some((_, infos)) = self.dialogue.get_mut(&parent) {
            infos.retain(|entry| !matches!(entry, Entry::Record(r) if r.form_id == form_id));
        }
    }

    fn remove_child(&mut self, parent: FormId, form_id: FormId) {
        let strip = |bundle: &mut CellBundle| {
            for list in [
                &mut bundle.persistent,
                &mut bundle.temporary,
                &mut bundle.visible_distant,
            ] {
                list.retain(|e| !matches!(e, Entry::Record(r) if r.form_id == form_id));
            }
        };
        if let Some(bundle) = self.interior_cells.get_mut(&parent) {
            strip(bundle);
            return;
        }
        for cells in self.exterior_cells.values_mut() {
            if let Some((bundle, _)) = cells.get_mut(&parent) {
                strip(bundle);
                return;
            }
        }
    }

    /// How many records were kept after clobbering.
    pub fn record_count(&self) -> usize {
        fn count(entries: &[Entry]) -> usize {
            entries
                .iter()
                .map(|e| match e {
                    Entry::Record(_) => 1,
                    Entry::Group(g) => count(&g.entries),
                })
                .sum()
        }
        let plain: usize = self.by_signature.values().map(IndexMap::len).sum();
        let interiors: usize = self
            .interior_cells
            .values()
            .map(|b| 1 + count(&b.persistent) + count(&b.temporary) + count(&b.visible_distant))
            .sum();
        let exteriors: usize = self
            .exterior_cells
            .values()
            .flat_map(IndexMap::values)
            .map(|(b, _)| {
                1 + count(&b.persistent) + count(&b.temporary) + count(&b.visible_distant)
            })
            .sum();
        let dialogue: usize = self.dialogue.values().map(|(_, i)| 1 + count(i)).sum();
        plain + interiors + exteriors + self.worlds.len() + dialogue
    }

    /// Emit the top-level GRUP tree in canonical order.
    pub fn build(self) -> Vec<Entry> {
        let mut out = Vec::new();

        for signature in TOP_LEVEL_ORDER {
            match signature {
                b"CELL" => {
                    if let Some(group) = self.build_interior_cells() {
                        out.push(group);
                    }
                }
                b"WRLD" => out.extend(self.build_worldspaces()),
                b"DIAL" => out.extend(self.build_dialogue()),
                _ => {
                    if let Some(records) = self.by_signature.get(signature) {
                        if !records.is_empty() {
                            out.push(Entry::Group(Group::new(
                                GroupType::TopLevel(*signature),
                                records.values().cloned().map(Entry::Record).collect(),
                            )));
                        }
                    }
                }
            }
        }

        out
    }

    fn build_interior_cells(&self) -> Option<Entry> {
        if self.interior_cells.is_empty() {
            return None;
        }

        // block -> sub-block -> cells, both ascending.
        let mut blocks: IndexMap<i32, IndexMap<i32, Vec<Entry>>> = IndexMap::new();
        for bundle in self.interior_cells.values() {
            let (block, sub_block) = interior_block(bundle.cell.form_id.object_index());
            blocks
                .entry(block)
                .or_default()
                .entry(sub_block)
                .or_default()
                .extend(clone_bundle(bundle).into_entries());
        }
        blocks.sort_keys();

        let mut block_entries = Vec::new();
        for (block, mut sub_blocks) in blocks {
            sub_blocks.sort_keys();
            let subs = sub_blocks
                .into_iter()
                .map(|(sub, entries)| {
                    Entry::Group(Group::new(GroupType::InteriorSubBlock(sub), entries))
                })
                .collect();
            block_entries.push(Entry::Group(Group::new(
                GroupType::InteriorBlock(block),
                subs,
            )));
        }

        Some(Entry::Group(Group::new(
            GroupType::TopLevel(*b"CELL"),
            block_entries,
        )))
    }

    fn build_worldspaces(&self) -> Option<Entry> {
        if self.worlds.is_empty() {
            return None;
        }

        let mut entries = Vec::new();
        for (world_id, world) in &self.worlds {
            entries.push(Entry::Record(world.clone()));

            let Some(cells) = self.exterior_cells.get(world_id) else {
                continue;
            };

            let mut blocks: IndexMap<(i16, i16), IndexMap<(i16, i16), Vec<Entry>>> =
                IndexMap::new();
            for (bundle, (x, y)) in cells.values() {
                let (block, sub_block) = exterior_block(*x, *y);
                blocks
                    .entry(block)
                    .or_default()
                    .entry(sub_block)
                    .or_default()
                    .extend(clone_bundle(bundle).into_entries());
            }
            blocks.sort_keys();

            let mut block_entries = Vec::new();
            for (block, mut subs) in blocks {
                subs.sort_keys();
                let sub_entries = subs
                    .into_iter()
                    .map(|((sy, sx), entries)| {
                        Entry::Group(Group::new(
                            GroupType::ExteriorSubBlock { y: sy, x: sx },
                            entries,
                        ))
                    })
                    .collect();
                block_entries.push(Entry::Group(Group::new(
                    GroupType::ExteriorBlock {
                        y: block.0,
                        x: block.1,
                    },
                    sub_entries,
                )));
            }

            entries.push(Entry::Group(Group::new(
                GroupType::WorldChildren(*world_id),
                block_entries,
            )));
        }

        Some(Entry::Group(Group::new(
            GroupType::TopLevel(*b"WRLD"),
            entries,
        )))
    }

    fn build_dialogue(&self) -> Option<Entry> {
        if self.dialogue.is_empty() {
            return None;
        }
        let mut entries = Vec::new();
        for (id, (record, infos)) in &self.dialogue {
            entries.push(Entry::Record(record.clone()));
            if !infos.is_empty() {
                entries.push(Entry::Group(Group::new(
                    GroupType::TopicChildren(*id),
                    infos.clone(),
                )));
            }
        }
        Some(Entry::Group(Group::new(
            GroupType::TopLevel(*b"DIAL"),
            entries,
        )))
    }
}

fn clone_bundle(bundle: &CellBundle) -> CellBundle {
    CellBundle {
        cell: bundle.cell.clone(),
        persistent: bundle.persistent.clone(),
        temporary: bundle.temporary.clone(),
        visible_distant: bundle.visible_distant.clone(),
    }
}

/// Grid coordinates from a CELL's XCLC, if it has one (exterior cells only).
fn cell_grid(record: &Record) -> Option<(i32, i32)> {
    let field = record.field(b"XCLC")?;
    if field.data.len() < 8 {
        return None;
    }
    Some((
        i32::from_le_bytes(field.data[0..4].try_into().ok()?),
        i32::from_le_bytes(field.data[4..8].try_into().ok()?),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{Group, Subrecord};

    fn dial(form_id: u32) -> Record {
        Record::new(b"DIAL", FormId(form_id), vec![Subrecord::new(b"EDID", b"t\0".to_vec())])
    }

    fn info(form_id: u32, marker: &str) -> Entry {
        Entry::Record(Record::new(
            b"INFO",
            FormId(form_id),
            vec![Subrecord::new(b"NAM1", format!("{marker}\0").into_bytes())],
        ))
    }

    /// One source's dialogue topic and the responses it files under it.
    fn topic(form_id: u32, infos: Vec<Entry>) -> Vec<Entry> {
        vec![
            Entry::Record(dial(form_id)),
            Entry::Group(Group::new(GroupType::TopicChildren(FormId(form_id)), infos)),
        ]
    }

    fn collected_infos(collected: &Collected, topic_id: u32) -> Vec<(FormId, String)> {
        collected.dialogue[&FormId(topic_id)]
            .1
            .iter()
            .filter_map(|entry| match entry {
                Entry::Record(record) => Some((
                    record.form_id,
                    record
                        .fields()
                        .iter()
                        .find(|f| &f.signature == b"NAM1")
                        .map(|f| {
                            String::from_utf8_lossy(&f.data).trim_end_matches('\0').to_string()
                        })
                        .unwrap_or_default(),
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn overriding_a_topic_keeps_the_responses_earlier_sources_filed_under_it() {
        // Two plugins both override the same vanilla topic, each adding its
        // own responses. Clobber replaces the DIAL *record*; the INFO children
        // have to accumulate, exactly as cell children do. Replacing the whole
        // entry silently loses the earlier plugin's dialogue.
        let mut collected = Collected::default();
        collected.absorb(topic(0x0100_0801, vec![info(0x0100_1000, "a")]));
        collected.absorb(topic(0x0100_0801, vec![info(0x0100_2000, "b")]));

        let infos = collected_infos(&collected, 0x0100_0801);
        assert_eq!(
            infos,
            vec![
                (FormId(0x0100_1000), "a".to_string()),
                (FormId(0x0100_2000), "b".to_string()),
            ]
        );
    }

    #[test]
    fn a_response_defined_twice_is_clobbered_not_duplicated() {
        let mut collected = Collected::default();
        collected.absorb(topic(0x0100_0801, vec![info(0x0100_1000, "first")]));
        collected.absorb(topic(0x0100_0801, vec![info(0x0100_1000, "second")]));

        assert_eq!(
            collected_infos(&collected, 0x0100_0801),
            vec![(FormId(0x0100_1000), "second".to_string())],
            "the later source must win outright"
        );
    }

    #[test]
    fn a_response_refiled_under_a_different_topic_does_not_appear_twice() {
        // The dialogue twin of the cell case: two sources can attach the same
        // INFO to different topics.
        let mut collected = Collected::default();
        collected.absorb(topic(0x0100_0801, vec![info(0x0100_1000, "first")]));
        collected.absorb(topic(0x0100_0802, vec![info(0x0100_1000, "second")]));

        assert!(collected_infos(&collected, 0x0100_0801).is_empty());
        assert_eq!(
            collected_infos(&collected, 0x0100_0802),
            vec![(FormId(0x0100_1000), "second".to_string())]
        );
    }

    #[test]
    fn interior_blocks_come_from_the_object_index() {
        // Verified against the real merged output: CELL 0x020026B6 has object
        // index 0x26B6 = 9910 -> block 0, sub-block 1.
        assert_eq!(interior_block(0x26B6), (0, 1));
        // Using the whole FormID would give 33565366 % 10 == 6, which is wrong.
        assert_ne!(interior_block(0x0200_26B6 & 0xFF_FFFF), (6, 0));
    }

    #[test]
    fn interior_blocks_cover_the_full_cycle() {
        for object_index in 0..100u32 {
            let (block, sub) = interior_block(object_index);
            assert_eq!(block, (object_index % 10) as i32);
            assert_eq!(sub, ((object_index / 10) % 10) as i32);
        }
    }

    #[test]
    fn exterior_blocks_floor_toward_negative_infinity() {
        // Truncation would map -1 to block 0, putting the cell in the wrong
        // group; Cyrodiil has many negative grid coordinates.
        assert_eq!(exterior_block(-1, -1), ((-1, -1), (-1, -1)));
        assert_eq!(exterior_block(0, 0), ((0, 0), (0, 0)));
        assert_eq!(exterior_block(31, 31), ((0, 0), (3, 3)));
        assert_eq!(exterior_block(32, 32), ((1, 1), (4, 4)));
        assert_eq!(exterior_block(-32, -32), ((-1, -1), (-4, -4)));
        assert_eq!(exterior_block(-33, -33), ((-2, -2), (-5, -5)));
    }

    #[test]
    fn top_level_order_contains_no_duplicates() {
        let mut seen = TOP_LEVEL_ORDER.to_vec();
        seen.sort();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "duplicate signature in TOP_LEVEL_ORDER");
    }

    #[test]
    fn top_level_order_matches_the_real_merged_output() {
        // The order observed in Unique Forts Merged.esp; every one of these
        // must appear, in this relative order.
        let observed: &[[u8; 4]] = &[
            *b"FACT", *b"SOUN", *b"SCPT", *b"ENCH", *b"ACTI", *b"BOOK", *b"CONT", *b"DOOR",
            *b"STAT", *b"WEAP", *b"NPC_", *b"KEYM", *b"CELL", *b"WRLD", *b"QUST", *b"PACK",
        ];
        let positions: Vec<usize> = observed
            .iter()
            .map(|sig| {
                TOP_LEVEL_ORDER
                    .iter()
                    .position(|s| s == sig)
                    .unwrap_or_else(|| panic!("{} missing", String::from_utf8_lossy(sig)))
            })
            .collect();
        assert!(
            positions.windows(2).all(|w| w[0] < w[1]),
            "canonical order disagrees with the real merged output: {positions:?}"
        );
    }
}
