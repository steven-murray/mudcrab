//! Archives fetched by hand: the page to open for each, and noticing when the
//! file has arrived -- in the downloads folder (where MO2's "Mod Manager
//! Download" saves), or in the browser's own downloads folder, from where it
//! is taken in.
//!
//! `download --browser` drives this from the terminal; a GUI drives it from
//! its own view. The click itself is never automated -- that is the person's,
//! by Nexus's terms as much as by design.

use crate::config::archive_store::{self, ArchiveRef};
use crate::config::download::Unfetched;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// What happens to an archive the browser saved in its own folder.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum BrowserDownloads {
    /// Leave it; you will put it in the downloads folder yourself.
    Leave,
    /// Move it into the downloads folder.
    Move,
    /// Hard-link it into the downloads folder, leaving it where it is too.
    Link,
}

/// The page to open for an archive: a Nexus file's download page, or a URL
/// itself. A `manual:` archive or a local path has none.
pub fn page_for(source: &str) -> Option<String> {
    if let Some((game, mod_id, file_id)) = ArchiveRef::new(source, None).nexus_ids() {
        return Some(nexus_page(game, mod_id, file_id, MOD_MANAGER_LINKS));
    }
    (source.starts_with("http://") || source.starts_with("https://")).then(|| source.to_string())
}

/// Whether to offer Nexus's "Mod Manager Download". On Linux it is not: the
/// `nxm:` link goes to whichever MO2 owns such links, which with more than
/// one instance is often not this list's, and the usual Linux handler (it
/// starts the game's Steam entry) does nothing once that is running. The
/// browser's own download always works.
pub const MOD_MANAGER_LINKS: bool = !cfg!(target_os = "linux");

/// A Nexus file's download page: with `mod_manager`, the one whose button
/// hands the file to MO2 (`nmm=1`); without, the one whose "Slow download"
/// has the browser save it.
pub fn nexus_page(game: &str, mod_id: &str, file_id: &str, mod_manager: bool) -> String {
    let nmm = if mod_manager { "&nmm=1" } else { "" };
    format!("https://www.nexusmods.com/{game}/mods/{mod_id}?tab=files&file_id={file_id}{nmm}")
}

/// The page to open for one unfetched archive: its source's, or the page the
/// list gives for a hand download.
pub fn page_of(item: &Unfetched) -> Option<String> {
    page_for(&item.source).or_else(|| item.page.clone())
}

/// The folder browsers save to: XDG's download directory on Linux, else
/// `~/Downloads`.
pub fn default_watch_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    if let Ok(text) = std::fs::read_to_string(config.join("user-dirs.dirs")) {
        for line in text.lines() {
            if let Some(value) = line.trim().strip_prefix("XDG_DOWNLOAD_DIR=") {
                let value = value.trim().trim_matches('"');
                let path = value.replace("$HOME", &home.to_string_lossy());
                return Some(PathBuf::from(path));
            }
        }
    }
    Some(home.join("Downloads"))
}

