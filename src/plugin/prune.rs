//! Removing records and groups from a plugin.
//!
//! xEdit's "remove this record" and "remove this group", which several guide
//! rows ask for by hand after a mod is installed. Doing it here rather than in
//! a GUI is what makes those rows reproducible.
//!
//! A record and the group holding its children are separate entries in the
//! file: a CELL is followed by a GRUP labelled with that CELL's FormID, and a
//! WRLD by its own. Deleting the record alone leaves the children behind,
//! parented to a record that no longer exists, so both go together.

use super::formid::FormId;
use super::group::{Entry, GroupType};
use super::Plugin;

/// What to take out.
#[derive(Debug, Default, Clone)]
pub struct PruneRequest {
    /// Whole top-level groups, by record signature -- `WRLD`, `CELL`.
    pub groups: Vec<[u8; 4]>,
    /// Individual records, with whatever children they carry.
    pub form_ids: Vec<FormId>,
}

/// What was taken out, so a caller can tell a real deletion from a no-op.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PruneReport {
    /// Top-level groups removed, in the order requested.
    pub groups: Vec<[u8; 4]>,
    /// Records removed, in the order requested.
    pub form_ids: Vec<FormId>,
    /// Records and groups removed in total, children included.
    pub entries_removed: usize,
}

impl Plugin {
    /// Remove the named groups and records, and fix up the header's count.
    ///
    /// Reports what it found rather than what it was asked for: a caller that
    /// wanted a record gone should be able to tell that it was there.
    pub fn prune(&mut self, request: &PruneRequest) -> PruneReport {
        let before = self.record_and_group_count();
        let mut report = PruneReport::default();

        for signature in &request.groups {
            let found = self.entries.iter().any(|entry| {
                matches!(entry, Entry::Group(group)
                    if group.group_type == GroupType::TopLevel(*signature))
            });
            if found {
                self.entries.retain(|entry| {
                    !matches!(entry, Entry::Group(group)
                        if group.group_type == GroupType::TopLevel(*signature))
                });
                report.groups.push(*signature);
            }
        }

        for form_id in &request.form_ids {
            if remove_record(&mut self.entries, *form_id) {
                report.form_ids.push(*form_id);
            }
        }

        report.entries_removed = before.saturating_sub(self.record_and_group_count());
        self.set_header_record_count(self.record_and_group_count());
        report
    }

    /// Rewrite HEDR's record count in place, leaving the rest of the header
    /// alone -- the author, description and next-object-id are not ours to
    /// change.
    fn set_header_record_count(&mut self, count: usize) {
        for field in self.header.fields_mut() {
            if &field.signature == b"HEDR" && field.data.len() >= 8 {
                field.data[4..8].copy_from_slice(&(count as u32).to_le_bytes());
            }
        }
    }
}

/// Drop a record and the group carrying its children, wherever they sit.
fn remove_record(entries: &mut Vec<Entry>, form_id: FormId) -> bool {
    let mut removed = false;

    entries.retain(|entry| match entry {
        Entry::Record(record) => {
            if record.form_id == form_id {
                removed = true;
                false
            } else {
                true
            }
        }
        // The children of the record just removed have nowhere to belong.
        Entry::Group(group) => children_of(group.group_type).is_none_or(|parent| parent != form_id),
    });

    if removed {
        return true;
    }

    for entry in entries {
        if let Entry::Group(group) = entry
            && remove_record(&mut group.entries, form_id)
        {
            return true;
        }
    }

    false
}

