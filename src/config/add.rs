//! Append a `[[mods]]` block to a source modlist without rewriting the file.
//!
//! `SourceModlist` is deliberately `Deserialize`-only, and the modlists this
//! tool exists to maintain carry their reasoning in comments: source URLs, why a
//! layout is overridden, what still needs checking. Parsing to a `toml::Table`
//! and re-emitting would drop every one of those, so `add` works the same way
//! `MOFAM-test/scripts/migrate-schema.py` does -- it parses only enough to find
//! the insertion point, splices in new text, and leaves every other byte alone.
//!
//! The two halves are cross-checked: the file is parsed normally to get the mod
//! ids and sections in order, and scanned line-by-line to get the line number of
//! each `[[mods]]` header. If the two disagree on how many mods there are, the
//! scanner has misread something and the edit is refused rather than guessed.

use crate::config::schema::ModEntry;
use std::path::Path;

/// A mod entry to be written, already resolved from `--from-oracle` or `--nexus`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewMod {
    /// Mod id, which is also the directory name it installs into.
    pub id: String,
    /// Section path, outermost first. Empty means emit no `section` key.
    pub section: Vec<String>,
    /// Archive descriptor, e.g. `nexus:oblivion/43752/1000029844`.
    pub path: Option<String>,
    pub download_handler: Option<String>,
    pub file_name: Option<String>,
    /// Oracle `version=`, rendered as a trailing `# oracle version ...` comment.
    /// Kept as a comment rather than a field because the schema has nowhere to
    /// put it and it is a note to the author, not something mudcrab acts on.
    pub version: Option<String>,
    /// The source is not on Nexus: emit the block with no `path` and a TODO, so
    /// the gap is visible in the file rather than filled with a guessed URL.
    pub non_nexus: bool,
}

/// Where a rendered block will be spliced in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anchor {
    /// After the last existing `[[mods]]` block carrying this section path.
    AfterSection(Vec<String>),
    /// Before the first block of a named section, for starting a new section
    /// somewhere other than the end.
    BeforeSection(Vec<String>),
    /// The section has no entries yet (or none was named), so the block goes at
    /// the end of the file.
    EndOfFile,
}

impl std::fmt::Display for Anchor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Anchor::AfterSection(path) => {
                write!(f, "after the last mod in section {}", path.join(" - "))
            }
            Anchor::BeforeSection(path) => {
                write!(f, "before the first mod in section {}", path.join(" - "))
            }
            Anchor::EndOfFile => f.write_str("at the end of the file"),
        }
    }
}

/// A planned edit. Holds the full new text so `--dry-run` and the real write
/// run through exactly the same code path.
#[derive(Debug, Clone)]
pub struct Insertion {
    /// 1-based line number the new block starts on.
    pub line: usize,
    pub anchor: Anchor,
    /// The block on its own, for printing.
    pub block: String,
    /// The complete file contents after the splice.
    pub new_text: String,
}

/// Render a mod entry as the TOML text that will be spliced into the file.
///
/// No leading or trailing newline: the caller decides the separators, because
/// they depend on what is already around the insertion point.
pub fn render_block(new_mod: &NewMod) -> String {
    let mut lines = vec!["[[mods]]".to_string(), format!("id = {}", toml_string(&new_mod.id))];

    if !new_mod.section.is_empty() {
        let joined = new_mod
            .section
            .iter()
            .map(|level| toml_string(level))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("section = [{joined}]"));
    }

    if new_mod.path.is_some() || new_mod.file_name.is_some() {
        lines.push(String::new());
        lines.push("[[mods.archives]]".to_string());
        if let Some(path) = &new_mod.path {
            lines.push(format!("path = {}", toml_string(path)));
        }
        if let Some(handler) = &new_mod.download_handler {
            lines.push(format!("download_handler = {}", toml_string(handler)));
        }
        if let Some(file_name) = &new_mod.file_name {
            lines.push(format!("file_name = {}", toml_string(file_name)));
        }
    }

    if new_mod.non_nexus {
        lines.push(
            "# TODO: non-Nexus source -- no `path` yet. Add one (or a local archive path plus \
             download_handler) before installing."
                .to_string(),
        );
    }
    if let Some(version) = &new_mod.version {
        lines.push(format!("# oracle version {version}"));
    }

    lines.join("\n")
}

