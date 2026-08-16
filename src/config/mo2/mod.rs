//! Mod Organizer 2 instance export.
//!
//! MO2 is the first concrete export target, not the internal abstraction --
//! the install pipeline itself stays launcher-agnostic.

use super::install::InstallSettings;
use crate::config::download;
use crate::config::schema::{Mo2ModlistEntry, PersonalizedPlan};
use crate::util::fs::{find_child_case_insensitive, write_text_file};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Suffix MO2 uses to hide a file from the virtual filesystem entirely.
/// Its `skip_file_suffixes` default includes this; "Merge Plugins Hide" uses it
/// to drop merged source plugins out of the plugin list while leaving their
/// mods enabled so the assets still load.
pub(crate) const MO2_HIDDEN_SUFFIX: &str = ".mohidden";

/// Outcome of trying to hide a plugin, so callers can report accurately rather
/// than claiming work they did not do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HideOutcome {
    /// Renamed `<plugin>` to `<plugin>.mohidden`.
    Hidden,
    /// Already hidden by a previous run.
    AlreadyHidden,
}

/// Hide one plugin inside a mod folder by renaming it.
///
/// Idempotent, because installs are re-run: hiding an already-hidden plugin is
/// a no-op, not an error. But if *both* names exist the situation is
/// ambiguous -- either the mod was reinstalled over a previous hide, or two
/// different files claim the same plugin -- and picking one silently would
/// either resurrect a stale plugin or discard a fresh one. Refuse instead.
pub(crate) fn hide_plugin(mod_dir: &Path, plugin: &str) -> anyhow::Result<HideOutcome> {
    let visible = find_child_case_insensitive(mod_dir, plugin);
    let hidden = find_child_case_insensitive(mod_dir, &format!("{plugin}{MO2_HIDDEN_SUFFIX}"));

    match (visible, hidden) {
        (Some(visible), Some(hidden)) => anyhow::bail!(
            "cannot hide {plugin}: both {} and {} exist, so which one is current is ambiguous. \
             Delete whichever is stale and re-run.",
            visible.display(),
            hidden.display()
        ),
        (Some(visible), None) => {
            let destination = visible.with_file_name(format!(
                "{}{MO2_HIDDEN_SUFFIX}",
                visible.file_name().unwrap_or_default().to_string_lossy()
            ));
            std::fs::rename(&visible, &destination).map_err(|err| {
                anyhow::anyhow!(
                    "failed to hide {} as {}: {err}",
                    visible.display(),
                    destination.display()
                )
            })?;
            Ok(HideOutcome::Hidden)
        }
        (None, Some(_)) => Ok(HideOutcome::AlreadyHidden),
        (None, None) => anyhow::bail!(
            "cannot hide {plugin}: it is not in {}. Check the merge's source mod id.",
            mod_dir.display()
        ),
    }
}

/// Reverse `hide_plugin`. Also idempotent: an already-visible plugin is fine.
pub(crate) fn unhide_plugin(mod_dir: &Path, plugin: &str) -> anyhow::Result<()> {
    let hidden_name = format!("{plugin}{MO2_HIDDEN_SUFFIX}");
    let Some(hidden) = find_child_case_insensitive(mod_dir, &hidden_name) else {
        return Ok(());
    };

    if let Some(visible) = find_child_case_insensitive(mod_dir, plugin) {
        anyhow::bail!(
            "cannot unhide {plugin}: {} already exists alongside {}",
            visible.display(),
            hidden.display()
        );
    }

    let file_name = hidden.file_name().unwrap_or_default().to_string_lossy().to_string();
    let restored = hidden.with_file_name(
        file_name
            .strip_suffix(MO2_HIDDEN_SUFFIX)
            .unwrap_or(&file_name),
    );
    std::fs::rename(&hidden, &restored).map_err(|err| {
        anyhow::anyhow!(
            "failed to unhide {} as {}: {err}",
            hidden.display(),
            restored.display()
        )
    })
}

