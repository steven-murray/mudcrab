//! Rewrite every FormID in a source plugin into the merged plugin's numbering.

use super::alloc::Allocation;
use crate::plugin::schema::{self, SchemaError};
use crate::plugin::{Entry, FormId, MasterTable, PluginName};

#[derive(Debug, thiserror::Error)]
pub enum RewriteError {
    #[error("{plugin}: {source}")]
    Schema {
        plugin: PluginName,
        source: SchemaError,
    },

    #[error(
        "{plugin}: record {record} references {form_id}, whose mod index {mod_index} is beyond \
         its {master_count} masters. The plugin is dirty; clean it in TES4Edit before merging."
    )]
    DanglingReference {
        plugin: PluginName,
        record: FormId,
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
        }
    }

    /// Translate one FormID.
    ///
    /// Three cases:
    /// - a reference into a master that survives: keep the object index, move
    ///   the mod index to that master's new position
    /// - a reference into a plugin that is being merged away (including this
    ///   plugin's own records): becomes an own-record of the merged plugin,
    ///   with the object index the allocator assigned
    /// - anything else is dangling and is an error, never silently zeroed
    pub fn map(&self, form_id: FormId) -> Result<FormId, RewriteError> {
        if form_id.is_null() {
            return Ok(form_id);
        }

        let mod_index = form_id.mod_index();
        let own_index = self.source_masters.own_mod_index();

        // The plugin's own records.
        if mod_index == own_index {
            let object = self.allocation.map(self.plugin, form_id.object_index());
            return Ok(FormId::new(self.merged_masters.own_mod_index(), object));
        }

        let Some(master) = self.source_masters.get(mod_index) else {
            return Err(RewriteError::DanglingReference {
                plugin: self.plugin.clone(),
                record: form_id,
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
            return Err(RewriteError::DanglingReference {
                plugin: self.plugin.clone(),
                record: form_id,
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
    fn dangling_references_are_an_error_not_a_zero() {
        let (src, dst, set, alloc) = setup();
        let name: PluginName = "patch.esp".into();
        let r = Remapper::new(&name, &src, &dst, &set, &alloc);
        // mod index 7 with only 2 masters and own index 2
        assert!(matches!(
            r.map(FormId(0x0700_0801)),
            Err(RewriteError::DanglingReference { .. })
        ));
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
