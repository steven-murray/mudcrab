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

    // Match the file's own spacing rather than imposing ours. Oblivion.ini is
    // written `Key=Value`, and Oblivion's parser takes everything after the `=`
    // literally -- so `SFontFile_1 = Data\Fonts\x.fnt` becomes a path with a
    // leading space, the font fails to load, and the game silently falls back
    // to vanilla. That is a broken UI produced by two space characters.
    //
    // The whole file's style decides, not the line being replaced, so a line
    // written in the wrong style by an earlier version is repaired rather than
    // preserved.
    let spaced = dominant_spacing(&lines, format);

    let mut replaced = false;
    for line in &mut lines {
        if is_ini_key_line(line, key, format) {
            *line = render_ini_assignment(key, value, format, spaced);
            replaced = true;
        }
    }

    if !replaced {
        lines.push(render_ini_assignment(key, value, format, spaced));
    }

    let mut content = lines.join("\n");
    if !content.ends_with('\n') {
        content.push('\n');
    }

    std::fs::write(path, content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))
}

fn render_ini_assignment(key: &str, value: &str, format: IniSetFormat, spaced: bool) -> String {
    match format {
        // `set X to Y` is a script command; its spacing is not optional.
        IniSetFormat::SetTo => format!("set {key} to {value}"),
        IniSetFormat::Standard if spaced => format!("{key} = {value}"),
        IniSetFormat::Standard => format!("{key}={value}"),
    }
}

/// Whether an assignment puts spaces around its `=`.
///
/// `None` for a line that is not an assignment of this format.
fn line_is_spaced(line: &str, format: IniSetFormat) -> Option<bool> {
    if format != IniSetFormat::Standard {
        return None;
    }
    let (lhs, rhs) = line.split_once('=')?;
    Some(lhs.ends_with(' ') || rhs.starts_with(' '))
}

/// The spacing style most of the file already uses, for a key being appended.
///
/// A file with no assignments at all gets the unspaced form, which is what
/// every Bethesda INI uses and what the game's parser is least surprised by.
fn dominant_spacing(lines: &[String], format: IniSetFormat) -> bool {
    let mut spaced = 0usize;
    let mut tight = 0usize;
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.starts_with(';') || trimmed.starts_with('[') {
            continue;
        }
        match line_is_spaced(trimmed, format) {
            Some(true) => spaced += 1,
            Some(false) => tight += 1,
            None => {}
        }
    }
    spaced > tight
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
