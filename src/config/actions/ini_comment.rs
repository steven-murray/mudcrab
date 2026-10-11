//! `ini_comment`: switch a setting off by commenting its line out.
//!
//! Some guide rows ask for exactly this ("comment out `set EasyMenu.iAltExitKey`
//! by adding a semi-colon"), and it is not the same as `ini_set`: a commented
//! line makes the script fall back to its own default, where any value written
//! in its place would be a choice the guide did not make.

use super::ActionCx;
use super::ini_set::{is_ini_key_line, write_ini_lines};
use crate::config::schema::IniCommentAction;
use crate::util::fs::{normalize_relative_path, resolve_existing_path_case_insensitive};

pub(super) fn apply(action: &IniCommentAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!(
            "{}: ini_comment is only valid on a per-mod action",
            cx.owner
        );
    };
    let wanted = mod_target.join(normalize_relative_path(&action.file)?);
    let path = resolve_existing_path_case_insensitive(&wanted).ok_or_else(|| {
        anyhow::anyhow!(
            "{}: ini_comment target does not exist: {}",
            cx.owner,
            wanted.display()
        )
    })?;

    if cx.settings.dry_run {
        tracing::info!(owner = cx.owner, ini = %path.display(), key = action.key, "install dry-run ini_comment action");
        return Ok(());
    }

    let original = std::fs::read_to_string(&path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    let (lines, changed) = comment_out(&original, &action.key, action.format);
    // Nothing to comment out means the file is not the one the guide meant, or
    // the setting moved: say so rather than report success.
    anyhow::ensure!(
        changed > 0,
        "{}: {} has no active `{}` line to comment out",
        cx.owner,
        path.display(),
        action.key
    );
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    write_ini_lines(&path, &lines, newline, original.ends_with('\n'))?;
    tracing::info!(owner = cx.owner, ini = %path.display(), key = action.key, lines = changed, "commented out an ini setting");
    Ok(())
}

/// `text`'s lines with every active `key` line commented, and how many were.
fn comment_out(
    text: &str,
    key: &str,
    format: crate::config::schema::IniSetFormat,
) -> (Vec<String>, usize) {
    let mut changed = 0;
    let lines = text
        .lines()
        .map(|line| {
            if is_ini_key_line(line, key, format) {
                changed += 1;
                format!(";{line}")
            } else {
                line.to_string()
            }
        })
        .collect();
    (lines, changed)
}

#[cfg(test)]
mod tests {
    use crate::config::schema::IniSetFormat;

    #[test]
    fn comments_out_only_the_active_line() {
        let ini = "; - iAltExitKey -\r\nset EasyMenu.iExitKey to 1\r\nset EasyMenu.iAltExitKey             to  57     ; 57  (Space)\r\n";
        let (lines, changed) = super::comment_out(ini, "EasyMenu.iAltExitKey", IniSetFormat::SetTo);
        assert_eq!(changed, 1);
        assert_eq!(
            lines[2],
            ";set EasyMenu.iAltExitKey             to  57     ; 57  (Space)"
        );
        assert_eq!(lines[1], "set EasyMenu.iExitKey to 1");
        // Already commented: nothing to do, which the action reports as an error.
        assert_eq!(
            super::comment_out(
                &lines.join("\r\n"),
                "EasyMenu.iAltExitKey",
                IniSetFormat::SetTo
            )
            .1,
            0
        );
    }
}
