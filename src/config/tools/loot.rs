//! LOOT integration: sort the load order by running LOOT against a synthesized
//! game tree.
//!
//! LOOT expects a real game install, so this hard-links Oblivion.exe, the root
//! binaries and every staged plugin into a temporary directory, runs
//! `LOOT --auto-sort` against it, reads the result back, and writes the sorted
//! order into the MO2 profile.

use crate::config::install::stage::is_plugin_file;
use crate::config::install::InstallSettings;
use crate::config::mo2::mo2_profile_dir;
use crate::config::schema::PersonalizedPlan;
use crate::util::fs::link_or_copy;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn run_loot_sort(plan: &PersonalizedPlan, settings: &InstallSettings) -> anyhow::Result<()> {
    let Some(game_dir) = &settings.game_dir else {
        anyhow::bail!("post-install action 'loot-sort' requires --game-dir");
    };
    let resolved_game_dir = resolve_oblivion_game_dir(game_dir).ok_or_else(|| {
        anyhow::anyhow!(
            "post-install action 'loot-sort' could not find Oblivion.exe for --game-dir {}",
            game_dir.display()
        )
    })?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let temp_root = std::env::temp_dir().join(format!("mudcrab-loot-{stamp}"));
    let temp_data = temp_root.join("Data");
    let temp_local = temp_root.join(".local");
    let loot_data = temp_root.join(".loot-data");
    std::fs::create_dir_all(&temp_data).map_err(|err| {
        anyhow::anyhow!(
            "failed to create temporary LOOT Data directory {}: {err}",
            temp_data.display()
        )
    })?;
    std::fs::create_dir_all(&temp_local).map_err(|err| {
        anyhow::anyhow!(
            "failed to create temporary LOOT local directory {}: {err}",
            temp_local.display()
        )
    })?;
    std::fs::create_dir_all(&loot_data).map_err(|err| {
        anyhow::anyhow!(
            "failed to create temporary LOOT data directory {}: {err}",
            loot_data.display()
        )
    })?;

    // If a real game AppData path is configured, use it directly so LOOT's
    // Steam auto-detection reads/writes the plugin list from our desired location.
    // Otherwise fall back to the sandboxed temp-dir approach.
    let game_appdata_path = settings
        .tools
        .loot
        .as_ref()
        .and_then(|l| l.game_appdata_path.clone());

    // Captured inside the closure so it can be restored on every exit path.
    let mut plugins_backup: Option<Vec<(PathBuf, Option<Vec<u8>>)>> = None;

    let sort_result = (|| -> anyhow::Result<()> {
        // Discover unlisted plugins by scanning installed mod staging dirs.
        stage_game_executable(&resolved_game_dir, &temp_root)?;
        stage_game_root_binaries(&resolved_game_dir, &temp_root)?;
        stage_game_plugins(&resolved_game_dir, &temp_data)?;
        let staged_mod_plugins = stage_installed_mod_plugins(plan, settings, &temp_data)?;
        let unlisted_plugins = find_unlisted_plugins(&plan.plugins, &staged_mod_plugins);
        if !unlisted_plugins.is_empty() {
            tracing::warn!(
                count = unlisted_plugins.len(),
                plugins = ?unlisted_plugins,
                "post-install loot-sort: discovered plugins not listed in top-level plugins; they will NOT be added automatically"
            );
        }
        let seeded_plugins = canonicalize_plugins_for_staged_data(&plan.plugins, &temp_data)?;

        // Determine which plugins.txt path LOOT will actually read/write.
        let plugins_dir: PathBuf =
            if let Some(ref appdata) = game_appdata_path {
                // Real-path strategy: write our desired list into the path LOOT
                // auto-detects, and back up whatever is already there so it
                // can be restored after a successful sort.
                std::fs::create_dir_all(appdata).map_err(|err| {
                    anyhow::anyhow!(
                        "failed to create game AppData directory {}: {err}",
                        appdata.display()
                    )
                })?;
                let backup = plugin_list_variant_paths(appdata)
                    .into_iter()
                    .map(|p| {
                        let original = std::fs::read(&p).ok();
                        (p, original)
                    })
                    .collect::<Vec<_>>();
                tracing::info!(
                    appdata = %appdata.display(),
                    "post-install loot-sort: using real game AppData path for plugins.txt"
                );
                write_plugin_list_files(appdata, &seeded_plugins)?;
                plugins_backup = Some(backup);
                appdata.clone()
            } else {
                tracing::warn!(
                    "post-install loot-sort: no loot.game_appdata_path configured; \
                     LOOT may auto-detect a different plugins.txt location and the \
                     sorted result may not be picked up correctly"
                );
                // Sandbox strategy: write to temp local dir as before.
                write_plugin_list_files(&temp_local, &seeded_plugins)?;
                write_loot_settings(&loot_data, &temp_root, &temp_local)?;
                temp_local.clone()
            };

        tracing::info!("post-install loot-sort: running LOOT --auto-sort");

        let mut cmd = settings.tools.loot_command();
        cmd.arg("--game=Oblivion");
        cmd.arg("--auto-sort");

        // Always point LOOT at our staged game tree so it scans the temp Data/
        // directory that contains both the base game plugins and the staged mod
        // plugins, rather than the real Steam installation (which has no mods).
        cmd.arg(format!("--game-path={}", temp_root.display()));

        // Only inject --loot-data-path in sandbox mode (when no real appdata path).
        if game_appdata_path.is_none() {
            cmd.arg(format!("--loot-data-path={}", loot_data.display()));
        }

        let loot_started_at = std::time::SystemTime::now();

        let status = cmd
            .spawn()
            .map_err(|err| anyhow::anyhow!("failed to execute LOOT: {err}"))?
            .wait()
            .map_err(|err| anyhow::anyhow!("failed to wait for LOOT: {err}"))?;

        if !status.success() {
            anyhow::bail!(
                "LOOT exited with status {}",
                status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".to_string())
            );
        }

        let (plugins_list_order, plugin_list_path, active_count, total_count) =
            read_sorted_loot_plugins_from_dir(&plugins_dir)?;

        tracing::info!(
            path = %plugin_list_path.display(),
            active_count,
            inactive_count = total_count.saturating_sub(active_count),
            "post-install loot-sort: plugin-list prefix marker summary (not LOOT active-state)"
        );

        // LOOT may persist ordering in loadorder.txt while plugins.txt is used
        // primarily for activation state. Prefer loadorder.txt when available.
        let (sorted_plugins, order_source_path, order_source) =
            if let Some((order, path)) = read_sorted_loot_load_order_from_dir_since(&plugins_dir, loot_started_at)? {
                (order, path, "loadorder")
            } else {
                (plugins_list_order, plugin_list_path.clone(), "plugins")
            };

        if sorted_plugins.is_empty() {
            anyhow::bail!(
                "LOOT produced an empty load order (plugins source: {})",
                plugin_list_path.display()
            );
        }

        let differs_from_input = plugin_lists_differ(&seeded_plugins, &sorted_plugins);
        tracing::info!(
            plugin_count = sorted_plugins.len(),
            order_source,
            order_source_path = %order_source_path.display(),
            differs_from_input,
            "post-install loot-sort: sorted plugins list produced"
        );

        write_sorted_plugins_to_profile(settings, &sorted_plugins)?;

        Ok(())
    })();

    // Restore whatever we overwrote in the real game AppData directory, whether
    // or not sorting succeeded.
    //
    // This previously only ran on success; the failure path instead *deleted*
    // plugins.txt without restoring the backup, and ignored the other filename
    // variants write_plugin_list_files had also overwritten. A LOOT crash
    // therefore destroyed the user's real load order.
    if let Some(backups) = &plugins_backup {
        for (path, original) in backups {
            match original {
                Some(bytes) => {
                    if let Err(err) = std::fs::write(path, bytes) {
                        tracing::error!(
                            path = %path.display(),
                            %err,
                            "failed to restore original plugin list after loot-sort"
                        );
                    }
                }
                // The file did not exist before we wrote it, so removing it
                // restores the original state.
                None => {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    let _ = std::fs::remove_dir_all(&temp_root);
    sort_result
}

pub(crate) fn stage_game_executable(game_dir: &Path, destination_root: &Path) -> anyhow::Result<()> {
    let source = game_dir.join("Oblivion.exe");
    if !source.exists() {
        anyhow::bail!(
            "post-install loot-sort requires Oblivion.exe in game directory {}, but it was not found",
            game_dir.display()
        );
    }

    link_or_copy(&source, &destination_root.join("Oblivion.exe"))
}

pub(crate) fn stage_game_root_binaries(game_dir: &Path, destination_root: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(game_dir)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", game_dir.display()))?
    {
        let entry = entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", game_dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read file type for {}: {err}", path.display()))?;
        if !file_type.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        let lower_name = name.to_ascii_lowercase();
        let is_root_binary = lower_name.ends_with(".dll") || lower_name.ends_with(".exe");
        if !is_root_binary {
            continue;
        }

        link_or_copy(&path, &destination_root.join(name))?;
    }

    Ok(())
}

pub(crate) fn resolve_oblivion_game_dir(preferred: &Path) -> Option<PathBuf> {
    let direct_exe = preferred.join("Oblivion.exe");
    if direct_exe.exists() {
        return Some(preferred.to_path_buf());
    }

    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(home.join(".local/share/Steam/steamapps/common/Oblivion"));
        candidates.push(home.join(".steam/steam/steamapps/common/Oblivion"));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.join("Oblivion.exe").exists())
}

pub(crate) fn write_plugin_list_file(path: &Path, plugins: &[String]) -> anyhow::Result<()> {
    let mut content = String::new();
    for plugin in plugins {
        // Oblivion plugin lists are plain names (no '*' active marker).
        let bare = plugin.trim().trim_start_matches('*');
        content.push_str(bare);
        content.push('\n');
    }
    std::fs::write(path, content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))
}

