//! Rewrite every FormID in a source plugin into the merged plugin's numbering.

use super::alloc::Allocation;
use crate::plugin::schema::{self, SchemaError};
use crate::plugin::{Entry, FormId, MasterTable, PluginName};
use std::cell::Cell;

#[derive(Debug, thiserror::Error)]
pub enum RewriteError {
    #[error("{plugin}: {source}")]
    Schema {
        plugin: PluginName,
        source: SchemaError,
    },

    #[error(
        "{plugin}: reference {form_id} names master index {mod_index} of {master_count}, but \
         that master is missing from the merged plugin's master list.\n\
         This is a mudcrab bug in merge::masters, not a problem with the input."
    )]
    MasterNotInMergedList {
        plugin: PluginName,
        form_id: FormId,
        mod_index: u8,
        master_count: usize,
    },
}

/// How one source plugin's FormIDs map into the merged plugin.
pub struct Remapper<'a> {
    plugin: &'a PluginName,
    source_masters: &'a MasterTable,
    merged_masters: &'a MasterTable,
    merged_set: &'a [PluginName],
    allocation: &'a Allocation,
    /// References whose mod index ran past the source's master list.
    non_canonical: Cell<usize>,
}

impl<'a> Remapper<'a> {
    pub fn new(
        plugin: &'a PluginName,
        source_masters: &'a MasterTable,
        merged_masters: &'a MasterTable,
        merged_set: &'a [PluginName],
        allocation: &'a Allocation,
    ) -> Self {
        Remapper {
            plugin,
            source_masters,
            merged_masters,
            merged_set,
            allocation,
            non_canonical: Cell::new(0),
        }
    }

    /// How many references used a mod index past the source's master list.
    ///
    /// They are translated correctly; this is worth surfacing because it says
    /// the source was written by a tool that does not emit canonical indices.
    pub fn non_canonical_count(&self) -> usize {
        self.non_canonical.get()
    }

    /// Translate without counting, for callers asking *whether* a FormID
    /// would change rather than performing the rewrite.
    ///
    /// `merge::audit` probes every SCRO value this way. Without this those
    /// probes inflate the non-canonical tally with references that are also
    /// counted for real moments later.
    pub fn probe(&self, form_id: FormId) -> Result<FormId, RewriteError> {
        let saved = self.non_canonical.get();
        let mapped = self.map(form_id);
        self.non_canonical.set(saved);
        mapped
    }

    /// Translate one FormID.
    ///
    /// Three cases:
    /// - a reference into a master that survives: keep the object index, move
    ///   the mod index to that master's new position
    /// - a reference into a plugin that is being merged away (including this
    ///   plugin's own records): becomes an own-record of the merged plugin,
    ///   with the object index the allocator assigned
    /// - a master that survived but is missing from the merged master list:
    ///   an error, because that is a bug in `masters::build` rather than
    ///   anything the input did
    pub fn map(&self, form_id: FormId) -> Result<FormId, RewriteError> {
        if form_id.is_null() {
            return Ok(form_id);
        }

        let mod_index = form_id.mod_index();
        let own_index = self.source_masters.own_mod_index();

        // The plugin's own records.
        //
        // `>=`, not `==`, is deliberate. A mod index *past* the master list is
        // not a dangling reference: every TES4 reader clamps it to the
        // plugin's own records, and real tools emit it -- zMerge writes 718 of
        // them into one MOFAM merge, and TES4Edit resolves all 7913 records of
        // that file with zero errors. Refusing here would mean mudcrab could
        // not merge a zMerge output as a source, for the sake of a distinction
        // nothing downstream makes.
        //
        // Counted rather than logged at the point of use: a per-reference
        // warning would print hundreds of times for one plugin. The merge
        // reports the total once.
        // See MOFAM-test/notes/zmerge-non-canonical-refs.md.
        if mod_index >= own_index {
            if mod_index > own_index {
                self.non_canonical.set(self.non_canonical.get() + 1);
            }
            let object = self.allocation.map(self.plugin, form_id.object_index());
            return Ok(FormId::new(self.merged_masters.own_mod_index(), object));
        }

        let Some(master) = self.source_masters.get(mod_index) else {
            // Unreachable: mod_index < own_index == masters.len(). Kept so a
            // future change to MasterTable cannot turn this into a silent zero.
            return Err(RewriteError::MasterNotInMergedList {
                plugin: self.plugin.clone(),
                form_id,
                mod_index,
                master_count: self.source_masters.len(),
            });
        };

        // A master that is itself being merged away: its records are now ours.
        if self.merged_set.contains(master) {
            let object = self.allocation.map(master, form_id.object_index());
            return Ok(FormId::new(self.merged_masters.own_mod_index(), object));
        }

        // A surviving master: same record, new index.
        let Some(new_index) = self.merged_masters.index_of(master) else {
            return Err(RewriteError::MasterNotInMergedList {
                plugin: self.plugin.clone(),
                form_id,
                mod_index,
                master_count: self.source_masters.len(),
            });
        };
        Ok(form_id.with_mod_index(new_index))
    }
}

