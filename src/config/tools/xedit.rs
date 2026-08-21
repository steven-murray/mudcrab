//! xEdit (TES4Edit) orchestration: QuickAutoClean.
//!
//! xEdit is a Windows GUI tool, so on Linux it runs through the configured
//! Wine/Proton prefix. `-autoexit` is accepted but **not honoured in Quick
//! Clean mode** in 4.1.5f: the window sits on its summary until someone closes
//! it, so waiting for the process to exit is waiting for a human.
//!
//! What makes this unattended instead is that xEdit says when it is done and
//! saves before it shuts down. Its log ends the job with `Quick Clean mode
//! finished`, and the cleaned plugin is already on disk by then, written beside
//! the original as `<plugin>.save.<timestamp>` and only *renamed* over it at
//! shutdown. So mudcrab watches the log for that line, takes the newest save
//! file, and closes xEdit itself.
//!
//! This is not reimplemented natively, and deliberately. QuickAutoClean removes
//! records that are identical to their master, but "identical" is xEdit's
//! judgement, not a byte comparison: it knows which fields are unordered sets
//! and which bytes are unused padding, per record type. Two records from this
//! very list make the point -- a CELL whose `XCLR` region list is the same three
//! FormIDs in a different order, and a LAND whose `BTXT`/`ATXT` entries differ
//! only in their unused byte. Both are identical to xEdit and neither is
//! identical to a memcmp. Matching that would mean reimplementing xEdit's whole
//! field schema, where erring loose silently deletes real edits.

use crate::config::install::InstallSettings;
use crate::config::schema::QacAction;
use crate::config::tools::ToolsConfig;
use crate::util::fs::link_or_copy;
use std::path::{Path, PathBuf};

/// How long one plugin may take. Generous: cleaning loads every master, and
/// Oblivion.esm alone is 265 MB through a Wine prefix.
const QAC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(900);