pub(crate) fn plugin_list_variant_paths(dir: &Path) -> Vec<PathBuf> {
    vec![dir.join("plugins.txt"), dir.join("Plugins.txt")]
}

pub(crate) fn load_order_variant_paths(dir: &Path) -> Vec<PathBuf> {
    vec![dir.join("loadorder.txt"), dir.join("LoadOrder.txt")]
}

pub(crate) fn write_plugin_list_files(dir: &Path, plugins: &[String]) -> anyhow::Result<()> {
    for path in plugin_list_variant_paths(dir) {
        write_plugin_list_file(&path, plugins)?;
    }
    Ok(())
}

pub(crate) fn find_unlisted_plugins(declared: &[String], discovered: &[String]) -> Vec<String> {
    let declared_set: HashSet<String> = declared
        .iter()
        .map(|plugin| plugin.trim().to_ascii_lowercase())
        .collect();

    let mut unlisted = Vec::new();
    let mut seen = HashSet::new();
    for plugin in discovered {
        let normalized = plugin.trim().to_ascii_lowercase();
        if normalized.is_empty() || declared_set.contains(&normalized) || seen.contains(&normalized) {
            continue;
        }

        unlisted.push(plugin.clone());
        seen.insert(normalized);
    }

    unlisted
}

pub(crate) fn canonicalize_plugins_for_staged_data(input: &[String], staged_data_dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut staged_paths: HashMap<String, PathBuf> = HashMap::new();
    collect_plugin_paths_for_stage(staged_data_dir, staged_data_dir, &mut staged_paths)?;

    let mut staged_case_map: HashMap<String, String> = HashMap::new();
    for plugin_name in staged_paths.keys() {
        staged_case_map.insert(plugin_name.to_ascii_lowercase(), plugin_name.clone());
    }

    let mut missing = Vec::new();
    let mut out = Vec::with_capacity(input.len());
    for plugin in input {
        let bare = plugin.trim().trim_start_matches('*');
        if bare.is_empty() {
            continue;
        }

        let normalized = bare.to_ascii_lowercase();
        if let Some(actual_name) = staged_case_map.get(&normalized) {
            out.push(actual_name.clone());
        } else {
            missing.push(bare.to_string());
            out.push(bare.to_string());
        }
    }

    if !missing.is_empty() {
        tracing::warn!(
            count = missing.len(),
            plugins = ?missing,
            "post-install loot-sort: plugins declared in top-level list were not found in staged Data directory"
        );
    }

    Ok(out)
}

