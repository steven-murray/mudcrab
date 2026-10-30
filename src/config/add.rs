//! Append a `[[mods]]` block to a source modlist without rewriting the file.
//!
//! `SourceModlist` is deliberately `Deserialize`-only, and the modlists this
//! tool exists to maintain carry their reasoning in comments: source URLs, why a
//! layout is overridden, what still needs checking. Parsing to a `toml::Table`
//! and re-emitting would drop every one of those, so `add` works the same way
//! the MOFAM case study's `migrate-schema.py` does -- it parses only enough to find
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
    /// The mod's `if`, when it is optional.
    pub condition: Option<String>,
    /// Plugins the mod ships. Each also has to be in the top-level `plugins`
    /// load order (or consumed by a merge), which `validate` insists on; `add`
    /// puts it there only where told to (`place_plugins`).
    pub plugins: Vec<String>,
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
    /// A Nexus mod id known without a file id, so the descriptor cannot be
    /// built yet. Recorded in the TODO so the lookup has somewhere to start.
    pub known_mod_id: Option<u64>,
    /// A complete `[[mods.archives]]` block, written as is in place of one
    /// built from `path` and `file_name`: what `inspect` makes of an archive.
    pub archive_block: Option<String>,
    /// A shim whose definition the entry takes (`use`), in place of archives.
    /// Its plugins are the shim's, so the entry does not repeat them.
    pub use_source: Option<String>,
}

/// Where a rendered block will be spliced in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anchor {
    /// Immediately before a named mod's block, wherever in the file it sits.
    BeforeMod(String),
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
            Anchor::BeforeMod(id) => write!(f, "immediately before '{id}'"),
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

/// Where the caller wants the block, when the default -- end of its section --
/// is not right.
///
/// Order within a section is MO2 install order, which decides who wins a file
/// conflict, so "append to the section" is only correct for a mod the guide
/// lists last. Anything else needs one of these.
#[derive(Debug, Clone, Default)]
pub struct Placement {
    /// Insert directly before this mod id. Takes precedence over everything
    /// else, and is how a row lands at the *start* of a section: name the row
    /// that currently comes first.
    pub before_mod: Option<String>,
    /// Only consulted when the new mod's section has no entries yet: start the
    /// section immediately before this existing one instead of at end of file.
    pub before_section: Option<Vec<String>>,
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

/// What [`plan_add`] would do.
#[derive(Debug, Clone)]
pub struct Added {
    /// Where the block goes, and the whole new text.
    pub insertion: Insertion,
    /// The mod's plugins that the load order did not list yet, now placed.
    pub placed: Vec<String>,
}

/// Add `new_mod` to `text` (whose parsed list is `source`): its block where
/// `placement` says, and any of its plugins the load order does not list yet
/// at `position`. A plugin in the wrong place fails silently in game, so a
/// new plugin with no `position` is refused rather than guessed at.
pub fn plan_add(
    text: &str,
    source: &crate::config::schema::SourceModlist,
    new_mod: &NewMod,
    placement: &Placement,
    position: Option<&LoadPosition>,
) -> anyhow::Result<Added> {
    if let Some(existing) = source.mods.iter().find(|entry| entry.id == new_mod.id) {
        anyhow::bail!(
            "the list already contains a mod with id '{}' (section {}); \
             pass --id to add this one under a different name",
            existing.id,
            if existing.section.is_empty() {
                "<none>".to_string()
            } else {
                existing.section.join(" - ")
            }
        );
    }
    let mut insertion = plan_insertion_at(text, &source.mods, new_mod, placement)?;

    let unplaced: Vec<String> = new_mod
        .plugins
        .iter()
        .filter(|plugin| {
            !source
                .plugins
                .iter()
                .any(|listed| listed.eq_ignore_ascii_case(plugin))
        })
        .cloned()
        .collect();
    match (position, unplaced.is_empty()) {
        (Some(position), false) => {
            let placed = place_plugins(&insertion.new_text, &unplaced, position)?;
            // The load order sits above every mod, so the block moves down by
            // however many lines went into it.
            insertion.line += placed.lines().count() - insertion.new_text.lines().count();
            insertion.new_text = placed;
        }
        (None, false) => anyhow::bail!(
            "{} {} not in the load order yet; say where with --load-after <plugin> or \
             --load-last (or add {} to `plugins` yourself first)",
            unplaced.join(", "),
            if unplaced.len() == 1 { "is" } else { "are" },
            if unplaced.len() == 1 { "it" } else { "them" },
        ),
        (_, true) => {}
    }
    Ok(Added {
        insertion,
        placed: unplaced,
    })
}

/// Render a mod entry as the TOML text that will be spliced into the file.
///
/// No leading or trailing newline: the caller decides the separators, because
/// they depend on what is already around the insertion point.
pub fn render_block(new_mod: &NewMod) -> String {
    let mut lines = vec![
        "[[mods]]".to_string(),
        format!("id = {}", toml_string(&new_mod.id)),
    ];

    if !new_mod.section.is_empty() {
        let joined = new_mod
            .section
            .iter()
            .map(|level| toml_string(level))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("section = [{joined}]"));
    }
    if let Some(condition) = &new_mod.condition {
        lines.push(format!("if = {}", toml_string(condition)));
    }
    if let Some(source) = &new_mod.use_source {
        lines.push(format!("use = {}", toml_string(source)));
    } else if !new_mod.plugins.is_empty() {
        let joined = new_mod
            .plugins
            .iter()
            .map(|p| toml_string(p))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("plugins = [{joined}]"));
    }

    if let Some(block) = &new_mod.archive_block {
        lines.push(String::new());
        lines.push(block.trim_end().to_string());
    } else if new_mod.path.is_some() || new_mod.file_name.is_some() {
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
        // `manual:` is the host-agnostic answer and the one to reach for first:
        // it keeps the modlist portable, resolves from --archive-search-path,
        // and fails with a message naming the file when the archive is absent.
        // An absolute local path works too but bakes this machine into the list.
        let hint = new_mod
            .file_name
            .as_deref()
            .map(|name| format!("path = \"manual:{name}\""))
            .unwrap_or_else(|| "path = \"manual:<archive filename>\"".to_string());
        match new_mod.known_mod_id {
            // Half-recorded: the mod id is real, only the file id is missing.
            // Saying so turns the TODO into a lookup rather than a search.
            Some(mod_id) => lines.push(format!(
                "# TODO: Nexus mod {mod_id}, file id not recorded by MO2, so the descriptor\n\
                 # cannot be built. Resolve it (`mudcrab identify`, or the mod's files tab)\n\
                 # and replace with path = \"nexus:oblivion/{mod_id}/<fileid>\". Until then:\n\
                 # {hint}"
            )),
            None => lines.push(format!(
                "# TODO: non-Nexus source -- no `path` yet. For a hand-downloaded archive use\n\
                 # {hint}\n\
                 # and record where to get it in a comment. A local path plus download_handler \
                 also works."
            )),
        }
    }
    if let Some(version) = &new_mod.version {
        lines.push(format!("# version {version}, as installed in MO2"));
    }

    lines.join("\n")
}

