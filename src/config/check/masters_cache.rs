//! Each archive's plugins' masters, kept between checks. Reading a plugin's
//! header means unpacking it, and in a solid 7z that is everything before it
//! too: MOO's one plugin, at the end of a 900 MB block, takes 25 seconds. An
//! archive is the same file while its path, size and modification time are,
//! so what it held is read once.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) struct ArchiveMasters {
    /// Where these are kept; `None` when the archive could not be read.
    file: Option<PathBuf>,
    /// Entry, as the archive names it -> the masters its header names.
    known: BTreeMap<String, Vec<String>>,
    changed: bool,
}

impl ArchiveMasters {
    pub(super) fn load(archive: &Path) -> Self {
        Self::load_kind(archive, "masters")
    }

    /// The same, for another kind of fact about each entry, kept apart: the
    /// files a plugin names, say.
    pub(super) fn load_kind(archive: &Path, kind: &str) -> Self {
        let file = key(archive).map(|key| {
            crate::util::fs::cache_root()
                .join(kind)
                .join(format!("{key}.json"))
        });
        let known = file
            .as_ref()
            .and_then(|file| std::fs::read(file).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            file,
            known,
            changed: false,
        }
    }

    pub(super) fn get(&self, entry: &str) -> Option<&Vec<String>> {
        self.known.get(entry)
    }

    pub(super) fn put(&mut self, entry: &str, masters: &[String]) {
        self.known.insert(entry.to_string(), masters.to_vec());
        self.changed = true;
    }

    /// Best effort: a cache that cannot be written is read again next time.
    pub(super) fn save(&self) {
        let Some(file) = self.file.as_ref().filter(|_| self.changed) else {
            return;
        };
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_vec(&self.known) {
            let _ = std::fs::write(file, json);
        }
    }
}

/// The archive's file list, kept as its masters are: listing a 7z or a RAR
/// is a process each time, and the list scan lists every archive.
pub(crate) fn listing(archive: &Path) -> anyhow::Result<Vec<String>> {
    let mut kept = ArchiveMasters::load_kind(archive, "listing");
    if let Some(paths) = kept.get("") {
        return Ok(paths.clone());
    }
    let paths = crate::archive::list_archive_paths(archive)?;
    kept.put("", &paths);
    kept.save();
    Ok(paths)
}

/// Where entries read from the archive (a FOMOD's config) are kept, while
/// it is the same file.
pub(crate) fn entries_dir(archive: &Path) -> Option<PathBuf> {
    key(archive).map(|key| crate::util::fs::cache_root().join("entries").join(key))
}

/// The archive's path, size and modification time, hashed.
fn key(archive: &Path) -> Option<String> {
    let meta = std::fs::metadata(archive).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let path = std::fs::canonicalize(archive).unwrap_or_else(|_| archive.to_path_buf());
    let mut hash = Sha256::new();
    hash.update(path.to_string_lossy().as_bytes());
    hash.update(meta.len().to_le_bytes());
    hash.update(modified.to_le_bytes());
    Some(format!("{:x}", hash.finalize()))
}
