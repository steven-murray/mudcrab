//! Minimal BSA builder for tests.
//!
//! Assembles real Oblivion (version 103) archive bytes by hand, so the
//! round-trip gate is a genuine test of the container model rather than the
//! writer checking its own arithmetic.
//!
//! The name hashes come from `mudcrab::bsa::hash` rather than being
//! reimplemented here: a second copy would drift silently, and the hash has its
//! own oracle -- all 86,209 file records in the reference install, checked by
//! `bsa_roundtrip_real.rs`.

#![allow(dead_code)]

use mudcrab::bsa::{hash_file_name, hash_folder_name};

pub const FLAG_FOLDER_NAMES: u32 = 0x1;
pub const FLAG_FILE_NAMES: u32 = 0x2;
pub const FLAG_COMPRESSED: u32 = 0x4;

const SIZE_COMPRESSION_DIFFERS: u32 = 0x4000_0000;

pub struct FileSpec {
    pub name: String,
    pub data: Vec<u8>,
    /// `None` follows the archive default; `Some` forces it and sets the
    /// compression-differs bit when it disagrees.
    pub compressed: Option<bool>,
}

pub fn file(name: &str, data: &[u8]) -> FileSpec {
    FileSpec {
        name: name.to_string(),
        data: data.to_vec(),
        compressed: None,
    }
}

/// A file whose compression differs from the archive default.
pub fn file_with_compression(name: &str, data: &[u8], compressed: bool) -> FileSpec {
    FileSpec {
        name: name.to_string(),
        data: data.to_vec(),
        compressed: Some(compressed),
    }
}

/// How to arrange the payloads, which real archives do not do uniformly.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DataOrder {
    /// Payloads in the same order as the file records.
    RecordOrder,
    /// Payloads in reverse record order, as many real archives do.
    Reversed,
    /// Identical payloads share one block, as many real archives do.
    Deduplicated,
    /// Every payload preceded by a redundant u32 length that no record points
    /// at -- what `WACIntegration.bsa` does.
    LengthPrefixed,
}

pub fn deflate(input: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input).unwrap();
    encoder.finish().unwrap()
}

/// The payload as stored: compressed files carry a u32 raw length first.
fn stored_bytes(spec: &FileSpec, compressed: bool) -> Vec<u8> {
    if !compressed {
        return spec.data.clone();
    }
    let mut out = (spec.data.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&deflate(&spec.data));
    out
}

/// Assemble an archive. `folders` is `(folder name, files)`.
pub fn archive(archive_flags: u32, folders: &[(&str, Vec<FileSpec>)], order: DataOrder) -> Vec<u8> {
    let flags = archive_flags | FLAG_FOLDER_NAMES | FLAG_FILE_NAMES;
    let compressed_by_default = flags & FLAG_COMPRESSED != 0;

    // Records are keyed by hash and must be sorted by it.
    let mut folders: Vec<(String, Vec<FileSpec>)> = folders
        .iter()
        .map(|(name, files)| {
            let mut files: Vec<FileSpec> = files
                .iter()
                .map(|f| FileSpec {
                    name: f.name.clone(),
                    data: f.data.clone(),
                    compressed: f.compressed,
                })
                .collect();
            files.sort_by_key(|f| hash_file_name(&f.name));
            (name.to_string(), files)
        })
        .collect();
    folders.sort_by_key(|(name, _)| hash_folder_name(name));

    let folder_count = folders.len();
    let file_count: usize = folders.iter().map(|(_, files)| files.len()).sum();
    let total_folder_name_length: usize =
        folders.iter().map(|(name, _)| name.len() + 1).sum();
    let total_file_name_length: usize = folders
        .iter()
        .flat_map(|(_, files)| files.iter())
        .map(|f| f.name.len() + 1)
        .sum();

    // Every payload, in record order.
    let stored: Vec<(bool, Vec<u8>)> = folders
        .iter()
        .flat_map(|(_, files)| files.iter())
        .map(|spec| {
            let compressed = spec.compressed.unwrap_or(compressed_by_default);
            (compressed, stored_bytes(spec, compressed))
        })
        .collect();

    let metadata_length =
        36 + folder_count * 16
            + folders
                .iter()
                .map(|(name, files)| 1 + name.len() + 1 + files.len() * 16)
                .sum::<usize>()
            + total_file_name_length;

    // Lay the data region out, recording where each file's payload landed.
    let mut region: Vec<u8> = Vec::new();
    let mut offsets = vec![0u32; stored.len()];
    let base = metadata_length as u32;
    match order {
        DataOrder::RecordOrder => {
            for (index, (_, bytes)) in stored.iter().enumerate() {
                offsets[index] = base + region.len() as u32;
                region.extend_from_slice(bytes);
            }
        }
        DataOrder::Reversed => {
            for index in (0..stored.len()).rev() {
                offsets[index] = base + region.len() as u32;
                region.extend_from_slice(&stored[index].1);
            }
        }
        DataOrder::Deduplicated => {
            let mut seen: Vec<(&[u8], u32)> = Vec::new();
            for (index, (_, bytes)) in stored.iter().enumerate() {
                if let Some((_, at)) = seen.iter().find(|(seen, _)| *seen == bytes.as_slice()) {
                    offsets[index] = *at;
                    continue;
                }
                let at = base + region.len() as u32;
                offsets[index] = at;
                region.extend_from_slice(bytes);
                seen.push((bytes.as_slice(), at));
            }
        }
        DataOrder::LengthPrefixed => {
            for (index, (_, bytes)) in stored.iter().enumerate() {
                region.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                offsets[index] = base + region.len() as u32;
                region.extend_from_slice(bytes);
            }
        }
    }

    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"BSA\0");
    out.extend_from_slice(&103u32.to_le_bytes());
    out.extend_from_slice(&36u32.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&(folder_count as u32).to_le_bytes());
    out.extend_from_slice(&(file_count as u32).to_le_bytes());
    out.extend_from_slice(&(total_folder_name_length as u32).to_le_bytes());
    out.extend_from_slice(&(total_file_name_length as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // file flags

    // Folder records. The offset is biased by the total file-name length and
    // points at the folder block's length byte.
    let mut block = 36 + folder_count * 16;
    for (name, files) in &folders {
        out.extend_from_slice(&hash_folder_name(name).to_le_bytes());
        out.extend_from_slice(&(files.len() as u32).to_le_bytes());
        out.extend_from_slice(&((block + total_file_name_length) as u32).to_le_bytes());
        block += 1 + name.len() + 1 + files.len() * 16;
    }

    // Folder blocks: name then file records.
    let mut index = 0usize;
    for (name, files) in &folders {
        out.push((name.len() + 1) as u8);
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        for spec in files {
            let (compressed, bytes) = &stored[index];
            let mut size = bytes.len() as u32;
            if *compressed != compressed_by_default {
                size |= SIZE_COMPRESSION_DIFFERS;
            }
            out.extend_from_slice(&hash_file_name(&spec.name).to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&offsets[index].to_le_bytes());
            index += 1;
        }
    }

    // File names, in record order.
    for (_, files) in &folders {
        for spec in files {
            out.extend_from_slice(spec.name.as_bytes());
            out.push(0);
        }
    }

    assert_eq!(out.len(), metadata_length, "metadata length miscomputed");
    out.extend_from_slice(&region);
    out
}