/// Plan the splice: find the insertion line and build the resulting file text.
///
/// `mods` must come from parsing `text`; the two are cross-checked.
pub fn plan_insertion(
    text: &str,
    mods: &[ModEntry],
    new_mod: &NewMod,
) -> anyhow::Result<Insertion> {
    plan_insertion_at(text, mods, new_mod, &Placement::default())
}

/// As `plan_insertion`, but honouring an explicit `Placement`.
///
/// Section order in the file is section order in MO2, and order within a
/// section is priority order, so neither "append to the file" nor "append to
/// the section" is universally right. Part 5 (LOD) needed both escapes: the
/// section itself had to land ahead of Part 6, and its first row had to land
/// ahead of the nine already written.
pub fn plan_insertion_at(
    text: &str,
    mods: &[ModEntry],
    new_mod: &NewMod,
    placement: &Placement,
) -> anyhow::Result<Insertion> {
    splice_block(
        text,
        mods,
        &new_mod.section,
        &render_block(new_mod),
        placement,
    )
}

/// Splice `block` (a whole `[[mods]]` entry, with any comments of its own)
/// into `text` where a mod in `section` belongs, honouring `placement`.
fn splice_block(
    text: &str,
    mods: &[ModEntry],
    section: &[String],
    block: &str,
    placement: &Placement,
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

    // Named neighbour first: it says exactly where to go, so nothing else is
    // consulted. Unknown ids are refused rather than quietly falling back to
    // the section default, which would put the block in the wrong place.
    let pinned = match &placement.before_mod {
        Some(wanted) => {
            let index = mods
                .iter()
                .position(|entry| entry.id.eq_ignore_ascii_case(wanted))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "cannot place the new mod before '{wanted}': the modlist has no mod \
                         with that id"
                    )
                })?;
            Some((index, wanted.clone()))
        }
        None => None,
    };

    let target = if section.is_empty() {
        None
    } else {
        mods.iter()
            .enumerate()
            .rfind(|(_, entry)| section_eq(&entry.section, section))
            .map(|(index, _)| index)
    };

    // A section that does not exist yet, placed relative to one that does.
    let before = placement.before_section.as_ref().and_then(|wanted| {
        mods.iter()
            .enumerate()
            .find(|(_, entry)| section_eq(&entry.section, wanted))
            .map(|(index, _)| (index, wanted.clone()))
    });

    if let Some(wanted) = &placement.before_section
        && pinned.is_none()
        && target.is_none()
        && before.is_none()
    {
        anyhow::bail!(
            "cannot place the new section before '{}': no mod in the modlist is in that section",
            wanted.join(" - ")
        );
    }

    let (boundary, anchor) = match (pinned, target, before) {
        // Landing ahead of a block means landing ahead of its own comment run
        // too, not between the comments and the block they describe.
        (Some((index, id)), _, _) => (
            back_up_to_content(&kinds, mod_headers[index], true),
            Anchor::BeforeMod(id),
        ),
        (None, None, Some((index, wanted))) => (
            back_up_to_content(&kinds, mod_headers[index], true),
            Anchor::BeforeSection(wanted),
        ),
        (None, Some(index), _) => {
            let start = mod_headers[index];
            let boundary = kinds[start + 1..]
                .iter()
                .position(|kind| matches!(kind, LineKind::ModHeader | LineKind::ForeignHeader))
                .map(|offset| start + 1 + offset)
                .unwrap_or(kinds.len());
            (boundary, Anchor::AfterSection(section.to_vec()))
        }
        (None, None, None) => (kinds.len(), Anchor::EndOfFile),
    };

    // Only a boundary that is an actual header can have comments belonging to
    // it; a trailing comment at end of file belongs to the file. The
    // insert-before boundaries have already been backed up past their comments.
    let insert_at = if matches!(anchor, Anchor::BeforeSection(_) | Anchor::BeforeMod(_)) {
        boundary
    } else {
        back_up_to_content(&kinds, boundary, boundary < kinds.len())
    };

    let block = block.trim_end_matches(['\n', '\r']).to_string();
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