pub(crate) fn plugin_lists_differ(input: &[String], output: &[String]) -> bool {
    let normalize = |items: &[String]| {
        items
            .iter()
            .map(|value| value.trim().trim_start_matches('*').to_ascii_lowercase())
            .collect::<Vec<_>>()
    };

    normalize(input) != normalize(output)
}

pub(crate) fn write_loot_settings(loot_data: &Path, game_path: &Path, local_path: &Path) -> anyhow::Result<()> {
    let settings = format!(
        "enableDebugLogging = false\n\
enableLootUpdateCheck = false\n\
updateMasterlist = false\n\
game = 'Oblivion'\n\
lastGame = 'Oblivion'\n\
\n\
[[games]]\n\
folder = 'Oblivion'\n\
gameId = 'Oblivion'\n\
hiddenMessages = []\n\
local_path = '{}'\n\
master = 'Oblivion.esm'\n\
masterlistSource = 'https://raw.githubusercontent.com/loot/oblivion/v0.29/masterlist.yaml'\n\
minimumHeaderVersion = 0.80000001192092896\n\
name = 'TES IV: Oblivion'\n\
path = '{}'\n",
        local_path.display(),
        game_path.display()
    );

    std::fs::write(loot_data.join("settings.toml"), settings).map_err(|err| {
        anyhow::anyhow!(
            "failed to write temporary LOOT settings at {}: {err}",
            loot_data.join("settings.toml").display()
        )
    })
}