pub(crate) fn prepare_mo2_profile(settings: &InstallSettings) -> anyhow::Result<()> {
    let root = mo2_instance_dir(settings).ok_or_else(|| anyhow::anyhow!("missing MO2 instance dir"))?;
    std::fs::create_dir_all(root.join("mods"))
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", root.join("mods").display()))?;
    std::fs::create_dir_all(root.join("downloads")).map_err(|err| {
        anyhow::anyhow!("failed to create {}: {err}", root.join("downloads").display())
    })?;

    let profile_dir = mo2_profile_dir(settings).ok_or_else(|| anyhow::anyhow!("missing MO2 profile dir"))?;
    std::fs::create_dir_all(&profile_dir)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", profile_dir.display()))?;

    if let Some(game_dir) = &settings.game_dir {
        let source = locate_game_ini_source(game_dir, "Oblivion.ini").ok_or_else(|| {
            anyhow::anyhow!(
                "failed to locate Oblivion.ini for game directory {}; expected either the game root or a Steam Proton compatdata Documents/My Games/Oblivion path",
                game_dir.display()
            )
        })?;
        let destination = profile_dir.join("oblivion.ini");
        std::fs::copy(&source, &destination).map_err(|err| {
            anyhow::anyhow!(
                "failed to copy {} to {}: {err}",
                source.display(),
                destination.display()
            )
        })?;
    }

    // MO2 profile expects this file; keep it empty by default.
    let prefs_path = profile_dir.join("oblivionprefs.ini");
    std::fs::write(&prefs_path, "").map_err(|err| {
        anyhow::anyhow!("failed to write {}: {err}", prefs_path.display())
    })?;

    Ok(())
}

pub(crate) fn export_mo2_instance(plan: &PersonalizedPlan, settings: &InstallSettings) -> anyhow::Result<()> {
    let root = mo2_instance_dir(settings).ok_or_else(|| anyhow::anyhow!("missing MO2 instance dir"))?;
    export_downloads(plan, settings, &root.join("downloads"))?;
    ensure_mo2_section_folders(plan, settings)?;

    let profile_dir = mo2_profile_dir(settings).ok_or_else(|| anyhow::anyhow!("missing MO2 profile dir"))?;
    write_text_file(&profile_dir.join("modlist.txt"), &render_mo2_modlist(plan))?;
    write_text_file(&profile_dir.join("plugins.txt"), &render_plugin_list(&plan.plugins))?;
    write_text_file(&profile_dir.join("archives.txt"), &render_archive_list(plan, settings)?)?;
    Ok(())
}

pub(crate) fn export_downloads(plan: &PersonalizedPlan, settings: &InstallSettings, downloads_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(downloads_dir)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", downloads_dir.display()))?;

    for mod_entry in &plan.mods {
        for (archive_index, archive) in mod_entry.archives.iter().enumerate() {
            if !archive.build.is_empty() {
                // Build archives have no single pre-built file to export; skip for now.
                continue;
            }
            let path = archive.path.as_deref().unwrap_or_default();
            let cache_name = download::cache_file_name(&mod_entry.id, archive_index, path);
            let source = download::resolve_cache_path(&settings.cache_dir, &cache_name)
                .unwrap_or_else(|| settings.cache_dir.join(&cache_name));
            if !source.exists() {
                anyhow::bail!("missing cached archive for MO2 export: {}", source.display());
            }

            let dest_name = read_cached_original_name(&settings.cache_dir, &cache_name)
                .unwrap_or_else(|| {
                    let ext = detect_archive_ext(&source);
                    format!("{cache_name}{ext}")
                });
            let destination = downloads_dir.join(&dest_name);
            std::fs::copy(&source, &destination).map_err(|err| {
                anyhow::anyhow!(
                    "failed to copy {} to {}: {err}",
                    source.display(),
                    destination.display()
                )
            })?;
        }
    }

    Ok(())
}

pub(crate) fn read_cached_original_name(cache_dir: &Path, cache_name: &str) -> Option<String> {
    let sidecar_name = format!("{cache_name}.orig-name");
    let metadata_path = download::resolve_cache_path(cache_dir, &sidecar_name)
        .unwrap_or_else(|| download::cache_original_name_path(cache_dir, cache_name));
    let name = std::fs::read_to_string(metadata_path).ok()?;
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains('\\') {
        return None;
    }
    Some(trimmed.to_string())
}

/// Detect the archive extension by reading magic bytes.
pub(crate) fn detect_archive_ext(path: &Path) -> &'static str {
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

pub(crate) fn render_mo2_modlist(plan: &PersonalizedPlan) -> String {
    let selected: std::collections::HashSet<&str> = plan.selected_mods.iter().map(String::as_str).collect();
    let mut out = String::new();
    for entry in plan.mo2_modlist_entries.iter().rev() {
        match entry {
            Mo2ModlistEntry::Mod { id } => {
                let prefix = if selected.contains(id.as_str()) { '+' } else { '-' };
                out.push(prefix);
                out.push_str(id);
            }
            Mo2ModlistEntry::Section { name } => {
                out.push('-');
                out.push_str(&mo2_section_mod_name(name));
            }
        }
        out.push('\n');
    }
    out
}

