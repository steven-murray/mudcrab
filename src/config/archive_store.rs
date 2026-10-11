//! The downloads folder: archives under their real names, as MO2 keeps them.
//!
//! An archive is identified by what it *is*, never by where the modlist uses
//! it. A Nexus file is its game, mod id and file id, read from the `.meta` file
//! MO2 writes beside every download; anything else is its filename. So renaming
//! a mod, reordering its archives or using one archive in seven mods changes
//! nothing on disk, and the folder is an ordinary MO2 downloads folder that MO2
//! can be pointed straight at.
//!
//! Lookup runs over the downloads folder first, then each read-only
//! `--archive-search-path` in order. An archive found in a search path is
//! hard-linked into the downloads folder under its own name, with its `.meta`,
//! so the downloads folder is always the one place install reads from.

use crate::util::fs::link_or_copy;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// What the modlist knows about one archive.
#[derive(Debug, Clone, Copy)]
pub struct ArchiveRef<'a> {
    /// `nexus:<game>/<mod>/<file>`, `manual:<name>`, a URL, or a local path.
    pub source: &'a str,
    /// The archive's own filename, when the modlist declares it.
    pub file_name: Option<&'a str>,
}

impl<'a> ArchiveRef<'a> {
    pub fn new(source: &'a str, file_name: Option<&'a str>) -> Self {
        Self { source, file_name }
    }

    /// `(game, mod id, file id)` for a Nexus source.
    pub fn nexus_ids(&self) -> Option<(&'a str, &'a str, &'a str)> {
        let rest = self.source.strip_prefix("nexus:")?;
        let mut parts = rest.split('/');
        let ids = (parts.next()?, parts.next()?, parts.next()?);
        parts.next().is_none().then_some(ids)
    }

    /// The filename this archive is expected to have, if anything says.
    pub fn expected_name(&self) -> Option<&'a str> {
        if let Some(name) = self.file_name.map(str::trim).filter(|n| !n.is_empty()) {
            return Some(name);
        }
        if let Some(name) = self.source.strip_prefix("manual:") {
            return Some(name.trim());
        }
        if self.nexus_ids().is_some() {
            return None;
        }
        // A URL or a local path: its last segment.
        let tail = self.source.rsplit(['/', '\\']).next()?;
        let tail = tail.split('?').next().unwrap_or(tail);
        (!tail.is_empty()).then_some(tail)
    }

    /// For error messages: the filename if known, else the source.
    pub fn describe(&self) -> &'a str {
        self.expected_name().unwrap_or(self.source)
    }
}

/// MO2's `gameName` for a Nexus game domain, where the two differ in a way
/// that matters. Unknown domains skip the game check rather than fail it:
/// file ids are unique within a game, and a collision across games is not
/// worth refusing a match over.
fn mo2_game_name(domain: &str) -> Option<&'static str> {
    Some(match domain.to_ascii_lowercase().as_str() {
        "oblivion" => "Oblivion",
        "morrowind" => "Morrowind",
        "skyrim" => "Skyrim",
        "skyrimspecialedition" => "SkyrimSE",
        "fallout3" => "Fallout3",
        "newvegas" => "FalloutNV",
        "fallout4" => "Fallout4",
        _ => return None,
    })
}

/// Archives carrying one Nexus `(mod id, file id)`, each with the `gameName`
/// its `.meta` names, if any.
type NexusMatches = Vec<(Option<String>, PathBuf)>;

/// One folder's archives, by Nexus ids and by lower-cased filename.
#[derive(Debug, Default, Clone)]
struct DirIndex {
    by_nexus: HashMap<(String, String), NexusMatches>,
    by_name: HashMap<String, PathBuf>,
    /// By the `url` in a `.meta`, for archives fetched from a plain URL.
    by_url: HashMap<String, PathBuf>,
}

fn is_sidecar(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".meta") || lower.ends_with(".orig-name") || lower.ends_with(".part")
}