/// Plan the splice: find the insertion line and build the resulting file text.
///
/// `mods` must come from parsing `text`; the two are cross-checked.
pub fn plan_insertion(text: &str, mods: &[ModEntry], new_mod: &NewMod) -> anyhow::Result<Insertion> {
    plan_insertion_before(text, mods, new_mod, None)
}

/// As `plan_insertion`, but when the new mod's section does not exist yet the
/// block goes immediately *before* `before_section` rather than at end of file.
///
/// Section order in the file is section order in MO2, so a section that
/// belongs in the middle of the guide cannot simply be appended. Part 5 (LOD)
/// is exactly this case: it was skipped, so it must be inserted ahead of
/// Part 6 rather than after it.
pub fn plan_insertion_before(
    text: &str,
    mods: &[ModEntry],
    new_mod: &NewMod,
    before_section: Option<&[String]>,
) -> anyhow::Result<Insertion> {
    let kinds = classify_lines(text);

    let mod_headers: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, kind)| **kind == LineKind::ModHeader)
        .map(|(index, _)| index)
        .collect();

    if mod_headers.len() != mods.len() {
        anyhow::bail!(
            "line scan found {} `[[mods]]` headers but the parsed modlist has {} mods; \
             refusing to edit a file this tool cannot read reliably",
            mod_headers.len(),
            mods.len()
        );
    }

    let target = if new_mod.section.is_empty() {
        None
    } else {
        mods.iter()
            .enumerate()
            .rfind(|(_, entry)| section_eq(&entry.section, &new_mod.section))
            .map(|(index, _)| index)
    };

    // A section that does not exist yet, placed relative to one that does.
    let before = before_section.and_then(|wanted| {
        mods.iter()
            .enumerate()
            .find(|(_, entry)| section_eq(&entry.section, wanted))
            .map(|(index, _)| (index, wanted.to_vec()))
    });

    if let Some(wanted) = before_section
        && target.is_none()
        && before.is_none()
    {
        anyhow::bail!(
            "cannot place the new section before '{}': no mod in the modlist is in that section",
            wanted.join(" - ")
        );
    }

    let (boundary, anchor) = match (target, before) {
        // Inserting before an existing section means landing ahead of that
        // block's own comment run, not between the comments and their block.
        (None, Some((index, wanted))) => (
            back_up_to_content(&kinds, mod_headers[index], true),
            Anchor::BeforeSection(wanted),
        ),
        (target, _) => match target {
        Some(index) => {
            let start = mod_headers[index];
            let boundary = kinds[start + 1..]
                .iter()
                .position(|kind| matches!(kind, LineKind::ModHeader | LineKind::ForeignHeader))
                .map(|offset| start + 1 + offset)
                .unwrap_or(kinds.len());
            (boundary, Anchor::AfterSection(new_mod.section.clone()))
        }
        None => (kinds.len(), Anchor::EndOfFile),
        },
    };

    // Only a boundary that is an actual header can have comments belonging to
    // it; a trailing comment at end of file belongs to the file. A
    // BeforeSection boundary has already been backed up past its comments.
    let insert_at = if matches!(anchor, Anchor::BeforeSection(_)) {
        boundary
    } else {
        back_up_to_content(&kinds, boundary, boundary < kinds.len())
    };

    let block = render_block(new_mod);
    let carriage = if text.contains("\r\n") { "\r" } else { "" };

    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let mut spliced: Vec<String> = vec![carriage.to_string()];
    spliced.extend(block.split('\n').map(|line| format!("{line}{carriage}")));
    lines.splice(insert_at..insert_at, spliced);

    Ok(Insertion {
        // +1 for 1-based, +1 more because the blank separator comes first.
        line: insert_at + 2,
        anchor,
        block,
        new_text: lines.join("\n"),
    })
}