/// Move an existing mod's block -- its own comments, its archives and actions,
/// any trailing note -- to `section`, placed as `add` would place a new one.
/// The block's `section =` line is rewritten; nothing else in it changes.
pub fn plan_move(
    text: &str,
    mods: &[ModEntry],
    id: &str,
    section: &[String],
    placement: &Placement,
) -> anyhow::Result<Insertion> {
    let kinds = classify_lines(text);
    let headers: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, kind)| **kind == LineKind::ModHeader)
        .map(|(index, _)| index)
        .collect();
    anyhow::ensure!(
        headers.len() == mods.len(),
        "line scan found {} `[[mods]]` headers but the parsed modlist has {} mods; \
         refusing to edit a file this tool cannot read reliably",
        headers.len(),
        mods.len()
    );
    let index = mods
        .iter()
        .position(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the modlist has no mod with id '{id}'"))?;
    if placement
        .before_mod
        .as_deref()
        .is_some_and(|m| m.eq_ignore_ascii_case(id))
    {
        anyhow::bail!("cannot move '{id}' before itself");
    }

    let (start, end) = block_span(&kinds, headers[index]);

    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let mut block: Vec<String> = lines[start..end].to_vec();
    let new_section = format!(
        "section = [{}]",
        section
            .iter()
            .map(|level| toml_string(level))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let carriage = if text.contains("\r\n") { "\r" } else { "" };
    match block
        .iter()
        .position(|line| line.trim_start().starts_with("section ="))
    {
        Some(at) => block[at] = format!("{new_section}{carriage}"),
        None => {
            let header = block
                .iter()
                .position(|line| line.trim() == "[[mods]]")
                .expect("block has its header");
            block.insert(header + 2, format!("{new_section}{carriage}"));
        }
    }
    lines.drain(cut_span(&kinds, start)..end);
    let without = lines.join("\n");

    let remaining: Vec<ModEntry> = mods
        .iter()
        .filter(|entry| entry.id != id)
        .cloned()
        .collect();
    let block = block
        .iter()
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>()
        .join("\n");
    splice_block(&without, &remaining, section, &block, placement)
}

/// The lines of the mod block whose header is line `header`: from its own
/// comment run to wherever the next block's comment run starts.
fn block_span(kinds: &[LineKind], header: usize) -> (usize, usize) {
    let mut start = header;
    while start > 0 && kinds[start - 1] == LineKind::Comment {
        start -= 1;
    }
    let next = kinds[header + 1..]
        .iter()
        .position(|kind| matches!(kind, LineKind::ModHeader | LineKind::ForeignHeader))
        .map(|offset| header + 1 + offset);
    let end = match next {
        Some(next) => back_up_to_content(kinds, next, true),
        None => back_up_to_content(kinds, kinds.len(), false),
    };
    (start, end)
}

/// Where to cut a block starting at `start` out: with the blank line that
/// separated it from the one before, so the gap does not double up.
fn cut_span(kinds: &[LineKind], start: usize) -> usize {
    if start > 0 && kinds[start - 1] == LineKind::Blank {
        start - 1
    } else {
        start
    }
}

/// `text` without mod `id`'s block -- its own comments, archives, actions and
/// any trailing note -- as `move` takes it out. `mods` must come from parsing
/// `text`.
pub fn plan_removal(text: &str, mods: &[ModEntry], id: &str) -> anyhow::Result<String> {
    let kinds = classify_lines(text);
    let headers: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, kind)| **kind == LineKind::ModHeader)
        .map(|(index, _)| index)
        .collect();
    anyhow::ensure!(
        headers.len() == mods.len(),
        "line scan found {} `[[mods]]` headers but the parsed modlist has {} mods; \
         refusing to edit a file this tool cannot read reliably",
        headers.len(),
        mods.len()
    );
    let index = mods
        .iter()
        .position(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the modlist has no mod with id '{id}'"))?;
    let (start, end) = block_span(&kinds, headers[index]);
    let mut lines: Vec<&str> = text.split('\n').collect();
    lines.drain(cut_span(&kinds, start)..end);
    Ok(lines.join("\n"))
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
        None => head
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']')),
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
    /// `[General] nexusLastModified`, an ISO-8601 instant: when Nexus last
    /// changed the file MO2 downloaded. Present for 704 of the Oracle's 745
    /// mods, and the only date available for an archive whose filename carries
    /// no Nexus timestamp.
    pub nexus_last_modified: Option<String>,
}

