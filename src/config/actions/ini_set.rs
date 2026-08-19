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

    let outcome = apply_ini_set_in_section(
        &ini_path,
        action.section.as_deref(),
        &action.key,
        &action.value.0,
        action.format,
    )?;

    // Every other action reports what it did; this one reported nothing at all,
    // which made its most surprising branch its quietest.
    match outcome {
        IniSetOutcome::Replaced => tracing::info!(
            owner = cx.owner,
            ini = %ini_path.display(),
            key = action.key,
            value = %action.value,
            "set an existing ini key"
        ),
        IniSetOutcome::AppendedToSection => tracing::info!(
            owner = cx.owner,
            ini = %ini_path.display(),
            key = action.key,
            value = %action.value,
            section = ?action.section,
            "added a missing ini key"
        ),
        IniSetOutcome::CreatedSection => tracing::warn!(
            owner = cx.owner,
            ini = %ini_path.display(),
            key = action.key,
            section = ?action.section,
            "ini_set created a [section] the file did not have. If the file marks \
             its sections with comments rather than headers, this is not the edit \
             you wanted -- the key the guide meant is elsewhere in the file, and \
             now there are two."
        ),
    }
    Ok(())
}

/// What an `ini_set` actually did, so the caller can say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IniSetOutcome {
    /// A key that was there had its value changed.
    Replaced,
    /// A key the file lacked was added to a section that existed.
    AppendedToSection,
    /// Both the key and its `[section]` were added. Legitimate for Oblivion.ini,
    /// which omits sections it has no non-default keys for -- and a red flag
    /// anywhere else, which is why it is the one outcome that warns.
    CreatedSection,
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
) -> anyhow::Result<IniSetOutcome> {
    apply_ini_set_in_section(path, None, key, value, format)
}

pub(crate) fn apply_ini_set_in_section(
    path: &Path,
    section: Option<&str>,
    key: &str,
    value: &str,
    format: IniSetFormat,
) -> anyhow::Result<IniSetOutcome> {
    let original = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    // These files come from Windows-authored archives and mostly use CRLF.
    // Rewriting them with LF changes every line of a file we were asked to
    // change one value in -- harmless to Oblivion, which reads both, but it
    // makes each INI we touch differ from the Oracle for no reason.
    let newline = if original.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing_newline = original.ends_with('\n');
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
                write_ini_lines(path, &lines, newline, trailing_newline)?;
                return Ok(IniSetOutcome::CreatedSection);
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

    let outcome = if matches.is_empty() {
        lines.insert(append_at, rendered);
        IniSetOutcome::AppendedToSection
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
        IniSetOutcome::Replaced
    };

    write_ini_lines(path, &lines, newline, trailing_newline)?;
    Ok(outcome)
}

/// The `[Section]` a line sits under, for error messages.
fn enclosing_section(lines: &[String], index: usize) -> Option<&str> {
    lines[..index].iter().rev().find_map(|line| section_header(line))
}

