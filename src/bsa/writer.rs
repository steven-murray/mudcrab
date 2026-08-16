//! Serialise a BSA back to bytes, and build one from a directory tree.

use super::hash::{hash_file_name, hash_folder_name};
use super::{
    Bsa, BsaError, File, Folder, FLAG_FILE_NAMES, FLAG_FOLDER_NAMES, HEADER_LEN, RECORD_LEN,
    VERSION_OBLIVION,
};
use crate::archive::ArchiveFilters;
use std::borrow::Cow;
use std::io::Write;
use std::path::Path;

/// Length of a folder's block: the name-length byte, the NUL-terminated name,
/// then its file records.
fn folder_block_length(folder: &Folder<'_>) -> usize {
    1 + folder.name.len() + 1 + folder.files.len() * RECORD_LEN
}

pub(super) fn write_to<W: Write>(bsa: &Bsa<'_>, out: &mut W) -> Result<(), BsaError> {
    let folder_count = bsa.folders.len();
    let file_count = bsa.file_count();
    let compressed_by_default = bsa.compressed_by_default();

    let total_file_name_length: usize = bsa
        .files()
        .map(|(_, file)| file.name.len() + 1)
        .sum::<usize>();
    let computed_folder_name_length: usize =
        bsa.folders.iter().map(|f| f.name.len() + 1).sum::<usize>();

    // One real archive (WACIntegration.bsa) declares a folder-name length that
    // disagrees with its own names. Replay whatever it declared: the field is
    // not load-bearing for parsing, and rewriting it would change the bytes.
    let total_folder_name_length = match &bsa.source {
        Some(source) => source.total_folder_name_length,
        None => u32::try_from(computed_folder_name_length).map_err(|_| BsaError::TooLarge)?,
    };

    let total_file_name_length =
        u32::try_from(total_file_name_length).map_err(|_| BsaError::TooLarge)?;

    // --- header ---
    out.write_all(b"BSA\0").map_err(io)?;
    write_u32(out, VERSION_OBLIVION)?;
    write_u32(out, HEADER_LEN as u32)?;
    write_u32(out, bsa.archive_flags | FLAG_FOLDER_NAMES | FLAG_FILE_NAMES)?;
    write_u32(out, u32::try_from(folder_count).map_err(|_| BsaError::TooLarge)?)?;
    write_u32(out, u32::try_from(file_count).map_err(|_| BsaError::TooLarge)?)?;
    write_u32(out, total_folder_name_length)?;
    write_u32(out, total_file_name_length)?;
    write_u32(out, bsa.file_flags)?;

    // --- folder records ---
    // Each carries the offset of its block, biased by the total file-name
    // length. Blocks follow the folder records in order.
    let mut block_offset = HEADER_LEN + folder_count * RECORD_LEN;
    for folder in &bsa.folders {
        write_u64(out, hash_folder_name(&folder.name))?;
        write_u32(
            out,
            u32::try_from(folder.files.len()).map_err(|_| BsaError::TooLarge)?,
        )?;
        let biased = u32::try_from(block_offset)
            .map_err(|_| BsaError::TooLarge)?
            .wrapping_add(total_file_name_length);
        write_u32(out, biased)?;
        block_offset += folder_block_length(folder);
    }

    // Where the payloads will start, which is what a file record's offset is
    // relative to.
    let metadata_length = block_offset + total_file_name_length as usize;

    // For a parsed archive the payload offsets are the originals, so the
    // rebuilt metadata has to be exactly as long as the original's. It is for
    // every archive in the corpus; if it ever is not, that is a modelling bug
    // and must not be written out as a silently corrupt archive.
    if let Some(source) = &bsa.source
        && metadata_length as u32 != source.data_region_start
    {
        return Err(BsaError::BadFolderRecordOffset {
            offset: metadata_length as u32,
        });
    }

    // --- folder blocks: name then file records ---
    let mut next_offset = u32::try_from(metadata_length).map_err(|_| BsaError::TooLarge)?;
    for folder in &bsa.folders {
        let name = folder.name.as_bytes();
        out.write_all(&[u8::try_from(name.len() + 1).map_err(|_| {
            BsaError::FolderNameTooLong {
                name: folder.name.clone(),
            }
        })?])
        .map_err(io)?;
        out.write_all(name).map_err(io)?;
        out.write_all(&[0]).map_err(io)?;

        for file in &folder.files {
            write_u64(out, hash_file_name(&file.name))?;
            write_u32(out, file.size_field(compressed_by_default))?;
            match file.offset {
                // A parsed archive keeps its original offsets, because its data
                // region is replayed verbatim and is not in record order.
                Some(offset) => write_u32(out, offset)?,
                None => {
                    write_u32(out, next_offset)?;
                    next_offset = next_offset
                        .checked_add(
                            u32::try_from(file.stored.len()).map_err(|_| BsaError::TooLarge)?,
                        )
                        .ok_or(BsaError::TooLarge)?;
                }
            }
        }
    }

    // --- file names ---
    for (_, file) in bsa.files() {
        out.write_all(file.name.as_bytes()).map_err(io)?;
        out.write_all(&[0]).map_err(io)?;
    }

    // --- payloads ---
    match &bsa.source {
        // Replayed verbatim: the region is permuted relative to record order,
        // deduplicated, and in one archive carries interstitial bytes that no
        // record points at.
        Some(source) => out.write_all(source.data_region).map_err(io)?,
        None => {
            for (_, file) in bsa.files() {
                out.write_all(file.stored.as_ref()).map_err(io)?;
            }
        }
    }

    Ok(())
}

