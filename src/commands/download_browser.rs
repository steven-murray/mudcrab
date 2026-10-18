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
use crate::config::by_hand::{
    BrowserDownloads, Found, MOD_MANAGER_LINKS, Watcher, open_url, page_of,
};
use crate::config::download::Unfetched;
use crate::ui::Tone;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How often the folders are looked at.
const POLL: Duration = Duration::from_secs(2);

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
    let mut watcher = Watcher::new(downloads, watch);
    let watch = watcher.browser_folder().map(Path::to_path_buf);
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
            let file = match watcher.look(item) {
                None => continue,
                Some(Found::Here(path)) => {
                    arrived.push((index, path));
                    continue;
                }
                Some(Found::InBrowserFolder(file)) => file,
            };
            let Some(watch) = &watch else { continue };
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
            arrived.push((index, watcher.take_in(&file, item, how)?));
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
