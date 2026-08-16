//! `extract_bsa`: unpack a BSA the mod ships, leaving its contents loose.
//!
//! The guide reaches for BAE here -- a Windows GUI tool -- for one reason: a
//! mod ships a BSA, something else has to be merged into it, and the only way
//! to do that is to unpack, overlay, and repack. mudcrab reads BSAs natively,
//! so the round trip is `extract_bsa` then `pack_bsa` with no external tool and
//! no WINE.
//!
//! The archive is deleted once unpacked. Leaving it would mean the following
//! `pack_bsa` swallowed the old archive into the new one, and the loose files
//! would be shadowed by the very archive they came from.

use super::ActionCx;
use crate::bsa::Bsa;
use crate::config::schema::ExtractBsaAction;
use crate::util::fs::normalize_relative_path;

pub(super) fn apply(action: &ExtractBsaAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!("{}: extract_bsa is only valid as a per-mod action", cx.owner);
    };

    let archive = mod_target.join(normalize_relative_path(&action.archive)?);

    if cx.settings.dry_run {
        tracing::info!(
            owner = cx.owner,
            archive = %archive.display(),
            "install dry-run extract_bsa action"
        );
        return Ok(());
    }

    if !archive.is_file() {
        anyhow::bail!(
            "{}: extract_bsa found no archive at {}. The path is relative to the \
             staged folder.",
            cx.owner,
            archive.display()
        );
    }

    // Read the whole archive rather than streaming: a BSA's file records point
    // at arbitrary offsets, so there is no useful sequential order to stream in,
    // and the largest in this list is around a gigabyte.
    let bytes = std::fs::read(&archive)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", archive.display()))?;
    let parsed = Bsa::parse(&bytes)
        .map_err(|err| anyhow::anyhow!("failed to parse {}: {err}", archive.display()))?;

    let extracted = parsed
        .extract_to(mod_target)
        .map_err(|err| anyhow::anyhow!("failed to extract {}: {err}", archive.display()))?;

    // Drop the buffer before deleting, so the archive's bytes are not held
    // while the next action starts reading the files just written.
    drop(parsed);
    drop(bytes);

    if !action.keep_archive {
        std::fs::remove_file(&archive)
            .map_err(|err| anyhow::anyhow!("failed to remove {}: {err}", archive.display()))?;
    }

    tracing::info!(
        owner = cx.owner,
        archive = %archive.display(),
        extracted,
        kept = action.keep_archive,
        "extracted BSA"
    );
    Ok(())
}