/// Parse the handful of keys `add` needs out of a `meta.ini`.
///
/// Not a general INI parser: MO2 writes values containing `=`, quotes, escaped
/// Qt variants and whole BBCode mod descriptions, so this reads only the four
/// keys it understands and ignores the rest rather than trying to model them.
/// Where `add` puts a new mod's plugins in the top-level `plugins` load order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadPosition {
    /// Straight after this plugin, which must already be in the list.
    After(String),
    /// At the end.
    Last,
}

/// Insert `plugins` into the top-level `plugins` array, in the order given, at
/// `position`; a plugin already listed is left where it is. Line-based like
/// the rest of `add`: nothing outside the lines it inserts changes, except a
/// comma added to the entry it follows. With no `plugins` array yet, one is
/// started before the file's first table.
/// The last plugin, in load order, of the nearest of the first `at` mods
/// that has any in it: where a mod's plugins go by default when it is put
/// after them.
pub fn last_plugin_before(
    source: &crate::config::schema::SourceModlist,
    at: usize,
) -> Option<String> {
    let order: Vec<String> = source
        .plugins
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    source.mods[..at.min(source.mods.len())]
        .iter()
        .rev()
        .find_map(|entry| {
            entry
                .plugins
                .iter()
                .filter_map(|p| order.iter().position(|o| o == &p.to_ascii_lowercase()))
                .max()
                .map(|index| source.plugins[index].clone())
        })
}

