//! What a mod puts into the virtual file system, without installing it.
//!
//! The primitive `conflicts_with` needs: "which files would mod X contribute?"
//! answered for a mod that may not be on disk yet. Section-by-section building
//! makes that the normal case -- Part 9 has to know what Part 24's mods will
//! bring.
//!
//! Two sources, in order of authority:
//!
//! * A mod already installed *is* the answer. Its folder is what MO2 sees.
//! * Anything else is computed by [`layout::plan_archive`] -- the same
//!   dispatch `install` uses, so an index cannot disagree with what install
//!   really does.
//!
//! Either way BSAs are opened and their contents counted in. MO2's VFS sees
//! inside them, and the whole reason conflict *direction* has to be declared
//! rather than derived is that loose files beat packed ones regardless of
//! priority -- so an index blind to packed files would miss precisely the
//! conflicts these rows are about.

use super::InstallSettings;
use crate::archive::ArchiveFilters;
use crate::config::download;
use crate::config::install::layout::{self, bain::list_relative_paths, EntryReader};
use crate::config::schema::{ModType, PersonalizedMod};
use crate::util::fs::staging_dir_for;
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

/// Every path a mod contributes, lowercased and `/`-separated.
///
/// Lowercased because comparison is against another mod's paths and the two
/// sides come from different places -- an archive spells them as the author
/// typed them, a BSA stores them folded, and staged directories are folded on
/// the way in. `diff.rs` compares the same way.
pub(crate) fn staged_paths(
    mod_entry: &PersonalizedMod,
    installed_at: Option<&Path>,
    settings: &InstallSettings,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<BTreeSet<String>> {
    Ok(staged_paths_detailed(mod_entry, installed_at, settings, active_plugins)?.visible)
}

/// What a mod contributes, plus what it *would* contribute if nothing were
/// hidden.
///
/// The two differ once a `file_hide` has run, and the gap is worth reporting
/// rather than swallowing: asking `mudcrab conflicts` about a row that has
/// already been installed otherwise answers "nothing", which reads as "the row
/// was wrong" when it means "the row already worked".
pub(crate) struct StagedPaths {
    /// In the virtual file system: what actually conflicts with anything.
    pub(crate) visible: BTreeSet<String>,
    /// Present on disk but hidden, so out of the VFS.
    pub(crate) hidden: BTreeSet<String>,
}

pub(crate) fn staged_paths_detailed(
    mod_entry: &PersonalizedMod,
    installed_at: Option<&Path>,
    settings: &InstallSettings,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<StagedPaths> {
    if let Some(dir) = installed_at.filter(|dir| dir.is_dir()) {
        return from_installed_folder(dir);
    }
    Ok(StagedPaths {
        visible: from_plan(mod_entry, settings, active_plugins)?,
        hidden: BTreeSet::new(),
    })
}

/// The paths an installed mod actually has, which is what MO2 sees.
fn from_installed_folder(dir: &Path) -> anyhow::Result<StagedPaths> {
    let mut out = BTreeSet::new();
    let mut hidden = BTreeSet::new();
    for relative in list_relative_paths(dir)? {
        // A hidden file is out of the VFS, so it cannot conflict with anything
        // -- but it is kept separately, because "hidden" and "absent" are very
        // different answers to give someone asking what a mod provides.
        if let Some(unhidden) = relative.to_lowercase().strip_suffix(".mohidden") {
            hidden.insert(unhidden.to_string());
            continue;
        }
        if is_bsa(&relative) {
            let bytes = std::fs::read(dir.join(&relative))
                .map_err(|err| anyhow::anyhow!("failed to read {relative}: {err}"))?;
            out.extend(bsa_paths(&bytes, &relative)?);
            continue;
        }
        out.insert(relative.to_lowercase());
    }
    Ok(StagedPaths {
        visible: out,
        hidden,
    })
}

/// The paths a mod would have, from its archives' entry lists.
fn from_plan(
    mod_entry: &PersonalizedMod,
    settings: &InstallSettings,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<BTreeSet<String>> {
    reject_unmodelled(mod_entry)?;

    let mut out = BTreeSet::new();
    for (archive_index, archive) in mod_entry.archives.iter().enumerate() {
        if !archive.build.is_empty() {
            anyhow::bail!(
                "cannot predict the files mod '{}' contributes: it assembles archive {} from \
                 build layers, and that is only modelled at install time. Install it first, or \
                 name a different mod.",
                mod_entry.id,
                archive_index
            );
        }

        let path = archive.path.as_deref().unwrap_or_default();
        let cache_name = download::cache_file_name(&mod_entry.id, archive_index, path);
        let source = download::resolve_cache_path(&settings.cache_dir, &cache_name)
            .or_else(|| {
                download::find_local_archive(
                    archive.file_name.as_deref(),
                    &settings.archive_search_paths,
                )
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "cannot predict the files mod '{}' contributes: archive {} is not cached. \
                     Run `mudcrab download` first.",
                    mod_entry.id,
                    archive_index
                )
            })?;

        // `game_root_files` land outside the mod folder, so they are not part
        // of what this mod contributes to the VFS -- and install excludes them
        // from the mod folder for the same reason.
        let effective_exclude: Vec<String> = archive
            .exclude
            .iter()
            .chain(archive.game_root_files.iter())
            .cloned()
            .collect();
        let filters = ArchiveFilters::new(&archive.include, &effective_exclude)?;

        let paths = crate::archive::list_archive_paths(&source)?;
        let scratch = staging_dir_for(&settings.mods_dir)?;
        let reader = EntryReader::new(&source, scratch.clone());
        let plan = layout::plan_archive(
            &source.display().to_string(),
            &paths,
            &reader,
            &mod_entry.id,
            archive,
            &filters,
            active_plugins,
        );
        let _ = std::fs::remove_dir_all(&scratch);
        let plan = plan?;

        let prefix = archive
            .target_subdir
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!("{}/", value.replace('\\', "/").trim_matches('/')))
            .unwrap_or_default()
            .to_lowercase();

        for destination in plan.destinations() {
            let staged = format!("{prefix}{destination}").to_lowercase();
            if is_bsa(&staged) {
                let entry = plan
                    .files
                    .iter()
                    .find(|file| file.destination.eq_ignore_ascii_case(destination))
                    .map(|file| file.source.as_str())
                    .unwrap_or(destination);
                out.extend(bsa_paths_from_archive(&source, entry, &staged)?);
                continue;
            }
            out.insert(staged);
        }
    }

    Ok(out)
}

/// Refuse rather than guess where an action would change the answer.
///
/// A quiet under-count here looks exactly like "no conflicts", which is the
/// failure mode that cost the first pass at Part 9. Packing and unpacking are
/// fine: BSAs are opened either way, so moving a file between loose and packed
/// leaves the set alone.
fn reject_unmodelled(mod_entry: &PersonalizedMod) -> anyhow::Result<()> {
    if mod_entry.mod_type == Some(ModType::BuildFromFiles) {
        anyhow::bail!(
            "cannot predict the files mod '{}' contributes: it is built from files already on \
             disk. Install it first, or name a different mod.",
            mod_entry.id
        );
    }

    if let Some(archive) = mod_entry
        .archives
        .iter()
        .find(|archive| archive.inner_archive.is_some())
    {
        anyhow::bail!(
            "cannot predict the files mod '{}' contributes: its content is inside '{}', a second \
             archive that has to be unpacked before its layout can be read. Install it first, or \
             name a different mod.",
            mod_entry.id,
            archive.inner_archive.as_deref().unwrap_or_default(),
        );
    }

    let changes_the_set: Vec<&str> = mod_entry
        .actions
        .iter()
        .map(|action| action.name())
        .filter(|name| {
            matches!(
                *name,
                "file_prune" | "file_hide" | "file_move" | "create_dummy_plugin"
            )
        })
        .collect();

    if !changes_the_set.is_empty() {
        anyhow::bail!(
            "cannot predict the files mod '{}' contributes: its '{}' action{} change{} what it \
             stages, and actions are only modelled at install time. Install it first, or name a \
             different mod.",
            mod_entry.id,
            changes_the_set.join("', '"),
            if changes_the_set.len() == 1 { "" } else { "s" },
            if changes_the_set.len() == 1 { "s" } else { "" },
        );
    }

    Ok(())
}

fn is_bsa(path: &str) -> bool {
    path.to_lowercase().ends_with(".bsa")
}

/// A BSA's file table, without decompressing a single payload.
///
/// BSAs store `folder\file`, Oblivion's own spelling, so the separator has to
/// be normalised before these can be compared with anything that came off a
/// filesystem. Skipping that turns every packed file into a path nothing will
/// ever match -- which reads as "this mod conflicts with nothing".
fn bsa_paths(bytes: &[u8], label: &str) -> anyhow::Result<Vec<String>> {
    let bsa = crate::bsa::Bsa::parse(bytes)
        .map_err(|err| anyhow::anyhow!("failed to read BSA {label}: {err}"))?;
    Ok(bsa
        .paths()
        .map(|path| path.replace('\\', "/").to_lowercase())
        .collect())
}

fn bsa_paths_from_archive(
    source: &Path,
    entry: &str,
    label: &str,
) -> anyhow::Result<Vec<String>> {
    let scratch = staging_dir_for(source)?;
    let result = (|| -> anyhow::Result<Vec<String>> {
        let reader = EntryReader::new(source, scratch.clone());
        bsa_paths(&reader.read(entry)?, label)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_paths_come_back_comparable_with_loose_ones() {
        // BSAs store `folder\file`, Oblivion's spelling. Left alone, every
        // packed file becomes a path that no filesystem-derived path will ever
        // equal -- and the mods this matters for are exactly the ones that pack
        // their assets, so the failure reads as "conflicts with nothing".
        let dir = tempfile::tempdir().expect("tempdir");
        let nested = dir.path().join("Textures/Architecture");
        std::fs::create_dir_all(&nested).expect("dirs");
        std::fs::write(nested.join("Wall01.dds"), b"DDS").expect("write");

        let filters = ArchiveFilters::new(&[] as &[String], &[] as &[String]).expect("filters");
        let bsa = crate::bsa::Bsa::from_directory(dir.path(), &filters).expect("pack");
        let bytes = bsa.to_bytes().expect("bytes");

        assert_eq!(
            bsa_paths(&bytes, "test.bsa").expect("read"),
            ["textures/architecture/wall01.dds"]
        );
    }

    #[test]
    fn a_hidden_file_is_not_in_the_virtual_file_system() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("meshes")).expect("dirs");
        std::fs::write(dir.path().join("meshes/a.nif"), b"NIF").expect("write");
        std::fs::write(dir.path().join("meshes/b.nif.mohidden"), b"NIF").expect("write");

        let paths = from_installed_folder(dir.path()).expect("walk");
        assert_eq!(paths.visible.into_iter().collect::<Vec<_>>(), ["meshes/a.nif"]);
        // Hidden is not the same as absent: the caller needs to be able to say
        // "already hidden" rather than "does not conflict".
        assert_eq!(paths.hidden.into_iter().collect::<Vec<_>>(), ["meshes/b.nif"]);
    }
}
