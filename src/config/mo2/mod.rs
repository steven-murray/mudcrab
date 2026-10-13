//! Mod Organizer 2 instance export.
//!
//! MO2 is the first concrete export target, not the internal abstraction --
//! the install pipeline itself stays launcher-agnostic.

pub mod running;

use super::install::InstallSettings;
use crate::config::install::is_plugin_file;
use crate::config::schema::{Mo2ModlistEntry, PersonalizedPlan};
use crate::util::fs::{find_child_case_insensitive, write_text_file};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Which mods an export should describe.
///
/// `None` is the unfiltered install: the export covers the whole plan, exactly
/// as it always has. `Some(ids)` is a filtered install, where the profile must
/// describe only what is actually on disk -- this run's mods plus whatever an
/// earlier run installed, as recorded in the install manifest.
pub(crate) type ExportScope<'a> = Option<&'a HashSet<String>>;

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

    let file_name = hidden
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
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
    let root =
        mo2_instance_dir(settings).ok_or_else(|| anyhow::anyhow!("missing MO2 instance dir"))?;
    std::fs::create_dir_all(root.join("mods")).map_err(|err| {
        anyhow::anyhow!("failed to create {}: {err}", root.join("mods").display())
    })?;
    std::fs::create_dir_all(root.join("downloads")).map_err(|err| {
        anyhow::anyhow!(
            "failed to create {}: {err}",
            root.join("downloads").display()
        )
    })?;

    let profile_dir =
        mo2_profile_dir(settings).ok_or_else(|| anyhow::anyhow!("missing MO2 profile dir"))?;
    std::fs::create_dir_all(&profile_dir)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", profile_dir.display()))?;

    if let Some(game_dir) = &settings.game_dir {
        let source = locate_game_ini_source(game_dir, "Oblivion.ini").ok_or_else(|| {
            anyhow::anyhow!(
                "failed to locate Oblivion.ini for game directory {}; looked in the game folder, Documents/My Games/Oblivion and Steam Proton's compatdata. Run the game once from its launcher to create it",
                game_dir.display()
            )
        })?;
        // Seed it once; never clobber it afterwards.
        //
        // This used to copy unconditionally on every run, which quietly made
        // the profile INI hold only the *last* section's edits: build Part 11
        // and the [Grass] block lands, build Part 12 and the file is reset to
        // vanilla with nothing to re-apply it. Six of eighteen game-scoped
        // settings were wrong on disk when this was found -- including the
        // DarNified font paths, which is a broken UI, and the exact failure
        // that cost a play session earlier in this build.
        //
        // `ini_set` is idempotent and edits in place, so an existing file is
        // the accumulated result of every section built so far. That is the
        // thing worth keeping.
        let destination = profile_dir.join("oblivion.ini");
        if destination.exists() {
            tracing::debug!(
                ini = %destination.display(),
                "profile Oblivion.ini already exists; keeping the edits it carries"
            );
        } else {
            std::fs::copy(&source, &destination).map_err(|err| {
                anyhow::anyhow!(
                    "failed to copy {} to {}: {err}",
                    source.display(),
                    destination.display()
                )
            })?;
            tracing::info!(
                source = %source.display(),
                destination = %destination.display(),
                "seeded the MO2 profile's Oblivion.ini from the game directory"
            );
        }
    }

    if settings.game_dir.is_some() {
        use_profile_ini(&profile_dir, settings.dry_run)?;
    }

    // MO2 profile expects this file; keep it empty by default.
    let prefs_path = profile_dir.join("oblivionprefs.ini");
    std::fs::write(&prefs_path, "")
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", prefs_path.display()))?;

    Ok(())
}