fn build_index(dir: &Path) -> DirIndex {
    let mut index = DirIndex::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return index;
    };
    let mut metas = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.path().is_file() {
            continue;
        }
        if name.to_ascii_lowercase().ends_with(".meta") {
            metas.push(entry.path());
        } else if !is_sidecar(&name) {
            index.by_name.insert(name.to_ascii_lowercase(), entry.path());
        }
    }
    for meta_path in metas {
        let archive = meta_path.with_extension("");
        if !archive.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&meta_path) else {
            continue;
        };
        let meta = parse_meta(&text);
        if let Some(url) = meta.get("url").filter(|url| !url.is_empty()) {
            index.by_url.entry(url.clone()).or_insert_with(|| archive.clone());
        }
        if let (Some(mod_id), Some(file_id)) = (meta.get("modid"), meta.get("fileid"))
            && mod_id != "0"
            && file_id != "0"
        {
            index
                .by_nexus
                .entry((mod_id.clone(), file_id.clone()))
                .or_default()
                .push((meta.get("gamename").cloned(), archive));
        }
    }
    index
}

/// `key=value` lines of an MO2 `.meta`, keys lower-cased, quotes stripped.
fn parse_meta(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            (
                key.trim().to_ascii_lowercase(),
                value.trim().trim_matches('"').to_string(),
            )
        })
        .collect()
}

/// Process-wide indexes, one per folder. A hit is re-checked on disk and a
/// miss rebuilds the folder's index once, so a file added or removed behind
/// our back is never misreported for long.
fn indexes() -> &'static Mutex<HashMap<PathBuf, DirIndex>> {
    static INDEXES: OnceLock<Mutex<HashMap<PathBuf, DirIndex>>> = OnceLock::new();
    INDEXES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lookup_in(dir: &Path, archive: &ArchiveRef<'_>, index: &DirIndex) -> Option<PathBuf> {
    if let Some((game, mod_id, file_id)) = archive.nexus_ids()
        && let Some(candidates) = index.by_nexus.get(&(mod_id.to_string(), file_id.to_string()))
    {
        let wanted_game = mo2_game_name(game);
        let found = candidates.iter().find(|(meta_game, path)| {
            path.is_file()
                && match (wanted_game, meta_game) {
                    (Some(wanted), Some(have)) => wanted.eq_ignore_ascii_case(have),
                    _ => true,
                }
        });
        if let Some((_, path)) = found {
            return Some(path.clone());
        }
    }
    if let Some(name) = archive.expected_name()
        && let Some(path) = index.by_name.get(&name.to_ascii_lowercase())
        && path.is_file()
    {
        return Some(path.clone());
    }
    if (archive.source.starts_with("http://") || archive.source.starts_with("https://"))
        && let Some(path) = index.by_url.get(archive.source)
        && path.is_file()
    {
        return Some(path.clone());
    }
    let _ = dir;
    None
}

/// Find `archive` in one folder, without writing anything.
fn find_in(dir: &Path, archive: &ArchiveRef<'_>) -> Option<PathBuf> {
    let mut indexes = indexes().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(index) = indexes.get(dir)
        && let Some(found) = lookup_in(dir, archive, index)
    {
        return Some(found);
    }
    let fresh = build_index(dir);
    let found = lookup_in(dir, archive, &fresh);
    indexes.insert(dir.to_path_buf(), fresh);
    found
}

/// Forget a folder's index after writing to it.
fn invalidate(dir: &Path) {
    let mut indexes = indexes().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    indexes.remove(dir);
}

/// Where an archive was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// Already in the downloads folder.
    Downloads(PathBuf),
    /// In a read-only search path, not yet in the downloads folder.
    SearchPath(PathBuf),
}

