//! BSA (Bethesda Softworks Archive) container: read, model and write Oblivion
//! `.bsa` files.
//!
//! Hand-rolled for the same reasons `plugin` is: the format is small, and the
//! alternative is shelling out to BSArch.exe under Wine for every one of the
//! ~17 pack/unpack steps the guide calls for. BSArch's CLI is also known to
//! fail on at least one real mod where its GUI succeeds, so this removes a
//! failure mode rather than just a dependency.
//!
//! # Scope
//!
//! Version 103 (Oblivion) only. All 48 archives in the reference install are
//! version 103; anything else is rejected with a clear error rather than
//! guessed at, because the later versions changed the record layout.
//!
//! # Layout
//!
//! ```text
//! header            36 bytes
//! folder records    16 bytes each: u64 hash, u32 file count, u32 offset
//! folder blocks     per folder: u8 length, NUL-terminated name, then that
//!                   folder's file records (u64 hash, u32 size, u32 offset)
//! file names        every file name, NUL-terminated, in record order
//! file data         the payloads
//! ```
//!
//! A folder record's `offset` is biased by the total file-name length -- it is
//! `total_file_name_length + <offset of the folder block>`, and it points at
//! the block's length byte, not past the name.
//!
//! # Round-tripping
//!
//! `write(parse(bytes)) == bytes` for every archive in the corpus. Two
//! properties of real archives make the naive approach insufficient, and both
//! are why a parsed archive keeps some of its source verbatim:
//!
//! * **The data region is not laid out in record order.** Most archives permute
//!   it, and many deduplicate identical payloads so that two file records point
//!   at the same bytes. One archive (`WACIntegration.bsa`) additionally prefixes
//!   every payload with a redundant `u32` length that no file record points at.
//! * **One archive's header disagrees with its own contents.**
//!   `WACIntegration.bsa` declares a total folder-name length 1750 bytes larger
//!   than the sum of its folder names.
//!
//! So a parsed archive replays its data region and its declared folder-name
//! length verbatim, exactly as `plugin`'s `Record::original_body` replays a
//! compressed record body it did not touch. Everything else -- every offset,
//! count and hash -- is recomputed, so the round-trip still proves the
//! structural model is complete.

pub mod hash;
pub mod reader;
pub mod writer;

pub use hash::{hash_file_name, hash_folder_name};

use std::borrow::Cow;
use std::io::Write;
use std::path::Path;

/// The only version this module handles: Oblivion.
pub const VERSION_OBLIVION: u32 = 103;

/// Archive flag: folder names are stored.
pub const FLAG_FOLDER_NAMES: u32 = 0x0000_0001;
/// Archive flag: file names are stored.
pub const FLAG_FILE_NAMES: u32 = 0x0000_0002;
/// Archive flag: payloads are zlib-compressed by default.
pub const FLAG_COMPRESSED: u32 = 0x0000_0004;

/// Size-field bit meaning "this file's compression differs from the archive
/// default".
pub const SIZE_COMPRESSION_DIFFERS: u32 = 0x4000_0000;
/// The bits of a size field that are actually a size.
pub const SIZE_MASK: u32 = 0x3FFF_FFFF;