pub(crate) fn export_mo2_instance(
    plan: &PersonalizedPlan,
    settings: &InstallSettings,
    scope: ExportScope<'_>,
) -> anyhow::Result<()> {
    let root =
        mo2_instance_dir(settings).ok_or_else(|| anyhow::anyhow!("missing MO2 instance dir"))?;
    point_at_downloads(root, &settings.cache_dir, settings.dry_run)?;
    // A list with a Bashed Patch needs Wrye Bash, and it has to be started from
    // MO2 to see the mods: offer it there, as a hand-built instance would.
    if plan
        .plugins
        .iter()
        .any(|p| crate::config::check::made_by_wrye_bash(p))
        && let Some(exe) = settings
            .game_dir
            .as_ref()
            .map(|dir| dir.join("Mopy").join("Wrye Bash.exe"))
            .filter(|exe| exe.is_file())
    {
        add_executable(root, "Wrye Bash", &exe, settings.dry_run)?;
    }
    if let Some(theme) = plan.mo2.as_ref().and_then(|mo2| mo2.theme.as_deref()) {
        apply_theme(root, theme, settings.dry_run)?;
    }
    ensure_mo2_section_folders(plan, settings, scope)?;

    let profile_dir =
        mo2_profile_dir(settings).ok_or_else(|| anyhow::anyhow!("missing MO2 profile dir"))?;
    write_text_file(
        &profile_dir.join("modlist.txt"),
        &render_mo2_modlist(plan, scope),
    )?;
    // Both files, and they are not the same file twice. Oblivion has no
    // load-order file: the order *is* the plugins' modification times, oldest
    // first, and `plugins.txt` records only which plugins are active. The order
    // lives in `loadorder.txt`, which MO2 reads when it opens the profile and
    // then applies by stamping the plugin files -- so writing `plugins.txt`
    // alone describes an order nothing has, and MO2 fills the gap by rewriting
    // `loadorder.txt` from whatever timestamps extraction happened to leave.
    //
    // MO2 reads this at profile load, so it has to be restarted for a new order
    // to take. Part 37 of the guide says the same thing about pasting its own
    // `loadorder.txt` in: "close & restart MO2 to have the plugins in order".
    let plugins = scoped_plugins(plan, settings, scope)?;
    // `loadorder.txt` holds every plugin and its order; `plugins.txt` holds only
    // the active ones. The two lists differ exactly by the plugins something
    // else has absorbed -- Wrye Bash unticks what it merges into the Bashed
    // Patch -- and writing them identically re-ticks those on every install,
    // which means rebuilding the patch just to undo it.
    let inactive: HashSet<String> = plan
        .inactive_plugins
        .iter()
        .map(|name| name.trim().to_ascii_lowercase())
        .collect();
    let active: Vec<String> = plugins
        .iter()
        .filter(|name| !inactive.contains(&name.trim().to_ascii_lowercase()))
        .cloned()
        .collect();
    if !inactive.is_empty() {
        tracing::info!(
            inactive = plugins.len() - active.len(),
            active = active.len(),
            "mo2 export: leaving declared-inactive plugins out of plugins.txt"
        );
    }
    write_text_file(
        &profile_dir.join("plugins.txt"),
        &render_plugin_list(&active),
    )?;
    write_text_file(
        &profile_dir.join("loadorder.txt"),
        &render_plugin_list(&plugins),
    )?;
    write_text_file(
        &profile_dir.join("archives.txt"),
        &render_archive_list(plan, settings, scope)?,
    )?;
    Ok(())
}

/// Whether a mod belongs in this export.
fn in_scope(scope: ExportScope<'_>, mod_id: &str) -> bool {
    scope.is_none_or(|ids| ids.contains(mod_id))
}