/// Find an archive in the downloads folder, then the search paths. Read-only.
pub fn locate(downloads: &Path, search_paths: &[PathBuf], archive: &ArchiveRef<'_>) -> Option<Found> {
    if let Some(path) = find_in(downloads, archive) {
        return Some(Found::Downloads(path));
    }
    search_paths
        .iter()
        .filter(|dir| dir.as_path() != downloads)
        .find_map(|dir| find_in(dir, archive))
        .map(Found::SearchPath)
}

/// Find an archive's path, in the downloads folder or a search path, without
/// writing anything. For commands that only read.
pub fn find(downloads: &Path, search_paths: &[PathBuf], archive: &ArchiveRef<'_>) -> Option<PathBuf> {
    locate(downloads, search_paths, archive).map(|found| match found {
        Found::Downloads(path) | Found::SearchPath(path) => path,
    })
}

/// Find an archive and make sure it is in the downloads folder, linking it in
/// from a search path if that is where it is. `None` if it is nowhere.
pub fn resolve(
    downloads: &Path,
    search_paths: &[PathBuf],
    archive: &ArchiveRef<'_>,
) -> anyhow::Result<Option<PathBuf>> {
    match locate(downloads, search_paths, archive) {
        None => Ok(None),
        Some(Found::Downloads(path)) => {
            if let Some(newer) = superseded_by(&path, downloads, search_paths) {
                std::fs::remove_file(&path)
                    .map_err(|err| anyhow::anyhow!("failed to replace {}: {err}", path.display()))?;
                link_or_copy(&newer, &path)?;
                invalidate(downloads);
            }
            Ok(Some(path))
        }
        Some(Found::SearchPath(source)) => adopt(downloads, &source, archive).map(Some),
    }
}

/// A same-named archive in a search path that is newer and a different size
/// than the downloads-folder copy.
///
/// A downloads-folder copy linked in from a search path does not change when
/// the search-path file is replaced: re-download a broken archive there and the
/// downloads folder keeps serving the broken bytes. That happened, silently,
/// with a MOFAM archive. Size rather than a hash, because this runs per archive
/// per install over tens of gigabytes; newer, so a copy deliberately updated in
/// the downloads folder itself is never overwritten by an older one.
fn superseded_by(path: &Path, downloads: &Path, search_paths: &[PathBuf]) -> Option<PathBuf> {
    let name = path.file_name()?;
    let ours = path.metadata().ok()?;
    search_paths
        .iter()
        .filter(|dir| dir.as_path() != downloads)
        .map(|dir| dir.join(name))
        .find(|candidate| {
            candidate.metadata().is_ok_and(|theirs| {
                theirs.is_file()
                    && theirs.len() != ours.len()
                    && matches!(
                        (theirs.modified(), ours.modified()),
                        (Ok(theirs), Ok(ours)) if theirs > ours
                    )
            })
        })
        .inspect(|newer| {
            tracing::warn!(
                stale = %path.display(),
                source = %newer.display(),
                "archive in the downloads folder is older than the one in a search path; replacing"
            );
        })
}

/// Link a search-path archive into the downloads folder under its own name,
/// bringing its `.meta` (or writing one for a Nexus source).
fn adopt(downloads: &Path, source: &Path, archive: &ArchiveRef<'_>) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(downloads)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", downloads.display()))?;
    let name = source
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("archive path has no file name: {}", source.display()))?;
    let destination = downloads.join(name);
    if !destination.exists() {
        link_or_copy(source, &destination)?;
    }
    let source_meta = meta_path(source);
    let destination_meta = meta_path(&destination);
    if !destination_meta.exists() {
        if source_meta.is_file() {
            link_or_copy(&source_meta, &destination_meta)?;
        } else {
            write_meta(&destination, archive)?;
        }
    }
    invalidate(downloads);
    tracing::info!(
        source = %source.display(),
        destination = %destination.display(),
        "archive linked into the downloads folder from a search path"
    );
    Ok(destination)
}

/// `<archive>.meta`.
pub fn meta_path(archive: &Path) -> PathBuf {
    let mut path = archive.as_os_str().to_os_string();
    path.push(".meta");
    PathBuf::from(path)
}