pub fn place_plugins(
    text: &str,
    plugins: &[String],
    position: &LoadPosition,
) -> anyhow::Result<String> {
    let kinds = classify_lines(text);
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let first_table = kinds
        .iter()
        .position(|kind| matches!(kind, LineKind::ModHeader | LineKind::ForeignHeader))
        .unwrap_or(lines.len());
    let indent = if lines.iter().any(|line| line.starts_with('\t')) {
        "\t"
    } else {
        "    "
    };
    let quoted = |line: &str| -> Option<String> {
        let line = strip_trailing_comment(line).trim();
        let rest = line.strip_prefix('"')?;
        Some(rest[..rest.find('"')?].to_string())
    };

    let start = (0..first_table).find(|&i| {
        matches!(kinds[i], LineKind::Other)
            && lines[i]
                .trim_start()
                .strip_prefix("plugins")
                .is_some_and(|rest| rest.trim_start().starts_with('='))
    });
    let Some(start) = start else {
        if let LoadPosition::After(after) = position {
            anyhow::bail!("--load-after {after}: the list has no `plugins` load order yet");
        }
        let mut block = vec!["plugins = [".to_string()];
        block.extend(
            plugins
                .iter()
                .map(|p| format!("{indent}{},", toml_string(p))),
        );
        block.push("]".to_string());
        block.push(String::new());
        lines.splice(first_table..first_table, block);
        return Ok(lines.join("\n"));
    };
    if strip_trailing_comment(&lines[start])
        .trim_end()
        .ends_with(']')
    {
        anyhow::bail!(
            "the top-level `plugins` array is written on one line; put one plugin per line \
             so entries can be added without rewriting it"
        );
    }
    let end = (start + 1..lines.len())
        .find(|&i| lines[i].trim_start().starts_with(']'))
        .ok_or_else(|| anyhow::anyhow!("the top-level `plugins` array is not closed"))?;
    let entries: Vec<(usize, String)> = (start + 1..end)
        .filter_map(|i| quoted(&lines[i]).map(|name| (i, name)))
        .collect();
    let listed = |name: &str| entries.iter().any(|(_, e)| e.eq_ignore_ascii_case(name));
    let new: Vec<&String> = plugins.iter().filter(|p| !listed(p)).collect();
    if new.is_empty() {
        return Ok(text.to_string());
    }

    // The line to insert after, and the indentation entries use.
    let anchor = match position {
        LoadPosition::After(after) => entries
            .iter()
            .find(|(_, e)| e.eq_ignore_ascii_case(after))
            .map(|(i, _)| *i)
            .ok_or_else(|| {
                anyhow::anyhow!("--load-after {after}: not in the `plugins` load order")
            })?,
        LoadPosition::Last => entries.last().map_or(start, |(i, _)| *i),
    };
    let entry_indent = entries
        .first()
        .map(|(i, _)| {
            let line = &lines[*i];
            line[..line.len() - line.trim_start().len()].to_string()
        })
        .unwrap_or_else(|| indent.to_string());
    // The entry followed needs its comma, if it was the last and had none.
    if anchor != start {
        let line = &lines[anchor];
        let body = strip_trailing_comment(line).trim_end();
        if !body.ends_with(',') {
            let at = body.len();
            lines[anchor] = format!("{},{}", &line[..at], &line[at..]);
        }
    }
    // A CRLF file's lines keep their `\r`; new ones get it too.
    let carriage = if lines[anchor].ends_with('\r') {
        "\r"
    } else {
        ""
    };
    let block: Vec<String> = new
        .iter()
        .map(|p| format!("{entry_indent}{},{carriage}", toml_string(p)))
        .collect();
    lines.splice(anchor + 1..anchor + 1, block);
    Ok(lines.join("\n"))
}

/// Add `lines` to one archive table of one mod: after its `file_name` line,
/// or its `path` line when it has none. Line-based like the rest of `add`, so
/// nothing else in the file changes. `mods` must come from parsing `text`.
pub fn plan_archive_lines(
    text: &str,
    mods: &[ModEntry],
    mod_index: usize,
    archive_index: usize,
    lines: &[String],
) -> anyhow::Result<String> {
    plan_table_lines(
        text,
        mods,
        mod_index,
        "[[mods.archives]]",
        archive_index,
        lines,
    )
}

