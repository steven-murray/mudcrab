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
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
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
                out.insert(
                    name.to_ascii_lowercase(),
                    (path.to_string(), archive.file_name.clone()),
                );
            }
            for (layer_index, layer) in archive.build.iter().enumerate() {
                let safe_id: String = entry
                    .id
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
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

fn is_sidecar_name(name: &str) -> bool {
    name.ends_with(".meta") || name.ends_with(".orig-name") || name.ends_with(".part")
}

fn valid_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0']))
        .then_some(name)
}

/// Whether a recorded "real" name is only the derived name again, with or
/// without its extension -- what an older mudcrab wrote for a download whose
/// server gave no usable name.
fn is_derived(real: &str, derived: &str) -> bool {
    let real = real.to_ascii_lowercase();
    let derived = derived.to_ascii_lowercase();
    real == derived
        || derived
            .rsplit_once('.')
            .is_some_and(|(stem, _)| real == stem)
}

/// The archive extension a name ends in, if any.
fn archive_extension(name: &str) -> Option<&str> {
    let lower = name.to_ascii_lowercase();
    [".tar.gz", ".7z", ".zip", ".rar", ".tgz", ".tar"]
        .into_iter()
        .find(|ext| lower.ends_with(ext))
        .map(|ext| &name[name.len() - ext.len()..])
}

/// Sources whose real name nothing on disk records: what `plan_tidy` would
/// need `known_names` to name them.
pub fn unnamed_sources(dir: &Path, plan: &PersonalizedPlan) -> anyhow::Result<Vec<String>> {
    let legacy = legacy_names(plan);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", dir.display()))?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((source, file_name)) = source_of(&name, &legacy) else {
            continue;
        };
        if file_name.as_deref().and_then(valid_name).is_some() {
            continue;
        }
        let stem = name
            .rsplit_once('.')
            .map_or(name.as_str(), |(stem, _)| stem);
        let recorded = [format!("{name}.orig-name"), format!("{stem}.orig-name")]
            .iter()
            .filter_map(|sidecar| std::fs::read_to_string(dir.join(sidecar)).ok())
            .filter_map(|text| valid_name(&text).map(str::to_string))
            .any(|real| !is_derived(&real, &name));
        if !recorded && source.starts_with("nexus:") && !out.contains(source) {
            out.push(source.clone());
        }
    }
    Ok(out)
}

/// Whether a filename has the shape of an old derived name:
/// `{id}_{index}_{file id}`, `{id}_{index}_manual_{...}` or
/// `{id}_{index}_layer{n}_{...}`, with or without an extension.
fn looks_derived(name: &str) -> bool {
    let stem = archive_extension(name).map_or(name, |ext| &name[..name.len() - ext.len()]);
    let parts: Vec<&str> = stem.rsplitn(3, '_').collect();
    // rsplitn from the right: [tail, index, id] only for the plain file-id shape.
    if parts.len() == 3
        && !parts[2].is_empty()
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && !parts[1].is_empty()
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && !parts[0].is_empty()
    {
        return true;
    }
    // `_<index>_manual_` or `_<index>_layer<n>_` anywhere.
    let bytes = stem.as_bytes();
    stem.match_indices('_').any(|(i, _)| {
        let rest = &stem[i + 1..];
        let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        digits > 0
            && bytes.get(i + 1 + digits) == Some(&b'_')
            && (rest[digits + 1..].starts_with("manual_")
                || rest[digits + 1..].starts_with("layer"))
    })
}

