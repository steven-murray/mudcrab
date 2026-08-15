//! xEdit (TES4Edit) orchestration: QuickAutoClean.
//!
//! xEdit is a Windows GUI tool, so on Linux it runs through the configured
//! Wine/Proton prefix. It has no headless mode, so completion is detected by
//! watching the output plugin stabilise and then waiting for the user to close
//! the window.

use crate::config::install::InstallSettings;
use crate::config::schema::QacAction;
use crate::config::tools::ToolsConfig;
use crate::util::fs::link_or_copy;
use std::path::{Path, PathBuf};

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
                "qac: waiting for xEdit; once the window says it is finished, close it manually"
            );

            let mut child = settings
                .tools
                .windows_tool_command(&qac_exe)?
                .args(["-TES4", "-QAC", "-autoload", "-save", &data_arg, &plugin_name])
                .spawn()
                .map_err(|err| anyhow::anyhow!("qac: failed to launch xEdit: {err}"))?;

            let initial_metadata = std::fs::metadata(&temp_plugin).ok();
            let mut previous_len = initial_metadata.as_ref().map(|m| m.len());
            let mut previous_modified = initial_metadata
                .as_ref()
                .and_then(|m| m.modified().ok());
            let mut saw_output_change = false;
            let mut stable_checks = 0u32;
            let mut prompted_manual_close = false;
            let start = std::time::Instant::now();

            loop {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|err| anyhow::anyhow!("qac: failed to poll xEdit: {err}"))?
                {
                    if !status.success() {
                        anyhow::bail!(
                            "qac: xEdit exited with status {}",
                            status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".to_string())
                        );
                    }
                    break;
                }

                let current_metadata = std::fs::metadata(&temp_plugin).ok();
                let current_len = current_metadata.as_ref().map(|m| m.len());
                let current_modified = current_metadata
                    .as_ref()
                    .and_then(|m| m.modified().ok());

                let changed = current_len != previous_len || current_modified != previous_modified;
                if changed {
                    saw_output_change = true;
                    stable_checks = 0;
                } else if saw_output_change {
                    stable_checks += 1;
                }

                previous_len = current_len;
                previous_modified = current_modified;

                if saw_output_change && stable_checks >= 6 && !prompted_manual_close {
                    tracing::info!(
                        owner = owner_label,
                        plugin = plugin_name,
                        "qac: finished writing output; if the xEdit window says it is done, it is now safe to close it"
                    );
                    prompted_manual_close = true;
                }

                if start.elapsed() > std::time::Duration::from_secs(600) {
                    anyhow::bail!(
                        "qac: timed out waiting for xEdit to close; if the window shows cleaning is finished, close it manually and rerun install"
                    );
                }

                std::thread::sleep(std::time::Duration::from_millis(500));
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

