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

    apply_ini_set_in_section(
        &ini_path,
        action.section.as_deref(),
        &action.key,
        &action.value.0,
        action.format,
    )
}

/// The `[Section]` name a header line declares, if it is one.
fn section_header(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
}

/// Line range of `[section]`'s body: from just after its header to just before
/// the next header (or end of file). `None` when the file has no such section.
///
/// Trailing blank lines are excluded, so a key appended to a section lands
/// against the last real entry rather than after the gap that separates it from
/// the next header.
fn section_body(lines: &[String], section: &str) -> Option<(usize, usize)> {
    let start = lines.iter().position(|line| {
        section_header(line).is_some_and(|name| name.eq_ignore_ascii_case(section))
    })? + 1;

    let mut end = lines.len();
    for (offset, line) in lines[start..].iter().enumerate() {
        if section_header(line).is_some() {
            end = start + offset;
            break;
        }
    }

    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }

    Some((start, end))
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

/// Section-less shim, kept for the tests that predate sections.
#[cfg(test)]
pub(crate) fn apply_ini_set(
    path: &Path,
    key: &str,
    value: &str,
    format: IniSetFormat,
) -> anyhow::Result<()> {
    apply_ini_set_in_section(path, None, key, value, format)
}

pub(crate) fn apply_ini_set_in_section(
    path: &Path,
    section: Option<&str>,
    key: &str,
    value: &str,
    format: IniSetFormat,
) -> anyhow::Result<()> {
    let original = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    // These files come from Windows-authored archives and mostly use CRLF.
    // Rewriting them with LF changes every line of a file we were asked to
    // change one value in -- harmless to Oblivion, which reads both, but it
    // makes each INI we touch differ from the Oracle for no reason.
    let newline = if original.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = original.lines().map(ToString::to_string).collect();

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
    let aligned = dominant_alignment(&lines, format);
    let rendered = render_ini_assignment(key, value, format, spaced);

    // Which lines this edit is allowed to touch, and where a missing key would
    // be appended. Without a section that is the whole file and the end of it;
    // with one it is that section's body, so a key the section lacks is added
    // *inside* it rather than after the last section in the file -- where the
    // game would read it as belonging to something else entirely.
    let (range, append_at) = match section {
        None => (0..lines.len(), lines.len()),
        Some(name) => match section_body(&lines, name) {
            Some((start, end)) => (start..end, end),
            // A section the file does not have yet is created rather than
            // treated as an error: Oblivion.ini omits sections it has no
            // non-default keys for, and the guide still asks for keys in them.
            None => {
                if lines.last().is_some_and(|line| !line.trim().is_empty()) {
                    lines.push(String::new());
                }
                lines.push(format!("[{name}]"));
                lines.push(rendered);
                return write_ini_lines(path, &lines, newline);
            }
        },
    };

    let matches: Vec<usize> = range
        .filter(|&index| is_ini_key_line(&lines[index], key, format))
        .collect();

    // Two matches means two different settings that happen to share a name, and
    // nothing in the request says which was meant. Writing both was the old
    // behaviour and is never what an author wants -- Part 14's Fog.ini has
    // `Amount` under both [World] and [Interior], and setting the interior fog
    // would silently have changed the weather.
    if matches.len() > 1 && section.is_none() {
        let sections = matches
            .iter()
            .map(|&index| enclosing_section(&lines, index).unwrap_or("(no section)"))
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!(
            "ini_set '{key}' matches {} lines in {} (sections: {sections}). \
             Add `section = \"<name>\"` to say which one is meant.",
            matches.len(),
            path.display(),
        );
    }

    if matches.is_empty() {
        lines.insert(append_at, rendered);
    } else {
        for index in matches {
            // Replacing a key sets its value; it does not reformat the line.
            // ORC's Fog.ini aligns its `=` into a column (`Amount        =0.0`)
            // and rewriting that as `Amount = 0.0` would be a gratuitous
            // difference in a file we were only asked to change one number in.
            // Appending a *new* key still uses the file's dominant style,
            // because there is no original line to preserve.
            lines[index] = replace_value_in_place(&lines[index], key, value, format, aligned, spaced)
                .unwrap_or_else(|| rendered.clone());
        }
    }

    write_ini_lines(path, &lines, newline)
}