/// As [`plan_archive_lines`], for the `nth` table of a mod headed `header`:
/// `[[mods.archives]]`, or `[[mods.archives.build]]` for a build layer
/// (counted across all of the mod's archives).
pub fn plan_table_lines(
    text: &str,
    mods: &[ModEntry],
    mod_index: usize,
    header: &str,
    archive_index: usize,
    lines: &[String],
) -> anyhow::Result<String> {
    let kinds = classify_lines(text);
    let headers: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, kind)| **kind == LineKind::ModHeader)
        .map(|(index, _)| index)
        .collect();
    anyhow::ensure!(
        headers.len() == mods.len(),
        "line scan found {} `[[mods]]` headers but the parsed modlist has {} mods; \
         refusing to edit a file this tool cannot read reliably",
        headers.len(),
        mods.len()
    );
    let mut all: Vec<String> = text.split('\n').map(str::to_string).collect();
    let start = headers[mod_index];
    let end = (start + 1..kinds.len())
        .find(|&i| matches!(kinds[i], LineKind::ModHeader | LineKind::ForeignHeader))
        .unwrap_or(kinds.len());
    let tables: Vec<usize> = (start..end)
        .filter(|&i| {
            kinds[i] == LineKind::Other && strip_trailing_comment(&all[i]).trim() == header
        })
        .collect();
    let table = *tables.get(archive_index).ok_or_else(|| {
        anyhow::anyhow!(
            "mod '{}' has no archive {} in the file",
            mods[mod_index].id,
            archive_index + 1
        )
    })?;
    let table_end = tables.get(archive_index + 1).copied().unwrap_or(end).min(
        (table + 1..end)
            .find(|&i| {
                strip_trailing_comment(&all[i])
                    .trim_start()
                    .starts_with('[')
            })
            .unwrap_or(end),
    );
    let key_line = |key: &str| {
        (table + 1..table_end).find(|&i| {
            kinds[i] == LineKind::Other
                && all[i]
                    .trim_start()
                    .strip_prefix(key)
                    .is_some_and(|rest| rest.trim_start().starts_with('='))
        })
    };
    let after = key_line("file_name")
        .or_else(|| key_line("path"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "mod '{}' archive {} has neither a file_name nor a path line",
                mods[mod_index].id,
                archive_index + 1
            )
        })?;
    let carriage = if all[after].ends_with('\r') { "\r" } else { "" };
    let new: Vec<String> = lines.iter().map(|l| format!("{l}{carriage}")).collect();
    all.splice(after + 1..after + 1, new);
    Ok(all.join("\n"))
}

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
            ("general", "nexusLastModified") if !value.is_empty() => {
                meta.nexus_last_modified = Some(value.to_string());
            }
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
    #[test]
    fn renders_an_optional_mod_with_its_plugins() {
        let block = super::render_block(&super::NewMod {
            id: "Swearing Rats".to_string(),
            section: vec!["31 - AUDIO".to_string()],
            condition: Some("swearing_rats".to_string()),
            plugins: vec!["Swearing Rats.esp".to_string()],
            path: Some("nexus:oblivion/52661/1000033714".to_string()),
            ..Default::default()
        });
        assert!(
            block.starts_with(
                "[[mods]]\nid = \"Swearing Rats\"\nsection = [\"31 - AUDIO\"]\n\
                 if = \"swearing_rats\"\nplugins = [\"Swearing Rats.esp\"]\n\n[[mods.archives]]"
            ),
            "{block}"
        );
    }

    use super::*;

    fn before_section(name: &str) -> Placement {
        Placement {
            before_section: Some(vec![name.to_string()]),
            ..Placement::default()
        }
    }

    fn before_mod(id: &str) -> Placement {
        Placement {
            before_mod: Some(id.to_string()),
            ..Placement::default()
        }
    }

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
        assert_eq!(
            kinds[8],
            LineKind::Other,
            "[[mods.archives]] is a sub-table"
        );
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
        let text = "name = \"t\"\n\n[[mods]]\nid = \"a\"\nsection = [\"S\"]\n# version 1.0, as installed in MO2\n\n# why b is odd\n[[mods]]\nid = \"b\"\nsection = [\"T\"]\n";
        let mods = [entry("a", &["S"]), entry("b", &["T"])];
        let new_mod = NewMod {
            id: "c".to_string(),
            section: vec!["S".to_string()],
            ..NewMod::default()
        };

        let plan = plan_insertion(text, &mods, &new_mod).expect("insertion should be planned");

        assert!(
            plan.new_text
                .contains("# version 1.0, as installed in MO2\n\n[[mods]]\nid = \"c\"")
        );
        assert!(
            plan.new_text
                .contains("# why b is odd\n[[mods]]\nid = \"b\"")
        );
    }

    #[test]
    fn meta_ini_reads_modid_fileid_installation_file_and_version() {
        let text = "[General]\ngameName=Oblivion\nmodid=43752\nversion=11.1.0.0\ncategory=\"7,\"\ninstallationFile=Blockhead-43752-11-1-1640043918.7z\nnexusLastModified=2021-12-20T22:25:18Z\n\n[installedFiles]\nsize=1\n1\\modid=43752\n1\\fileid=1000029844\n";
        let meta = parse_meta_ini(text);

        assert_eq!(meta.mod_id, Some(43752));
        assert_eq!(meta.file_id, Some(1000029844));
        assert_eq!(
            meta.nexus_last_modified.as_deref(),
            Some("2021-12-20T22:25:18Z")
        );
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

        assert!(
            !plan.new_text.contains("\n\n"),
            "no bare LF should be introduced"
        );
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
            known_mod_id: None,
            ..NewMod::default()
        };

        let plan = plan_insertion_at(text, &mods, &new_mod, &before_section("6 - UI"))
            .expect("should plan");
        assert_eq!(
            plan.anchor,
            Anchor::BeforeSection(vec!["6 - UI".to_string()])
        );

        let five = plan.new_text.find("5 - LOD").expect("new section present");
        let six = plan
            .new_text
            .find("6 - UI")
            .expect("existing section present");
        assert!(
            five < six,
            "the new section must precede the one it was placed before"
        );
        assert!(
            plan.new_text.contains("id = \"four\""),
            "existing mods survive"
        );
    }

    #[test]
    fn placing_before_a_section_that_does_not_exist_is_an_error() {
        let text =
            "name = \"x\"\nplugins = []\n\n[[mods]]\nid = \"four\"\nsection = [\"4 - TWEAKS\"]\n";
        let mods = vec![entry("four", &["4 - TWEAKS"])];
        let new_mod = NewMod {
            id: "lod".to_string(),
            section: vec!["5 - LOD".to_string()],
            path: None,
            download_handler: None,
            file_name: None,
            version: None,
            non_nexus: false,
            known_mod_id: None,
            ..NewMod::default()
        };

        let err = plan_insertion_at(text, &mods, &new_mod, &before_section("9 - NOPE"))
            .expect_err("should refuse rather than silently append");
        assert!(
            err.to_string()
                .contains("no mod in the modlist is in that section"),
            "{err}"
        );
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
            known_mod_id: None,
            ..NewMod::default()
        };

        let plan = plan_insertion_at(text, &mods, &new_mod, &before_section("6 - UI"))
            .expect("should plan");
        assert_eq!(
            plan.anchor,
            Anchor::AfterSection(vec!["5 - LOD".to_string()])
        );
        let lod2 = plan.new_text.find("lod2").expect("added");
        let six = plan.new_text.find("6 - UI").expect("existing");
        assert!(lod2 < six);
    }

    #[test]
    fn before_mod_puts_a_row_at_the_start_of_its_section() {
        // Part 5's row 1 was authored last, so it had to land ahead of the nine
        // rows already written -- order within a section is MO2 priority order.
        let text = "name = \"x\"\nplugins = []\n\n[[mods]]\nid = \"vwd\"\nsection = [\"5 - LOD\"]\n\n[[mods]]\nid = \"j3\"\nsection = [\"5 - LOD\"]\n";
        let mods = vec![entry("vwd", &["5 - LOD"]), entry("j3", &["5 - LOD"])];
        let new_mod = NewMod {
            id: "evenstars".to_string(),
            section: vec!["5 - LOD".to_string()],
            ..NewMod::default()
        };

        let plan =
            plan_insertion_at(text, &mods, &new_mod, &before_mod("vwd")).expect("should plan");

        assert_eq!(plan.anchor, Anchor::BeforeMod("vwd".to_string()));
        let added = plan.new_text.find("evenstars").expect("added");
        let first = plan
            .new_text
            .find("id = \"vwd\"")
            .expect("existing first row");
        assert!(
            added < first,
            "the new row must precede the one it was placed before"
        );
    }

    #[test]
    fn before_mod_lands_ahead_of_that_mods_own_comments() {
        let text = "name = \"x\"\nplugins = []\n\n# why vwd is odd\n[[mods]]\nid = \"vwd\"\nsection = [\"5 - LOD\"]\n";
        let mods = vec![entry("vwd", &["5 - LOD"])];
        let new_mod = NewMod {
            id: "evenstars".to_string(),
            section: vec!["5 - LOD".to_string()],
            ..NewMod::default()
        };

        let plan =
            plan_insertion_at(text, &mods, &new_mod, &before_mod("vwd")).expect("should plan");

        assert!(
            plan.new_text
                .contains("# why vwd is odd\n[[mods]]\nid = \"vwd\""),
            "the comment must stay glued to the block it describes:\n{}",
            plan.new_text
        );
    }

    #[test]
    fn before_mod_beats_the_section_default() {
        // The new mod's own section exists and ends elsewhere; --before-mod
        // must win rather than appending to it.
        let text = "name = \"x\"\nplugins = []\n\n[[mods]]\nid = \"a\"\nsection = [\"S\"]\n\n[[mods]]\nid = \"b\"\nsection = [\"S\"]\n";
        let mods = vec![entry("a", &["S"]), entry("b", &["S"])];
        let new_mod = NewMod {
            id: "c".to_string(),
            section: vec!["S".to_string()],
            ..NewMod::default()
        };

        let plan = plan_insertion_at(text, &mods, &new_mod, &before_mod("b")).expect("should plan");

        let a = plan.new_text.find("id = \"a\"").expect("a");
        let c = plan.new_text.find("id = \"c\"").expect("c");
        let b = plan.new_text.find("id = \"b\"").expect("b");
        assert!(a < c && c < b, "c belongs between a and b");
    }

    #[test]
    fn before_mod_naming_an_unknown_id_is_an_error() {
        let text = "name = \"x\"\nplugins = []\n\n[[mods]]\nid = \"a\"\nsection = [\"S\"]\n";
        let mods = vec![entry("a", &["S"])];
        let new_mod = NewMod {
            id: "c".to_string(),
            section: vec!["S".to_string()],
            ..NewMod::default()
        };

        let err = plan_insertion_at(text, &mods, &new_mod, &before_mod("nope"))
            .expect_err("should refuse rather than fall back to the section default");
        assert!(err.to_string().contains("no mod with that id"), "{err}");
    }
}