/// Separator names the in-scope mods need.
///
/// Built the same way `SourceModlist::mo2_modlist_entries` builds them -- one
/// joined name per level of the path -- so a name computed here matches a
/// `Mo2ModlistEntry::Section` exactly. Doing it from the mods' own section
/// paths, rather than guessing which mods sit under a separator in the
/// flattened list, is why `section` had to survive into the plan.
fn scoped_section_names(
    plan: &PersonalizedPlan,
    scope: ExportScope<'_>,
) -> Option<HashSet<String>> {
    let ids = scope?;

    let mut out = HashSet::new();
    for mod_entry in &plan.mods {
        if !ids.contains(&mod_entry.id) {
            continue;
        }
        for depth in 0..mod_entry.section.len() {
            out.insert(mod_entry.section[..=depth].join(" - "));
        }
    }

    Some(out)
}

/// Turn on the profile's own game INI files (MO2's "Use profile-specific Game
/// INI Files"): `LocalSettings=true` in its `settings.ini`. Game-scoped edits
/// are made to the profile's copy of `Oblivion.ini`, never the game's own, and
/// without this MO2 ignores that copy and every edit with it.
fn use_profile_ini(profile_dir: &Path, dry_run: bool) -> anyhow::Result<()> {
    let path = profile_dir.join("settings.ini");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = set_ini_key(&existing, "General", "LocalSettings", "true");
    if updated == existing {
        return Ok(());
    }
    if dry_run {
        tracing::info!(settings = %path.display(), "install dry-run: would turn on profile-specific game INI files");
        return Ok(());
    }
    std::fs::write(&path, updated)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))?;
    tracing::info!(settings = %path.display(), "turned on profile-specific game INI files");
    Ok(())
}

/// Point the instance's MO2 at the downloads folder: `download_directory`
/// under `[Settings]` in `ModOrganizer.ini`.
///
/// Rather than linking each archive into the instance's own `downloads/`, MO2
/// is shown the folder itself, so it lists everything there -- including
/// archives the user may want to install by hand afterwards -- and one
/// central folder can serve every instance on the machine. MO2 stores the path
/// with forward slashes, under Wine as a `Z:` path.
pub(crate) fn point_at_downloads(
    instance: &Path,
    downloads: &Path,
    dry_run: bool,
) -> anyhow::Result<()> {
    let absolute = std::path::absolute(downloads)
        .map_err(|err| anyhow::anyhow!("failed to resolve {}: {err}", downloads.display()))?;
    let value = mo2_path(&absolute);
    let ini_path = instance.join("ModOrganizer.ini");
    if dry_run {
        tracing::info!(ini = %ini_path.display(), download_directory = %value, "install dry-run: would point MO2 at the downloads folder");
        return Ok(());
    }
    let existing = std::fs::read_to_string(&ini_path).unwrap_or_default();
    let updated = set_ini_key(&existing, "Settings", "download_directory", &value);
    if updated != existing {
        std::fs::write(&ini_path, updated)
            .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", ini_path.display()))?;
        tracing::info!(ini = %ini_path.display(), download_directory = %value, "pointed MO2 at the downloads folder");
    }
    Ok(())
}

/// Where an MO2 instance saves downloads: `download_directory` in its
/// `ModOrganizer.ini`, else its own `downloads/`. `None` when the setting names
/// a Wine drive other than `Z:`.
pub fn instance_download_dir(instance: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(instance.join("ModOrganizer.ini")).unwrap_or_default();
    let mut in_settings = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_settings = line.eq_ignore_ascii_case("[Settings]");
            continue;
        }
        let Some(value) = line
            .strip_prefix("download_directory=")
            .filter(|_| in_settings)
        else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            break;
        }
        if cfg!(windows) {
            return Some(PathBuf::from(value));
        }
        let unix = value
            .strip_prefix("Z:")
            .or_else(|| value.strip_prefix("z:"))?;
        return Some(PathBuf::from(unix.replace('\\', "/")));
    }
    Some(instance.join("downloads"))
}

