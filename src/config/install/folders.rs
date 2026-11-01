//! Which folder in `mods/` holds each mod, when one MO2 instance has several
//! profiles installed from lists (the same list with other answers, say).
//!
//! MO2's profiles share the instance's `mods/` folder: each profile's
//! `modlist.txt` names the folders it uses. So a mod whose definition is the
//! same in two profiles is installed once and used by both, and one whose
//! definition differs -- another FOMOD choice, an action only one set of
//! answers asks for, a plugin only one profile's merge hides -- needs a
//! folder of its own in each. The rule this module keeps is that an install
//! never changes a folder another profile uses: it shares a folder whose
//! contents are exactly what it would install, reinstalls its own folder only
//! when no other profile uses it, and otherwise installs into a new folder,
//! `<id> [<profile>]`.
//!
//! What each profile has where is in its `install_manifest.json`
//! (`installed_path`, relative to `mods/`); a profile installed before this
//! module existed has every mod in `mods/<id>`.

use super::InstallSettings;
use super::manifest::{InstallManifest, InstalledMod};
use crate::config::schema::{ModType, PersonalizedMod, PersonalizedPlan};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

/// Where the mods of the profile being installed are, by id, relative to
/// `mods/`: what its manifest records.
#[derive(Debug, Clone, Default)]
pub struct ModFolders {
    folders: HashMap<String, String>,
}

impl ModFolders {
    /// What the current profile's manifest records.
    pub fn load(settings: &InstallSettings) -> Self {
        let path = super::manifest::get_install_manifest_path(settings);
        let folders = super::manifest::load_install_manifest(&path)
            .ok()
            .flatten()
            .map(|manifest| {
                manifest
                    .installed_mods
                    .into_iter()
                    .filter(|entry| !entry.installed_path.is_empty())
                    .map(|entry| (entry.id, entry.installed_path))
                    .collect()
            })
            .unwrap_or_default();
        Self { folders }
    }

    /// The folder's name in `mods/` for mod `id`: its own, as recorded, or
    /// the id.
    pub fn name<'a>(&'a self, id: &'a str) -> &'a str {
        self.folders.get(id).map(String::as_str).unwrap_or(id)
    }

    /// The folder holding mod `id`.
    pub fn dir(&self, settings: &InstallSettings, id: &str) -> PathBuf {
        settings.mods_dir.join(self.name(id))
    }
}

/// The folder holding mod `id` for the profile being installed.
pub fn mod_dir(settings: &InstallSettings, id: &str) -> PathBuf {
    ModFolders::load(settings).dir(settings, id)
}

/// One mod folder another profile uses, and what is in it.
#[derive(Debug, Clone)]
struct Use {
    profile: String,
    folder: String,
    hash: String,
    /// Its plugins that profile's merges renamed to `.mohidden`.
    hidden: BTreeSet<String>,
}

/// The mod folders the instance's other profiles use, from their manifests.
#[derive(Debug, Default)]
pub(crate) struct OtherProfiles {
    uses: HashMap<String, Vec<Use>>,
}

impl OtherProfiles {
    pub(crate) fn load(settings: &InstallSettings) -> Self {
        let Some(instance) = settings.mo2_instance_dir.as_deref() else {
            return Self::default();
        };
        let Ok(entries) = std::fs::read_dir(crate::config::mo2::layout::profiles_dir(instance))
        else {
            return Self::default();
        };
        let mut uses: HashMap<String, Vec<Use>> = HashMap::new();
        for entry in entries.flatten() {
            let profile = entry.file_name().to_string_lossy().into_owned();
            if profile == settings.profile_name {
                continue;
            }
            let path = entry.path().join("install_manifest.json");
            let Ok(Some(manifest)) = super::manifest::load_install_manifest(&path) else {
                continue;
            };
            for (id, folder, hash, hidden) in folders_of(&manifest) {
                uses.entry(id).or_default().push(Use {
                    profile: profile.clone(),
                    folder,
                    hash,
                    hidden,
                });
            }
        }
        Self { uses }
    }

    /// The other profiles using folder `folder` (of mod `id`).
    fn users(&self, id: &str, folder: &str) -> Vec<&str> {
        self.uses
            .get(id)
            .into_iter()
            .flatten()
            .filter(|other| other.folder.eq_ignore_ascii_case(folder))
            .map(|other| other.profile.as_str())
            .collect()
    }

