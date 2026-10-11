//! `mudcrab downloads tidy`: from the old derived-name cache to real names.
//!
//! mudcrab used to keep each archive under a name derived from where the
//! modlist used it -- `{mod id}_{archive index}_{source tail}` -- with the real
//! filename in a `.orig-name` sidecar. The downloads folder now holds archives
//! under their real names with MO2 `.meta` files (see `archive_store`), so a
//! folder from before that needs converting once:
//!
//! - a derived name that is the same file as the real-named one beside it (a
//!   hard link, or identical bytes) is deleted, with its sidecar;
//! - one with no real-named twin is renamed to its real name;
//! - one whose real name is taken by a *different* file is left alone and
//!   reported;
//! - with `--plan`, any Nexus or URL archive still lacking a `.meta` gets one,
//!   since an archive the modlist names by source alone is found through it.
//!
//! Everything is planned before anything is touched, and nothing is touched
//! without `--apply`: this is the user's downloads folder.

use crate::config::archive_store::{self, ArchiveRef};
use crate::config::schema::PersonalizedPlan;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The derived name is a second name for a file already under its real one.
    Delete { derived: PathBuf, real: PathBuf },
    /// The derived name is the only copy: give it its real name.
    Rename { derived: PathBuf, real: PathBuf },
    /// The real name holds a different file. Left for the user.
    Conflict { derived: PathBuf, real: PathBuf },
    /// A derived name whose real name is not known. Left in place.
    Unknown { derived: PathBuf },
    /// Write a `.meta` for an archive the modlist knows the source of.
    WriteMeta { archive: PathBuf, source: String },
    /// A `.orig-name` with no archive beside it.
    DeleteSidecar { sidecar: PathBuf },
}

/// The derived names a plan's archives had, lower-cased, to `(source, file_name)`.
fn legacy_names(plan: &PersonalizedPlan) -> HashMap<String, (String, Option<String>)> {
    fn sanitize(tail: &str) -> String {
        tail.chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '_' })
            .collect()
    }
    fn tail(source: &str) -> &str {
        let tail = source.rsplit('/').next().unwrap_or("archive.bin");
        tail.split('?').next().unwrap_or(tail)
    }
    let mut out = HashMap::new();
    for entry in &plan.mods {
        for (index, archive) in entry.archives.iter().enumerate() {
            if let Some(path) = archive.path.as_deref() {
                let name = format!("{}_{}_{}", entry.id, index, sanitize(tail(path)));
                out.insert(name.to_ascii_lowercase(), (path.to_string(), archive.file_name.clone()));
            }
            for (layer_index, layer) in archive.build.iter().enumerate() {
                let safe_id: String = entry
                    .id
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                    .collect();
                let name = format!(
                    "{safe_id}_{index}_layer{layer_index}_{}",
                    sanitize(tail(&layer.path))
                );
                out.insert(name.to_ascii_lowercase(), (layer.path.clone(), None));
            }
        }
    }
    out
}

/// The plan entry a derived file came from: its name, or its name less the
/// archive extension mudcrab appended after download.
fn source_of<'a>(
    name: &str,
    legacy: &'a HashMap<String, (String, Option<String>)>,
) -> Option<&'a (String, Option<String>)> {
    let lower = name.to_ascii_lowercase();
    legacy.get(&lower).or_else(|| {
        let (stem, _) = lower.rsplit_once('.')?;
        legacy.get(stem)
    })
}