/// Set MO2's theme to the list's, unless one is already chosen: a theme picked
/// in MO2 is the user's, and a reinstall must not undo it. A theme MO2 does
/// not have is reported, with the ones it does.
pub(crate) fn apply_theme(instance: &Path, theme: &str, dry_run: bool) -> anyhow::Result<()> {
    let file = if theme.to_ascii_lowercase().ends_with(".qss") {
        theme.to_string()
    } else {
        format!("{theme}.qss")
    };
    let stylesheets = instance.join("stylesheets");
    if stylesheets.is_dir() && !stylesheets.join(&file).is_file() {
        let mut available: Vec<String> = std::fs::read_dir(&stylesheets)
            .map(|entries| {
                entries
                    .flatten()
                    .filter_map(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .strip_suffix(".qss")
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default();
        available.sort();
        tracing::warn!(
            "the list asks for MO2's \"{theme}\" theme, which this MO2 does not have; it has: {}",
            available.join(", ")
        );
        return Ok(());
    }
    let ini_path = instance.join("ModOrganizer.ini");
    let existing = std::fs::read_to_string(&ini_path).unwrap_or_default();
    if ini_value(&existing, "Settings", "style").is_some_and(|v| !v.trim().is_empty()) {
        return Ok(());
    }
    if dry_run {
        tracing::info!(theme = %file, "install dry-run: would set MO2's theme");
        return Ok(());
    }
    let updated = set_ini_key(&existing, "Settings", "style", &file);
    std::fs::write(&ini_path, updated)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", ini_path.display()))?;
    tracing::info!(theme = %file, "set MO2's theme");
    Ok(())
}

/// The value of `key` in `[section]`, if set.
fn ini_value<'a>(ini: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut inside = false;
    for line in ini.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line.eq_ignore_ascii_case(&format!("[{section}]"));
            continue;
        }
        if inside
            && let Some((k, v)) = line.split_once('=')
            && k.trim().eq_ignore_ascii_case(key)
        {
            return Some(v);
        }
    }
    None
}

/// Add `binary` to MO2's list of executables under `title`, unless an entry
/// already runs it (or has that title): MO2 keeps them in
/// `[customExecutables]`, numbered from 1, with `size` the count.
pub(crate) fn add_executable(
    instance: &Path,
    title: &str,
    binary: &Path,
    dry_run: bool,
) -> anyhow::Result<()> {
    let ini_path = instance.join("ModOrganizer.ini");
    let value = mo2_path(binary);
    if dry_run {
        tracing::info!(title, binary = %value, "install dry-run: would add an MO2 executable");
        return Ok(());
    }
    let existing = std::fs::read_to_string(&ini_path).unwrap_or_default();
    if let Some(updated) = with_executable(&existing, title, &value) {
        std::fs::write(&ini_path, updated)
            .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", ini_path.display()))?;
        tracing::info!(title, binary = %value, "added an MO2 executable");
    }
    Ok(())
}

