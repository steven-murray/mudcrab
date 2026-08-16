//! Parse a BSA from bytes.

use super::{
    Bsa, BsaError, File, Folder, Source, FLAG_COMPRESSED, FLAG_FILE_NAMES, FLAG_FOLDER_NAMES,
    HEADER_LEN, SIZE_COMPRESSION_DIFFERS, SIZE_MASK, VERSION_OBLIVION,
};

/// A cursor that fails with `Truncated` rather than panicking.
struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], BsaError> {
        let end = self
            .at
            .checked_add(len)
            .ok_or(BsaError::Truncated { offset: self.at })?;
        if end > self.data.len() {
            return Err(BsaError::Truncated { offset: self.at });
        }
        let slice = &self.data[self.at..end];
        self.at = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32, BsaError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, BsaError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn u8(&mut self) -> Result<u8, BsaError> {
        Ok(self.take(1)?[0])
    }

    /// A NUL-terminated string, consuming the terminator.
    fn zstring(&mut self) -> Result<String, BsaError> {
        let start = self.at;
        let end = self.data[start..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(BsaError::Truncated { offset: start })?;
        let text = String::from_utf8_lossy(&self.data[start..start + end]).into_owned();
        self.at = start + end + 1;
        Ok(text)
    }
}

pub fn parse(data: &[u8]) -> Result<Bsa<'_>, BsaError> {
    if data.len() < HEADER_LEN || &data[0..4] != b"BSA\0" {
        return Err(BsaError::NotAnArchive);
    }

    let mut cursor = Cursor { data, at: 4 };
    let version = cursor.u32()?;
    if version != VERSION_OBLIVION {
        return Err(BsaError::UnsupportedVersion { version });
    }

    let folder_records_offset = cursor.u32()?;
    let archive_flags = cursor.u32()?;
    let folder_count = cursor.u32()? as usize;
    let file_count = cursor.u32()? as usize;
    let total_folder_name_length = cursor.u32()?;
    let total_file_name_length = cursor.u32()?;
    let file_flags = cursor.u32()?;

    if archive_flags & FLAG_FOLDER_NAMES == 0 {
        return Err(BsaError::MissingNames {
            which: "folder",
            flags: archive_flags,
        });
    }
    if archive_flags & FLAG_FILE_NAMES == 0 {
        return Err(BsaError::MissingNames {
            which: "file",
            flags: archive_flags,
        });
    }
    // Every archive in the corpus puts these immediately after the header, and
    // the folder-offset arithmetic below assumes it.
    if folder_records_offset as usize != HEADER_LEN {
        return Err(BsaError::BadFolderRecordOffset {
            offset: folder_records_offset,
        });
    }

    // Pass 1: the folder records, which give each folder's file count and the
    // offset of its block.
    cursor.at = HEADER_LEN;
    let mut folder_headers = Vec::with_capacity(folder_count);
    for _ in 0..folder_count {
        let _hash = cursor.u64()?;
        let count = cursor.u32()? as usize;
        let offset = cursor.u32()?;
        folder_headers.push((count, offset));
    }

    // Pass 2: each folder's name followed by its file records.
    let mut folders: Vec<Folder<'_>> = Vec::with_capacity(folder_count);
    let mut counts: Vec<usize> = Vec::with_capacity(folder_count);
    let mut records: Vec<(u32, u32)> = Vec::with_capacity(file_count);
    for (count, declared_offset) in folder_headers {
        let block_start = cursor.at as u32;
        // The stored offset is biased by the total file-name length and points
        // at the block's length byte. Checking it here means a malformed
        // archive fails with a precise error instead of silently mis-parsing.
        let expected = declared_offset.wrapping_sub(total_file_name_length);
        if expected != block_start {
            let name_length = cursor.u8()? as usize;
            let name = String::from_utf8_lossy(cursor.take(name_length.saturating_sub(1))?);
            return Err(BsaError::BadFolderOffset {
                folder: name.into_owned(),
                declared: declared_offset,
                actual: block_start.wrapping_add(total_file_name_length),
            });
        }

        // The length byte counts the NUL terminator.
        let name_length = cursor.u8()? as usize;
        let raw = cursor.take(name_length)?;
        let name = String::from_utf8_lossy(&raw[..name_length.saturating_sub(1)]).into_owned();

        for _ in 0..count {
            let _hash = cursor.u64()?;
            let size = cursor.u32()?;
            let offset = cursor.u32()?;
            records.push((size, offset));
        }

        counts.push(count);
        folders.push(Folder {
            name,
            files: Vec::with_capacity(count),
        });
    }

    // Pass 3: the file names, one NUL-terminated run per file in record order.
    let names_start = cursor.at;
    let mut names = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        names.push(cursor.zstring()?);
    }
    let names_length = (cursor.at - names_start) as u32;
    if names_length != total_file_name_length {
        // The folder-offset arithmetic above depends on this being right, so a
        // disagreement means the archive is not laid out the way we think.
        return Err(BsaError::Truncated { offset: names_start });
    }

    let data_region_start = cursor.at;
    let compressed_by_default = archive_flags & FLAG_COMPRESSED != 0;

    // Pass 4: attach names and payloads.
    let mut index = 0usize;
    for (folder, count) in folders.iter_mut().zip(counts) {
        for _ in 0..count {
            let (size_field, offset) = records[index];
            let name = std::mem::take(&mut names[index]);
            index += 1;

            let size = (size_field & SIZE_MASK) as usize;
            let start = offset as usize;
            let end = start
                .checked_add(size)
                .filter(|end| *end <= data.len() && start >= data_region_start)
                .ok_or_else(|| BsaError::FileOutOfBounds {
                    path: format!("{}\\{}", folder.name, name),
                    offset,
                    size: size as u32,
                })?;

            folder.files.push(File {
                name,
                compressed: compressed_by_default
                    != (size_field & SIZE_COMPRESSION_DIFFERS != 0),
                stored: std::borrow::Cow::Borrowed(&data[start..end]),
                offset: Some(offset),
            });
        }
    }

    Ok(Bsa {
        archive_flags,
        file_flags,
        folders,
        source: Some(Source {
            total_folder_name_length,
            data_region: &data[data_region_start..],
            data_region_start: data_region_start as u32,
        }),
    })
}