/// The fixed 36-byte header.
pub(crate) const HEADER_LEN: usize = 36;
/// A folder record, and also a file record: both are 16 bytes.
pub(crate) const RECORD_LEN: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum BsaError {
    #[error("not a BSA archive (expected a 'BSA\\0' signature)")]
    NotAnArchive,

    #[error(
        "unsupported BSA version {version}: mudcrab reads and writes version \
         {VERSION_OBLIVION} (Oblivion) only"
    )]
    UnsupportedVersion { version: u32 },

    #[error(
        "archive does not store {which} names (archive flags {flags:#x}), which \
         mudcrab requires in order to name its contents"
    )]
    MissingNames { which: &'static str, flags: u32 },

    #[error("archive data ends unexpectedly at offset {offset}")]
    Truncated { offset: usize },

    #[error(
        "archive declares folder records at offset {offset}, but the header is \
         {HEADER_LEN} bytes"
    )]
    BadFolderRecordOffset { offset: u32 },

    #[error(
        "refusing to extract '{path}': the name would escape the destination \
         directory"
    )]
    UnsafePath { path: String },

    #[error(
        "internal: rebuilt metadata is {computed} bytes but the source archive's \
         payloads begin at {expected}, so its stored offsets would be wrong"
    )]
    MetadataLengthChanged { computed: u32, expected: u32 },

    #[error(
        "folder '{folder}' declares its file records at offset {declared}, but \
         they are at {actual}"
    )]
    BadFolderOffset {
        folder: String,
        declared: u32,
        actual: u32,
    },

    #[error("file '{path}' runs past the end of the archive (offset {offset}, size {size})")]
    FileOutOfBounds {
        path: String,
        offset: u32,
        size: u32,
    },

    #[error("failed to decompress '{path}': {source}")]
    Decompress {
        path: String,
        source: std::io::Error,
    },

    #[error("compressed file '{path}' is missing its length prefix")]
    MissingLengthPrefix { path: String },

    #[error(
        "cannot pack '{name}': archive names must be ASCII, and mudcrab will \
         not guess an encoding for the Windows-1252 names Oblivion expects"
    )]
    NonAsciiName { name: String },

    #[error("internal: '{path}' was found while walking '{root}' but is not inside it")]
    PathOutsideRoot { path: String, root: String },

    #[error("cannot pack '{name}': a folder name must be at most 254 bytes")]
    FolderNameTooLong { name: String },

    #[error("cannot pack: the archive would exceed the 4 GiB the format can address")]
    TooLarge,

    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

/// A parsed or constructed archive.
///
/// Borrows the buffer it was parsed from, so payloads are slices rather than
/// copies -- the corpus contains archives approaching 2 GiB.
#[derive(Debug, Clone)]
pub struct Bsa<'a> {
    /// Archive flags, preserved verbatim. Only bits 0, 1 and 2 are interpreted.
    pub archive_flags: u32,
    /// File flags (which asset kinds the archive contains). Preserved verbatim
    /// and never interpreted: Oblivion recomputes them from the contents.
    pub file_flags: u32,
    /// Folders, in hash order.
    pub folders: Vec<Folder<'a>>,
    /// Present only for a parsed archive; see the module docs.
    source: Option<Source<'a>>,
}

/// The parts of a parsed archive replayed verbatim rather than recomputed.
#[derive(Debug, Clone)]
struct Source<'a> {
    /// The header's declared total folder-name length. Kept because
    /// `WACIntegration.bsa` declares a value inconsistent with its own names.
    total_folder_name_length: u32,
    /// The file-data region exactly as it appeared.
    data_region: &'a [u8],
    /// Where that region began, used to verify the rebuilt metadata is the
    /// same size as the original.
    data_region_start: u32,
}

#[derive(Debug, Clone)]
pub struct Folder<'a> {
    /// Backslash-separated and lowercase, as stored.
    pub name: String,
    /// Files, in hash order.
    pub files: Vec<File<'a>>,
}

#[derive(Debug, Clone)]
pub struct File<'a> {
    /// The bare file name, lowercase, as stored.
    pub name: String,
    /// Whether this payload is zlib-compressed.
    pub compressed: bool,
    /// The payload exactly as stored: for a compressed file this is a u32
    /// decompressed length followed by a zlib stream.
    stored: Cow<'a, [u8]>,
    /// Where the payload lives in a parsed archive. `None` for one built in
    /// memory, whose data region is laid out by the writer.
    offset: Option<u32>,
}

