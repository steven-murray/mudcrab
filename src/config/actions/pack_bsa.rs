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

    let packed = archive.file_count();
    let folders = archive.folders.len();

    // Deleting what was just packed, from the pack's own file list rather than
    // from a hand-written glob. Naming the folders by hand means guessing at
    // the archive's top-level layout, and a guess that is wrong either deletes
    // nothing or leaves loose copies shadowing the archive they came from.
    let mut pruned = 0usize;
    if action.prune_packed {
        // Matched against what is on disk, not against the archive's own paths.
        // A BSA stores names lowercased, so rejoining them to the staged folder
        // finds nothing wherever the tree is actually cased -- which silently
        // left OOO's 1554 `Sound/` files loose, shadowing the archive holding
        // them, while reporting a healthy-looking 4406 deletions.
        let packed: std::collections::HashSet<String> = archive
            .files()
            .map(|(folder, file)| file.path_in(folder).replace('\\', "/").to_ascii_lowercase())
            .collect();
        prune_matching(mod_target, mod_target, &packed, &mut pruned)?;
        remove_empty_dirs(mod_target, mod_target)?;
    }

    tracing::info!(
        owner = cx.owner,
        output = %output.display(),
        files = packed,
        folders,
        pruned,
        "packed BSA"
    );
    Ok(())
}

/// Delete every staged file whose folder-relative path is in `packed`.
fn prune_matching(
    current: &std::path::Path,
    root: &std::path::Path,
    packed: &std::collections::HashSet<String>,
    pruned: &mut usize,
) -> anyhow::Result<()> {
    let entries = std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?;

    for entry in entries {
        let entry = entry
            .map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to stat {}: {err}", path.display()))?;

        if file_type.is_dir() {
            prune_matching(&path, root, packed, pruned)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .map_err(|_| anyhow::anyhow!("{} escaped {}", path.display(), root.display()))?
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();

        if packed.contains(&relative) {
            std::fs::remove_file(&path)
                .map_err(|err| anyhow::anyhow!("failed to remove {}: {err}", path.display()))?;
            *pruned += 1;
        }
    }

    Ok(())
}

/// Remove directories left empty, deepest first, without touching `root`.
fn remove_empty_dirs(current: &std::path::Path, root: &std::path::Path) -> anyhow::Result<bool> {
    let entries = std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?;

    let mut remaining = 0usize;
    for entry in entries {
        let entry = entry
            .map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", current.display()))?;
        let path = entry.path();
        let is_dir = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to stat {}: {err}", path.display()))?
            .is_dir();

        if is_dir && remove_empty_dirs(&path, root)? {
            std::fs::remove_dir(&path)
                .map_err(|err| anyhow::anyhow!("failed to remove {}: {err}", path.display()))?;
        } else {
            remaining += 1;
        }
    }

    Ok(remaining == 0 && current != root)
}
