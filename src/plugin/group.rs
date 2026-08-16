//! GRUP containers.

use super::formid::FormId;
use super::record::Record;

/// What a GRUP's 4-byte label means, which depends on its type.
///
/// Types 1 and 6..=10 carry a raw little-endian FormID, so those labels must be
/// rewritten when a merge renumbers records. Getting this wrong leaves a
/// worldspace or cell's children pointing at the wrong parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupType {
    /// 0: label is a record signature.
    TopLevel([u8; 4]),
    /// 1: label is the parent WRLD FormID.
    WorldChildren(FormId),
    /// 2: label is an interior block number.
    InteriorBlock(i32),
    /// 3: label is an interior sub-block number.
    InteriorSubBlock(i32),
    /// 4: label is an exterior block, as `(y, x)` i16 pair.
    ExteriorBlock { y: i16, x: i16 },
    /// 5: label is an exterior sub-block, as `(y, x)` i16 pair.
    ExteriorSubBlock { y: i16, x: i16 },
    /// 6: label is the parent CELL FormID.
    CellChildren(FormId),
    /// 7: label is the parent DIAL FormID.
    TopicChildren(FormId),
    /// 8: label is the parent CELL FormID.
    CellPersistentChildren(FormId),
    /// 9: label is the parent CELL FormID.
    CellTemporaryChildren(FormId),
    /// 10: label is the parent CELL FormID.
    CellVisibleDistantChildren(FormId),
}

impl GroupType {
    pub fn from_raw(group_type: i32, label: [u8; 4]) -> Option<Self> {
        let form_id = || FormId(u32::from_le_bytes(label));
        let coords = || {
            (
                i16::from_le_bytes([label[0], label[1]]),
                i16::from_le_bytes([label[2], label[3]]),
            )
        };
        Some(match group_type {
            0 => GroupType::TopLevel(label),
            1 => GroupType::WorldChildren(form_id()),
            2 => GroupType::InteriorBlock(i32::from_le_bytes(label)),
            3 => GroupType::InteriorSubBlock(i32::from_le_bytes(label)),
            4 => {
                let (y, x) = coords();
                GroupType::ExteriorBlock { y, x }
            }
            5 => {
                let (y, x) = coords();
                GroupType::ExteriorSubBlock { y, x }
            }
            6 => GroupType::CellChildren(form_id()),
            7 => GroupType::TopicChildren(form_id()),
            8 => GroupType::CellPersistentChildren(form_id()),
            9 => GroupType::CellTemporaryChildren(form_id()),
            10 => GroupType::CellVisibleDistantChildren(form_id()),
            _ => return None,
        })
    }

    pub fn raw_type(self) -> i32 {
        match self {
            GroupType::TopLevel(_) => 0,
            GroupType::WorldChildren(_) => 1,
            GroupType::InteriorBlock(_) => 2,
            GroupType::InteriorSubBlock(_) => 3,
            GroupType::ExteriorBlock { .. } => 4,
            GroupType::ExteriorSubBlock { .. } => 5,
            GroupType::CellChildren(_) => 6,
            GroupType::TopicChildren(_) => 7,
            GroupType::CellPersistentChildren(_) => 8,
            GroupType::CellTemporaryChildren(_) => 9,
            GroupType::CellVisibleDistantChildren(_) => 10,
        }
    }

    pub fn raw_label(self) -> [u8; 4] {
        match self {
            GroupType::TopLevel(sig) => sig,
            GroupType::WorldChildren(f)
            | GroupType::CellChildren(f)
            | GroupType::TopicChildren(f)
            | GroupType::CellPersistentChildren(f)
            | GroupType::CellTemporaryChildren(f)
            | GroupType::CellVisibleDistantChildren(f) => f.0.to_le_bytes(),
            GroupType::InteriorBlock(n) | GroupType::InteriorSubBlock(n) => n.to_le_bytes(),
            GroupType::ExteriorBlock { y, x } | GroupType::ExteriorSubBlock { y, x } => {
                let (y, x) = (y.to_le_bytes(), x.to_le_bytes());
                [y[0], y[1], x[0], x[1]]
            }
        }
    }

    /// The parent FormID, when the label is one.
    pub fn parent_form_id(self) -> Option<FormId> {
        match self {
            GroupType::WorldChildren(f)
            | GroupType::CellChildren(f)
            | GroupType::TopicChildren(f)
            | GroupType::CellPersistentChildren(f)
            | GroupType::CellTemporaryChildren(f)
            | GroupType::CellVisibleDistantChildren(f) => Some(f),
            _ => None,
        }
    }