/// Work out what tidying `dir` would do. `known_names` maps a source to its
/// real filename, from somewhere other than the folder (the Nexus API).
pub fn plan_tidy(
    dir: &Path,
    plan: Option<&PersonalizedPlan>,
    known_names: &HashMap<String, String>,
) -> anyhow::Result<Vec<Step>> {
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
                steps.push(Step::DeleteSidecar {
                    sidecar: dir.join(name),
                });
            }
            continue;
        }
        if name.ends_with(".part") {
            continue;
        }
        if let Some(archive) = name.strip_suffix(".meta") {
            // A .meta left behind by a derived name that is gone.
            if !present.contains(archive) && source_of(archive, &legacy).is_some() {
                steps.push(Step::DeleteSidecar {
                    sidecar: dir.join(name),
                });
            }
            continue;
        }

        let path = dir.join(name);
        let sidecar_name = |base: &str| format!("{base}.orig-name");
        let sidecar = [
            name.as_str(),
            name.rsplit_once('.')
                .map_or(name.as_str(), |(stem, _)| stem),
        ]
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
            .filter(|real| !is_derived(real, name))
            .or_else(|| {
                source
                    .and_then(|(_, file_name)| file_name.as_deref())
                    .and_then(valid_name)
                    .map(str::to_string)
            })
            .or_else(|| {
                source
                    .and_then(|(src, _)| known_names.get(src))
                    .and_then(|n| valid_name(n))
                    .map(str::to_string)
            })
            // Never lose the extension: MO2 lists archives by it.
            .map(
                |real| match (archive_extension(&real), archive_extension(name)) {
                    (None, Some(ext)) => format!("{real}{ext}"),
                    (None, None) => format!("{real}{}", archive_store::detect_archive_ext(&path)),
                    _ => real,
                },
            );

        // Never leave an archive without an extension (MO2 lists archives by
        // it): failing everything else, the file's own magic bytes say.
        let real = real.or_else(|| {
            if archive_extension(name).is_some() {
                return None;
            }
            let ext = archive_store::detect_archive_ext(&path);
            (!ext.is_empty()).then(|| format!("{name}{ext}"))
        });
        let Some(real) = real else {
            steps.push(Step::Unknown {
                derived: path.clone(),
            });
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
                Step::Delete {
                    derived: path.clone(),
                    real: real_path.clone(),
                }
            } else {
                Step::Conflict {
                    derived: path.clone(),
                    real: real_path.clone(),
                }
            })
        } else if real_path.exists() {
            Some(if same_file(&path, &real_path) {
                Step::Delete {
                    derived: path.clone(),
                    real: real_path.clone(),
                }
            } else {
                Step::Conflict {
                    derived: path.clone(),
                    real: real_path.clone(),
                }
            })
        } else {
            claimed.insert(lower, path.clone());
            Some(Step::Rename {
                derived: path.clone(),
                real: real_path.clone(),
            })
        };
        let settled = !matches!(step, Some(Step::Conflict { .. }));
        if let Some(step) = step {
            steps.push(step);
        }
        if settled && let Some((src, _)) = source {
            sourced.push((real_path, src.clone()));
        }
    }

    // Derived names nothing above recognised -- an old mod id the plan no
    // longer has, whose sidecar is gone -- that are byte-for-byte a real-named
    // archive here. Deleting one loses nothing, by construction.
    let handled: std::collections::HashSet<PathBuf> = steps
        .iter()
        .filter_map(|step| match step {
            Step::Delete { derived, .. }
            | Step::Rename { derived, .. }
            | Step::Conflict { derived, .. }
            | Step::Unknown { derived } => Some(derived.clone()),
            _ => None,
        })
        .collect();
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for name in &names {
        if is_sidecar_name(name) || looks_derived(name) {
            continue;
        }
        let path = dir.join(name);
        if let Ok(meta) = path.metadata() {
            by_size.entry(meta.len()).or_default().push(path);
        }
    }
    for name in &names {
        let path = dir.join(name);
        if is_sidecar_name(name) || !looks_derived(name) || handled.contains(&path) {
            continue;
        }
        let Ok(meta) = path.metadata() else { continue };
        if let Some(twin) = by_size.get(&meta.len()).and_then(|same_size| {
            same_size
                .iter()
                .find(|candidate| same_file(&path, candidate))
        }) {
            steps.push(Step::Delete {
                derived: path,
                real: twin.clone(),
            });
        } else if archive_extension(name).is_none() {
            // Unique, so kept -- but with an extension, or MO2 never lists it.
            let ext = archive_store::detect_archive_ext(&path);
            let renamed = dir.join(format!("{name}{ext}"));
            if !ext.is_empty() && !renamed.exists() {
                steps.push(Step::Rename {
                    derived: path,
                    real: renamed,
                });
            }
        }
    }

    for (archive, source) in sourced {
        let is_meta_worthy = source.starts_with("nexus:")
            || source.starts_with("http://")
            || source.starts_with("https://");
        let moved_with_rename = steps.iter().any(|s| matches!(s,
            Step::Rename { derived, real } if *real == archive && archive_store::meta_path(derived).is_file()));
        let already = moved_with_rename
            || archive_store::meta_is_informative(&archive)
            || steps
                .iter()
                .any(|s| matches!(s, Step::WriteMeta { archive: a, .. } if *a == archive));
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
        for sidecar in [
            format!("{name}.orig-name"),
            format!("{}.orig-name", stem.display()),
        ] {
            let sidecar = PathBuf::from(sidecar);
            if sidecar.is_file() {
                remove(&sidecar)?;
            }
        }
        Ok(())
    };
    for step in steps {
        match step {
            Step::Delete { derived, real } => {
                remove(derived)?;
                remove_sidecars(derived)?;
                // A derived name's .meta is kept when it identifies the file
                // and the real name's does not (MO2's fileID=0 placeholders).
                let meta = archive_store::meta_path(derived);
                if meta.is_file() {
                    if !archive_store::meta_is_informative(real) {
                        std::fs::rename(&meta, archive_store::meta_path(real)).map_err(|err| {
                            anyhow::anyhow!("failed to move {}: {err}", meta.display())
                        })?;
                    } else {
                        remove(&meta)?;
                    }
                }
            }
            Step::Rename { derived, real } => {
                std::fs::rename(derived, real).map_err(|err| {
                    anyhow::anyhow!(
                        "failed to rename {} to {}: {err}",
                        derived.display(),
                        real.display()
                    )
                })?;
                remove_sidecars(derived)?;
                // A .meta follows its archive, unless the new name has one.
                let (old_meta, new_meta) = (
                    archive_store::meta_path(derived),
                    archive_store::meta_path(real),
                );
                if old_meta.is_file() {
                    if new_meta.exists() {
                        remove(&old_meta)?;
                    } else {
                        std::fs::rename(&old_meta, &new_meta).map_err(|err| {
                            anyhow::anyhow!("failed to move {}: {err}", old_meta.display())
                        })?;
                    }
                }
            }
            Step::WriteMeta { archive, source } => {
                archive_store::fill_meta(archive, &ArchiveRef::new(source, None))?;
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

        let steps = plan_tidy(d, None, &HashMap::new()).unwrap();
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
    fn a_recorded_name_that_is_only_the_derived_one_is_not_used() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "xobse_0_77.zip", b"x");
        write(dir.path(), "xobse_0_77.orig-name", b"xobse_0_77");
        let steps = plan_tidy(dir.path(), None, &HashMap::new()).unwrap();
        assert!(
            matches!(steps.as_slice(), [Step::Unknown { .. }]),
            "{steps:?}"
        );
    }

    #[test]
    fn a_name_from_elsewhere_keeps_the_archive_extension() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write(d, "solo_0_9.7z", b"x");
        write(d, "solo_0_9.orig-name", b"solo_0_9");
        let plan: PersonalizedPlan = serde_json::from_value(serde_json::json!({
            "schema_version": 1, "name": "t", "responses": {}, "mod_order": [],
            "selected_mods": [], "plugins": [],
            "mods": [{"id": "solo", "archives": [{"path": "nexus:oblivion/1/9",
                "download_handler": null, "layout": null, "data_folder": null,
                "target_subdir": null, "include": [], "exclude": []}]}]
        }))
        .unwrap();
        let names = HashMap::from([(
            "nexus:oblivion/1/9".to_string(),
            "Solo Mod-1-2-3".to_string(),
        )]);
        let steps = plan_tidy(d, Some(&plan), &names).unwrap();
        assert!(
            steps.contains(&Step::Rename {
                derived: d.join("solo_0_9.7z"),
                real: d.join("Solo Mod-1-2-3.7z")
            }),
            "{steps:?}"
        );
    }

    #[test]
    fn an_extensionless_archive_gets_one_from_its_magic_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write(d, "solo_0_9", &[0x50, 0x4B, 0x03, 0x04, 0, 0]);
        write(d, "solo_0_9.orig-name", b"solo_0_9");
        write(d, "solo_0_9.meta", b"[General]\nmodID=1\nfileID=9\n");
        let steps = plan_tidy(d, None, &HashMap::new()).unwrap();
        assert!(
            steps.contains(&Step::Rename {
                derived: d.join("solo_0_9"),
                real: d.join("solo_0_9.zip")
            }),
            "{steps:?}"
        );
        apply(&steps).unwrap();
        assert!(d.join("solo_0_9.zip").is_file() && d.join("solo_0_9.zip.meta").is_file());
        assert!(!d.join("solo_0_9.meta").exists());
    }

    #[test]
    fn derived_shapes_are_recognised_and_real_names_are_not() {
        for derived in [
            "Add Actor Values_0_88252",
            "mod_1_1000012318.7z",
            "Hesu_0_manual_Hesu_Mods.7z",
            "D_0_layer1_34632.7z",
        ] {
            assert!(looks_derived(derived), "{derived}");
        }
        for real in [
            "Better Letters Version 1_3-5392-1-3.zip",
            "1_Better Letters.zip",
            "WACv_1beta.7z",
            "Mod-1-2-3.7z",
        ] {
            assert!(!looks_derived(real), "{real}");
        }
    }

    #[test]
    fn an_unrecognised_derived_copy_of_a_real_archive_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write(d, "Real Mod-1-2-3.rar", b"same bytes");
        write(d, "Old Id_0_88252", b"same bytes");
        write(d, "Other Old Id_0_99", b"different");
        let steps = plan_tidy(d, None, &HashMap::new()).unwrap();
        assert_eq!(
            steps,
            [Step::Delete {
                derived: d.join("Old Id_0_88252"),
                real: d.join("Real Mod-1-2-3.rar")
            }]
        );
    }

    #[test]
    fn a_useful_meta_survives_its_duplicate_name_and_fills_a_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let real = write(d, "Real-1.zip", b"same");
        std::fs::hard_link(&real, d.join("mod_0_210177.zip")).unwrap();
        write(d, "mod_0_210177.zip.orig-name", b"Real-1.zip");
        write(
            d,
            "mod_0_210177.zip.meta",
            b"[General]\ngameName=SkyrimSE\nmodID=50129\nfileID=210177\n",
        );
        write(
            d,
            "Real-1.zip.meta",
            b"[General]\ngameName=Oblivion\nmodID=3\nfileID=0\n",
        );
        apply(&plan_tidy(d, None, &HashMap::new()).unwrap()).unwrap();
        let meta = std::fs::read_to_string(d.join("Real-1.zip.meta")).unwrap();
        assert!(meta.contains("fileID=210177"), "{meta}");
    }

    #[test]
    fn nothing_changes_without_apply() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "solo_0_2.7z", b"only");
        write(dir.path(), "solo_0_2.7z.orig-name", b"Solo-2.7z");
        let steps = plan_tidy(dir.path(), None, &HashMap::new()).unwrap();
        assert_eq!(steps.len(), 1);
        assert!(dir.path().join("solo_0_2.7z").exists());
    }
}
