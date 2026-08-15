//! Parse a TES4 plugin from bytes.

use super::formid::{FormId, MasterTable, PluginName};
use super::group::{Entry, Group, GroupType};
use super::record::{parse_fields, Record, FLAG_COMPRESSED};
use super::{Plugin, PluginError};

const RECORD_HEADER_LEN: usize = 20;
const GROUP_HEADER_LEN: usize = 20;

pub fn parse(data: &[u8]) -> Result<Plugin, PluginError> {
    if data.len() < RECORD_HEADER_LEN || &data[0..4] != b"TES4" {
        return Err(PluginError::NotAPlugin);
    }

    let header = read_record(data, 0)?;
    let masters = header
        .record
        .fields_with(b"MAST")
        .map(|field| {
            let end = field.data.iter().position(|b| *b == 0).unwrap_or(field.data.len());
            PluginName::new(String::from_utf8_lossy(&field.data[..end]).into_owned())
        })
        .collect();

    let mut entries = Vec::new();
    let mut offset = header.next_offset;
    while offset < data.len() {
        let (entry, next) = read_entry(data, offset)?;
        entries.push(entry);
        offset = next;
    }

    Ok(Plugin {
        header: header.record,
        masters: MasterTable::new(masters),
        entries,
    })
}

struct ReadRecord {
    record: Record,
    next_offset: usize,
}

fn read_record(data: &[u8], offset: usize) -> Result<ReadRecord, PluginError> {
    if offset + RECORD_HEADER_LEN > data.len() {
        return Err(PluginError::Truncated { offset });
    }

    let signature: [u8; 4] = data[offset..offset + 4].try_into().unwrap();
    let size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;
    let flags = u32::from_le_bytes(data[offset + 8..offset + 12].try_into().unwrap());
    let form_id = FormId(u32::from_le_bytes(
        data[offset + 12..offset + 16].try_into().unwrap(),
    ));
    let version_control =
        u32::from_le_bytes(data[offset + 16..offset + 20].try_into().unwrap());

    let body_start = offset + RECORD_HEADER_LEN;
    let body_end = body_start
        .checked_add(size)
        .ok_or(PluginError::Truncated { offset })?;
    if body_end > data.len() {
        return Err(PluginError::Truncated { offset });
    }
    let raw_body = &data[body_start..body_end];

    // Compressed bodies are a u32 decompressed length followed by a zlib stream.
    let decompressed;
    let body: &[u8] = if flags & FLAG_COMPRESSED != 0 {
        if raw_body.len() < 4 {
            return Err(PluginError::Truncated { offset });
        }
        let expected =
            u32::from_le_bytes(raw_body[0..4].try_into().unwrap()) as usize;
        decompressed = inflate(&raw_body[4..], expected).map_err(|source| {
            PluginError::Decompress {
                record: signature,
                form_id,
                source,
            }
        })?;
        &decompressed
    } else {
        raw_body
    };

    let fields = parse_fields(body, signature)?;

    Ok(ReadRecord {
        record: Record::from_parts(
            signature,
            flags,
            form_id,
            version_control,
            fields,
            raw_body.to_vec(),
        ),
        next_offset: body_end,
    })
}

fn read_entry(data: &[u8], offset: usize) -> Result<(Entry, usize), PluginError> {
    if offset + 4 > data.len() {
        return Err(PluginError::Truncated { offset });
    }

    if &data[offset..offset + 4] != b"GRUP" {
        let read = read_record(data, offset)?;
        return Ok((Entry::Record(read.record), read.next_offset));
    }

    if offset + GROUP_HEADER_LEN > data.len() {
        return Err(PluginError::Truncated { offset });
    }
    // NOTE: a GRUP's size *includes* its own 20-byte header, unlike a record's.
    let size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;
    let label: [u8; 4] = data[offset + 8..offset + 12].try_into().unwrap();
    let raw_type = i32::from_le_bytes(data[offset + 12..offset + 16].try_into().unwrap());
    let stamp = u32::from_le_bytes(data[offset + 16..offset + 20].try_into().unwrap());

    let group_type = GroupType::from_raw(raw_type, label)
        .ok_or(PluginError::UnknownGroupType { raw_type, offset })?;

    let end = offset
        .checked_add(size)
        .ok_or(PluginError::Truncated { offset })?;
    if end > data.len() || size < GROUP_HEADER_LEN {
        return Err(PluginError::Truncated { offset });
    }

    let mut entries = Vec::new();
    let mut cursor = offset + GROUP_HEADER_LEN;
    while cursor < end {
        let (entry, next) = read_entry(data, cursor)?;
        entries.push(entry);
        cursor = next;
    }

    Ok((
        Entry::Group(Group {
            group_type,
            stamp,
            entries,
        }),
        end,
    ))
}

fn inflate(input: &[u8], expected: usize) -> Result<Vec<u8>, std::io::Error> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut out = Vec::with_capacity(expected);
    ZlibDecoder::new(input).read_to_end(&mut out)?;
    Ok(out)
}