/// Quick Auto Clean action.
///
/// For each file matched by the `plugins` glob patterns (resolved relative to
/// the mod's staging directory), this action:
///
/// 1. Creates a temporary data directory containing:
///    - Hard-links / copies of all `.esm` master files from `%GAME_DIR%/Data/`
///      so that xEdit can resolve master references.
///    - A copy of the plugin file being cleaned.
/// 2. Invokes xEdit via the configured Wine/Proton prefix (on non-Windows
///    hosts), preferring `TES4EditQuickAutoClean.exe` when available.
/// 3. After the cleaned plugin output changes and then stabilizes, prints a
///    simple instruction telling the user it is safe to close the QAC window.
///    mudcrab then waits for the user to close xEdit manually.
/// 4. Copies the cleaned plugin back into the mod's staging directory,
///    replacing the original in-place.
/// 5. Removes the temporary directory.
pub(crate) fn apply_qac_action(
    owner_label: &str,
    action: &QacAction,
    settings: &InstallSettings,
    mod_target: &Path,
) -> anyhow::Result<()> {
    use globset::{Glob, GlobSetBuilder};
    use std::time::{SystemTime, UNIX_EPOCH};

    let tes4edit_config = settings.tools.tes4edit.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "{owner_label} qac action requires [tes4edit] configuration in tools.toml"
        )
    })?;

    let game_dir = settings
        .game_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("{owner_label} qac action requires --game-dir"))?;
    let game_data = game_dir.join("Data");

    // Plugin glob patterns, resolved relative to the mod's staged data folder.
    let patterns = &action.plugins;

    // Build a globset to match files in the mod's staging dir.
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            Glob::new(pattern)
                .map_err(|err| anyhow::anyhow!("{owner_label} qac: invalid glob '{pattern}': {err}"))?,
        );
    }
    let globset = builder.build().map_err(|err| {
        anyhow::anyhow!("{owner_label} qac: failed to build glob set: {err}")
    })?;

    // Collect matching plugin files from the staging dir.
    let matched: Vec<PathBuf> = {
        let mut v = Vec::new();
        for entry in std::fs::read_dir(mod_target).map_err(|err| {
            anyhow::anyhow!("{owner_label} qac: failed to read staging dir {}: {err}", mod_target.display())
        })? {
            let entry = entry.map_err(|err| {
                anyhow::anyhow!("{owner_label} qac: failed to read dir entry: {err}")
            })?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if globset.is_match(name_str.as_ref()) {
                v.push(entry.path());
            }
        }
        v
    };

    if matched.is_empty() {
        tracing::warn!(
            owner = owner_label,
            patterns = ?patterns,
            staging_dir = %mod_target.display(),
            "qac: no plugin files matched; action is a no-op"
        );
        return Ok(());
    }

    for plugin_path in &matched {
        let plugin_name = plugin_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("{owner_label} qac: plugin path has no filename"))?
            .to_string_lossy()
            .into_owned();

        if settings.dry_run {
            tracing::info!(
                owner = owner_label,
                plugin = plugin_name,
                "install dry-run qac action"
            );
            continue;
        }

        tracing::info!(owner = owner_label, plugin = plugin_name, "qac: cleaning plugin");

        // Build a temporary data sandbox.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_root = std::env::temp_dir().join(format!("mudcrab-qac-{stamp}"));
        let temp_data = temp_root.join("Data");
        std::fs::create_dir_all(&temp_data).map_err(|err| {
            anyhow::anyhow!(
                "{owner_label} qac: failed to create temp dir {}: {err}",
                temp_data.display()
            )
        })?;

        let qac_result = (|| -> anyhow::Result<()> {
            // Link/copy master ESMs from the real game Data folder so xEdit can
            // resolve master file references during cleaning.
            for entry in std::fs::read_dir(&game_data).map_err(|err| {
                anyhow::anyhow!("qac: failed to read game data dir {}: {err}", game_data.display())
            })? {
                let entry = entry?;
                let name = entry.file_name();
                let name_s = name.to_string_lossy();
                let is_esm = name_s.to_ascii_lowercase().ends_with(".esm");
                if is_esm {
                    link_or_copy(&entry.path(), &temp_data.join(&name))?;
                }
            }

            // Copy the plugin to clean into the sandbox.
            let temp_plugin = temp_data.join(&plugin_name);
            std::fs::copy(plugin_path, &temp_plugin).map_err(|err| {
                anyhow::anyhow!(
                    "qac: failed to copy plugin {} to temp dir: {err}",
                    plugin_path.display()
                )
            })?;

            // Build the xEdit command.
            // -TES4       : force Oblivion game mode
            // -QAC        : Quick Auto Clean
            // -autoload   : automatically load the specified plugin
            // -D:<path>   : data directory (Windows-style path when via Wine)
            #[cfg(not(target_os = "windows"))]
            let data_arg = format!("-D:{}", ToolsConfig::unix_path_to_wine(&temp_data));
            #[cfg(target_os = "windows")]
            let data_arg = format!("-D:{}", temp_data.display());

            let qac_exe = tes4edit_config.qac_executable();

            tracing::info!(
                owner = owner_label,
                plugin = plugin_name,
                exe = %qac_exe.display(),
                "qac: cleaning"
            );

            // xEdit appends to one log across runs, so remember where this run
            // starts. `-autoexit` is passed because later builds may honour it
            // in this mode; 4.1.5f does not, and nothing depends on it.
            let log_path = xedit_log_path(&qac_exe);
            let log_offset = log_path
                .as_deref()
                .and_then(|path| std::fs::metadata(path).ok())
                .map(|meta| meta.len())
                .unwrap_or(0);

            let mut child = settings
                .tools
                .windows_tool_command(&qac_exe)?
                .args([
                    "-TES4",
                    "-QAC",
                    "-autoload",
                    "-autoexit",
                    "-save",
                    &data_arg,
                    &plugin_name,
                ])
                .spawn()
                .map_err(|err| anyhow::anyhow!("qac: failed to launch xEdit: {err}"))?;

            // Wait for xEdit to say it is finished, then close it. Polling the
            // log from the offset it had at launch, so a previous run's
            // completion line cannot be mistaken for this one's.
            let start = std::time::Instant::now();
            let mut finished = false;
            loop {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|err| anyhow::anyhow!("qac: failed to poll xEdit: {err}"))?
                {
                    // Closed on its own, or someone closed it. Either way the
                    // work is over; the save file below is the evidence.
                    let _ = status;
                    finished = true;
                    break;
                }

                if log_says_finished(log_path.as_deref(), log_offset) {
                    tracing::debug!(
                        owner = owner_label,
                        plugin = plugin_name,
                        "qac: xEdit reported Quick Clean finished; closing it"
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                    finished = true;
                    break;
                }

                if start.elapsed() > QAC_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(250));
            }

            if !finished {
                anyhow::bail!(
                    "qac: xEdit never reported finishing within {} seconds cleaning \
                     {plugin_name}. Its log is {}; if a dialog is waiting for an answer, run \
                     the same command by hand to see it.",
                    QAC_TIMEOUT.as_secs(),
                    log_path
                        .as_deref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "not where it was expected".to_string()),
                );
            }

            // xEdit renames the save over the original only at shutdown, which
            // it may not have reached. Prefer the newest save file, and fall
            // back to the plugin itself for the case where it did.
            if let Some(save) = newest_save_file(&temp_data, &plugin_name)? {
                std::fs::rename(&save, &temp_plugin).map_err(|err| {
                    anyhow::anyhow!(
                        "qac: failed to move {} over the cleaned plugin: {err}",
                        save.display()
                    )
                })?;
            }

            // Copy the cleaned plugin back to the staging directory.
            std::fs::copy(&temp_plugin, plugin_path).map_err(|err| {
                anyhow::anyhow!(
                    "qac: failed to copy cleaned plugin back to staging dir: {err}"
                )
            })?;

            Ok(())
        })();

        let _ = std::fs::remove_dir_all(&temp_root);
        qac_result?;

        tracing::info!(owner = owner_label, plugin = plugin_name, "qac: plugin cleaned successfully");
    }

    Ok(())
}