/// Write the `.meta` MO2 would, so MO2 lists the archive as what it is and a
/// later lookup by Nexus ids finds it. Only for sources that say something:
/// a Nexus file, or a URL. Never overwrites one.
pub fn write_meta(archive_path: &Path, archive: &ArchiveRef<'_>) -> anyhow::Result<()> {
    let path = meta_path(archive_path);
    if path.exists() {
        return Ok(());
    }
    let name = archive_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let body = if let Some((game, mod_id, file_id)) = archive.nexus_ids() {
        format!(
            "[General]\ngameName={}\nmodID={mod_id}\nfileID={file_id}\nname={name}\n\
             repository=Nexus\ninstalled=false\nuninstalled=false\nremoved=false\n",
            mo2_game_name(game).unwrap_or(game),
        )
    } else if archive.source.starts_with("http://") || archive.source.starts_with("https://") {
        format!(
            "[General]\nurl={}\nname={name}\ninstalled=false\nuninstalled=false\nremoved=false\n",
            archive.source
        )
    } else {
        return Ok(());
    };
    std::fs::write(&path, body)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))?;
    Ok(())
}

/// Whether an archive's `.meta` says what it is: Nexus ids that are not zero,
/// or a URL. MO2 writes `modID=3`/`fileID=0`-style placeholders for downloads
/// it could not identify, which find nothing.
pub fn meta_is_informative(archive: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(meta_path(archive)) else {
        return false;
    };
    let meta = parse_meta(&text);
    let id = |key: &str| meta.get(key).is_some_and(|v| !v.is_empty() && v != "0");
    (id("modid") && id("fileid")) || meta.get("url").is_some_and(|url| !url.is_empty())
}

/// Give an archive a `.meta` that identifies it: write one if there is none,
/// or fill the ids into an uninformative one, keeping its other lines.
pub fn fill_meta(archive_path: &Path, archive: &ArchiveRef<'_>) -> anyhow::Result<()> {
    let path = meta_path(archive_path);
    if !path.exists() {
        return write_meta(archive_path, archive);
    }
    if meta_is_informative(archive_path) {
        return Ok(());
    }
    let mut keys: Vec<(&str, String)> = Vec::new();
    if let Some((game, mod_id, file_id)) = archive.nexus_ids() {
        keys.push(("gameName", mo2_game_name(game).unwrap_or(game).to_string()));
        keys.push(("modID", mod_id.to_string()));
        keys.push(("fileID", file_id.to_string()));
    } else if archive.source.starts_with("http://") || archive.source.starts_with("https://") {
        keys.push(("url", archive.source.to_string()));
    } else {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    for (key, value) in keys {
        let found = lines.iter().position(|line| {
            line.split_once('=').is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(key))
        });
        let assignment = format!("{key}={value}");
        match found {
            Some(index) => lines[index] = assignment,
            None => {
                let at = lines
                    .iter()
                    .position(|line| line.trim().eq_ignore_ascii_case("[General]"))
                    .map_or(lines.len(), |index| index + 1);
                lines.insert(at, assignment);
            }
        }
    }
    let mut out = lines.join(newline);
    out.push_str(newline);
    std::fs::write(&path, out).map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))
}

/// Record that something was written into `dir`, so the next lookup sees it.
pub fn note_written(dir: &Path) {
    invalidate(dir);
}