fn io(source: std::io::Error) -> BsaError {
    BsaError::Io {
        path: "<output>".to_string(),
        source,
    }
}

fn write_u32<W: Write>(out: &mut W, value: u32) -> Result<(), BsaError> {
    out.write_all(&value.to_le_bytes()).map_err(io)
}

fn write_u64<W: Write>(out: &mut W, value: u64) -> Result<(), BsaError> {
    out.write_all(&value.to_le_bytes()).map_err(io)
}

/// Build an archive from every file below `root` that passes `filters`.
pub(super) fn from_directory(
    root: &Path,
    filters: &ArchiveFilters,
) -> Result<Bsa<'static>, BsaError> {
    let mut collected: Vec<(String, String, Vec<u8>)> = Vec::new();
    collect(root, root, filters, &mut collected)?;

    // Folders and the files within them are keyed and looked up by hash, so
    // both must be sorted by it.
    let mut by_folder: std::collections::BTreeMap<String, Vec<(String, Vec<u8>)>> =
        std::collections::BTreeMap::new();
    for (folder, name, data) in collected {
        by_folder.entry(folder).or_default().push((name, data));
    }

    let mut folders: Vec<Folder<'static>> = by_folder
        .into_iter()
        .map(|(name, mut files)| {
            files.sort_by_key(|(file_name, _)| hash_file_name(file_name));
            Folder {
                name,
                files: files
                    .into_iter()
                    .map(|(name, data)| File {
                        name,
                        compressed: false,
                        stored: Cow::Owned(data),
                        offset: None,
                    })
                    .collect(),
            }
        })
        .collect();
    folders.sort_by_key(|folder| hash_folder_name(&folder.name));

    // Oblivion uses the asset-kind flags to decide whether this archive can
    // serve a given kind of request, so an archive of meshes that declares none
    // is invisible to the game while still parsing and extracting perfectly.
    // Every archive in the corpus sets them; none is zero.
    let file_flags = super::file_flags::derive(folders.iter().flat_map(|folder| {
        folder
            .files
            .iter()
            .map(move |file| format!("{}\\{}", folder.name, file.name))
    }).collect::<Vec<_>>().iter().map(String::as_str));

    Ok(Bsa {
        // Names present, nothing compressed.
        archive_flags: FLAG_FOLDER_NAMES | FLAG_FILE_NAMES,
        file_flags,
        folders,
        source: None,
    })
}

fn collect(
    current: &Path,
    root: &Path,
    filters: &ArchiveFilters,
    out: &mut Vec<(String, String, Vec<u8>)>,
) -> Result<(), BsaError> {
    let entries = std::fs::read_dir(current).map_err(|source| BsaError::Io {
        path: current.display().to_string(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| BsaError::Io {
            path: current.display().to_string(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| BsaError::Io {
            path: path.display().to_string(),
            source,
        })?;

        if file_type.is_dir() {
            collect(&path, root, filters, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .map_err(|_| BsaError::PathOutsideRoot {
                path: path.display().to_string(),
                root: root.display().to_string(),
            })?
            .to_string_lossy()
            .replace('\\', "/");

        if !filters.should_extract(&relative) {
            continue;
        }

        // Oblivion stores every file inside a folder; the format cannot
        // address one at the archive root. Such files are skipped rather than
        // treated as an error -- a staged mod routinely has a readme or a
        // plugin at its top level, and those have to stay loose anyway.
        let Some((folder, name)) = relative.rsplit_once('/') else {
            tracing::debug!(
                file = %relative,
                "skipping file at the mod root: a BSA cannot store a file outside a folder"
            );
            continue;
        };

        // Names are stored lowercase, which is also what the hash requires.
        let folder = folder.replace('/', "\\").to_ascii_lowercase();
        let name = name.to_ascii_lowercase();

        if !folder.is_ascii() {
            return Err(BsaError::NonAsciiName { name: folder });
        }
        if !name.is_ascii() {
            return Err(BsaError::NonAsciiName { name });
        }
        if folder.len() > 254 {
            return Err(BsaError::FolderNameTooLong { name: folder });
        }

        let data = std::fs::read(&path).map_err(|source| BsaError::Io {
            path: path.display().to_string(),
            source,
        })?;
        out.push((folder, name, data));
    }

    Ok(())
}