/// Where xEdit writes the log for this run.
///
/// Beside the executable, named for it. Returns `None` rather than guessing if
/// the path cannot be built; the caller degrades to the timeout.
fn xedit_log_path(qac_exe: &Path) -> Option<PathBuf> {
    let dir = qac_exe.parent()?;
    let candidates = [
        qac_exe
            .file_stem()
            .map(|stem| format!("{}_log.txt", stem.to_string_lossy())),
        Some("TES4Edit_log.txt".to_string()),
    ];
    candidates
        .into_iter()
        .flatten()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

/// Has xEdit logged the end of the job since this run started?
fn log_says_finished(log_path: Option<&Path>, from: u64) -> bool {
    use std::io::{Read, Seek, SeekFrom};

    let Some(path) = log_path else { return false };
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    if file.seek(SeekFrom::Start(from)).is_err() {
        return false;
    }
    let mut tail = String::new();
    // Lossy on purpose: the log is not always valid UTF-8, and the marker is
    // plain ASCII either way.
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return false;
    }
    tail.push_str(&String::from_utf8_lossy(&bytes));
    tail.contains("Quick Clean mode finished")
}

/// The most recent `<plugin>.save.<timestamp>` xEdit wrote, if any.
///
/// Quick Clean saves between passes, so there can be several; the last one is
/// the fully cleaned plugin.
fn newest_save_file(data_dir: &Path, plugin_name: &str) -> anyhow::Result<Option<PathBuf>> {
    let prefix = format!("{plugin_name}.save.");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;

    for entry in std::fs::read_dir(data_dir)
        .map_err(|err| anyhow::anyhow!("qac: failed to read {}: {err}", data_dir.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(&prefix) {
            continue;
        }
        let modified = entry.metadata().and_then(|meta| meta.modified())?;
        if best.as_ref().is_none_or(|(best, _)| modified >= *best) {
            best = Some((modified, entry.path()));
        }
    }

    Ok(best.map(|(_, path)| path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn the_finish_marker_is_only_read_from_this_run() {
        let dir = tempfile::tempdir().expect("temp dir");
        let log = dir.path().join("TES4Edit_log.txt");
        std::fs::write(&log, "[00:00] Quick Clean mode finished.\n").expect("previous run");
        let offset = std::fs::metadata(&log).expect("metadata").len();

        // A previous run's marker sits below the offset and must not count.
        assert!(!log_says_finished(Some(&log), offset));

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .expect("append");
        writeln!(file, "[00:00] Start: Applying Filter").expect("write");
        assert!(!log_says_finished(Some(&log), offset));

        writeln!(file, "[00:00] Quick Clean mode finished.").expect("write");
        assert!(log_says_finished(Some(&log), offset));
    }

    #[test]
    fn a_missing_log_is_not_a_finished_one() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(!log_says_finished(None, 0));
        assert!(!log_says_finished(Some(&dir.path().join("absent.txt")), 0));
    }

    /// Quick Clean saves between passes, so the last save is the cleaned one.
    #[test]
    fn the_newest_save_file_wins() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = dir.path();
        for (name, contents) in [
            ("Thing.esp", &b"original"[..]),
            ("Thing.esp.save.2026_01_01_00_00_01", &b"pass one"[..]),
            ("Thing.esp.save.2026_01_01_00_00_02", &b"pass two"[..]),
            // A different plugin's save must not be picked up.
            ("Other.esp.save.2026_01_01_00_00_09", &b"not ours"[..]),
        ] {
            std::fs::write(data.join(name), contents).expect("fixture");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let found = newest_save_file(data, "Thing.esp")
            .expect("scan should succeed")
            .expect("a save file should be found");
        assert_eq!(
            std::fs::read(&found).expect("read"),
            b"pass two",
            "the last pass is the cleaned plugin"
        );
    }

    /// xEdit reached shutdown and renamed the save over the original itself.
    #[test]
    fn no_save_file_means_xedit_already_renamed_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("Thing.esp"), b"cleaned").expect("fixture");
        assert!(
            newest_save_file(dir.path(), "Thing.esp")
                .expect("scan should succeed")
                .is_none()
        );
    }
}