#[cfg(test)]
mod place_tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    const LIST: &str = "name = \"x\"\nplugins = [\n\t\"Oblivion.esm\",\n\t# fixes\n\t\"A.esp\", # after the esm\n\t\"B.esp\"\n]\n\n[[mods]]\nid = \"m\"\nplugins = [\"B.esp\"]\n";

    #[test]
    fn a_plugin_goes_after_the_one_named_keeping_its_comment() {
        let out = place_plugins(
            LIST,
            &names(&["New.esp"]),
            &LoadPosition::After("a.esp".into()),
        )
        .unwrap();
        assert!(
            out.contains("\t\"A.esp\", # after the esm\n\t\"New.esp\",\n\t\"B.esp\"\n]"),
            "{out}"
        );
    }

    #[test]
    fn last_adds_the_missing_comma_and_ignores_mod_level_plugins() {
        let out = place_plugins(LIST, &names(&["N1.esp", "N2.esp"]), &LoadPosition::Last).unwrap();
        assert!(
            out.contains("\t\"B.esp\",\n\t\"N1.esp\",\n\t\"N2.esp\",\n]"),
            "{out}"
        );
        assert!(out.ends_with("plugins = [\"B.esp\"]\n"), "{out}");
    }

    #[test]
    fn listed_plugins_stay_put_and_unknown_anchors_are_refused() {
        assert_eq!(
            place_plugins(LIST, &names(&["b.esp"]), &LoadPosition::Last).unwrap(),
            LIST
        );
        assert!(
            place_plugins(
                LIST,
                &names(&["N.esp"]),
                &LoadPosition::After("Z.esp".into())
            )
            .is_err()
        );
    }

    #[test]
    fn a_list_without_a_load_order_gets_one_before_its_first_table() {
        let text = "name = \"x\"\n\n[[mods]]\nid = \"m\"\n";
        let out = place_plugins(text, &names(&["M.esp"]), &LoadPosition::Last).unwrap();
        assert_eq!(
            out,
            "name = \"x\"\n\nplugins = [\n    \"M.esp\",\n]\n\n[[mods]]\nid = \"m\"\n"
        );
        let parsed: toml::Table = toml::from_str(&out).unwrap();
        assert_eq!(parsed["plugins"].as_array().unwrap().len(), 1);
    }
}

