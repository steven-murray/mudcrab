//! Install manifest: what was installed, where, and whether it is still current.

use crate::config::mo2::{MO2_HIDDEN_SUFFIX, mo2_profile_dir};
use crate::config::schema::{MergeSpec, PersonalizedMod};
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
    /// Merges already built, and from what. Defaulted so a manifest written
    /// before this field existed still parses -- it simply rebuilds once.
    #[serde(default)]
    pub(crate) built_merges: Vec<BuiltMerge>,
    /// The instance as the last install that changed it left it, so a manual
    /// step's plugin (a Bashed Patch) counts as built only when it was built
    /// after that. `None` until an install has recorded it.
    #[serde(default)]
    pub(crate) by_hand_since: Option<ByHandSince>,
}

/// The last install that changed what is installed or the load order, and
/// the copies of the manual steps' plugins already there when it finished.
/// Those were built for an earlier state -- by an earlier list, or before the
/// mods changed -- so they are to be built again.
///
/// By contents rather than modified time: Oblivion's load order *is* the
/// plugins' modified times, so Wrye Bash and MO2 both rewrite them, and a
/// fresh patch can carry a date from 2000.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ByHandSince {
    /// Fingerprint of the installed mods and the load order.
    pub(crate) installed: String,
    pub(crate) found: Vec<FoundPlugin>,
}

/// A copy of a plugin a manual step waits for, by its contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct FoundPlugin {
    pub(crate) plugin: String,
    pub(crate) sha256: String,
}

/// The contents' fingerprint, which a rebuild changes.
pub(crate) fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut std::fs::File::open(path)?, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// A merge that has been built, and the fingerprint of what it was built from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BuiltMerge {
    pub(crate) id: String,
    pub(crate) input_hash: String,
    /// The plugin filename, so the skip check can confirm the output is still
    /// on disk without consulting the plan.
    #[serde(default)]
    pub(crate) output: String,
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
    force: bool,
) -> bool {
    // The fingerprint covers the plan, not mudcrab itself, so a mod whose spec
    // is unchanged is skipped even when the code that installs it has changed.
    // `--force` is the way back out of that.
    if force || dry_run {
        return false;
    }

    let Some(previous) = previous else {
        return false;
    };

    mod_target.exists() && previous.definition_hash == definition_hash
}

/// Fingerprint of everything about a mod that decides what lands on disk.
///
/// `section` is deliberately excluded, as is the declared `plugins` list (what
/// lands on disk comes from the archives, not from the declaration). It only decides where the mod appears in
/// MO2's list, so hashing it would have invalidated every existing manifest the
/// first time a plan carried sections and forced a full reinstall of a 700-mod
/// list for a purely cosmetic field. An archive's `size` and `md5`, and its
/// download page and note, are left out for the same reason: they say how to
/// *find* an archive, not what it installs, so recording them must not
/// reinstall the mods they belong to.
pub(crate) fn hash_personalized_mod(mod_entry: &PersonalizedMod) -> anyhow::Result<String> {
    let mut hashable = mod_entry.clone();
    hashable.section.clear();
    hashable.plugins.clear();
    for archive in &mut hashable.archives {
        archive.size = None;
        archive.md5 = None;
        archive.download_page = None;
        archive.download_note = None;
    }

    let payload = serde_json::to_vec(&hashable).map_err(|err| {
        anyhow::anyhow!(
            "failed to serialize mod {} for hashing: {err}",
            mod_entry.id
        )
    })?;
    Ok(hex_digest(&payload))
}

fn hex_digest(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Fingerprint of everything a merge is built from.
///
/// The spec (output name, method, ordered sources) plus, for each source
/// plugin, where it is and what state it is in -- size and modification time,
/// which is what a re-extracted or hand-edited plugin changes. The plan's load
/// order is included too: it is a genuine input, since it decides the order of
/// the merged plugin's master table, and a merge cached across a load-order
/// change would be a plausible-looking file with the wrong header.
///
/// Source paths are recorded with any `.mohidden` suffix stripped, because a
/// merge hides its own sources as its last step: without that, the very act of
/// building a merge would change its inputs' fingerprint and it could never be
/// skipped.
pub(crate) fn hash_merge_inputs(
    merge_id: &str,
    spec: &MergeSpec,
    load_order: &[String],
    source_paths: &[PathBuf],
    mods_dir: &Path,
) -> anyhow::Result<String> {
    let mut sources = Vec::with_capacity(source_paths.len());
    for path in source_paths {
        let metadata = std::fs::metadata(path).map_err(|err| {
            anyhow::anyhow!("merge {merge_id}: failed to stat {}: {err}", path.display())
        })?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_nanos());
        sources.push(serde_json::json!({
            "path": stable_source_key(path, mods_dir),
            "size": metadata.len(),
            "mtime_nanos": modified,
        }));
    }

    let payload = serde_json::to_vec(&serde_json::json!({
        "merge": merge_id,
        "spec": spec,
        "load_order": load_order,
        "sources": sources,
    }))
    .map_err(|err| anyhow::anyhow!("failed to serialize merge {merge_id} for hashing: {err}"))?;

    Ok(hex_digest(&payload))
}

/// A source plugin's identity, stable across the hiding step and across the
/// instance moving: relative to the mods directory where possible, forward
/// slashed, without the `.mohidden` suffix.
fn stable_source_key(path: &Path, mods_dir: &Path) -> String {
    let relative = path
        .strip_prefix(mods_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    match relative
        .len()
        .checked_sub(MO2_HIDDEN_SUFFIX.len())
        .filter(|cut| relative[*cut..].eq_ignore_ascii_case(MO2_HIDDEN_SUFFIX))
    {
        Some(cut) => relative[..cut].to_string(),
        None => relative,
    }
}

/// Skip a rebuild only when the recorded inputs still match *and* the plugin
/// the last build produced is still there. Mirrors `should_skip_mod_install`:
/// a matching hash over a missing output is not a completed build.
pub(crate) fn should_skip_merge(
    output_path: &Path,
    input_hash: &str,
    previous: Option<&BuiltMerge>,
    dry_run: bool,
    force: bool,
) -> bool {
    if dry_run || force {
        return false;
    }

    let Some(previous) = previous else {
        return false;
    };

    output_path.is_file() && previous.input_hash == input_hash
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

/// Fingerprint of an install's result as far as a Bashed Patch is concerned:
/// which mods are in, as defined, and the load order.
pub(crate) fn hash_installed(
    installed: &[InstalledMod],
    plugins: &[String],
    inactive: &[String],
) -> String {
    let mut mods: Vec<(&str, &str)> = installed
        .iter()
        .map(|entry| (entry.id.as_str(), entry.definition_hash.as_str()))
        .collect();
    mods.sort_unstable();
    let payload = serde_json::to_vec(&(mods, plugins, inactive)).unwrap_or_default();
    format!("{:x}", Sha256::digest(payload))
}

#[cfg(test)]
mod content_tests {
    use super::*;

    #[test]
    fn recording_an_archives_size_and_md5_does_not_reinstall_its_mod() {
        let archive = crate::config::schema::CompiledArchive {
            path: Some("manual:h.zip".to_string()),
            ..Default::default()
        };
        let mut entry: PersonalizedMod = serde_json::from_value(serde_json::json!({
            "id": "Handmade",
            "archives": [archive],
        }))
        .unwrap();
        let before = hash_personalized_mod(&entry).unwrap();
        entry.archives[0].size = Some(7);
        entry.archives[0].md5 = Some("0".repeat(32));
        assert_eq!(hash_personalized_mod(&entry).unwrap(), before);
    }
}