pub(crate) fn read_sorted_loot_plugins_from_dir(path: &Path) -> anyhow::Result<(Vec<String>, PathBuf, usize, usize)> {
    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = plugin_list_variant_paths(path)
        .into_iter()
        .filter_map(|p| {
            let modified = std::fs::metadata(&p).ok()?.modified().ok()?;
            Some((p, modified))
        })
        .collect();

    if candidates.is_empty() {
        anyhow::bail!("failed to find LOOT plugins list in {}", path.display());
    }

    // Prefer the candidate with the largest non-empty parsed list; if tied,
    // prefer the most recently modified.
    let mut best: Option<(Vec<String>, PathBuf, usize, usize, std::time::SystemTime)> = None;

    for (candidate_path, modified) in candidates.drain(..) {
        let content = std::fs::read_to_string(&candidate_path).map_err(|err| {
            anyhow::anyhow!("failed to read sorted LOOT plugins from {}: {err}", candidate_path.display())
        })?;

        let mut out = Vec::new();
        let mut active_count = 0usize;
        let mut total_count = 0usize;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            total_count += 1;
            if trimmed.starts_with('*') {
                active_count += 1;
            }

            let plugin = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();
            if plugin.is_empty() {
                continue;
            }

            out.push(plugin.to_string());
        }

        match &best {
            None => {
                best = Some((out, candidate_path, active_count, total_count, modified));
            }
            Some((best_out, _, _, _, best_modified)) => {
                if out.len() > best_out.len() || (out.len() == best_out.len() && modified > *best_modified) {
                    best = Some((out, candidate_path, active_count, total_count, modified));
                }
            }
        }
    }

    let (out, selected, active_count, total_count, _) = best
        .ok_or_else(|| anyhow::anyhow!("failed to choose LOOT plugins list in {}", path.display()))?;

    Ok((out, selected, active_count, total_count))
}