/// The record a group hangs off, for the group types that name one.
fn children_of(group_type: GroupType) -> Option<FormId> {
    match group_type {
        GroupType::WorldChildren(form_id)
        | GroupType::CellChildren(form_id)
        | GroupType::TopicChildren(form_id)
        | GroupType::CellPersistentChildren(form_id)
        | GroupType::CellTemporaryChildren(form_id)
        | GroupType::CellVisibleDistantChildren(form_id) => Some(form_id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::group::Group;
    use crate::plugin::record::Record;

    fn record(sig: &[u8; 4], form_id: u32) -> Entry {
        Entry::Record(Record::new(sig, FormId(form_id), Vec::new()))
    }

    fn group(group_type: GroupType, entries: Vec<Entry>) -> Entry {
        Entry::Group(Group {
            group_type,
            stamp: 0,
            entries,
        })
    }

    fn plugin(entries: Vec<Entry>) -> Plugin {
        use crate::plugin::formid::MasterTable;
        use crate::plugin::record::Subrecord;
        let mut hedr = vec![0u8; 12];
        hedr[4..8].copy_from_slice(&0u32.to_le_bytes());
        Plugin {
            header: Record::new(b"TES4", FormId::NULL, vec![Subrecord::new(b"HEDR", hedr)]),
            masters: MasterTable::new(Vec::new()),
            entries,
        }
    }

    /// Part 11 row 23: "remove the Worldspace group".
    #[test]
    fn a_top_level_group_goes_with_everything_in_it() {
        let mut plugin = plugin(vec![
            group(GroupType::TopLevel(*b"FLOR"), vec![record(b"FLOR", 0x0100_8520)]),
            group(
                GroupType::TopLevel(*b"WRLD"),
                vec![
                    record(b"WRLD", 0x0000_003C),
                    group(
                        GroupType::WorldChildren(FormId(0x0000_003C)),
                        vec![record(b"CELL", 0x0000_4535)],
                    ),
                ],
            ),
        ]);

        let report = plugin.prune(&PruneRequest {
            groups: vec![*b"WRLD"],
            ..Default::default()
        });

        assert_eq!(report.groups, vec![*b"WRLD"]);
        assert_eq!(plugin.records().count(), 1, "only the FLOR record survives");
        assert_eq!(report.entries_removed, 4, "group, record, child group, cell");
    }

    /// A CELL and the GRUP holding its references are separate entries. Taking
    /// the record alone would leave the children parented to nothing.
    #[test]
    fn removing_a_record_takes_the_group_of_its_children() {
        let mut plugin = plugin(vec![group(
            GroupType::TopLevel(*b"CELL"),
            vec![
                record(b"CELL", 0x0100_0001),
                group(
                    GroupType::CellChildren(FormId(0x0100_0001)),
                    vec![record(b"REFR", 0x0100_0002)],
                ),
                record(b"CELL", 0x0100_0003),
            ],
        )]);

        let report = plugin.prune(&PruneRequest {
            form_ids: vec![FormId(0x0100_0001)],
            ..Default::default()
        });

        assert_eq!(report.form_ids, vec![FormId(0x0100_0001)]);
        let left: Vec<u32> = plugin.records().map(|record| record.form_id.0).collect();
        assert_eq!(left, vec![0x0100_0003], "the cell, its group and its ref all go");
    }

    #[test]
    fn what_was_not_there_is_not_reported_as_removed() {
        let mut plugin = plugin(vec![record(b"FLOR", 0x0100_0001)]);
        let report = plugin.prune(&PruneRequest {
            groups: vec![*b"WRLD"],
            form_ids: vec![FormId(0x0BAD_BEEF)],
        });
        assert_eq!(report, PruneReport::default());
        assert_eq!(plugin.records().count(), 1);
    }

    /// The header's count is what the engine trusts, so it has to follow.
    #[test]
    fn the_header_count_follows_the_deletion() {
        let mut plugin = plugin(vec![
            record(b"FLOR", 0x0100_0001),
            record(b"FLOR", 0x0100_0002),
        ]);
        plugin.prune(&PruneRequest {
            form_ids: vec![FormId(0x0100_0001)],
            ..Default::default()
        });

        let hedr = plugin
            .header
            .fields_with(b"HEDR")
            .next()
            .expect("header should carry HEDR");
        assert_eq!(
            u32::from_le_bytes(hedr.data[4..8].try_into().unwrap()),
            1,
            "one record left"
        );
    }
}
