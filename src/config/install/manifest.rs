//! Install manifest: what was installed, where, and whether it is still current.

use crate::config::mo2::mo2_profile_dir;
use crate::config::schema::PersonalizedMod;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use super::InstallSettings;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct InstallManifest {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) installed_mods: Vec<InstalledMod>,
    /// Source plugins renamed to `.mohidden` on behalf of a merge. Recorded so
    /// `unhide-merges` can undo exactly what was done, not what the modlist
    /// currently says should have been done.
    #[serde(default)]
    pub(crate) hidden_plugins: Vec<super::merge::HiddenPlugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InstalledMod {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) definition_hash: String,
    pub(crate) extracted_files: usize,
    #[serde(default)]
    pub(crate) actions_applied: bool,
    /// Relative path (within mods/) where this mod was actually installed.
    /// If empty, defaults to mods/<id>. Populated when a mod is renamed due to conflicts.
    #[serde(default)]
    pub(crate) installed_path: String,
}

pub(crate) fn get_install_manifest_path(settings: &InstallSettings) -> PathBuf {
    if let Some(profile_dir) = mo2_profile_dir(settings) {
        profile_dir.join("install_manifest.json")
    } else {
        settings.mods_dir.join("install_manifest.json")
    }
}

/// Manifest path from the locations alone, for commands that operate on an
/// existing install and have no plan or tool config to build settings from.
pub fn manifest_path(
    mods_dir: &Path,
    mo2_instance_dir: Option<&Path>,
    profile_name: &str,
) -> PathBuf {
    match mo2_instance_dir {
        Some(instance) => instance
            .join("profiles")
            .join(profile_name)
            .join("install_manifest.json"),
        None => mods_dir.join("install_manifest.json"),
    }
}

pub(crate) fn load_install_manifest(path: &Path) -> anyhow::Result<Option<InstallManifest>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    let parsed: InstallManifest = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            tracing::warn!(
                manifest = %path.display(),
                error = %err,
                "install: ignoring unreadable manifest and performing full reinstall"
            );
            return Ok(None);
        }
    };

    Ok(Some(parsed))
}

pub(crate) fn should_skip_mod_install(
    mod_target: &Path,
    definition_hash: &str,
    previous: Option<&InstalledMod>,
    dry_run: bool,
) -> bool {
    if dry_run {
        return false;
    }

    let Some(previous) = previous else {
        return false;
    };

    mod_target.exists() && previous.definition_hash == definition_hash
}

pub(crate) fn hash_personalized_mod(mod_entry: &PersonalizedMod) -> anyhow::Result<String> {
    let payload = serde_json::to_vec(mod_entry)
        .map_err(|err| anyhow::anyhow!("failed to serialize mod {} for hashing: {err}", mod_entry.id))?;
    let digest = Sha256::digest(&payload);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)

}

pub(crate) fn relative_path_to_mod(mod_path: &Path, mods_dir: &Path) -> String {
    mod_path
        .strip_prefix(mods_dir)
        .ok()
        .and_then(|rel| rel.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            mod_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        })
}

