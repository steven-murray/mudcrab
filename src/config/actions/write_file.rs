//! `write_file`: content the modlist authors rather than downloads.

use super::ActionCx;
use crate::config::schema::WriteFileAction;
use crate::util::fs::normalize_relative_path;

pub(super) fn apply(action: &WriteFileAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!("{}: write_file is only valid as a per-mod action", cx.owner);
    };
    let target = mod_target.join(normalize_relative_path(&action.path)?);

    if cx.settings.dry_run {
        tracing::info!(owner = cx.owner, target = %target.display(), "install dry-run write_file");
        return Ok(());
    }
    if target.exists() {
        anyhow::bail!(
            "{}: write_file would overwrite {}, which is already there",
            cx.owner,
            target.display()
        );
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
    }
    std::fs::write(&target, &action.content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", target.display()))?;
    tracing::info!(owner = cx.owner, target = %target.display(), "wrote file");
    Ok(())
}
