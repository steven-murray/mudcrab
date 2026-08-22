//! `ini_append_block`: paste a verbatim block of lines onto the end of an INI.

use super::ActionCx;
use crate::config::schema::IniAppendBlockAction;
use crate::util::fs::{normalize_relative_path, resolve_existing_path_case_insensitive};
use std::path::Path;

pub(super) fn apply(action: &IniAppendBlockAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!(
            "{}: ini_append_block is only valid on a per-mod action",
            cx.owner
        );
    };

    let declared = mod_target.join(normalize_relative_path(&action.file)?);
    let ini_path = resolve_existing_path_case_insensitive(&declared).ok_or_else(|| {
        anyhow::anyhow!(
            "{} ini_append_block target file does not exist: {}",
            cx.owner,
            declared.display()
        )
    })?;

    if cx.settings.dry_run {
        tracing::info!(
            owner = cx.owner,
            ini = %ini_path.display(),
            lines = action.block.lines().count(),
            "install dry-run ini_append_block action"
        );
        return Ok(());
    }

    match append_block(&ini_path, &action.block)? {
        AppendOutcome::Appended { lines } => tracing::info!(
            owner = cx.owner,
            ini = %ini_path.display(),
            lines,
            "appended a block to an ini"
        ),
        AppendOutcome::AlreadyPresent => tracing::info!(
            owner = cx.owner,
            ini = %ini_path.display(),
            "ini already ends with this block; nothing appended"
        ),
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppendOutcome {
    Appended { lines: usize },
    /// The block is already there. Installs are re-run often enough that an
    /// append which is not idempotent would grow the file every time.
    AlreadyPresent,
}

/// The line ending a file already uses.
///
/// A single CRLF is enough: these files are Windows-authored, and a mixed file
/// should gain lines in the convention it mostly has rather than the one its
/// first line happens to use.
fn line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

pub(crate) fn append_block(path: &Path, block: &str) -> anyhow::Result<AppendOutcome> {
    let existing = std::fs::read_to_string(path)?;
    let ending = line_ending(&existing);

    // Split on '\n' and drop any '\r', so a block authored with either
    // convention -- or with none, being a TOML string -- lands in the file's.
    let mut lines: Vec<&str> = block
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    // The newline a multi-line TOML string carries before its closing delimiter
    // is the delimiter's, not content.
    if lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    // Leading blank lines are the gap before the paste, so they are content --
    // but a block that is *only* blank lines says nothing and would defeat the
    // idempotence check, every file already containing a blank line.
    if lines.iter().all(|line| line.trim().is_empty()) {
        anyhow::bail!("ini_append_block was given a block with no content");
    }
    let rendered = lines.join(ending);

    if existing.contains(&rendered) {
        return Ok(AppendOutcome::AlreadyPresent);
    }

    // Restore the file's own shape: one that ended mid-line still does.
    let had_final_newline = existing.ends_with('\n');
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(ending);
    }
    out.push_str(&rendered);
    if had_final_newline {
        out.push_str(ending);
    }

    std::fs::write(path, out)?;
    Ok(AppendOutcome::Appended {
        lines: lines.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    /// The Part 30 row 7 shape exactly: a CRLF file ending mid-line, a block
    /// whose leading blank lines are the gap the guide's paste left.
    #[test]
    fn appends_in_the_files_own_convention_and_keeps_its_final_line_bare() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "t.ini", "a\r\nb");

        let outcome = append_block(&path, "\n\nset x to 1\nSetStage q 1\n").unwrap();

        assert_eq!(outcome, AppendOutcome::Appended { lines: 4 });
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "a\r\nb\r\n\r\n\r\nset x to 1\r\nSetStage q 1"
        );
    }

    #[test]
    fn a_file_that_ended_with_a_newline_still_does() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "t.ini", "a\n");

        append_block(&path, "x\n").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\nx\n");
    }

    #[test]
    fn appending_twice_appends_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "t.ini", "a\r\nb");

        append_block(&path, "\nset x to 1\n").unwrap();
        let after_first = std::fs::read_to_string(&path).unwrap();
        let outcome = append_block(&path, "\nset x to 1\n").unwrap();

        assert_eq!(outcome, AppendOutcome::AlreadyPresent);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), after_first);
    }

    /// Repetition is the content of these blocks, so the guard has to match the
    /// whole thing. Three lines that each appear elsewhere in the file are not
    /// the same as the block being present.
    #[test]
    fn a_block_whose_lines_all_appear_separately_is_still_appended() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            "t.ini",
            "set x to 1\r\nSetStage q 1\r\nset x to 2\r\nSetStage q 1",
        );

        let outcome = append_block(&path, "set x to 1\nset x to 2\n").unwrap();

        assert!(matches!(outcome, AppendOutcome::Appended { .. }));
    }

    #[test]
    fn an_empty_block_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "t.ini", "a\n");

        assert!(append_block(&path, "\n").is_err());
    }
}