/// Write the lines back, keeping the file's own line ending *and* whether it
/// ended with one.
///
/// `trailing_newline` is not a detail: `Av Latta Magicka.ini` ships without a
/// final newline, and adding one made a file we changed one value in differ
/// from the reference by two bytes. Harmless to the game and pure noise in a
/// report, which is the sort of thing that trains people to skim the report.
fn write_ini_lines(
    path: &Path,
    lines: &[String],
    newline: &str,
    trailing_newline: bool,
) -> anyhow::Result<()> {
    let mut content = lines.join(newline);
    if trailing_newline && !content.ends_with('\n') {
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
    if format == IniSetFormat::SetTo {
        return replace_set_to_value(line, key, value);
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

/// Replace only the value in a `set <key> to <value>` line.
///
/// These files are hand-aligned with tabs and carry a trailing comment on almost
/// every line -- `set dcvars.ini_NPCdodgePercent\t\tto\t70\t;Default 90`. Re-rendering
/// the line threw both away, which is the same mistake the standard format made
/// and had fixed: an edit should set a value, not reformat somebody's file.
///
/// Everything before the value and everything after it is kept exactly.
fn replace_set_to_value(line: &str, key: &str, value: &str) -> Option<String> {
    // Found by scanning for the key at a token boundary rather than anywhere in
    // the line, so this does not depend on `is_ini_key_line` having filtered
    // the line first. Two parsers of one syntax that agree only because of call
    // order is the kind of coupling this file has been bitten by before.
    let key_at = line.match_indices(key).find(|(at, _)| {
        let before_ok = line[..*at].ends_with(char::is_whitespace);
        let after = &line[at + key.len()..];
        before_ok && after.starts_with(char::is_whitespace)
    })?.0;
    let after_key = &line[key_at + key.len()..];

    // The `to` separator, with whatever whitespace surrounds it.
    let gap = after_key.len() - after_key.trim_start().len();
    let rest = &after_key[gap..];
    let to = rest.get(..2)?;
    if !to.eq_ignore_ascii_case("to") {
        return None;
    }
    let after_to = &rest[2..];

    // The value runs to the next whitespace; anything past it -- padding, a
    // `;Default 90` comment, trailing spaces -- belongs to the author.
    let value_gap = after_to.len() - after_to.trim_start().len();
    let old_value = after_to[value_gap..]
        .split_whitespace()
        .next()
        .unwrap_or_default();

    let head_len = line.len() - after_to.len() + value_gap;
    Some(format!(
        "{}{value}{}",
        &line[..head_len],
        &after_to[value_gap + old_value.len()..]
    ))
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

#[cfg(test)]
mod set_to_tests {
    use super::replace_set_to_value;

    #[test]
    fn only_the_value_changes() {
        // Part 23's Dynamic Oblivion Combat.ini, which is tab-aligned and
        // annotates every line. Re-rendering these threw away both the columns
        // and the author's note about the default.
        assert_eq!(
            replace_set_to_value(
                "set dcvars.ini_NPCdodgePercent\t\tto\t70\t;Default 90",
                "dcvars.ini_NPCdodgePercent",
                "50"
            )
            .as_deref(),
            Some("set dcvars.ini_NPCdodgePercent\t\tto\t50\t;Default 90")
        );
        assert_eq!(
            replace_set_to_value(
                "set dcvars.ini_DodgeKeyCode\t\tto\t35\t;Default 35 (the keyboard H key) ",
                "dcvars.ini_DodgeKeyCode",
                "42"
            )
            .as_deref(),
            Some("set dcvars.ini_DodgeKeyCode\t\tto\t42\t;Default 35 (the keyboard H key) ")
        );
    }

    #[test]
    fn a_plain_line_survives_too() {
        assert_eq!(
            replace_set_to_value("set almQ.bDisableREHEShader to 0", "almQ.bDisableREHEShader", "1")
                .as_deref(),
            Some("set almQ.bDisableREHEShader to 1")
        );
    }

    #[test]
    fn a_line_that_is_not_this_assignment_is_declined() {
        assert_eq!(replace_set_to_value("set other to 1", "wanted", "2"), None);
        assert_eq!(replace_set_to_value("wanted = 1", "wanted", "2"), None);
    }
}

#[cfg(test)]
mod trailing_newline_tests {
    use super::{apply_ini_set, IniSetFormat};

    #[test]
    fn a_file_without_a_final_newline_does_not_gain_one() {
        // `Av Latta Magicka.ini` ships this way. Adding a newline made a file
        // we changed one value in differ from the reference by two bytes.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("no-newline.ini");
        std::fs::write(&path, "set a.b to 0\r\nset c.d to 0").expect("write");

        apply_ini_set(&path, "a.b", "1", IniSetFormat::SetTo).expect("ini_set");

        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "set a.b to 1\r\nset c.d to 0"
        );
    }

    #[test]
    fn a_file_with_a_final_newline_keeps_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("newline.ini");
        std::fs::write(&path, "set a.b to 0\n").expect("write");

        apply_ini_set(&path, "a.b", "1", IniSetFormat::SetTo).expect("ini_set");

        assert_eq!(std::fs::read_to_string(&path).expect("read"), "set a.b to 1\n");
    }
}

#[cfg(test)]
mod outcome_tests {
    use super::{apply_ini_set, apply_ini_set_in_section, replace_set_to_value, IniSetFormat, IniSetOutcome};

    #[test]
    fn each_kind_of_edit_reports_itself() {
        let dir = tempfile::tempdir().expect("tempdir");

        let path = dir.path().join("existing.ini");
        std::fs::write(&path, "[A]\nkey=0\n").expect("write");
        assert_eq!(
            apply_ini_set_in_section(&path, Some("A"), "key", "1", IniSetFormat::Standard).unwrap(),
            IniSetOutcome::Replaced
        );

        let path = dir.path().join("missing-key.ini");
        std::fs::write(&path, "[A]\nother=0\n").expect("write");
        assert_eq!(
            apply_ini_set_in_section(&path, Some("A"), "key", "1", IniSetFormat::Standard).unwrap(),
            IniSetOutcome::AppendedToSection
        );

        // The branch Part 23 hit, and the one that warns: a file whose sections
        // are comment banners rather than headers gets a header invented, and
        // the key the guide meant stays untouched somewhere above.
        let path = dir.path().join("no-section.ini");
        std::fs::write(&path, ";===== A =====\nkey=0\n").expect("write");
        assert_eq!(
            apply_ini_set_in_section(&path, Some("A"), "key", "1", IniSetFormat::Standard).unwrap(),
            IniSetOutcome::CreatedSection
        );
    }

    #[test]
    fn the_key_is_matched_at_a_token_boundary() {
        // `replace_set_to_value` used to rely on `is_ini_key_line` having
        // already vetted the line, so a substring match could not bite. It no
        // longer does, and must not.
        assert_eq!(
            replace_set_to_value("set prefix.wanted to 0", "wanted", "1"),
            None,
            "`wanted` is a suffix of the real key, not the key"
        );
        assert_eq!(
            replace_set_to_value("set wanted to 0", "wanted", "1").as_deref(),
            Some("set wanted to 1")
        );
    }

    #[test]
    fn a_plain_apply_still_reports() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("plain.ini");
        std::fs::write(&path, "set a.b to 0\n").expect("write");
        assert_eq!(
            apply_ini_set(&path, "a.b", "1", IniSetFormat::SetTo).unwrap(),
            IniSetOutcome::Replaced
        );
    }
}