/// Detect the archive extension by reading magic bytes.
pub fn detect_archive_ext(path: &Path) -> &'static str {
    let Ok(mut file) = std::fs::File::open(path) else {
        return "";
    };
    let mut buf = [0u8; 8];
    let n = file.read(&mut buf).unwrap_or(0);
    if n < 2 {
        return "";
    }
    // 7z: 37 7A BC AF 27 1C
    if n >= 6 && buf[..6] == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
        return ".7z";
    }
    // Zip: 50 4B 03 04
    if n >= 4 && buf[..4] == [0x50, 0x4B, 0x03, 0x04] {
        return ".zip";
    }
    // Rar: 52 61 72 21 (Rar!)
    if n >= 4 && buf[..4] == [0x52, 0x61, 0x72, 0x21] {
        return ".rar";
    }
    // gzip (tar.gz): 1F 8B
    if n >= 2 && buf[..2] == [0x1F, 0x8B] {
        return ".tar.gz";
    }
    ""
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn a_nexus_archive_is_found_by_its_meta_whatever_it_is_called() {
        let dir = tempfile::tempdir().unwrap();
        let archive = touch(dir.path(), "Candid ENB Reborn-47810-6-0 (1).rar", "x");
        touch(
            dir.path(),
            "Candid ENB Reborn-47810-6-0 (1).rar.meta",
            "[General]\ngameName=Oblivion\nmodID=47810\nfileID=1000018329\n",
        );
        let wanted = ArchiveRef::new("nexus:oblivion/47810/1000018329", None);
        assert_eq!(locate(dir.path(), &[], &wanted), Some(Found::Downloads(archive)));

        let wrong_game = ArchiveRef::new("nexus:skyrimspecialedition/47810/1000018329", None);
        assert_eq!(locate(dir.path(), &[], &wrong_game), None);
    }

    #[test]
    fn a_manual_archive_is_found_by_name_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        let archive = touch(dir.path(), "WACv_1beta.7z", "x");
        let wanted = ArchiveRef::new("manual:wacv_1BETA.7z", None);
        assert_eq!(locate(dir.path(), &[], &wanted), Some(Found::Downloads(archive)));
    }

    #[test]
    fn a_search_path_archive_is_linked_in_with_its_meta() {
        let downloads = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        touch(elsewhere.path(), "Mod-1-2-3.7z", "payload");
        touch(
            elsewhere.path(),
            "Mod-1-2-3.7z.meta",
            "[General]\ngameName=Oblivion\nmodID=1\nfileID=3\n",
        );
        let wanted = ArchiveRef::new("nexus:oblivion/1/3", None);
        let resolved = resolve(downloads.path(), &[elsewhere.path().to_path_buf()], &wanted)
            .unwrap()
            .unwrap();
        assert_eq!(resolved, downloads.path().join("Mod-1-2-3.7z"));
        assert!(downloads.path().join("Mod-1-2-3.7z.meta").is_file());
        // And now it is simply there.
        assert_eq!(
            locate(downloads.path(), &[], &wanted),
            Some(Found::Downloads(resolved))
        );
    }

    #[test]
    fn a_file_added_after_a_lookup_is_still_found() {
        let dir = tempfile::tempdir().unwrap();
        let wanted = ArchiveRef::new("manual:late.zip", None);
        assert_eq!(locate(dir.path(), &[], &wanted), None);
        let archive = touch(dir.path(), "late.zip", "x");
        assert_eq!(locate(dir.path(), &[], &wanted), Some(Found::Downloads(archive)));
    }

    #[test]
    fn a_url_archive_is_found_by_the_url_in_its_meta() {
        let dir = tempfile::tempdir().unwrap();
        let archive = touch(dir.path(), "Map.7z", "x");
        let source = ArchiveRef::new("https://example.com/start/218291?referer=x", None);
        write_meta(&archive, &source).unwrap();
        note_written(dir.path());
        assert_eq!(locate(dir.path(), &[], &source), Some(Found::Downloads(archive)));
    }

    #[test]
    fn written_meta_is_what_a_nexus_lookup_reads() {
        let dir = tempfile::tempdir().unwrap();
        let archive = touch(dir.path(), "Fetched.7z", "x");
        let source = ArchiveRef::new("nexus:oblivion/5/9", None);
        write_meta(&archive, &source).unwrap();
        note_written(dir.path());
        assert_eq!(locate(dir.path(), &[], &source), Some(Found::Downloads(archive)));
    }
}
