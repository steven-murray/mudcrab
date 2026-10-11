//! Installing a Mod Organizer 2 plugin into the instance's `plugins/` folder.
//!
//! Small, so it is simply rebuilt on every run rather than tracked in the
//! manifest: extracted beside its destination, then swapped in, so a failed
//! extraction leaves the previous copy in place.

use super::layout::install_mod_archives;
use super::{InstallSettings, safe_mod_dir_name};
use crate::config::schema::PersonalizedMod;
use std::collections::HashSet;

pub(super) fn install(
    mod_entry: &PersonalizedMod,
    settings: &InstallSettings,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<()> {
    let Some(instance) = &settings.mo2_instance_dir else {
        anyhow::bail!(
            "mod '{}' is an MO2 plugin, which only an install with --mo2-instance-dir can place",
            mod_entry.id
        );
    };
    let plugins_dir = instance.join("plugins");
    let target = plugins_dir.join(safe_mod_dir_name(&mod_entry.id)?);

    if settings.dry_run {
        tracing::info!(mod_id = %mod_entry.id, target = %target.display(), "install dry-run: MO2 plugin");
        return Ok(());
    }

    let staging = plugins_dir.join(format!(".{}.mudcrab-staging", mod_entry.id));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .map_err(|err| anyhow::anyhow!("failed to clear {}: {err}", staging.display()))?;
    }
    let extracted = install_mod_archives(mod_entry, settings, &staging, active_plugins)?;
    if extracted == 0 {
        anyhow::bail!(
            "mod '{}': MO2 plugin archive produced no files",
            mod_entry.id
        );
    }

    if target.exists() {
        std::fs::remove_dir_all(&target)
            .map_err(|err| anyhow::anyhow!("failed to replace {}: {err}", target.display()))?;
    }
    std::fs::rename(&staging, &target)
        .map_err(|err| anyhow::anyhow!("failed to move {} into place: {err}", staging.display()))?;

    tracing::info!(
        mod_id = %mod_entry.id,
        target = %target.display(),
        extracted,
        "installed MO2 plugin"
    );
    Ok(())
}
