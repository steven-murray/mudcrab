//! An INI read for editing a mod's settings: each line as written, its
//! settings picked out as `ini_set` matches them, and the mod's own INI
//! actions on it grouped by their condition.
//!
//! The GUI's INI editor shows the file in its own order, comments and all,
//! with an input for each setting; what the author changes there is written
//! back as the mod's `ini_set`, `ini_comment` and `ini_append_block` actions
//! for one condition at a time ([`crate::config::edit::doc::set_ini_layer`]).

use crate::config::schema::{IniScope, IniSetFormat, ModAction, ModEntry};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One line of an INI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IniLine {
    /// `[Name]`.
    Section { name: String },
    /// A setting `ini_set` can change: `key=value`, or `set key to value`.
    Setting {
        section: Option<String>,
        key: String,
        value: String,
        format: IniSetFormat,
        /// A comment after it on the line, `;` and all.
        #[serde(skip_serializing_if = "Option::is_none")]
        comment: Option<String>,
        /// How many lines set this key in its section: more than one is a
        /// script building a list, which `ini_set` cannot change one line
        /// of.
        times: usize,
        text: String,
    },
    /// A comment, a blank line, or anything else, as written.
    Text { text: String },
}

/// `text`'s lines, each read as `ini_set` reads it.
pub fn read_lines(text: &str) -> Vec<IniLine> {
    let mut section: Option<String> = None;
    let lines = text
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if let Some(name) = trimmed
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                section = Some(name.to_string());
                return IniLine::Section {
                    name: name.to_string(),
                };
            }
            match setting(trimmed) {
                Some((key, value, format, comment)) => IniLine::Setting {
                    section: section.clone(),
                    key,
                    value,
                    format,
                    comment,
                    times: 1,
                    text: line.to_string(),
                },
                None => IniLine::Text {
                    text: line.to_string(),
                },
            }
        })
        .collect();
    count_times(lines)
}

/// Each setting with how many lines in its section set its key.
fn count_times(mut lines: Vec<IniLine>) -> Vec<IniLine> {
    let mut times: std::collections::HashMap<(Option<String>, String), usize> =
        std::collections::HashMap::new();
    for line in &lines {
        if let IniLine::Setting { section, key, .. } = line {
            *times
                .entry((section.as_ref().map(|s| s.to_lowercase()), key.clone()))
                .or_default() += 1;
        }
    }
    for line in &mut lines {
        if let IniLine::Setting {
            section,
            key,
            times: n,
            ..
        } = line
        {
            *n = times[&(section.as_ref().map(|s| s.to_lowercase()), key.clone())];
        }
    }
    lines
}

/// A setting's key, value, form and trailing comment, where `line` is one.
fn setting(line: &str) -> Option<(String, String, IniSetFormat, Option<String>)> {
    if line.starts_with(';') || line.starts_with('#') {
        return None;
    }
    let mut words = line.split_whitespace();
    if let (Some(command), Some(key), Some(to)) = (words.next(), words.next(), words.next())
        && command.eq_ignore_ascii_case("set")
        && to.eq_ignore_ascii_case("to")
    {
        // As `ini_set` reads it: the value runs to a `;` comment.
        let after_key = line[3..].trim_start().strip_prefix(key)?;
        let after_to = &after_key.trim_start()[2..];
        let (start, end) = crate::config::actions::ini_set::set_to_value_span(after_to);
        let comment = Some(after_to[end..].trim())
            .filter(|c| !c.is_empty())
            .map(str::to_string);
        return Some((
            key.to_string(),
            after_to[start..end].to_string(),
            IniSetFormat::SetTo,
            comment,
        ));
    }
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    (!key.is_empty()).then(|| {
        (
            key.to_string(),
            value.trim().to_string(),
            IniSetFormat::Standard,
            None,
        )
    })
}

/// One of a mod's INI actions on a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IniAction {
    Set {
        section: Option<String>,
        key: String,
        value: String,
        format: IniSetFormat,
    },
    Comment {
        key: String,
        format: IniSetFormat,
    },
    Append {
        block: String,
    },
}

/// A mod's INI actions on one file that share a condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IniGroup {
    #[serde(rename = "if")]
    pub condition: Option<String>,
    pub actions: Vec<IniAction>,
}

/// Whether two INI paths name the same file: case and slashes aside.
pub fn same_file(a: &str, b: &str) -> bool {
    let norm = |p: &str| p.replace('\\', "/").trim_start_matches("./").to_lowercase();
    norm(a) == norm(b)
}