/// `ini` with an executable entry added; `None` when one is there already.
fn with_executable(ini: &str, title: &str, binary: &str) -> Option<String> {
    let newline = if ini.contains("\r\n") || ini.is_empty() {
        "\r\n"
    } else {
        "\n"
    };
    let lines: Vec<&str> = ini.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim().eq_ignore_ascii_case("[customExecutables]"));
    let end = start.map(|s| {
        lines[s + 1..]
            .iter()
            .position(|l| l.trim_start().starts_with('['))
            .map_or(lines.len(), |p| s + 1 + p)
    });
    let section = match (start, end) {
        (Some(s), Some(e)) => &lines[s + 1..e],
        _ => &[][..],
    };
    let normal = |path: &str| path.replace('\\', "/").to_ascii_lowercase();
    let taken = section.iter().any(|line| {
        line.split_once('=').is_some_and(|(key, v)| {
            (key.ends_with("\\binary") && normal(v) == normal(binary))
                || (key.ends_with("\\title") && v.eq_ignore_ascii_case(title))
        })
    });
    if taken {
        return None;
    }
    let size: usize = section
        .iter()
        .find_map(|line| line.strip_prefix("size="))
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(0);
    let n = size + 1;
    let entry = [
        format!("{n}\\arguments="),
        format!("{n}\\binary={binary}"),
        format!("{n}\\hide=false"),
        format!("{n}\\ownicon=false"),
        format!("{n}\\steamAppID="),
        format!("{n}\\title={title}"),
        format!("{n}\\toolbar=false"),
        format!("{n}\\workingDirectory="),
    ];
    let mut out: Vec<String> = Vec::new();
    match (start, end) {
        (Some(s), Some(e)) => {
            out.extend(lines[..=s].iter().map(|l| l.to_string()));
            let mut had_size = false;
            for line in &lines[s + 1..e] {
                if line.starts_with("size=") {
                    out.push(format!("size={n}"));
                    had_size = true;
                } else {
                    out.push(line.to_string());
                }
            }
            if !had_size {
                out.insert(s + 1, format!("size={n}"));
            }
            // Before any blank lines that close the section.
            let mut tail = Vec::new();
            while out.last().is_some_and(|l| l.trim().is_empty()) {
                tail.push(out.pop().unwrap_or_default());
            }
            out.extend(entry);
            out.extend(tail);
            out.extend(lines[e..].iter().map(|l| l.to_string()));
        }
        _ => {
            out.extend(lines.iter().map(|l| l.to_string()));
            if out.last().is_some_and(|l| !l.trim().is_empty()) {
                out.push(String::new());
            }
            out.push("[customExecutables]".to_string());
            out.push(format!("size={n}"));
            out.extend(entry);
        }
    }
    let mut text = out.join(newline);
    text.push_str(newline);
    Some(text)
}

/// A path as MO2 writes one: forward slashes, and under Wine the `Z:` drive
/// that maps the Unix root.
fn mo2_path(path: &Path) -> String {
    let forward = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        forward
    } else {
        format!("Z:{forward}")
    }
}

/// Set `key=value` in `[section]` of an INI, keeping everything else -- line
/// endings included -- as it was. Adds the section if it is missing.
fn set_ini_key(text: &str, section: &str, key: &str, value: &str) -> String {
    let newline = if text.contains("\r\n") || text.is_empty() {
        "\r\n"
    } else {
        "\n"
    };
    let header = format!("[{section}]");
    let assignment = format!("{key}={value}");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();

    let section_start = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case(&header));
    match section_start {
        Some(start) => {
            let end = lines[start + 1..]
                .iter()
                .position(|line| line.trim_start().starts_with('['))
                .map_or(lines.len(), |offset| start + 1 + offset);
            let existing = lines[start + 1..end].iter().position(|line| {
                line.split_once('=')
                    .is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(key))
            });
            match existing {
                Some(offset) => lines[start + 1 + offset] = assignment,
                None => lines.insert(start + 1, assignment),
            }
        }
        None => {
            if lines.last().is_some_and(|line| !line.is_empty()) {
                lines.push(String::new());
            }
            lines.push(header);
            lines.push(assignment);
        }
    }
    let mut out = lines.join(newline);
    out.push_str(newline);
    out
}

pub(crate) fn render_mo2_modlist(plan: &PersonalizedPlan, scope: ExportScope<'_>) -> String {
    let selected: HashSet<&str> = plan.selected_mods.iter().map(String::as_str).collect();
    let sections = scoped_section_names(plan, scope);

    let mut out = String::new();
    for entry in plan.mo2_modlist_entries.iter().rev() {
        match entry {
            Mo2ModlistEntry::Mod { id } => {
                if !in_scope(scope, id) {
                    continue;
                }
                let prefix = if selected.contains(id.as_str()) {
                    '+'
                } else {
                    '-'
                };
                out.push(prefix);
                out.push_str(id);
            }
            Mo2ModlistEntry::Section { name } => {
                // A separator with nothing under it yet is noise, so drop the
                // ones whose mods this run has not installed.
                if let Some(sections) = &sections
                    && !sections.contains(name)
                {
                    continue;
                }
                out.push('-');
                out.push_str(&mo2_section_mod_name(name));
            }
        }
        out.push('\n');
    }
    out
}