/// The `[Section]` a line sits under, for error messages.
fn enclosing_section(lines: &[String], index: usize) -> Option<&str> {
    lines[..index].iter().rev().find_map(|line| section_header(line))
}

fn write_ini_lines(path: &Path, lines: &[String], newline: &str) -> anyhow::Result<()> {
    let mut content = lines.join(newline);
    if !content.ends_with('\n') {
        content.push_str(newline);
    }

    std::fs::write(path, content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))
}

/// Rewrite only what follows `=`, keeping the line's own left-hand side.
///
/// Returns `None` for a line this cannot be done to -- a `set X to Y` command,
/// or an assignment with no `=` -- leaving the caller to render from scratch.
///
/// The right-hand side's spacing is taken from the line being replaced, not from
/// the file: a line written `Key=Value` keeps its tight form, which is what
/// Oblivion's literal parser needs. A line an earlier version wrote as
/// `Key = Value` is still repaired, because `dominant_spacing` decides that and
/// this only runs when the two agree.
fn replace_value_in_place(
    line: &str,
    key: &str,
    value: &str,
    format: IniSetFormat,
    aligned: bool,
    spaced: bool,
) -> Option<String> {
    if format != IniSetFormat::Standard {
        return None;
    }
    let (lhs, _rhs) = line.split_once('=')?;
    if lhs.trim() != key {
        return None;
    }
    // Keep the padding only when padding is what this file does. In an aligned
    // file it is the author's column and removing it would rewrite every line
    // we touch; in a tight file it is an anomaly -- very likely one an earlier
    // version of mudcrab introduced -- and repairing it is the point.
    let lhs = if aligned { lhs.to_string() } else { key.to_string() };
    // The space *after* the `=` is the half Oblivion reads literally, so it
    // comes from the whole file's habit rather than from this one line -- which
    // is how a line an earlier version wrote as `Key = Value` still gets
    // repaired in a file that is otherwise tight.
    let separator = if spaced { " " } else { "" };
    Some(format!("{lhs}={separator}{value}"))
}

/// Whether the file pads its keys out to a column before the `=`.
///
/// Distinct from `dominant_spacing`, which is about the space *after* the `=` --
/// the half Oblivion's parser reads literally. Padding before it is only ever
/// cosmetic, and ORC's `Fog.ini` (`Amount        =0.0`) is written that way
/// throughout.
fn dominant_alignment(lines: &[String], format: IniSetFormat) -> bool {
    if format != IniSetFormat::Standard {
        return false;
    }
    let mut padded = 0usize;
    let mut tight = 0usize;
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.starts_with(';') || trimmed.starts_with('[') {
            continue;
        }
        let Some((lhs, _)) = trimmed.split_once('=') else {
            continue;
        };
        if lhs.ends_with(' ') {
            padded += 1;
        } else {
            tight += 1;
        }
    }
    padded > tight
}

fn render_ini_assignment(key: &str, value: &str, format: IniSetFormat, spaced: bool) -> String {
    match format {
        // `set X to Y` is a script command; its spacing is not optional.
        IniSetFormat::SetTo => format!("set {key} to {value}"),
        IniSetFormat::Standard if spaced => format!("{key} = {value}"),
        IniSetFormat::Standard => format!("{key}={value}"),
    }
}

/// Whether an assignment puts a space *after* its `=`.
///
/// Deliberately blind to padding before the `=`. The two are different things:
/// the space after is read literally by Oblivion's parser and is what broke the
/// DarNified font paths, while padding before it is the author aligning a
/// column, as ORC's `Fog.ini` does throughout. Counting alignment as "spaced"
/// made every aligned file look like it wanted `Key = Value`.
///
/// `None` for a line that is not an assignment of this format.
fn line_is_spaced(line: &str, format: IniSetFormat) -> Option<bool> {
    if format != IniSetFormat::Standard {
        return None;
    }
    let (_lhs, rhs) = line.split_once('=')?;
    Some(rhs.starts_with(' '))
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