    /// Whether any other profile uses a folder of this name, for any mod.
    fn taken(&self, folder: &str) -> bool {
        self.uses
            .values()
            .flatten()
            .any(|other| other.folder.eq_ignore_ascii_case(folder))
    }

    /// A folder another profile has mod `id` in, installed exactly as
    /// `hash` and `hidden` say.
    fn same(&self, id: &str, hash: &str, hidden: &BTreeSet<String>) -> Option<&Use> {
        self.uses
            .get(id)?
            .iter()
            .find(|other| other.hash == hash && &other.hidden == hidden)
    }
}

/// Each mod a manifest records: its id, folder, definition and what its
/// merges hid in it.
fn folders_of(manifest: &InstallManifest) -> Vec<(String, String, String, BTreeSet<String>)> {
    manifest
        .installed_mods
        .iter()
        .map(|entry| {
            let folder = folder_of(entry);
            let hidden = manifest
                .hidden_plugins
                .iter()
                .filter(|h| h.installed_path.eq_ignore_ascii_case(&folder))
                .map(|h| h.plugin.to_ascii_lowercase())
                .collect();
            (
                entry.id.clone(),
                folder,
                entry.definition_hash.clone(),
                hidden,
            )
        })
        .collect()
}

fn folder_of(entry: &InstalledMod) -> String {
    if entry.installed_path.is_empty() {
        entry.id.clone()
    } else {
        entry.installed_path.clone()
    }
}

/// The plugins of mod `id` the plan's merges hide.
pub(crate) fn hidden_by_plan(plan: &PersonalizedPlan, id: &str) -> BTreeSet<String> {
    plan.merges()
        .filter(|(_, merge)| merge.hide_sources)
        .flat_map(|(_, merge)| merge.sources.iter())
        .filter(|source| source.mod_id == id)
        .map(|source| source.plugin.to_ascii_lowercase())
        .collect()
}

/// Where mod `entry` goes for this profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Place {
    /// Into this folder (relative to `mods/`), which this profile alone
    /// uses: as before, if what is there is current, or installed afresh.
    Own(String),
    /// Another profile already has it in this folder exactly as this one
    /// would install it: use that, and install nothing.
    Shared(String),
}

/// Where mod `entry` (definition `hash`) goes for the profile being
/// installed, given where it was (`previous`) and what the other profiles
/// use.
pub(crate) fn place(
    entry: &PersonalizedMod,
    hash: &str,
    hidden: &BTreeSet<String>,
    previous: Option<&InstalledMod>,
    others: &OtherProfiles,
    settings: &InstallSettings,
) -> Place {
    let own = previous.map(folder_of);
    // A merge's output depends on the load order and its sources' contents,
    // which its definition does not say, so one is never shared.
    let shareable = entry.mod_type != Some(ModType::Merge);
    if shareable
        && let Some(same) = others.same(&entry.id, hash, hidden)
        && settings.mods_dir.join(&same.folder).is_dir()
    {
        return Place::Shared(same.folder.clone());
    }
    // This profile's own folder, if no other profile uses it.
    if let Some(own) = own
        && others.users(&entry.id, &own).is_empty()
    {
        return Place::Own(own);
    }
    // A new folder: the mod's own name if nobody has it, or one named for
    // this profile.
    let plain = entry.id.clone();
    if !others.taken(&plain) && previous.is_none_or(|p| folder_of(p) == plain) {
        return Place::Own(plain);
    }
    let mut n = 1;
    loop {
        let name = if n == 1 {
            format!("{} [{}]", entry.id, settings.profile_name)
        } else {
            format!("{} [{} {n}]", entry.id, settings.profile_name)
        };
        if !others.taken(&name) {
            return Place::Own(name);
        }
        n += 1;
    }
}

/// The plugins other profiles of the instance at `instance` have hidden, as
/// (folder, plugin) in lower case: what un-hiding this profile's must leave
/// alone, in a folder they share.
pub(crate) fn hidden_by_others(
    instance: &std::path::Path,
    profile: &str,
) -> std::collections::HashSet<(String, String)> {
    let Ok(entries) = std::fs::read_dir(crate::config::mo2::layout::profiles_dir(instance)) else {
        return Default::default();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy() != profile)
        .filter_map(|entry| {
            super::manifest::load_install_manifest(&entry.path().join("install_manifest.json"))
                .ok()
                .flatten()
        })
        .flat_map(|manifest| manifest.hidden_plugins)
        .map(|h| {
            (
                h.installed_path.to_ascii_lowercase(),
                h.plugin.to_ascii_lowercase(),
            )
        })
        .collect()
}