/// Rewrite every FormID in `entries` in place.
///
/// Covers three places, all of which matter:
/// - each record's own header FormID
/// - every schema-described FormID position inside its fields
/// - **every GRUP label of type 1 and 6..=10**, which are raw little-endian
///   FormIDs naming a parent worldspace, cell or dialogue topic. Missing these
///   leaves children attached to a record that no longer exists.
pub fn rewrite_entries(
    entries: &mut [Entry],
    remapper: &Remapper<'_>,
) -> Result<(), RewriteError> {
    for entry in entries {
        match entry {
            Entry::Record(record) => {
                record.form_id = remapper.map(record.form_id)?;

                // Collect first so a schema failure aborts before any partial
                // rewrite: map_form_ids would otherwise leave the record
                // half-translated.
                let mut error = None;
                schema::map_form_ids(record, |old| match remapper.map(old) {
                    Ok(new) => new,
                    Err(err) => {
                        error.get_or_insert(err);
                        old
                    }
                })
                .map_err(|source| RewriteError::Schema {
                    plugin: remapper.plugin.clone(),
                    source,
                })?;
                if let Some(err) = error {
                    return Err(err);
                }
            }
            Entry::Group(group) => {
                if let Some(parent) = group.group_type.parent_form_id() {
                    group.group_type = group.group_type.with_parent_form_id(remapper.map(parent)?);
                }
                rewrite_entries(&mut group.entries, remapper)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::alloc::allocate;
    use crate::plugin::{Group, GroupType, Record, Subrecord};
    use std::collections::BTreeSet;

    fn setup() -> (MasterTable, MasterTable, Vec<PluginName>, Allocation) {
        // source: masters [Oblivion.esm, fort.esp], own index 2
        let source_masters = MasterTable::new(vec!["Oblivion.esm".into(), "fort.esp".into()]);
        // merged: fort.esp was merged away, so only Oblivion.esm survives
        let merged_masters = MasterTable::new(vec!["Oblivion.esm".into()]);
        let merged_set: Vec<PluginName> = vec!["fort.esp".into(), "patch.esp".into()];
        let allocation = allocate(&[
            ("fort.esp".into(), BTreeSet::from([0x801])),
            ("patch.esp".into(), BTreeSet::from([0x801])),
        ]);
        (source_masters, merged_masters, merged_set, allocation)
    }

    #[test]
    fn surviving_master_references_keep_their_object_index() {
        let (src, dst, set, alloc) = setup();
        let name: PluginName = "patch.esp".into();
        let r = Remapper::new(&name, &src, &dst, &set, &alloc);
        // 0x00000014 -> Oblivion.esm is still index 0
        assert_eq!(r.map(FormId(0x0000_0014)).unwrap(), FormId(0x0000_0014));
    }

    #[test]
    fn references_into_a_merged_away_plugin_become_own_records() {
        let (src, dst, set, alloc) = setup();
        let name: PluginName = "patch.esp".into();
        let r = Remapper::new(&name, &src, &dst, &set, &alloc);
        // 0x01000801 points at fort.esp, which kept object index 0x801.
        // The merged plugin's own index is 1 (one surviving master).
        assert_eq!(r.map(FormId(0x0100_0801)).unwrap(), FormId(0x0100_0801));
    }

    #[test]
    fn own_records_pick_up_their_allocated_object_index() {
        let (src, dst, set, alloc) = setup();
        let name: PluginName = "patch.esp".into();
        let r = Remapper::new(&name, &src, &dst, &set, &alloc);
        // patch.esp's own 0x801 collided with fort.esp's, so it moved to 0x802.
        assert_eq!(r.map(FormId(0x0200_0801)).unwrap(), FormId(0x0100_0802));
    }

    #[test]
    fn null_form_ids_are_left_alone() {
        let (src, dst, set, alloc) = setup();
        let name: PluginName = "patch.esp".into();
        let r = Remapper::new(&name, &src, &dst, &set, &alloc);
        assert_eq!(r.map(FormId::NULL).unwrap(), FormId::NULL);
    }

    #[test]
    fn a_mod_index_past_the_master_list_is_read_as_an_own_record() {
        // zMerge emits these and every TES4 reader clamps them, so refusing
        // would make a zMerge output unmergeable. Same result as if the index
        // had been the canonical own index.
        let (src, dst, set, alloc) = setup();
        let name: PluginName = "patch.esp".into();
        let r = Remapper::new(&name, &src, &dst, &set, &alloc);

        let canonical = r.map(FormId(0x0200_0801)).unwrap();
        let sloppy = r.map(FormId(0x0700_0801)).unwrap();
        assert_eq!(sloppy, canonical);
        assert_eq!(sloppy, FormId(0x0100_0802));
    }

    #[test]
    fn probing_does_not_count() {
        // merge::audit probes every SCRO value to ask whether the merge moves
        // it. Counting those would double-count references that the rewrite
        // pass counts for real moments later.
        let (src, dst, set, alloc) = setup();
        let name: PluginName = "patch.esp".into();
        let r = Remapper::new(&name, &src, &dst, &set, &alloc);

        r.probe(FormId(0x0700_0801)).unwrap();
        r.probe(FormId(0x0900_0801)).unwrap();
        assert_eq!(r.non_canonical_count(), 0, "probes must not count");

        r.map(FormId(0x0700_0801)).unwrap();
        assert_eq!(r.non_canonical_count(), 1, "the real rewrite still counts");
    }

    #[test]
    fn only_the_non_canonical_ones_are_counted() {
        let (src, dst, set, alloc) = setup();
        let name: PluginName = "patch.esp".into();
        let r = Remapper::new(&name, &src, &dst, &set, &alloc);

        r.map(FormId(0x0200_0801)).unwrap(); // canonical own
        r.map(FormId(0x0000_0014)).unwrap(); // master reference
        assert_eq!(r.non_canonical_count(), 0);

        r.map(FormId(0x0700_0801)).unwrap();
        r.map(FormId(0x0900_0801)).unwrap();
        assert_eq!(r.non_canonical_count(), 2);
    }

    #[test]
    fn group_labels_that_are_form_ids_get_rewritten() {
        let (src, dst, set, alloc) = setup();
        let name: PluginName = "patch.esp".into();
        let remapper = Remapper::new(&name, &src, &dst, &set, &alloc);

        let mut entries = vec![Entry::Group(Group::new(
            GroupType::TopLevel(*b"CELL"),
            vec![Entry::Group(Group::new(
                // children of the plugin's own cell 0x02000801
                GroupType::CellChildren(FormId(0x0200_0801)),
                vec![Entry::Record(Record::new(
                    b"REFR",
                    FormId(0x0200_0801),
                    vec![Subrecord::new(b"NAME", 0x0100_0801u32.to_le_bytes().to_vec())],
                ))],
            ))],
        ))];

        rewrite_entries(&mut entries, &remapper).unwrap();

        let Entry::Group(top) = &entries[0] else {
            panic!("expected group")
        };
        assert_eq!(top.group_type, GroupType::TopLevel(*b"CELL"), "signature labels untouched");

        let Entry::Group(children) = &top.entries[0] else {
            panic!("expected nested group")
        };
        assert_eq!(
            children.group_type.parent_form_id(),
            Some(FormId(0x0100_0802)),
            "the cell-children label must follow its cell"
        );
    }

    #[test]
    fn a_schema_gap_aborts_instead_of_writing_a_partial_record() {
        let (src, dst, set, alloc) = setup();
        let name: PluginName = "patch.esp".into();
        let remapper = Remapper::new(&name, &src, &dst, &set, &alloc);

        let mut entries = vec![Entry::Record(Record::new(
            b"ZZZZ", // not in the schema
            FormId(0x0200_0801),
            vec![Subrecord::new(b"NAME", 0x0100_0801u32.to_le_bytes().to_vec())],
        ))];

        assert!(matches!(
            rewrite_entries(&mut entries, &remapper),
            Err(RewriteError::Schema { .. })
        ));
    }
}