impl<'a> Bsa<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, BsaError> {
        reader::parse(data)
    }

    /// Whether payloads are compressed unless a file record says otherwise.
    pub fn compressed_by_default(&self) -> bool {
        self.archive_flags & FLAG_COMPRESSED != 0
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, BsaError> {
        let mut out = Vec::new();
        self.write_to(&mut out)?;
        Ok(out)
    }

    pub fn write_to<W: Write>(&self, out: &mut W) -> Result<(), BsaError> {
        writer::write_to(self, out)
    }

    pub fn write_to_file(&self, path: &Path) -> Result<(), BsaError> {
        let file = std::fs::File::create(path).map_err(|source| BsaError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let mut out = std::io::BufWriter::new(file);
        self.write_to(&mut out)?;
        out.flush().map_err(|source| BsaError::Io {
            path: path.display().to_string(),
            source,
        })
    }

    /// Total number of files across all folders.
    pub fn file_count(&self) -> usize {
        self.folders.iter().map(|folder| folder.files.len()).sum()
    }

    /// Every file, paired with the folder holding it, in archive order.
    pub fn files(&self) -> impl Iterator<Item = (&Folder<'a>, &File<'a>)> {
        self.folders
            .iter()
            .flat_map(|folder| folder.files.iter().map(move |file| (folder, file)))
    }

    /// Archive-relative paths of every file, backslash-separated as stored.
    pub fn paths(&self) -> impl Iterator<Item = String> + '_ {
        self.files().map(|(folder, file)| file.path_in(folder))
    }

    /// Extract every file below `destination`, creating folders as needed.
    ///
    /// Returns the number of files written. Refuses any name that would escape
    /// `destination`.
    pub fn extract_to(&self, destination: &Path) -> Result<usize, BsaError> {
        let mut written = 0usize;
        for (folder, file) in self.files() {
            let relative = crate::util::fs::normalize_relative_path(
                &file.path_in(folder).replace('\\', "/"),
            )
            .map_err(|_| BsaError::UnsafePath {
                path: file.path_in(folder),
            })?;

            let target = destination.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|source| BsaError::Io {
                    path: parent.display().to_string(),
                    source,
                })?;
            }

            let data = file.data(&file.path_in(folder))?;
            std::fs::write(&target, data.as_ref()).map_err(|source| BsaError::Io {
                path: target.display().to_string(),
                source,
            })?;
            written += 1;
        }
        Ok(written)
    }

    /// Build an archive from every file below `root` that passes `filters`.
    ///
    /// Payloads are stored uncompressed: Oblivion cannot read compressed voice
    /// files, and an uncompressed archive is what BSArch produces for Oblivion
    /// by default.
    ///
    /// Files sitting directly in `root` are skipped, because the format has no
    /// way to address a file outside a folder. Use [`root_level_files`] to
    /// report them.
    pub fn from_directory(
        root: &Path,
        filters: &crate::archive::ArchiveFilters,
    ) -> Result<Bsa<'static>, BsaError> {
        writer::from_directory(root, filters)
    }
}

/// Names of the files sitting directly in `root`, which no BSA can store.
///
/// Separate from packing so a caller can tell the user what stayed loose
/// instead of the archive silently omitting it.
pub fn root_level_files(root: &Path) -> Result<Vec<String>, BsaError> {
    let entries = std::fs::read_dir(root).map_err(|source| BsaError::Io {
        path: root.display().to_string(),
        source,
    })?;

    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| BsaError::Io {
            path: root.display().to_string(),
            source,
        })?;
        if entry.file_type().is_ok_and(|kind| kind.is_file()) {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

impl<'a> File<'a> {
    /// The size the file record carries, including the compression-differs bit.
    pub(crate) fn size_field(&self, compressed_by_default: bool) -> u32 {
        let mut size = self.stored.len() as u32;
        if self.compressed != compressed_by_default {
            size |= SIZE_COMPRESSION_DIFFERS;
        }
        size
    }

    /// The payload exactly as stored, still compressed if it is compressed.
    pub fn stored_bytes(&self) -> &[u8] {
        self.stored.as_ref()
    }

    /// `folder\name`, the path Oblivion knows the file by.
    pub fn path_in(&self, folder: &Folder<'_>) -> String {
        format!("{}\\{}", folder.name, self.name)
    }

    /// The file's real contents, decompressing if needed.
    ///
    /// `path` is used only to name the file in an error.
    pub fn data(&self, path: &str) -> Result<Cow<'_, [u8]>, BsaError> {
        if !self.compressed {
            return Ok(Cow::Borrowed(self.stored.as_ref()));
        }

        let stored = self.stored.as_ref();
        if stored.len() < 4 {
            return Err(BsaError::MissingLengthPrefix {
                path: path.to_string(),
            });
        }
        let expected = u32::from_le_bytes(stored[0..4].try_into().unwrap()) as usize;

        use flate2::read::ZlibDecoder;
        use std::io::Read;
        let mut out = Vec::with_capacity(expected);
        ZlibDecoder::new(&stored[4..])
            .read_to_end(&mut out)
            .map_err(|source| BsaError::Decompress {
                path: path.to_string(),
                source,
            })?;
        Ok(Cow::Owned(out))
    }
}