/// Walk back from a block boundary to the last line that belongs to the block.
///
/// A comment run sitting directly against the following header belongs to that
/// header ("# this one needs a custom data folder") and must stay glued to it,
/// while a comment run separated from it by a blank line is a trailing note on
/// the block above -- which is exactly the shape of the `# oracle version` line
/// this module emits, so repeated inserts into one section stay stable.
fn back_up_to_content(kinds: &[LineKind], boundary: usize, strip_header_comments: bool) -> usize {
    let mut index = boundary;
    if strip_header_comments {
        while index > 0 && kinds[index - 1] == LineKind::Comment {
            index -= 1;
        }
    }
    while index > 0 && kinds[index - 1] == LineKind::Blank {
        index -= 1;
    }
    index
}

fn section_eq(left: &[String], right: &[String]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// Quote a value as a TOML basic string. Mod ids routinely contain spaces,
/// brackets, apostrophes and ampersands; none need escaping here, but backslash
/// and double quote do.
fn toml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 || other as u32 == 0x7f => {
                out.push_str(&format!("\\u{:04X}", other as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    /// A top-level `[[mods]]` header: the start of a mod block.
    ModHeader,
    /// Any other top-level header (`[ini]`, `[inputs]`): ends the mod block.
    ForeignHeader,
    Comment,
    Blank,
    /// Key/value lines, sub-table headers (`[[mods.archives]]`), and any line
    /// that is the continuation of a multi-line value.
    Other,
}

fn classify_lines(text: &str) -> Vec<LineKind> {
    let mut scanner = ValueScanner::default();
    let mut out = Vec::new();

    for line in text.split('\n') {
        let kind = if scanner.mid_value() {
            LineKind::Other
        } else {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                LineKind::Blank
            } else if trimmed.starts_with('#') {
                LineKind::Comment
            } else if trimmed.starts_with('[') {
                header_kind(trimmed)
            } else {
                LineKind::Other
            }
        };
        scanner.scan(line);
        out.push(kind);
    }

    out
}

fn header_kind(trimmed: &str) -> LineKind {
    let head = strip_trailing_comment(trimmed).trim_end();

    let inner = match head.strip_prefix("[[") {
        Some(rest) => rest.strip_suffix("]]"),
        None => head.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')),
    };
    let Some(inner) = inner else {
        return LineKind::Other;
    };

    let inner = inner.trim();
    if inner == "mods" {
        LineKind::ModHeader
    } else if inner.starts_with("mods.") {
        // `[[mods.archives]]`, `[mods.merge]`, ... all belong to the block above.
        LineKind::Other
    } else {
        LineKind::ForeignHeader
    }
}

fn strip_trailing_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut quote: Option<u8> = None;

    while index < bytes.len() {
        let byte = bytes[index];
        match quote {
            Some(delimiter) => {
                if delimiter == b'"' && byte == b'\\' {
                    index += 2;
                    continue;
                }
                if byte == delimiter {
                    quote = None;
                }
            }
            None => {
                if byte == b'#' {
                    return &line[..index];
                }
                if byte == b'"' || byte == b'\'' {
                    quote = Some(byte);
                }
            }
        }
        index += 1;
    }

    line
}

/// Tracks whether the scanner is part-way through a multi-line TOML value, so
/// that a line inside a multi-line array or string is never mistaken for a
/// table header.
#[derive(Debug, Default)]
struct ValueScanner {
    /// Delimiter of an open multi-line string (`"` or `'`).
    open_string: Option<char>,
    /// Unclosed `[` carried over from a multi-line array value.
    depth: i32,
}

impl ValueScanner {
    fn mid_value(&self) -> bool {
        self.open_string.is_some() || self.depth > 0
    }

    fn scan(&mut self, line: &str) {
        let chars: Vec<char> = line.chars().collect();
        let mut index = 0;

        if let Some(delimiter) = self.open_string {
            match find_triple(&chars, 0, delimiter) {
                Some(end) => {
                    self.open_string = None;
                    index = end + 3;
                }
                None => return,
            }
        }

        while index < chars.len() {
            match chars[index] {
                '#' => return,
                '[' => {
                    self.depth += 1;
                    index += 1;
                }
                ']' => {
                    self.depth = (self.depth - 1).max(0);
                    index += 1;
                }
                delimiter @ ('"' | '\'') => {
                    let is_triple = chars.len() > index + 2
                        && chars[index + 1] == delimiter
                        && chars[index + 2] == delimiter;
                    if is_triple {
                        match find_triple(&chars, index + 3, delimiter) {
                            Some(end) => index = end + 3,
                            None => {
                                self.open_string = Some(delimiter);
                                return;
                            }
                        }
                    } else {
                        let mut cursor = index + 1;
                        let mut closed = false;
                        while cursor < chars.len() {
                            if delimiter == '"' && chars[cursor] == '\\' {
                                cursor += 2;
                                continue;
                            }
                            if chars[cursor] == delimiter {
                                closed = true;
                                cursor += 1;
                                break;
                            }
                            cursor += 1;
                        }
                        index = cursor;
                        if !closed {
                            // Unterminated single-line string: malformed TOML.
                            // The parse step will report it; stop reading here.
                            return;
                        }
                    }
                }
                _ => index += 1,
            }
        }
    }
}

fn find_triple(chars: &[char], from: usize, delimiter: char) -> Option<usize> {
    (from..chars.len().saturating_sub(2))
        .find(|&i| chars[i] == delimiter && chars[i + 1] == delimiter && chars[i + 2] == delimiter)
}

/// The fields `add` cares about in a Mod Organizer 2 `meta.ini`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OracleMeta {
    /// `[General] modid`. Zero means the mod was not installed from Nexus.
    pub mod_id: Option<u64>,
    /// `[installedFiles] 1\fileid`.
    pub file_id: Option<u64>,
    /// `[General] installationFile`, the archive's own filename.
    pub installation_file: Option<String>,
    /// `[General] version`.
    pub version: Option<String>,
}

