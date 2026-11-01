//! A plugin a step by hand makes -- the Bashed Patch Wrye Bash builds -- moved
//! out of MO2's `overwrite/` into a mod folder of the profile's own.
//!
//! A tool run from MO2 writes into `overwrite/`, which belongs to the whole
//! instance, not to a profile: with the list installed into two profiles, the
//! second one's Bashed Patch replaces the first's there. So once a step's
//! plugin turns up in `overwrite/`, it is moved into `mods/<plugin> [<profile>]`
//! (with the report Wrye Bash writes beside it, in `Docs/`), which the
//! profile's `modlist.txt` lists first, above everything else, as MO2 lists a
//! patch made from overwrite. The folders moved are recorded in the profile's
//! `mudcrab-by-hand.json`, so each install keeps them in the mod list.
//!
//! Nothing is moved while MO2 has the instance open: it keeps its own copy of
//! the mod list, and the files under `overwrite/` are in its view.

use super::InstallSettings;
use crate::config::schema::PersonalizedPlan;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A step's plugin, moved into a mod folder of the profile's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByHandMod {
    pub plugin: String,
    /// Its folder in `mods/`.
    pub folder: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Record {
    #[serde(default)]
    mods: Vec<ByHandMod>,
}

/// What looking in `overwrite/` came to.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Collected {
    /// Moved now.
    pub moved: Vec<ByHandMod>,
    /// In `overwrite/`, and left there because MO2 is open.
    pub waiting: Vec<String>,
}

fn record_path(settings: &InstallSettings) -> Option<PathBuf> {
    Some(crate::config::mo2::mo2_profile_dir(settings)?.join("mudcrab-by-hand.json"))
}

/// The step plugins already moved into this profile's own folders.
pub fn recorded(settings: &InstallSettings) -> Vec<ByHandMod> {
    record_path(settings)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<Record>(&text).ok())
        .map(|record| record.mods)
        .unwrap_or_default()
}

/// The folder a step's `plugin` is moved into, for this profile.
pub fn folder_for(plugin: &str, settings: &InstallSettings) -> String {
    let stem = Path::new(plugin)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| plugin.to_string());
    format!("{stem} [{}]", settings.profile_name)
}

/// Move each step's plugin found in `overwrite/` into this profile's own
/// folder for it, and list that folder first in the profile's mod list.
pub fn collect(plan: &PersonalizedPlan, settings: &InstallSettings) -> anyhow::Result<Collected> {
    let Some(instance) = settings.mo2_instance_dir.as_deref() else {
        return Ok(Collected::default());
    };
    let overwrite = crate::config::mo2::layout::overwrite_dir(instance);
    let found: Vec<(String, PathBuf)> = plan
        .manual_steps
        .iter()
        .filter_map(|step| step.done_when_plugin.clone())
        .filter_map(|plugin| child(&overwrite, &plugin).map(|path| (plugin, path)))
        .collect();
    if found.is_empty() || settings.dry_run {
        return Ok(Collected::default());
    }
    if !crate::config::mo2::running::running_in(instance).is_empty() {
        return Ok(Collected {
            moved: Vec::new(),
            waiting: found.into_iter().map(|(plugin, _)| plugin).collect(),
        });
    }

    let mut record = recorded(settings);
    let mut moved = Vec::new();
    for (plugin, source) in found {
        let folder = folder_for(&plugin, settings);
        let dest = settings.mods_dir.join(&folder);
        std::fs::create_dir_all(&dest)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", dest.display()))?;
        let name = source.file_name().expect("a file in overwrite/");
        replace(&source, &dest.join(name))?;
        // Wrye Bash's report on what it put in the patch, beside it.
        let stem = Path::new(&plugin)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_ascii_lowercase());
        if let (Some(stem), Some(docs)) = (stem, child(&overwrite, "Docs")) {
            for entry in std::fs::read_dir(&docs).into_iter().flatten().flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let own = Path::new(&name)
                    .file_stem()
                    .is_some_and(|s| s.to_string_lossy().to_ascii_lowercase() == stem);
                if own && entry.path().is_file() {
                    std::fs::create_dir_all(dest.join("Docs"))?;
                    replace(&entry.path(), &dest.join("Docs").join(&name))?;
                }
            }
        }
        tracing::info!(plugin = %plugin, folder = %folder, "by hand: moved out of overwrite/");
        record.retain(|m| !m.plugin.eq_ignore_ascii_case(&plugin));
        let entry = ByHandMod { plugin, folder };
        record.push(entry.clone());
        moved.push(entry);
    }

    if let Some(path) = record_path(settings) {
        let text = serde_json::to_string_pretty(&Record { mods: record })?;
        std::fs::write(&path, text)
            .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))?;
    }
    if let Some(profile) = crate::config::mo2::mo2_profile_dir(settings) {
        let path = profile.join("modlist.txt");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        std::fs::write(&path, list_first(&text, &moved))
            .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))?;
    }
    Ok(Collected {
        moved,
        waiting: Vec::new(),
    })
}

/// `modlist.txt` with each of `mods` first, ticked, and nowhere else. MO2
/// lists the highest priority first.
pub fn list_first(modlist: &str, mods: &[ByHandMod]) -> String {
    let mut lines: Vec<String> = mods.iter().map(|m| format!("+{}", m.folder)).collect();
    lines.extend(
        modlist
            .lines()
            .filter(|line| {
                !mods
                    .iter()
                    .any(|m| line.get(1..).is_some_and(|name| name == m.folder))
            })
            .map(str::to_string),
    );
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// `dir`'s entry named `name`, in any case.
fn child(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
        })
        .map(|entry| entry.path())
}

/// Move `from` to `to`, over whatever is there.
fn replace(from: &Path, to: &Path) -> anyhow::Result<()> {
    if to.exists() {
        std::fs::remove_file(to)
            .map_err(|err| anyhow::anyhow!("failed to replace {}: {err}", to.display()))?;
    }
    std::fs::rename(from, to).map_err(|err| {
        anyhow::anyhow!(
            "failed to move {} to {}: {err}",
            from.display(),
            to.display()
        )
    })
}