pub(crate) fn read_sorted_loot_load_order_from_dir_since(
    path: &Path,
    modified_after: std::time::SystemTime,
) -> anyhow::Result<Option<(Vec<String>, PathBuf)>> {
    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = load_order_variant_paths(path)
        .into_iter()
        .filter_map(|p| {
            let modified = std::fs::metadata(&p).ok()?.modified().ok()?;
            if modified < modified_after {
                return None;
            }
            Some((p, modified))
        })
        .collect();

    if candidates.is_empty() {
        return Ok(None);
    }

    candidates.sort_by_key(|(_, modified)| *modified);
    let selected = candidates
        .last()
        .map(|(p, _)| p.clone())
        .ok_or_else(|| anyhow::anyhow!("failed to choose LOOT load order in {}", path.display()))?;

    let content = std::fs::read_to_string(&selected)
        .map_err(|err| anyhow::anyhow!("failed to read LOOT load order from {}: {err}", selected.display()))?;

    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let plugin = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();
        if plugin.is_empty() {
            continue;
        }

        out.push(plugin.to_string());
    }

    if out.is_empty() {
        return Ok(None);
    }

    Ok(Some((out, selected)))
}

pub(crate) fn write_sorted_plugins_to_profile(settings: &InstallSettings, plugins: &[String]) -> anyhow::Result<()> {
    let Some(profile_dir) = mo2_profile_dir(settings) else {
        tracing::warn!("post-install loot-sort: no MO2 profile directory configured; skipping plugins.txt write-back");
        return Ok(());
    };

    // MO2 plugins.txt uses bare names without the '*' active-marker prefix.
    let path = profile_dir.join("plugins.txt");
    let mut content = String::new();
    for plugin in plugins {
        let bare = plugin.trim().trim_start_matches('*');
        content.push_str(bare);
        content.push('\n');
    }
    std::fs::write(&path, content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))?;
    Ok(())
}

pub(crate) fn stage_game_plugins(game_dir: &Path, destination_data_dir: &Path) -> anyhow::Result<()> {
    let source_data = game_dir.join("Data");
    if !source_data.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(&source_data)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", source_data.display()))?
    {
        let entry = entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", source_data.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read file type for {}: {err}", path.display()))?;
        if !file_type.is_file() || !is_plugin_file(&path) {
            continue;
        }

        let Some(name) = path.file_name() else {
            continue;
        };
        link_or_copy(&path, &destination_data_dir.join(name))?;
    }

    Ok(())
}

pub(crate) fn stage_installed_mod_plugins(
    plan: &PersonalizedPlan,
    settings: &InstallSettings,
    destination_data_dir: &Path,
) -> anyhow::Result<Vec<String>> {
    let selected: HashSet<&str> = plan.selected_mods.iter().map(String::as_str).collect();
    let mut winning_plugins: HashMap<String, PathBuf> = HashMap::new();

    for mod_id in &plan.mod_order {
        if !selected.contains(mod_id.as_str()) {
            continue;
        }

        let mod_dir = settings.mods_dir.join(mod_id);
        if !mod_dir.exists() {
            continue;
        }

        collect_plugin_paths_for_stage(&mod_dir, &mod_dir, &mut winning_plugins)?;
    }

    let mut staged_plugins = Vec::new();
    for (plugin_name, source_path) in winning_plugins {
        link_or_copy(&source_path, &destination_data_dir.join(&plugin_name))?;
        staged_plugins.push(plugin_name);
    }

    staged_plugins.sort();

    Ok(staged_plugins)
}

pub(crate) fn collect_plugin_paths_for_stage(
    root: &Path,
    current: &Path,
    out: &mut HashMap<String, PathBuf>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?
    {
        let entry = entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read file type for {}: {err}", path.display()))?;

        if file_type.is_dir() {
            collect_plugin_paths_for_stage(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() || !is_plugin_file(&path) {
            continue;
        }

        let rel = path.strip_prefix(root).map_err(|err| {
            anyhow::anyhow!(
                "failed to compute relative plugin path for {} from {}: {err}",
                path.display(),
                root.display()
            )
        })?;
        let plugin_name = rel
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| anyhow::anyhow!("invalid plugin filename {}", path.display()))?
            .to_string();

        out.insert(plugin_name, path);
    }

    Ok(())
}