pub(crate) fn ensure_mo2_section_folders(plan: &PersonalizedPlan, settings: &InstallSettings) -> anyhow::Result<()> {
    for entry in &plan.mo2_modlist_entries {
        let Mo2ModlistEntry::Section { name } = entry else {
            continue;
        };

        let path = settings.mods_dir.join(mo2_section_mod_name(name));
        std::fs::create_dir_all(&path)
            .map_err(|err| anyhow::anyhow!("failed to create MO2 section folder {}: {err}", path.display()))?;
    }

    Ok(())
}

pub(crate) fn mo2_section_mod_name(section_name: &str) -> String {
    format!("{section_name}_separator")
}

pub(crate) fn render_plugin_list(plugins: &[String]) -> String {
    let mut out = String::new();
    for plugin in plugins {
        out.push_str(plugin);
        out.push('\n');
    }
    out
}

pub(crate) fn render_archive_list(plan: &PersonalizedPlan, settings: &InstallSettings) -> anyhow::Result<String> {
    let mut out = String::new();

    for mod_id in &plan.mod_order {
        let mod_dir = settings.mods_dir.join(mod_id);
        if !mod_dir.exists() {
            continue;
        }

        let mut archives = Vec::new();
        collect_bsa_files(&mod_dir, &mod_dir, &mut archives)?;
        archives.sort();
        for archive in archives {
            out.push_str(&archive);
            out.push('\n');
        }
    }

    Ok(out)
}

pub(crate) fn collect_bsa_files(root: &Path, current: &Path, out: &mut Vec<String>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?
    {
        let entry = entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read file type for {}: {err}", path.display()))?;

        if file_type.is_dir() {
            collect_bsa_files(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let lower = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !lower.ends_with(".bsa") {
            continue;
        }

        let rel = path.strip_prefix(root).map_err(|err| {
            anyhow::anyhow!(
                "failed to compute archive relative path for {} from {}: {err}",
                path.display(),
                root.display()
            )
        })?;
        out.push(rel.to_string_lossy().replace('\\', "/"));
    }

    Ok(())
}


pub(crate) fn mo2_instance_dir(settings: &InstallSettings) -> Option<&Path> {
    settings.mo2_instance_dir.as_deref()
}

pub(crate) fn mo2_profile_dir(settings: &InstallSettings) -> Option<PathBuf> {
    Some(mo2_instance_dir(settings)?.join("profiles").join(&settings.profile_name))
}

pub(crate) fn locate_game_ini_source(game_dir: &Path, file_name: &str) -> Option<PathBuf> {
    let direct = game_dir.join(file_name);
    if direct.exists() {
        return Some(direct);
    }

    if !file_name.eq_ignore_ascii_case("Oblivion.ini") {
        return None;
    }

    let mut candidates = Vec::new();
    for compatdata_root in steam_compatdata_roots(game_dir) {
        let Ok(read_dir) = std::fs::read_dir(&compatdata_root) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let app_dir = entry.path();
            if !app_dir.is_dir() {
                continue;
            }

            for suffix in [
                "pfx/drive_c/users/steamuser/Documents/My Games/Oblivion/Oblivion.ini",
                "pfx/drive_c/users.bak/steamuser/Documents/My Games/Oblivion/Oblivion.ini",
            ] {
                let candidate = app_dir.join(suffix);
                if candidate.exists() {
                    let modified = candidate
                        .metadata()
                        .and_then(|meta| meta.modified())
                        .unwrap_or(UNIX_EPOCH);
                    candidates.push((modified, candidate));
                }
            }
        }
    }

    candidates.sort_by_key(|(modified, _)| *modified);
    candidates.into_iter().last().map(|(_, path)| path)
}

pub(crate) fn steam_compatdata_roots(game_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(steamapps_dir) = game_dir.parent().and_then(Path::parent) {
        roots.push(steamapps_dir.join("compatdata"));
    }

    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".local/share/Steam/steamapps/compatdata"));
        roots.push(home.join(".steam/steam/steamapps/compatdata"));
    }

    roots.sort();
    roots.dedup();
    roots.into_iter().filter(|path| path.exists()).collect()
}
