//! TES4 (Oblivion) plugin container: read, model and write `.esp`/`.esm` files.
//!
//! Hand-rolled rather than taken from a crate. The only real candidate,
//! `esplugin`, has no writer, never parses subrecords, and is GPL-3.0. The
//! container format itself is small: a 20-byte record header, a 20-byte GRUP
//! header, a 6-byte field header, and zlib for compressed record bodies.
//!
//! This module knows the *structure* of a plugin but nothing about what any
//! field means -- which fields hold FormIDs lives in `schema` (M3), so that a
//! merge can never rewrite a field the table does not describe.

pub mod formid;
pub mod group;
pub mod reader;
pub mod record;
pub mod schema;
pub mod writer;

pub use formid::{FormId, MasterTable, Origin, PluginName};
pub use group::{Entry, Group, GroupType};
pub use record::{Record, Subrecord};

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("not a TES4 plugin (no TES4 header record)")]
    NotAPlugin,

    #[error("plugin data ends unexpectedly at offset {offset}")]
    Truncated { offset: usize },

    #[error("unknown GRUP type {raw_type} at offset {offset}")]
    UnknownGroupType { raw_type: i32, offset: usize },

    #[error("record {} has a truncated field at body offset {offset}", sig(*record))]
    TruncatedField { record: [u8; 4], offset: usize },

    #[error("record {} has a malformed XXXX overflow field at body offset {offset}", sig(*record))]
    MalformedOverflowField { record: [u8; 4], offset: usize },

    #[error("failed to decompress {} record {form_id}: {source}", sig(*record))]
    Decompress {
        record: [u8; 4],
        form_id: FormId,
        source: std::io::Error,
    },

    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

fn sig(value: [u8; 4]) -> String {
    String::from_utf8_lossy(&value).into_owned()
}

/// A parsed plugin.
#[derive(Debug, Clone)]
pub struct Plugin {
    /// The TES4 header record.
    pub header: Record,
    /// Masters, in order. Index into this == mod index.
    pub masters: MasterTable,
    /// Top-level entries, in file order.
    pub entries: Vec<Entry>,
}

impl Plugin {
    pub fn parse(data: &[u8]) -> Result<Self, PluginError> {
        reader::parse(data)
    }

    pub fn read(path: &Path) -> Result<Self, PluginError> {
        let data = std::fs::read(path).map_err(|source| PluginError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::parse(&data)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        writer::write(self)
    }

    /// Every record below the header, in file order.
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

    /// Count of records and groups below the header.
    ///
    /// This is what the TES4 header's HEDR.numRecords holds, and it counts
    /// groups as well as records.
    pub fn record_and_group_count(&self) -> usize {
        fn walk(entries: &[Entry]) -> usize {
            entries
                .iter()
                .map(|entry| match entry {
                    Entry::Record(_) => 1,
                    Entry::Group(group) => 1 + walk(&group.entries),
                })
                .sum()
        }
        walk(&self.entries)
    }

    /// Object indices of records the plugin defines itself.
    ///
    /// Deduplicated and ascending, which is the order the merge allocator
    /// requires: some plugins contain duplicate FormIDs (Fort Vlastarus has
    /// three) and iterating them in file order produces phantom allocations.
    pub fn own_object_indices(&self) -> std::collections::BTreeSet<u32> {
        let own = self.masters.own_mod_index();
        self.records()
            .filter(|record| record.form_id.mod_index() == own)
            .map(|record| record.form_id.object_index())
            .collect()
    }
}