/// Open `url` in the default browser, without waiting for it.
pub fn open_url(url: &str) -> bool {
    use std::process::{Command, Stdio};
    let mut command = if cfg!(windows) {
        let mut c = crate::util::process::console("cmd");
        c.args(["/C", "start", "", url]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

/// A file a browser or MO2 has finished writing: not empty, not shadowed by a
/// partial-download file, and the same size as when last looked at.
pub fn settled(path: &Path, sizes: &mut HashMap<PathBuf, u64>) -> bool {
    let Ok(len) = path.metadata().map(|m| m.len()) else {
        return false;
    };
    let partial = [".part", ".crdownload", ".unfinished", ".download"]
        .iter()
        .any(|suffix| {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            Path::new(&name).exists()
        });
    let steady = sizes.insert(path.to_path_buf(), len) == Some(len);
    len > 0 && !partial && steady
}

/// Move (or link) a browser download into the downloads folder, with the
/// `.meta` MO2 would have written.
pub fn adopt(
    file: &Path,
    downloads: &Path,
    archive: &ArchiveRef<'_>,
    how: BrowserDownloads,
) -> anyhow::Result<PathBuf> {
    let name = file
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("{} has no file name", file.display()))?;
    let destination = downloads.join(name);
    match how {
        BrowserDownloads::Leave => return Ok(file.to_path_buf()),
        BrowserDownloads::Link => crate::util::fs::link_or_copy(file, &destination)?,
        BrowserDownloads::Move => {
            if std::fs::rename(file, &destination).is_err() {
                // Another filesystem: copy, then remove the original.
                std::fs::copy(file, &destination).map_err(|err| {
                    anyhow::anyhow!(
                        "failed to move {} into {}: {err}",
                        file.display(),
                        downloads.display()
                    )
                })?;
                std::fs::remove_file(file)
                    .map_err(|err| anyhow::anyhow!("failed to remove {}: {err}", file.display()))?;
            }
        }
    }
    archive_store::write_meta(&destination, archive)?;
    Ok(destination)
}

/// Where an archive being waited for has turned up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// In the downloads folder, finished.
    Here(PathBuf),
    /// Finished, in the browser's folder: to be taken in (`adopt`) or left.
    InBrowserFolder(PathBuf),
}

/// Looks for archives as they arrive.
pub struct Watcher {
    downloads: PathBuf,
    browser: Option<PathBuf>,
    sizes: HashMap<PathBuf, u64>,
}

impl Watcher {
    /// Watch `downloads`, and the browser's folder (`browser`, else the
    /// usual one) unless that is the downloads folder itself.
    pub fn new(downloads: &Path, browser: Option<PathBuf>) -> Self {
        let browser = browser
            .or_else(default_watch_dir)
            .filter(|dir| dir.is_dir())
            .filter(|dir| std::fs::canonicalize(dir).ok() != std::fs::canonicalize(downloads).ok());
        Self {
            downloads: downloads.to_path_buf(),
            browser,
            sizes: HashMap::new(),
        }
    }

    /// The browser's folder being watched, if any.
    pub fn browser_folder(&self) -> Option<&Path> {
        self.browser.as_deref()
    }

    /// Where `item` is, if it has arrived and finished arriving. A file still
    /// being written is seen on a later look, once its size holds.
    pub fn look(&mut self, item: &Unfetched) -> Option<Found> {
        let reference = reference_of(item);
        if let Some(path) = archive_store::find(&self.downloads, &[], &reference)
            && settled(&path, &mut self.sizes)
        {
            return Some(Found::Here(path));
        }
        let file = archive_store::find(self.browser.as_ref()?, &[], &reference)?;
        settled(&file, &mut self.sizes).then_some(Found::InBrowserFolder(file))
    }

    /// Take a file from the browser's folder into the downloads folder, as
    /// `how` says.
    pub fn take_in(
        &self,
        file: &Path,
        item: &Unfetched,
        how: BrowserDownloads,
    ) -> anyhow::Result<PathBuf> {
        adopt(file, &self.downloads, &reference_of(item), how)
    }
}

/// How an archive being waited for is recognised.
pub fn reference_of(item: &Unfetched) -> ArchiveRef<'_> {
    let mut reference = ArchiveRef::new(&item.source, item.file_name.as_deref());
    reference.content = item
        .content
        .as_ref()
        .map(|(size, md5)| (*size, md5.as_str()));
    reference
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_for_nexus_files_and_urls_only() {
        assert_eq!(
            page_for("nexus:oblivion/47085/1000022995"),
            Some(nexus_page(
                "oblivion",
                "47085",
                "1000022995",
                MOD_MANAGER_LINKS
            ))
        );
        assert_eq!(
            nexus_page("oblivion", "47085", "1000022995", true),
            "https://www.nexusmods.com/oblivion/mods/47085?tab=files&file_id=1000022995&nmm=1"
        );
        assert_eq!(
            nexus_page("oblivion", "47085", "1000022995", false),
            "https://www.nexusmods.com/oblivion/mods/47085?tab=files&file_id=1000022995"
        );
        assert_eq!(
            page_for("https://x.example/a.7z").as_deref(),
            Some("https://x.example/a.7z")
        );
        assert_eq!(page_for("manual:a.7z"), None);
    }

    #[test]
    fn a_file_still_being_written_is_not_taken() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.7z");
        let mut sizes = HashMap::new();
        std::fs::write(&file, b"").unwrap();
        assert!(!settled(&file, &mut sizes), "empty: Firefox's placeholder");
        std::fs::write(&file, b"abc").unwrap();
        std::fs::write(dir.path().join("a.7z.part"), b"").unwrap();
        assert!(!settled(&file, &mut sizes), "a .part beside it");
        std::fs::remove_file(dir.path().join("a.7z.part")).unwrap();
        assert!(settled(&file, &mut sizes), "same size as last time");
        std::fs::write(&file, b"abcd").unwrap();
        assert!(!settled(&file, &mut sizes), "grew since");
        assert!(settled(&file, &mut sizes));
    }

    #[test]
    fn a_browser_download_is_moved_in_with_its_meta() {
        let watch = tempfile::tempdir().unwrap();
        let downloads = tempfile::tempdir().unwrap();
        let file = watch.path().join("Mod-1-2.7z");
        std::fs::write(&file, b"x").unwrap();
        let reference = ArchiveRef::new("nexus:oblivion/1/2", Some("Mod-1-2.7z"));
        let moved = adopt(&file, downloads.path(), &reference, BrowserDownloads::Move).unwrap();
        assert!(!file.exists());
        assert!(moved.exists());
        let meta = std::fs::read_to_string(archive_store::meta_path(&moved)).unwrap();
        assert!(
            meta.contains("modID=1") && meta.contains("fileID=2"),
            "{meta}"
        );
    }
}