    /// Replace the parent FormID, for groups whose label is one.
    pub fn with_parent_form_id(self, form_id: FormId) -> Self {
        match self {
            GroupType::WorldChildren(_) => GroupType::WorldChildren(form_id),
            GroupType::CellChildren(_) => GroupType::CellChildren(form_id),
            GroupType::TopicChildren(_) => GroupType::TopicChildren(form_id),
            GroupType::CellPersistentChildren(_) => GroupType::CellPersistentChildren(form_id),
            GroupType::CellTemporaryChildren(_) => GroupType::CellTemporaryChildren(form_id),
            GroupType::CellVisibleDistantChildren(_) => {
                GroupType::CellVisibleDistantChildren(form_id)
            }
            other => other,
        }
    }
}

/// A record or a nested group, in file order.
#[derive(Debug, Clone)]
pub enum Entry {
    Record(Record),
    Group(Group),
}

#[derive(Debug, Clone)]
pub struct Group {
    pub group_type: GroupType,
    /// Timestamp/version bytes from the GRUP header. Preserved, never interpreted.
    pub stamp: u32,
    pub entries: Vec<Entry>,
}

impl Group {
    pub fn new(group_type: GroupType, entries: Vec<Entry>) -> Self {
        Group {
            group_type,
            stamp: 0,
            entries,
        }
    }

    /// Every record in this group and its descendants, in file order.
    pub fn records(&self) -> impl Iterator<Item = &Record> {
        fn walk<'a>(entries: &'a [Entry], out: &mut Vec<&'a Record>) {
            for entry in entries {
                match entry {
                    Entry::Record(record) => out.push(record),
                    Entry::Group(group) => walk(&group.entries, out),
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.entries, &mut out);
        out.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_group_type() {
        // All eleven types occur across the six MOFAM merge outputs.
        let cases = [
            (0i32, *b"CELL"),
            (1, 0x0100_2345u32.to_le_bytes()),
            (2, 0i32.to_le_bytes()),
            (3, 1i32.to_le_bytes()),
            (4, [0xF0, 0xFF, 0x10, 0x00]),
            (5, [0xF8, 0xFF, 0x02, 0x00]),
            (6, 0x0200_26B6u32.to_le_bytes()),
            (7, 0x0100_0999u32.to_le_bytes()),
            (8, 0x0200_26B6u32.to_le_bytes()),
            (9, 0x0200_26B6u32.to_le_bytes()),
            (10, 0x0200_26B6u32.to_le_bytes()),
        ];
        for (raw, label) in cases {
            let parsed = GroupType::from_raw(raw, label)
                .unwrap_or_else(|| panic!("group type {raw} should parse"));
            assert_eq!(parsed.raw_type(), raw);
            assert_eq!(parsed.raw_label(), label, "label round-trip for type {raw}");
        }
    }

    #[test]
    fn unknown_group_type_is_rejected() {
        assert!(GroupType::from_raw(11, [0; 4]).is_none());
        assert!(GroupType::from_raw(-1, [0; 4]).is_none());
    }

    #[test]
    fn formid_labels_are_identified_and_rewritable() {
        for raw in [1, 6, 7, 8, 9, 10] {
            let g = GroupType::from_raw(raw, 0x0100_0801u32.to_le_bytes()).unwrap();
            assert_eq!(g.parent_form_id(), Some(FormId(0x0100_0801)));
            let moved = g.with_parent_form_id(FormId(0x0200_0999));
            assert_eq!(moved.parent_form_id(), Some(FormId(0x0200_0999)));
            assert_eq!(moved.raw_type(), raw);
        }
        // and non-FormID labels are left alone
        for raw in [0, 2, 3, 4, 5] {
            let g = GroupType::from_raw(raw, *b"CELL").unwrap();
            assert_eq!(g.parent_form_id(), None);
            assert_eq!(g.with_parent_form_id(FormId(1)), g);
        }
    }

    #[test]
    fn exterior_coordinates_survive_negative_values() {
        // Cyrodiil has plenty of negative grid coordinates.
        let g = GroupType::ExteriorBlock { y: -8, x: -3 };
        let back = GroupType::from_raw(4, g.raw_label()).unwrap();
        assert_eq!(back, g);
    }
}
