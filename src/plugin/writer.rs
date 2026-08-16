//! Serialise a plugin back to bytes.

use super::group::{Entry, Group};
use super::record::{write_fields, Record, FLAG_COMPRESSED};
use super::Plugin;

pub fn write(plugin: &Plugin) -> Vec<u8> {
    let mut out = Vec::new();
    write_record(&plugin.header, &mut out);
    for entry in &plugin.entries {
        write_entry(entry, &mut out);
    }
    out
}

fn write_entry(entry: &Entry, out: &mut Vec<u8>) {
    match entry {
        Entry::Record(record) => write_record(record, out),
        Entry::Group(group) => write_group(group, out),
    }
}

fn write_record(record: &Record, out: &mut Vec<u8>) {
    let body = record_body(record);

    out.extend_from_slice(&record.signature);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&record.flags.to_le_bytes());
    out.extend_from_slice(&record.form_id.0.to_le_bytes());
    out.extend_from_slice(&record.version_control.to_le_bytes());
    out.extend_from_slice(&body);
}

/// The on-disk body for a record.
///
/// Untouched records are written back from their cached original bytes. That is
/// what makes round-trips byte-identical: re-deflating a compressed body yields
/// a valid but different byte stream, since the compression level and zlib
/// implementation that produced the file are not knowable.
fn record_body(record: &Record) -> Vec<u8> {
    if let Some(original) = record.original_body() {
        return original.to_vec();
    }

    let raw = write_fields(record.fields());
    if record.flags & FLAG_COMPRESSED == 0 {
        return raw;
    }

    let mut body = (raw.len() as u32).to_le_bytes().to_vec();
    body.extend_from_slice(&deflate(&raw));
    body
}

fn write_group(group: &Group, out: &mut Vec<u8>) {
    let start = out.len();

    out.extend_from_slice(b"GRUP");
    out.extend_from_slice(&0u32.to_le_bytes()); // size patched below
    out.extend_from_slice(&group.group_type.raw_label());
    out.extend_from_slice(&group.group_type.raw_type().to_le_bytes());
    out.extend_from_slice(&group.stamp.to_le_bytes());

    for entry in &group.entries {
        write_entry(entry, out);
    }

    // A GRUP's size includes its own header, unlike a record's.
    let size = (out.len() - start) as u32;
    out[start + 4..start + 8].copy_from_slice(&size.to_le_bytes());
}

fn deflate(input: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input).expect("writing to a Vec cannot fail");
    encoder.finish().expect("finishing a Vec encoder cannot fail")
}