/// Whether two paths are the same file: one inode, or identical bytes.
fn same_file(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (a.metadata(), b.metadata()) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if ma.dev() == mb.dev() && ma.ino() == mb.ino() {
            return true;
        }
    }
    if ma.len() != mb.len() {
        return false;
    }
    match (std::fs::read(a), std::fs::read(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn valid_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0']))
        .then_some(name)
}

/// Work out what tidying `dir` would do.
pub fn plan_tidy(dir: &Path, plan: Option<&PersonalizedPlan>) -> anyhow::Result<Vec<Step>> {
    let legacy = plan.map(legacy_names).unwrap_or_default();
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", dir.display()))?
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let present: std::collections::HashSet<String> = names.iter().cloned().collect();

    let mut steps = Vec::new();
    // Real names this run will create, and from which file, so a second derived
    // name for the same archive is recognised before anything is renamed.
    let mut claimed: HashMap<String, PathBuf> = HashMap::new();
    // Archives that end up holding a known source, for `.meta`.
    let mut sourced: Vec<(PathBuf, String)> = Vec::new();

    for name in &names {
        if name.ends_with(".orig-name") {
            let archive = name.trim_end_matches(".orig-name");
            let has_archive = present.contains(archive)
                || names.iter().any(|other| {
                    other.len() > archive.len()
                        && other.starts_with(archive)
                        && other.as_bytes()[archive.len()] == b'.'
                        && !other.ends_with(".orig-name")
                        && !other.ends_with(".meta")
                });
            if !has_archive {
                steps.push(Step::DeleteSidecar { sidecar: dir.join(name) });
            }
            continue;
        }
        if name.ends_with(".meta") || name.ends_with(".part") {
            continue;
        }

        let path = dir.join(name);
        let sidecar_name = |base: &str| format!("{base}.orig-name");
        let sidecar = [name.as_str(), name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem)]
            .into_iter()
            .map(sidecar_name)
            .find(|candidate| present.contains(candidate));
        let source = source_of(name, &legacy);
        if sidecar.is_none() && source.is_none() {
            // An ordinary, real-named archive. It may still want a `.meta`.
            continue;
        }

        let real = sidecar
            .as_ref()
            .and_then(|s| std::fs::read_to_string(dir.join(s)).ok())
            .and_then(|text| valid_name(&text).map(str::to_string))
            .or_else(|| source.and_then(|(_, file_name)| file_name.as_deref()).and_then(valid_name).map(str::to_string));

        let Some(real) = real else {
            steps.push(Step::Unknown { derived: path.clone() });
            if let Some((src, _)) = source {
                sourced.push((path, src.clone()));
            }
            continue;
        };
        let real_path = dir.join(&real);
        let lower = real.to_ascii_lowercase();

        let step = if real == *name {
            None
        } else if let Some(first) = claimed.get(&lower) {
            Some(if same_file(&path, first) {
                Step::Delete { derived: path.clone(), real: real_path.clone() }
            } else {
                Step::Conflict { derived: path.clone(), real: real_path.clone() }
            })
        } else if real_path.exists() {
            Some(if same_file(&path, &real_path) {
                Step::Delete { derived: path.clone(), real: real_path.clone() }
            } else {
                Step::Conflict { derived: path.clone(), real: real_path.clone() }
            })
        } else {
            claimed.insert(lower, path.clone());
            Some(Step::Rename { derived: path.clone(), real: real_path.clone() })
        };
        let settled = !matches!(step, Some(Step::Conflict { .. }));
        if let Some(step) = step {
            steps.push(step);
        }
        if settled && let Some((src, _)) = source {
            sourced.push((real_path, src.clone()));
        }
    }

    for (archive, source) in sourced {
        let is_meta_worthy = source.starts_with("nexus:")
            || source.starts_with("http://")
            || source.starts_with("https://");
        let already = archive_store::meta_path(&archive).exists()
            || steps.iter().any(|s| matches!(s, Step::WriteMeta { archive: a, .. } if *a == archive));
        if is_meta_worthy && !already {
            steps.push(Step::WriteMeta { archive, source });
        }
    }
    Ok(steps)
}

/// Carry out a planned tidy.
pub fn apply(steps: &[Step]) -> anyhow::Result<()> {
    let remove = |path: &Path| {
        std::fs::remove_file(path)
            .map_err(|err| anyhow::anyhow!("failed to remove {}: {err}", path.display()))
    };
    let remove_sidecars = |derived: &Path| -> anyhow::Result<()> {
        let name = derived.to_string_lossy();
        let stem = derived.with_extension("");
        for sidecar in [format!("{name}.orig-name"), format!("{}.orig-name", stem.display())] {
            let sidecar = PathBuf::from(sidecar);
            if sidecar.is_file() {
                remove(&sidecar)?;
            }
        }
        Ok(())
    };
    for step in steps {
        match step {
            Step::Delete { derived, .. } => {
                remove(derived)?;
                remove_sidecars(derived)?;
            }
            Step::Rename { derived, real } => {
                std::fs::rename(derived, real).map_err(|err| {
                    anyhow::anyhow!("failed to rename {} to {}: {err}", derived.display(), real.display())
                })?;
                remove_sidecars(derived)?;
            }
            Step::WriteMeta { archive, source } => {
                archive_store::write_meta(archive, &ArchiveRef::new(source, None))?;
            }
            Step::DeleteSidecar { sidecar } => remove(sidecar)?,
            Step::Conflict { .. } | Step::Unknown { .. } => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn duplicates_are_deleted_singletons_renamed_and_conflicts_left() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        // A hard-linked second name for a real-named file.
        let real = write(d, "Real Mod-1-0.zip", b"same");
        std::fs::hard_link(&real, d.join("mod_0_1.zip")).unwrap();
        write(d, "mod_0_1.zip.orig-name", b"Real Mod-1-0.zip");
        // The only copy, under its derived name.
        write(d, "solo_0_2.7z", b"only");
        write(d, "solo_0_2.7z.orig-name", b"Solo-2.7z");
        // Two derived names for one file (a renamed mod), real name absent.
        write(d, "old name_0_3.zip", b"twin");
        write(d, "old name_0_3.zip.orig-name", b"Twin-3.zip");
        write(d, "new name_0_3.zip", b"twin");
        write(d, "new name_0_3.zip.orig-name", b"Twin-3.zip");
        // Real name taken by different bytes.
        write(d, "Clash.zip", b"one thing");
        write(d, "clash_0_4.zip", b"another");
        write(d, "clash_0_4.zip.orig-name", b"Clash.zip");
        // A sidecar whose archive is gone.
        write(d, "gone_0_5.orig-name", b"Gone.zip");

        let steps = plan_tidy(d, None).unwrap();
        apply(&steps).unwrap();

        let mut left: Vec<String> = std::fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "Clash.zip",
                "Real Mod-1-0.zip",
                "Solo-2.7z",
                "Twin-3.zip",
                "clash_0_4.zip",
                "clash_0_4.zip.orig-name",
            ]
        );
        assert!(steps.iter().any(|s| matches!(s, Step::Conflict { .. })));
    }

    #[test]
    fn nothing_changes_without_apply() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "solo_0_2.7z", b"only");
        write(dir.path(), "solo_0_2.7z.orig-name", b"Solo-2.7z");
        let steps = plan_tidy(dir.path(), None).unwrap();
        assert_eq!(steps.len(), 1);
        assert!(dir.path().join("solo_0_2.7z").exists());
    }
}
