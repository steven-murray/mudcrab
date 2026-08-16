//! `ini_set`: set a key in an INI file.

use super::ActionCx;
use crate::config::install::InstallSettings;
use crate::config::mo2::mo2_profile_dir;
use crate::config::schema::{IniScope, IniSetAction, IniSetFormat};
use crate::util::fs::{normalize_relative_path, resolve_existing_path_case_insensitive};
use std::path::{Path, PathBuf};

pub(super) fn apply(action: &IniSetAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let ini_path = match action.scope {
        IniScope::Mod => {
            let Some(mod_target) = cx.mod_target else {
                anyhow::bail!(
                    "{}: ini_set with scope \"mod\" is only valid on a per-mod action",
                    cx.owner
                );
            };
            mod_target.join(normalize_relative_path(&action.file)?)
        }
        IniScope::Game => {
            let Some(path) = resolve_game_scoped_ini_path(cx.settings, &action.file) else {
                tracing::warn!(
                    owner = cx.owner,
                    file = action.file,
                    "skipping game-scoped ini_set: no game-dir available in staging mode"
                );
                return Ok(());
            };
            path
        }
    };

    let ini_path = resolve_existing_path_case_insensitive(&ini_path).ok_or_else(|| {
        anyhow::anyhow!(
            "{} ini_set target file does not exist: {}",
            cx.owner,
            ini_path.display()
        )
    })?;

    if cx.settings.dry_run {
        tracing::info!(
            owner = cx.owner,
            ini = %ini_path.display(),
            key = action.key,
            value = %action.value,
            scope = ?action.scope,
            "install dry-run ini_set action"
        );
        return Ok(());
    }

    apply_ini_set(&ini_path, &action.key, &action.value.0, action.format)
}

/// Locate a game-scoped INI.
///
/// Prefers the MO2 profile-local copy: mudcrab must never modify the original
/// Oblivion.ini in the game directory.
pub(crate) fn resolve_game_scoped_ini_path(
    settings: &InstallSettings,
    file: &str,
) -> Option<PathBuf> {
    let rel = normalize_relative_path(file).ok()?;
    if let Some(profile_dir) = mo2_profile_dir(settings) {
        return Some(profile_dir.join(rel));
    }
    Some(settings.game_dir.as_ref()?.join(rel))
}

pub(crate) fn apply_ini_set(
    path: &Path,
    key: &str,
    value: &str,
    format: IniSetFormat,
) -> anyhow::Result<()> {
    let mut lines: Vec<String> = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?
        .lines()
        .map(ToString::to_string)
        .collect();

    let mut replaced = false;
    for line in &mut lines {
        if is_ini_key_line(line, key, format) {
            *line = render_ini_assignment(key, value, format);
            replaced = true;
        }
    }

    if !replaced {
        lines.push(render_ini_assignment(key, value, format));
    }

    let mut content = lines.join("\n");
    if !content.ends_with('\n') {
        content.push('\n');
    }

    std::fs::write(path, content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))
}

fn render_ini_assignment(key: &str, value: &str, format: IniSetFormat) -> String {
    match format {
        IniSetFormat::Standard => format!("{key} = {value}"),
        IniSetFormat::SetTo => format!("set {key} to {value}"),
    }
}

fn is_ini_key_line(line: &str, key: &str, format: IniSetFormat) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with(';') {
        return false;
    }

    match format {
        IniSetFormat::Standard => trimmed
            .split_once('=')
            .is_some_and(|(lhs, _)| lhs.trim() == key),
        IniSetFormat::SetTo => {
            let mut parts = trimmed.split_whitespace();
            let (Some(command), Some(found_key), Some(separator)) =
                (parts.next(), parts.next(), parts.next())
            else {
                return false;
            };
            command.eq_ignore_ascii_case("set")
                && separator.eq_ignore_ascii_case("to")
                && found_key == key
        }
    }
}