/// `entry`'s INI actions on `file` (in its folder, or the game's), grouped
/// by their condition in the order each first appears.
pub fn groups(entry: &ModEntry, file: &str, scope: IniScope) -> Vec<IniGroup> {
    let mut out: Vec<IniGroup> = Vec::new();
    for conditional in &entry.actions {
        let actions: Vec<IniAction> = match &conditional.action {
            ModAction::IniSet(spec) if spec.scope == scope && same_file(&spec.file, file) => spec
                .settings()
                .into_iter()
                .map(|s| IniAction::Set {
                    section: s.section.map(str::to_string),
                    key: s.key.to_string(),
                    value: s.value.to_string(),
                    format: spec.format,
                })
                .collect(),
            ModAction::IniComment(spec)
                if scope == IniScope::Mod && same_file(&spec.file, file) =>
            {
                vec![IniAction::Comment {
                    key: spec.key.clone(),
                    format: spec.format,
                }]
            }
            ModAction::IniAppendBlock(spec)
                if scope == IniScope::Mod && same_file(&spec.file, file) =>
            {
                vec![IniAction::Append {
                    block: spec.block.clone(),
                }]
            }
            _ => continue,
        };
        match out
            .iter_mut()
            .find(|group| group.condition == conditional.condition)
        {
            Some(group) => group.actions.extend(actions),
            None => out.push(IniGroup {
                condition: conditional.condition.clone(),
                actions,
            }),
        }
    }
    out
}

/// What the INI editor writes for one file under one condition: in place of
/// the mod's INI actions on it with that condition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct IniLayer {
    pub file: String,
    #[serde(default)]
    pub scope: IniScope,
    #[serde(rename = "if", default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub sets: Vec<IniSetting>,
    /// Settings to comment out.
    #[serde(default)]
    pub comments: Vec<IniKey>,
    /// Lines to append.
    #[serde(default)]
    pub block: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IniSetting {
    pub section: Option<String>,
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub format: IniSetFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IniKey {
    pub key: String,
    #[serde(default)]
    pub format: IniSetFormat,
}

/// The text of `entry` in `archive`: UTF-8, else read byte for byte as
/// Latin-1, which Windows-written INIs nearly always are.
pub fn archive_text(archive: &Path, entry: &str) -> anyhow::Result<String> {
    // A folder of its own per call: threads share the process's, and each
    // removes its folder when done.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let scratch = crate::util::fs::cache_scratch_dir(&format!("ini-{call}"));
    let bytes = crate::archive::read_entry(archive, entry, &scratch)?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    Ok(match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_read_as_ini_set_reads_them() {
        let text = "; Shadows\n[Display]\niShadowMapResolution=1024\n;bOld=1\n\nset dcvars.ini_x\t\tto\t70\t;Default 90\n";
        let lines = read_lines(text);
        assert_eq!(
            lines[2],
            IniLine::Setting {
                section: Some("Display".to_string()),
                key: "iShadowMapResolution".to_string(),
                value: "1024".to_string(),
                format: IniSetFormat::Standard,
                comment: None,
                times: 1,
                text: "iShadowMapResolution=1024".to_string(),
            }
        );
        assert!(matches!(&lines[3], IniLine::Text { text } if text == ";bOld=1"));
        assert!(matches!(
            &lines[5],
            IniLine::Setting { key, value, format: IniSetFormat::SetTo, .. }
                if key == "dcvars.ini_x" && value == "70"
        ));
        assert!(matches!(&lines[0], IniLine::Text { .. }));
        assert!(matches!(&lines[1], IniLine::Section { name } if name == "Display"));
    }

    #[test]
    fn a_script_ini_is_read_whole_values_and_repeats_counted() {
        let text = "set Q.sListType to sv_Construct \"Silver Weapons\"\nset Q.rListAdd to (GetFormFromMod \"A.esp\" 07169A) ;a token\nSetStage Q 5\nset Q.rListAdd to (GetFormFromMod \"B.esp\" 00A0B4)\n";
        let lines = read_lines(text);
        assert!(matches!(
            &lines[0],
            IniLine::Setting { value, times: 1, comment: None, .. } if value == "sv_Construct \"Silver Weapons\""
        ));
        assert!(matches!(
            &lines[1],
            IniLine::Setting { value, times: 2, comment: Some(c), .. }
                if value == "(GetFormFromMod \"A.esp\" 07169A)" && c == ";a token"
        ));
        assert!(matches!(&lines[2], IniLine::Text { .. }));
    }

    #[test]
    fn paths_match_whatever_their_case_and_slashes() {
        assert!(same_file("INI\\Foo.ini", "ini/foo.INI"));
        assert!(!same_file("ini/foo.ini", "foo.ini"));
    }
}