pub(crate) fn ensure_mo2_section_folders(
    plan: &PersonalizedPlan,
    settings: &InstallSettings,
    scope: ExportScope<'_>,
) -> anyhow::Result<()> {
    let sections = scoped_section_names(plan, scope);

    for entry in &plan.mo2_modlist_entries {
        let Mo2ModlistEntry::Section { name } = entry else {
            continue;
        };
        if let Some(sections) = &sections
            && !sections.contains(name)
        {
            continue;
        }

        let path = settings.mods_dir.join(mo2_section_mod_name(name));
        std::fs::create_dir_all(&path).map_err(|err| {
            anyhow::anyhow!(
                "failed to create MO2 section folder {}: {err}",
                path.display()
            )
        })?;
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

/// The load order, reduced to the plugins that will actually load.
///
/// Unfiltered, this is the declared order verbatim. Filtered, a plugin only
/// belongs in the profile if its file is there to load: either inside an
/// in-scope mod or in the game's own Data folder. The alternative -- trusting
/// the declared list -- would name plugins from sections that have not been
/// installed yet, which is precisely the state a section-at-a-time build is in.
fn scoped_plugins(
    plan: &PersonalizedPlan,
    settings: &InstallSettings,
    scope: ExportScope<'_>,
) -> anyhow::Result<Vec<String>> {
    let Some(ids) = scope else {
        return Ok(plan.plugins.clone());
    };

    let mut available: HashSet<String> = HashSet::new();
    for mod_id in ids {
        let mod_dir = settings.mods_dir.join(mod_id);
        if !mod_dir.exists() {
            continue;
        }
        collect_plugin_names(&mod_dir, &mut available)?;
    }

    // Base game masters belong to no mod, so they are found here or not at all.
    if let Some(game_dir) = &settings.game_dir {
        let data_dir = game_dir.join("Data");
        if data_dir.is_dir() {
            collect_plugin_names(&data_dir, &mut available)?;
        }
    }

    let mut dropped = Vec::new();
    let mut kept = Vec::new();
    for plugin in &plan.plugins {
        if available.contains(&plugin.trim().to_ascii_lowercase()) {
            kept.push(plugin.clone());
        } else {
            dropped.push(plugin.as_str());
        }
    }

    if !dropped.is_empty() {
        tracing::info!(
            count = dropped.len(),
            plugins = ?dropped,
            "mo2 export: omitting plugins from plugins.txt because they are not installed yet"
        );
    }

    Ok(kept)
}

/// Lowercased names of every plugin file under a directory.
fn collect_plugin_names(root: &Path, out: &mut HashSet<String>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(root)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", root.display()))?
    {
        let entry =
            entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", root.display()))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            anyhow::anyhow!("failed to read file type for {}: {err}", path.display())
        })?;

        if file_type.is_dir() {
            collect_plugin_names(&path, out)?;
            continue;
        }
        if !file_type.is_file() || !is_plugin_file(&path) {
            continue;
        }

        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            out.insert(name.to_ascii_lowercase());
        }
    }

    Ok(())
}

