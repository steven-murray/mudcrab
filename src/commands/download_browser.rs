//! `download --browser`: the archives a direct download could not fetch,
//! fetched by hand with the browser, as fast as clicking allows.
//!
//! A free Nexus account cannot be given a download link by the API, and a
//! `manual:` archive has none at all; someone has to click. This opens each
//! file's page, a batch at a time, and watches for the result: in the downloads
//! folder, where MO2 saves what its "Mod Manager Download" button fetches (it
//! is mudcrab's downloads folder when `install` has set the instance up), and
//! in the browser's own downloads folder. Each arrival opens the next page.
//!
//! The click itself is never automated -- that is the person's, by Nexus's
//! terms as much as by design.

use super::choose::{Availability, Choice, Question, Rule, ask};
use crate::cli::BrowserDownloads;
use crate::config::archive_store::{self, ArchiveRef};
use crate::config::download::Unfetched;
use crate::ui::Tone;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How often the folders are looked at.
const POLL: Duration = Duration::from_secs(2);

/// The page to open for an archive: a Nexus file's download page, or a URL
/// itself. A `manual:` archive or a local path has none.
fn page_for(source: &str) -> Option<String> {
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
const MOD_MANAGER_LINKS: bool = !cfg!(target_os = "linux");

/// A Nexus file's download page: with `mod_manager`, the one whose button
/// hands the file to MO2 (`nmm=1`); without, the one whose "Slow download"
/// has the browser save it.
fn nexus_page(game: &str, mod_id: &str, file_id: &str, mod_manager: bool) -> String {
    let nmm = if mod_manager { "&nmm=1" } else { "" };
    format!("https://www.nexusmods.com/{game}/mods/{mod_id}?tab=files&file_id={file_id}{nmm}")
}

/// The page to open for one unfetched archive: its source's, or the page the
/// list gives for a hand download.
fn page_of(item: &Unfetched) -> Option<String> {
    page_for(&item.source).or_else(|| item.page.clone())
}

/// The folder browsers save to: XDG's download directory on Linux, else
/// `~/Downloads`.
fn default_watch_dir() -> Option<PathBuf> {
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
fn open_url(url: &str) -> bool {
    use std::process::{Command, Stdio};
    let mut command = if cfg!(windows) {
        let mut c = Command::new("cmd");
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

/// Whether pages can be opened for someone to click: a terminal to talk to,
/// and (on Linux) a desktop session with `xdg-open`.
pub(crate) fn available() -> bool {
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        return false;
    }
    if cfg!(windows) || cfg!(target_os = "macos") {
        return true;
    }
    let desktop = ["DISPLAY", "WAYLAND_DISPLAY"]
        .iter()
        .any(|var| std::env::var_os(var).is_some_and(|v| !v.is_empty()));
    let xdg_open = std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("xdg-open").is_file()));
    desktop && xdg_open
}

/// A browser started from here takes its profile from `XDG_CONFIG_HOME`. If
/// that is not where the usual one lives, a second browser starts on a fresh
/// profile -- even while the usual one is open, since Chromium-based browsers
/// keep their one-instance lock in the profile. No bookmarks, no Nexus login,
/// and an unbranded look that can pass for another browser.
fn profile_warning() -> Option<String> {
    if cfg!(windows) || cfg!(target_os = "macos") {
        return None;
    }
    let config = PathBuf::from(std::env::var_os("XDG_CONFIG_HOME")?);
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let usual = home.join(".config");
    if std::fs::canonicalize(&config).ok() == std::fs::canonicalize(&usual).ok() {
        return None;
    }
    Some(format!(
        "note: XDG_CONFIG_HOME is {}, so the browser opened from here looks for its \
         profile there and may start fresh, as a second window without your Nexus \
         login. If it does, run this command with XDG_CONFIG_HOME unset (to keep a \
         separate tools.toml, MUDCRAB_TOOLS_CONFIG names one without moving anything else).",
        config.display()
    ))
}

/// A file a browser or MO2 has finished writing: not empty, not shadowed by a
/// partial-download file, and the same size as when last looked at.
fn settled(path: &Path, sizes: &mut HashMap<PathBuf, u64>) -> bool {
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
fn adopt(
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

/// Ask what to do with browser downloads, the first time one arrives.
fn ask_policy(watch: &Path, downloads: &Path) -> anyhow::Result<BrowserDownloads> {
    let stdin = std::io::stdin();
    let question = Question {
        title: format!(
            "An archive arrived in {}. What should happen to the ones that land there?",
            watch.display()
        ),
        rule: Rule::ExactlyOne,
        choices: vec![
            Choice {
                name: "Leave them".to_string(),
                description: format!(
                    "You will put them in {} yourself (or change where the browser saves).",
                    downloads.display()
                ),
                availability: Availability::Free,
            },
            Choice {
                name: "Move them into the downloads folder".to_string(),
                description: String::new(),
                availability: Availability::Recommended,
            },
            Choice {
                name: "Hard-link them into the downloads folder".to_string(),
                description: "They stay where they are too.".to_string(),
                availability: Availability::Free,
            },
        ],
        default: vec![1],
    };
    let picked = ask(&mut stdin.lock(), &mut std::io::stdout(), &question)?;
    Ok(match picked.first() {
        Some(0) => BrowserDownloads::Leave,
        Some(2) => BrowserDownloads::Link,
        _ => BrowserDownloads::Move,
    })
}

/// Say whether MO2 can take the "Mod Manager Download" clicks.
fn mo2_advice(downloads: &Path) {
    let note = |text: &str| crate::ui::status(Tone::Note, text);
    if !MOD_MANAGER_LINKS {
        note(
            "On each page, click \"Slow download\" (or \"Manual download\"): your browser \
             saves the file, and it is picked up from there.",
        );
        return;
    }
    let running = crate::config::mo2::running::running_instances();
    let saving_here = |instance: &PathBuf| {
        crate::config::mo2::instance_download_dir(instance).is_some_and(|dir| {
            std::fs::canonicalize(&dir).ok() == std::fs::canonicalize(downloads).ok()
        })
    };
    match running {
        Some(instances) if instances.iter().any(saving_here) => note(
            "MO2 is running and saves into the downloads folder: on each page, \"Mod Manager \
             Download\" sends the file straight there.",
        ),
        Some(instances) if !instances.is_empty() => crate::ui::status(
            Tone::Warning,
            format!(
                "MO2 is running, but saves its downloads somewhere other than {}; files from \
             \"Mod Manager Download\" will not be seen. Use \"Manual Download\", or point that \
             MO2 at the downloads folder.",
                downloads.display()
            ),
        ),
        _ => note(
            "To use the \"Mod Manager Download\" button (MO2 fetches the file into the downloads \
             folder), open MO2 now, if it is not already open. Otherwise use \"Manual \
             Download\": your browser saves the file, and it is picked up from there.",
        ),
    }
}

pub(crate) async fn run(
    unfetched: Vec<Unfetched>,
    downloads: &Path,
    batch: usize,
    watch: Option<PathBuf>,
    mut policy: Option<BrowserDownloads>,
) -> anyhow::Result<()> {
    if unfetched.is_empty() {
        return Ok(());
    }
    let watch = watch
        .or_else(default_watch_dir)
        .filter(|dir| dir.is_dir())
        .filter(|dir| std::fs::canonicalize(dir).ok() != std::fs::canonicalize(downloads).ok());
    let interactive = std::io::stdin().is_terminal();
    let total = unfetched.len();

    crate::ui::heading(
        Tone::Note,
        format!("{total} archive(s) to download by hand"),
    );
    crate::ui::say(format!(
        "  Their pages open {} at a time; each file is picked up as it arrives{}.",
        batch.max(1),
        watch
            .as_ref()
            .map(|dir| format!(
                " in the downloads folder or in {}",
                crate::ui::dim(dir.display())
            ))
            .unwrap_or_default()
    ));
    mo2_advice(downloads);
    if let Some(warning) = profile_warning() {
        crate::ui::status(Tone::Warning, warning);
    }
    let no_page: Vec<&Unfetched> = unfetched.iter().filter(|u| page_of(u).is_none()).collect();
    if !no_page.is_empty() {
        let items: Vec<String> = no_page
            .iter()
            .map(|item| {
                format!(
                    "{} ({})\n{}",
                    item.mod_id,
                    crate::ui::name(&item.label),
                    item.note
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("(the list does not say where from)")
                )
            })
            .collect();
        crate::ui::section(
            Tone::Warning,
            "No page to open: download these by hand",
            &items,
        );
    }
    crate::ui::say(crate::ui::dim(
        "\nCtrl-C stops; running the same command again carries on.\n",
    ));

    let mut pending: Vec<Unfetched> = unfetched;
    // Sources whose page is open, and those still to open, first last.
    let mut opened: Vec<String> = Vec::new();
    let mut to_open: Vec<(String, String)> = pending
        .iter()
        .filter_map(|u| Some((u.source.clone(), page_of(u)?)))
        .collect();
    to_open.reverse();
    let mut sizes: HashMap<PathBuf, u64> = HashMap::new();
    let mut told_left: Vec<PathBuf> = Vec::new();
    let mut done = 0usize;

    loop {
        // Keep `batch` pages open for archives still missing.
        let open_now = pending
            .iter()
            .filter(|u| opened.contains(&u.source))
            .count();
        for _ in open_now..batch.max(1) {
            let Some((source, url)) = to_open.pop() else {
                break;
            };
            if !open_url(&url) {
                crate::ui::bullet(Tone::Warning, format!("open this page yourself: {url}"));
            }
            // A hand download's page is not a Nexus file page: say what to get.
            if let Some(item) = pending.iter().find(|u| u.source == source)
                && let Some(note) = &item.note
            {
                crate::ui::bullet(Tone::Note, format!("{}: {}", item.mod_id, note.trim()));
            }
            opened.push(source);
            // A browser handed many tabs in the same instant drops some.
            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        let mut arrived = Vec::new();
        for (index, item) in pending.iter().enumerate() {
            let mut reference = ArchiveRef::new(&item.source, item.file_name.as_deref());
            reference.content = item
                .content
                .as_ref()
                .map(|(size, md5)| (*size, md5.as_str()));
            if let Some(path) = archive_store::find(downloads, &[], &reference)
                && settled(&path, &mut sizes)
            {
                arrived.push((index, path));
                continue;
            }
            let Some(watch) = &watch else { continue };
            let Some(file) = archive_store::find(watch, &[], &reference) else {
                continue;
            };
            if !settled(&file, &mut sizes) {
                continue;
            }
            let how = match policy {
                Some(how) => how,
                None if interactive => {
                    let how = ask_policy(watch, downloads)?;
                    policy = Some(how);
                    how
                }
                None => {
                    crate::ui::status(
                        Tone::Note,
                        format!(
                            "{} arrived in {}; it is left there (--browser-downloads move \
                             takes such files in)",
                            file.display(),
                            watch.display()
                        ),
                    );
                    policy = Some(BrowserDownloads::Leave);
                    BrowserDownloads::Leave
                }
            };
            if how == BrowserDownloads::Leave {
                if !told_left.contains(&file) {
                    crate::ui::bullet(
                        Tone::Note,
                        format!(
                            "{} is in {}; waiting for it in the downloads folder",
                            crate::ui::name(&item.label),
                            crate::ui::dim(watch.display())
                        ),
                    );
                    told_left.push(file);
                }
                continue;
            }
            arrived.push((index, adopt(&file, downloads, &reference, how)?));
        }

        // Highest index first, so removing one does not shift the next.
        arrived.sort_by(|a, b| b.0.cmp(&a.0));
        for (index, path) in arrived {
            let item = pending.remove(index);
            done += 1;
            crate::ui::say(format!(
                "  {} {} {} {}",
                crate::ui::dim(format!("[{done}/{total}]")),
                console::style(console::Emoji("✔", "ok")).green(),
                item.mod_id,
                crate::ui::dim(path.file_name().unwrap_or_default().to_string_lossy())
            ));
        }
        if pending.is_empty() {
            crate::ui::say("");
            crate::ui::status(
                Tone::Good,
                format!(
                    "All {total} archive(s) are in {}.",
                    crate::ui::dim(downloads.display())
                ),
            );
            return Ok(());
        }
        tokio::time::sleep(POLL).await;
    }
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