#[cfg(test)]
mod move_tests {
    use super::*;

    const TEXT: &str = "name = \"x\"\n\n[[mods]]\nid = \"a\"\nsection = [\"One\"]\n\n# b needs its patch\n[[mods]]\nid = \"b\"\nsection = [\"One\"]\n\n[[mods.archives]]\npath = \"manual:b.7z\"\n# oracle version 1\n\n[[mods]]\nid = \"c\"\nsection = [\"Two\"]\n";

    fn mods() -> Vec<ModEntry> {
        toml::from_str::<crate::config::schema::SourceModlist>(TEXT)
            .unwrap()
            .mods
    }

    #[test]
    fn a_block_moves_with_its_comments_and_trailing_note() {
        let plan = plan_move(
            TEXT,
            &mods(),
            "b",
            &["Two".to_string()],
            &Placement::default(),
        )
        .unwrap();
        assert_eq!(
            plan.new_text,
            "name = \"x\"\n\n[[mods]]\nid = \"a\"\nsection = [\"One\"]\n\n[[mods]]\nid = \"c\"\nsection = [\"Two\"]\n\n# b needs its patch\n[[mods]]\nid = \"b\"\nsection = [\"Two\"]\n\n[[mods.archives]]\npath = \"manual:b.7z\"\n# oracle version 1\n"
        );
    }

    #[test]
    fn a_block_moves_before_a_named_mod() {
        let placement = Placement {
            before_mod: Some("a".into()),
            before_section: None,
        };
        let plan = plan_move(TEXT, &mods(), "c", &["One".to_string()], &placement).unwrap();
        assert!(
            plan.new_text.starts_with(
                "name = \"x\"\n\n[[mods]]\nid = \"c\"\nsection = [\"One\"]\n\n[[mods]]\nid = \"a\""
            ),
            "{}",
            plan.new_text
        );
        assert!(
            plan_move(
                TEXT,
                &mods(),
                "c",
                &[],
                &Placement {
                    before_mod: Some("c".into()),
                    before_section: None
                }
            )
            .is_err()
        );
    }
}