/// Parse the handful of keys `add` needs out of a `meta.ini`.
///
/// Not a general INI parser: MO2 writes values containing `=`, quotes, escaped
/// Qt variants and whole BBCode mod descriptions, so this reads only the four
/// keys it understands and ignores the rest rather than trying to model them.
pub fn parse_meta_ini(text: &str) -> OracleMeta {
    let mut meta = OracleMeta::default();
    let mut section = String::new();

    for raw in text.lines() {
        let line = raw.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[')
            && let Some(name) = rest.strip_suffix(']')
        {
            section = name.trim().to_ascii_lowercase();
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = unquote(value.trim());

        match (section.as_str(), key) {
            ("general", "modid") => meta.mod_id = value.parse().ok(),
            ("general", "version") if !value.is_empty() => meta.version = Some(value.to_string()),
            ("general", "installationFile") if !value.is_empty() => {
                meta.installation_file = Some(value.to_string());
            }
            // MO2 numbers installed files from 1; entry 1 is the archive the mod
            // was actually installed from.
            ("installedfiles", "1\\fileid") => meta.file_id = value.parse().ok(),
            _ => {}
        }
    }

    meta
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

/// Plugin files shipped by an Oracle mod folder, as folder-relative paths.
///
/// `.mohidden` files are skipped: MO2 renames a plugin that way precisely to
/// take it out of the load order, so reporting one would be telling the author
/// to add something that is deliberately disabled.
pub fn find_plugins(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    collect_plugins(dir, dir, &mut found);
    found.sort_by_key(|name| name.to_ascii_lowercase());
    found
}

fn collect_plugins(current: &Path, root: &Path, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_plugins(&path, root, found);
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".esp") || name.ends_with(".esm") {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            found.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, section: &[&str]) -> ModEntry {
        let toml = format!(
            "id = {}\nsection = [{}]\n",
            toml_string(id),
            section
                .iter()
                .map(|level| toml_string(level))
                .collect::<Vec<_>>()
                .join(", ")
        );
        toml::from_str(&toml).expect("fixture mod entry should parse")
    }

    #[test]
    fn classifies_headers_and_ignores_multiline_values() {
        let text = "name = \"x\"\nplugins = [\n\t\"a.esp\",\n]\n\n[[mods]]\nid = \"a\"\n\n[[mods.archives]]\npath = \"p\"\n\n[ini]\nkey = 1\n";
        let kinds = classify_lines(text);

        assert_eq!(kinds[1], LineKind::Other, "plugins = [ opens an array");
        assert_eq!(kinds[2], LineKind::Other, "array element, not a header");
        assert_eq!(kinds[5], LineKind::ModHeader);
        assert_eq!(kinds[8], LineKind::Other, "[[mods.archives]] is a sub-table");
        assert_eq!(kinds[11], LineKind::ForeignHeader, "[ini] ends the block");
    }

    #[test]
    fn a_bracketed_line_inside_a_multiline_string_is_not_a_header() {
        let text = "name = \"\"\"\n[[mods]]\n\"\"\"\n\n[[mods]]\nid = \"real\"\n";
        let kinds = classify_lines(text);

        assert_eq!(kinds[1], LineKind::Other, "inside a multi-line string");
        assert_eq!(kinds[4], LineKind::ModHeader);
        assert_eq!(
            kinds.iter().filter(|k| **k == LineKind::ModHeader).count(),
            1
        );
    }

    #[test]
    fn comment_glued_to_the_next_header_stays_with_it() {
        let text = "name = \"t\"\n\n[[mods]]\nid = \"a\"\nsection = [\"S\"]\n# oracle version 1.0\n\n# why b is odd\n[[mods]]\nid = \"b\"\nsection = [\"T\"]\n";
        let mods = [entry("a", &["S"]), entry("b", &["T"])];
        let new_mod = NewMod {
            id: "c".to_string(),
            section: vec!["S".to_string()],
            ..NewMod::default()
        };

        let plan = plan_insertion(text, &mods, &new_mod).expect("insertion should be planned");

        assert!(plan.new_text.contains("# oracle version 1.0\n\n[[mods]]\nid = \"c\""));
        assert!(plan.new_text.contains("# why b is odd\n[[mods]]\nid = \"b\""));
    }

    #[test]
    fn meta_ini_reads_modid_fileid_installation_file_and_version() {
        let text = "[General]\ngameName=Oblivion\nmodid=43752\nversion=11.1.0.0\ncategory=\"7,\"\ninstallationFile=Blockhead-43752-11-1-1640043918.7z\n\n[installedFiles]\nsize=1\n1\\modid=43752\n1\\fileid=1000029844\n";
        let meta = parse_meta_ini(text);

        assert_eq!(meta.mod_id, Some(43752));
        assert_eq!(meta.file_id, Some(1000029844));
        assert_eq!(meta.version.as_deref(), Some("11.1.0.0"));
        assert_eq!(
            meta.installation_file.as_deref(),
            Some("Blockhead-43752-11-1-1640043918.7z")
        );
    }

    #[test]
    fn meta_ini_survives_values_containing_equals_and_brackets() {
        // A real MO2 line; a naive parser reads `\0\0\0\x43...` as a section.
        let text = "[General]\nmodid=0\ncolor=@Variant(\\0\\0\\0\\x43\\0\\xff)\nnexusDescription=\"a=b [url=http://x]y[/url]\"\ninstallationFile=DarNified UI 132 FOMOD - Merged.7z\nversion=1.3.2.0\n\n[installedFiles]\n1\\fileid=0\n";
        let meta = parse_meta_ini(text);

        assert_eq!(meta.mod_id, Some(0));
        assert_eq!(meta.file_id, Some(0));
        assert_eq!(
            meta.installation_file.as_deref(),
            Some("DarNified UI 132 FOMOD - Merged.7z")
        );
    }

    #[test]
    fn renders_ids_containing_quotes_safely() {
        let new_mod = NewMod {
            id: "Say \"what\" & co".to_string(),
            ..NewMod::default()
        };
        let block = render_block(&new_mod);

        assert!(block.contains(r#"id = "Say \"what\" & co""#));
        let parsed: toml::Table = toml::from_str(&block).expect("block should be valid TOML");
        assert!(parsed.contains_key("mods"));
    }

    #[test]
    fn refuses_when_the_scan_and_the_parse_disagree() {
        // Two parsed mods, but the scanner is told about only one.
        let text = "name = \"t\"\n\n[[mods]]\nid = \"a\"\n";
        let mods = [entry("a", &[]), entry("b", &[])];
        let new_mod = NewMod {
            id: "c".to_string(),
            ..NewMod::default()
        };

        let err = plan_insertion(text, &mods, &new_mod).expect_err("should refuse");
        assert!(err.to_string().contains("refusing to edit"));
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let text = "name = \"t\"\r\n\r\n[[mods]]\r\nid = \"a\"\r\nsection = [\"S\"]\r\n";
        let mods = [entry("a", &["S"])];
        let new_mod = NewMod {
            id: "b".to_string(),
            section: vec!["S".to_string()],
            ..NewMod::default()
        };

        let plan = plan_insertion(text, &mods, &new_mod).expect("insertion should be planned");

        assert!(!plan.new_text.contains("\n\n"), "no bare LF should be introduced");
        assert!(plan.new_text.contains("[[mods]]\r\nid = \"b\"\r\n"));
    }
    #[test]
    fn a_new_section_can_start_before_an_existing_one() {
        // Part 5 was skipped, so it has to land ahead of Part 6 rather than at
        // the end -- section order in the file is section order in MO2.
        let text = "name = \"x\"\nplugins = []\n\n[[mods]]\nid = \"four\"\nsection = [\"4 - TWEAKS\"]\n\n[[mods]]\nid = \"six\"\nsection = [\"6 - UI\"]\n";
        let mods = vec![entry("four", &["4 - TWEAKS"]), entry("six", &["6 - UI"])];
        let new_mod = NewMod {
            id: "lod".to_string(),
            section: vec!["5 - LOD".to_string()],
            path: Some("nexus:oblivion/1/2".to_string()),
            download_handler: Some("nexus".to_string()),
            file_name: None,
            version: None,
            non_nexus: false,
        };

        let plan = plan_insertion_before(text, &mods, &new_mod, Some(&["6 - UI".to_string()]))
            .expect("should plan");
        assert_eq!(plan.anchor, Anchor::BeforeSection(vec!["6 - UI".to_string()]));

        let five = plan.new_text.find("5 - LOD").expect("new section present");
        let six = plan.new_text.find("6 - UI").expect("existing section present");
        assert!(five < six, "the new section must precede the one it was placed before");
        assert!(plan.new_text.contains("id = \"four\""), "existing mods survive");
    }

    #[test]
    fn placing_before_a_section_that_does_not_exist_is_an_error() {
        let text = "name = \"x\"\nplugins = []\n\n[[mods]]\nid = \"four\"\nsection = [\"4 - TWEAKS\"]\n";
        let mods = vec![entry("four", &["4 - TWEAKS"])];
        let new_mod = NewMod {
            id: "lod".to_string(),
            section: vec!["5 - LOD".to_string()],
            path: None,
            download_handler: None,
            file_name: None,
            version: None,
            non_nexus: false,
        };

        let err = plan_insertion_before(text, &mods, &new_mod, Some(&["9 - NOPE".to_string()]))
            .expect_err("should refuse rather than silently append");
        assert!(err.to_string().contains("no mod in the modlist is in that section"), "{err}");
    }

    #[test]
    fn before_section_is_ignored_once_the_section_exists() {
        // The second mod of a new section must join the first, not keep
        // jumping ahead of the anchor.
        let text = "name = \"x\"\nplugins = []\n\n[[mods]]\nid = \"lod\"\nsection = [\"5 - LOD\"]\n\n[[mods]]\nid = \"six\"\nsection = [\"6 - UI\"]\n";
        let mods = vec![entry("lod", &["5 - LOD"]), entry("six", &["6 - UI"])];
        let new_mod = NewMod {
            id: "lod2".to_string(),
            section: vec!["5 - LOD".to_string()],
            path: None,
            download_handler: None,
            file_name: None,
            version: None,
            non_nexus: false,
        };

        let plan = plan_insertion_before(text, &mods, &new_mod, Some(&["6 - UI".to_string()]))
            .expect("should plan");
        assert_eq!(plan.anchor, Anchor::AfterSection(vec!["5 - LOD".to_string()]));
        let lod2 = plan.new_text.find("lod2").expect("added");
        let six = plan.new_text.find("6 - UI").expect("existing");
        assert!(lod2 < six);
    }

}