pub(crate) fn render_archive_list(
    plan: &PersonalizedPlan,
    settings: &InstallSettings,
    scope: ExportScope<'_>,
) -> anyhow::Result<String> {
    let mut out = String::new();

    for mod_id in &plan.mod_order {
        if !in_scope(scope, mod_id) {
            continue;
        }

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

pub(crate) fn collect_bsa_files(
    root: &Path,
    current: &Path,
    out: &mut Vec<String>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?
    {
        let entry = entry
            .map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", current.display()))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            anyhow::anyhow!("failed to read file type for {}: {err}", path.display())
        })?;

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
    Some(
        mo2_instance_dir(settings)?
            .join("profiles")
            .join(&settings.profile_name),
    )
}

/// The game folder an MO2 instance was set up with: `gamePath` in its
/// `ModOrganizer.ini`. `None` when the instance has none, or names it by a
/// Wine drive this side cannot see.
pub fn instance_game_path(instance: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(instance.join("ModOrganizer.ini")).ok()?;
    parse_game_path(&text, cfg!(windows))
}

/// MO2 writes `gamePath=@ByteArray(C:\\Games\\Oblivion)`: a Qt byte array,
/// backslashes escaped. Under Wine the Unix root is drive `Z:`; any other drive
/// is inside a prefix we do not know, so it is not guessed at.
fn parse_game_path(ini: &str, windows: bool) -> Option<PathBuf> {
    let mut in_general = false;
    for line in ini.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_general = line.eq_ignore_ascii_case("[General]");
            continue;
        }
        let Some(value) = line.strip_prefix("gamePath=").filter(|_| in_general) else {
            continue;
        };
        let value = value
            .strip_prefix("@ByteArray(")
            .and_then(|v| v.strip_suffix(')'))
            .unwrap_or(value)
            .replace("\\\\", "\\");
        if value.is_empty() {
            return None;
        }
        if windows {
            return Some(PathBuf::from(value));
        }
        let unix = value
            .strip_prefix("Z:")
            .or_else(|| value.strip_prefix("z:"))?;
        return Some(PathBuf::from(unix.replace('\\', "/")));
    }
    None
}

