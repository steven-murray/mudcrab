//! `pack_bsa`: pack the mod's staged files into a BSA.

use super::ActionCx;
use crate::archive::ArchiveFilters;
use crate::bsa::Bsa;
use crate::config::schema::PackBsaAction;
use crate::util::fs::normalize_relative_path;

pub(super) fn apply(action: &PackBsaAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!("{}: pack_bsa is only valid as a per-mod action", cx.owner);
    };

    let relative = normalize_relative_path(&action.output)?;
    let output = mod_target.join(&relative);

    // Never pack the archive into itself. Without this a re-run would fold the
    // previous archive into the new one, doubling its size each time.
    let mut exclude = action.exclude.clone();
    exclude.push(relative.to_string_lossy().replace('\\', "/"));

    let filters = ArchiveFilters::new(&action.include, &exclude)?;

    if cx.settings.dry_run {
        tracing::info!(
            owner = cx.owner,
            output = %output.display(),
            include = ?action.include,
            exclude = ?action.exclude,
            "install dry-run pack_bsa action"
        );
        return Ok(());
    }

    let archive = Bsa::from_directory(mod_target, &filters).map_err(|err| {
        anyhow::anyhow!("{}: failed to pack {}: {err}", cx.owner, output.display())
    })?;

    if archive.file_count() == 0 {
        anyhow::bail!(
            "{}: pack_bsa matched no files under {}",
            cx.owner,
            mod_target.display()
        );
    }

    // A BSA cannot hold a file outside a folder, so anything at the top level
    // of the staged mod stays loose. Say so rather than let it go missing.
    let loose = crate::bsa::root_level_files(mod_target)
        .unwrap_or_default()
        .into_iter()
        .filter(|name| !name.eq_ignore_ascii_case(&relative.to_string_lossy()))
        .collect::<Vec<_>>();
    if !loose.is_empty() {
        tracing::info!(
            owner = cx.owner,
            files = ?loose,
            "not packed: a BSA cannot store files outside a folder, so these stay loose"
        );
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
    }

    archive.write_to_file(&output).map_err(|err| {
        anyhow::anyhow!("{}: failed to write {}: {err}", cx.owner, output.display())
    })?;

    tracing::info!(
        owner = cx.owner,
        output = %output.display(),
        files = archive.file_count(),
        folders = archive.folders.len(),
        "packed BSA"
    );
    Ok(())
}