pub(crate) fn locate_game_ini_source(game_dir: &Path, file_name: &str) -> Option<PathBuf> {
    let direct = game_dir.join(file_name);
    if direct.exists() {
        return Some(direct);
    }

    if !file_name.eq_ignore_ascii_case("Oblivion.ini") {
        return None;
    }

    // On Windows the game keeps it in Documents, which OneDrive may have
    // moved.
    if cfg!(windows) {
        let documents = ["OneDrive", "USERPROFILE"]
            .iter()
            .filter_map(std::env::var_os)
            .map(|root| PathBuf::from(root).join("Documents"));
        for documents in documents {
            let candidate = documents.join("My Games/Oblivion/Oblivion.ini");
            if candidate.exists() {
                return Some(candidate);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// As MO2 wrote it for a new instance: Windows line endings, three
    /// executables, then the next section.
    const NEW_INSTANCE: &str = "[General]\r\ngameName=Oblivion\r\n\r\n[customExecutables]\r\nsize=3\r\n1\\arguments=\r\n1\\binary=Z:/game/Oblivion.exe\r\n1\\hide=false\r\n1\\ownicon=true\r\n1\\steamAppID=\r\n1\\title=Oblivion\r\n1\\toolbar=false\r\n1\\workingDirectory=Z:/game\r\n2\\binary=Z:/game/OblivionLauncher.exe\r\n2\\title=Oblivion Launcher\r\n3\\binary=Z:/list/explorer++/Explorer++.exe\r\n3\\title=Explore Virtual Folder\r\n\r\n[recentDirectories]\r\nsize=0\r\n";

    #[test]
    fn the_theme_is_set_once_and_a_chosen_one_is_kept() {
        let instance = tempfile::tempdir().unwrap();
        let root = instance.path();
        std::fs::create_dir_all(root.join("stylesheets")).unwrap();
        std::fs::write(root.join("stylesheets/vs15 Dark.qss"), b"").unwrap();
        let ini = root.join("ModOrganizer.ini");
        let read = || std::fs::read_to_string(&ini).unwrap();

        // MO2 has none chosen: the list's is set.
        std::fs::write(&ini, "[Settings]\r\ndownload_directory=Z:/d\r\n").unwrap();
        apply_theme(root, "vs15 Dark", false).unwrap();
        assert_eq!(
            ini_value(&read(), "Settings", "style"),
            Some("vs15 Dark.qss")
        );

        // One MO2 does not have: nothing changes.
        std::fs::write(&ini, "[Settings]\r\n").unwrap();
        apply_theme(root, "Nonesuch", false).unwrap();
        assert_eq!(ini_value(&read(), "Settings", "style"), None);

        // Chosen in MO2 since: kept.
        std::fs::write(&ini, "[Settings]\r\nstyle=Paper Dark.qss\r\n").unwrap();
        apply_theme(root, "vs15 Dark.qss", false).unwrap();
        assert_eq!(
            ini_value(&read(), "Settings", "style"),
            Some("Paper Dark.qss")
        );
    }

    #[test]
    fn an_executable_is_added_after_the_others_once() {
        let added = with_executable(NEW_INSTANCE, "Wrye Bash", "Z:/game/Mopy/Wrye Bash.exe")
            .expect("not there yet");
        assert!(
            added.contains("[customExecutables]\r\nsize=4\r\n"),
            "{added}"
        );
        assert!(
            added.contains(
                "3\\title=Explore Virtual Folder\r\n4\\arguments=\r\n4\\binary=Z:/game/Mopy/Wrye Bash.exe\r\n"
            ),
            "{added}"
        );
        assert!(added.contains("4\\title=Wrye Bash\r\n4\\toolbar=false\r\n4\\workingDirectory=\r\n\r\n[recentDirectories]"), "{added}");
        assert!(
            !added.contains("\n\n") || added.contains("\r\n\r\n"),
            "line endings kept"
        );
        // Already there, by its program or by its name: left alone.
        assert_eq!(
            with_executable(&added, "Wrye Bash", "Z:/game/Mopy/Wrye Bash.exe"),
            None
        );
        assert_eq!(
            with_executable(&added, "Wrye Bash", "Z:\\game\\Mopy\\wrye bash.exe"),
            None
        );
        assert_eq!(
            with_executable(&added, "WB", "Z:/game/Mopy/Wrye Bash.exe"),
            None
        );
    }

    #[test]
    fn an_instance_with_no_executables_gets_the_section() {
        let added =
            with_executable("[General]\ngameName=Oblivion\n", "Wrye Bash", "Z:/w.exe").unwrap();
        assert!(added.ends_with("[customExecutables]\nsize=1\n1\\arguments=\n1\\binary=Z:/w.exe\n1\\hide=false\n1\\ownicon=false\n1\\steamAppID=\n1\\title=Wrye Bash\n1\\toolbar=false\n1\\workingDirectory=\n"), "{added}");
    }

    #[test]
    fn the_game_path_comes_from_mo2s_own_settings() {
        let ini = "[General]\r\ngameName=Oblivion\r\ngamePath=@ByteArray(Z:\\\\home\\\\me\\\\Steam\\\\common\\\\Oblivion)\r\n";
        assert_eq!(
            parse_game_path(ini, false),
            Some(PathBuf::from("/home/me/Steam/common/Oblivion"))
        );
        let windows = "[General]\ngamePath=@ByteArray(C:\\\\Games\\\\Oblivion)\n";
        assert_eq!(
            parse_game_path(windows, true),
            Some(PathBuf::from("C:\\Games\\Oblivion"))
        );
        // A Wine drive other than Z: is inside a prefix; not guessed at.
        assert_eq!(parse_game_path(windows, false), None);
        assert_eq!(parse_game_path("[Settings]\ngamePath=x\n", false), None);
    }

    #[test]
    fn profile_ini_files_are_turned_on_and_the_rest_kept() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.ini"),
            "[General]\nLocalSaves=true\nLocalSettings=false\n",
        )
        .unwrap();
        use_profile_ini(dir.path(), false).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("settings.ini")).unwrap(),
            "[General]\nLocalSaves=true\nLocalSettings=true\n"
        );
    }
}
