//! `assets`: the meshes and sounds an installed instance's plugins name, and
//! whether the game would find them.
//!
//! Run on an installed MO2 instance, where everything is on disk: the game's
//! own `Data` (with its BSAs), each enabled mod's files (with the BSAs inside
//! them), and `overwrite/`. Reading a BSA here means reading its file table
//! only, so the whole check costs about as much as parsing the plugins.
//!
//! Only the paths plugins name directly are checked: models (`MODL`, and the
//! `MOD2`-`MOD4` world and female models of armour and clothing; a tree's is a
//! SpeedTree file under `Trees`) and sound files. Textures are named by the meshes, not the plugins, and would mean
//! reading every NIF. References the game's own plugins make to files that are
//! not there either are left out -- Oblivion has always shipped some -- so what
//! is reported is what the list added.

use crate::config::check::GAME_PLUGINS;
use crate::plugin::Plugin;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// A plugin's reference to a file the game would not find.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// `meshes/...` or `sound/...`, lowercased.
    pub path: String,
    /// The record naming it: signature, form id and editor id.
    pub record: String,
}

#[derive(Debug, Default)]
pub struct AssetReport {
    pub plugins_read: usize,
    pub references: usize,
    /// Missing files the game's own plugins already name, left out below.
    pub missing_in_game: usize,
    /// By plugin, in load order.
    pub missing: Vec<(String, Vec<Missing>)>,
}

/// Every file the game would see, lowercased and `/`-separated, and where
/// each active plugin is.
pub struct VirtualData {
    files: BTreeSet<String>,
    plugins: HashMap<String, PathBuf>,
}

impl VirtualData {
    /// The game's `Data`, then each enabled mod (`modlist.txt`, which lists
    /// the highest priority first), then `overwrite/`.
    pub fn of_instance(instance: &Path, profile: &str, game_dir: &Path) -> anyhow::Result<Self> {
        let mut data = Self {
            files: BTreeSet::new(),
            plugins: HashMap::new(),
        };
        let modlist =
            std::fs::read_to_string(instance.join("profiles").join(profile).join("modlist.txt"))
                .map_err(|err| {
                    anyhow::anyhow!("failed to read profile {profile}'s modlist.txt: {err}")
                })?;
        // Lowest priority first, so a later folder's plugin replaces an
        // earlier one's, as MO2's does.
        let mut folders = vec![game_dir.join("Data")];
        let enabled: Vec<&str> = modlist
            .lines()
            .filter_map(|line| line.trim().strip_prefix('+'))
            .collect();
        folders.extend(
            enabled
                .iter()
                .rev()
                .map(|name| instance.join("mods").join(name)),
        );
        folders.push(instance.join("overwrite"));
        for folder in folders.iter().filter(|f| f.is_dir()) {
            data.add_folder(folder)?;
        }
        Ok(data)
    }

    fn add_folder(&mut self, folder: &Path) -> anyhow::Result<()> {
        for relative in crate::config::install::layout::bain::list_relative_paths(folder)? {
            let lower = relative.replace('\\', "/").to_lowercase();
            // Mod Organizer's bookkeeping, hidden files, and what Root Builder
            // deploys beside `Oblivion.exe`: none of it is in `Data`.
            if lower == "meta.ini" || lower.ends_with(".mohidden") || lower.starts_with("root/") {
                continue;
            }
            if !lower.contains('/') && (lower.ends_with(".esp") || lower.ends_with(".esm")) {
                self.plugins.insert(lower.clone(), folder.join(&relative));
            }
            if !lower.contains('/') && lower.ends_with(".bsa") {
                let paths =
                    crate::bsa::reader::list_paths(&folder.join(&relative)).map_err(|err| {
                        anyhow::anyhow!(
                            "failed to read {}: {err}",
                            folder.join(&relative).display()
                        )
                    })?;
                self.files
                    .extend(paths.iter().map(|p| p.replace('\\', "/").to_lowercase()));
            }
            self.files.insert(lower);
        }
        Ok(())
    }

    fn has(&self, path: &str) -> bool {
        if self.files.contains(path) {
            return true;
        }
        // A sound may name a folder, from which the game picks a file.
        let folder = format!("{path}/");
        self.files
            .range(folder.clone()..)
            .next()
            .is_some_and(|first| first.starts_with(&folder))
    }
}

/// The active plugins of `profile`, in load order.
pub fn active_plugins(instance: &Path, profile: &str) -> anyhow::Result<Vec<String>> {
    let profile_dir = instance.join("profiles").join(profile);
    let active = std::fs::read_to_string(profile_dir.join("plugins.txt"))
        .map_err(|err| anyhow::anyhow!("failed to read profile {profile}'s plugins.txt: {err}"))?;
    let active: Vec<String> = active
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_start_matches('*').to_string())
        .collect();
    // `loadorder.txt` has the order; `plugins.txt` may not.
    let order = std::fs::read_to_string(profile_dir.join("loadorder.txt")).unwrap_or_default();
    let mut ordered: Vec<String> = order
        .lines()
        .map(str::trim)
        .filter(|line| active.iter().any(|a| a.eq_ignore_ascii_case(line)))
        .map(str::to_string)
        .collect();
    for plugin in active {
        if !ordered.iter().any(|o| o.eq_ignore_ascii_case(&plugin)) {
            ordered.push(plugin);
        }
    }
    Ok(ordered)
}

/// The files `plugin` names: each with the record naming it.
pub fn references(plugin: &Plugin) -> Vec<Missing> {
    let mut out = Vec::new();
    for record in plugin.records() {
        let mut named = |path: &str, folder: &str| {
            let path = path
                .replace('\\', "/")
                .split('/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("/")
                .to_lowercase();
            if path.is_empty() {
                return;
            }
            let path = if path.starts_with(&format!("{folder}/")) {
                path
            } else {
                format!("{folder}/{path}")
            };
            let editor_id = record
                .field(b"EDID")
                .map(|field| zstring(&field.data))
                .unwrap_or_default();
            out.push(Missing {
                path,
                record: format!("{} {:08X} {editor_id}", record.sig_str(), record.form_id.0)
                    .trim_end()
                    .to_string(),
            });
        };
        // SpeedTree files live in `Trees`; every other model in `Meshes`.
        let models = if &record.signature == b"TREE" {
            "trees"
        } else {
            "meshes"
        };
        for signature in [b"MODL", b"MOD2", b"MOD3", b"MOD4"] {
            for field in record.fields_with(signature) {
                named(&zstring(&field.data), models);
            }
        }
        if &record.signature == b"SOUN"
            && let Some(field) = record.field(b"FNAM")
        {
            named(&zstring(&field.data), "sound");
        }
    }
    out
}

fn zstring(data: &[u8]) -> String {
    let end = data.iter().position(|b| *b == 0).unwrap_or(data.len());
    String::from_utf8_lossy(&data[..end]).into_owned()
}

/// Every active plugin's references, against `data`.
pub fn check(active: &[String], data: &VirtualData) -> anyhow::Result<AssetReport> {
    let mut report = AssetReport::default();
    let mut by_plugin: Vec<(String, Vec<Missing>)> = Vec::new();
    for name in active {
        let Some(path) = data.plugins.get(&name.to_ascii_lowercase()) else {
            tracing::warn!(plugin = %name, "assets: active plugin not found; skipped");
            continue;
        };
        let plugin =
            Plugin::read(path).map_err(|err| anyhow::anyhow!("failed to read {name}: {err}"))?;
        report.plugins_read += 1;
        let refs = references(&plugin);
        report.references += refs.len();
        let mut missing: Vec<Missing> = refs.into_iter().filter(|r| !data.has(&r.path)).collect();
        missing.sort_by(|a, b| a.path.cmp(&b.path));
        missing.dedup_by(|a, b| a.path == b.path);
        by_plugin.push((name.clone(), missing));
    }
    // What the game's own plugins already lack is the game's, not the list's.
    let from_game: BTreeSet<String> = by_plugin
        .iter()
        .filter(|(name, _)| GAME_PLUGINS.contains(&name.to_ascii_lowercase().as_str()))
        .flat_map(|(_, missing)| missing.iter().map(|m| m.path.clone()))
        .collect();
    report.missing_in_game = from_game.len();
    for (name, missing) in by_plugin {
        if GAME_PLUGINS.contains(&name.to_ascii_lowercase().as_str()) {
            continue;
        }
        let missing: Vec<Missing> = missing
            .into_iter()
            .filter(|m| !from_game.contains(&m.path))
            .collect();
        if !missing.is_empty() {
            report.missing.push((name, missing));
        }
    }
    Ok(report)
}

/// Missing paths shared by several plugins, most-named first: often one mod
/// left out, which several plugins depend on.
pub fn most_named(report: &AssetReport) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (_, missing) in &report.missing {
        for m in missing {
            *counts.entry(m.path.clone()).or_default() += 1;
        }
    }
    let mut out: Vec<(String, usize)> = counts.into_iter().filter(|(_, n)| *n > 1).collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::formid::{FormId, MasterTable};
    use crate::plugin::group::{Entry, Group, GroupType};
    use crate::plugin::record::{Record, Subrecord};

    /// A plugin of STAT records, each naming one model.
    fn plugin_naming(models: &[&str]) -> Vec<u8> {
        let header = Record::new(
            b"TES4",
            FormId(0),
            vec![Subrecord::new(b"HEDR", vec![0; 12])],
        );
        let records = models
            .iter()
            .enumerate()
            .map(|(index, model)| {
                let mut path = model.as_bytes().to_vec();
                path.push(0);
                Entry::Record(Record::new(
                    b"STAT",
                    FormId(0x0100_0800 + index as u32),
                    vec![
                        Subrecord::new(b"EDID", format!("Thing{index}\0").into_bytes()),
                        Subrecord::new(b"MODL", path),
                    ],
                ))
            })
            .collect();
        Plugin {
            header,
            masters: MasterTable::new(Vec::new()),
            entries: vec![Entry::Group(Group::new(
                GroupType::TopLevel(*b"STAT"),
                records,
            ))],
        }
        .to_bytes()
    }

    fn write(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn an_instance_reports_what_its_plugins_name_and_nothing_has() {
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("game");
        let instance = root.path().join("instance");
        // The game names one model it does not ship.
        write(
            &game.join("Data/Oblivion.esm"),
            &plugin_naming(&["Vanilla\\Gone.nif"]),
        );
        write(
            &instance.join("mods/Mod/Mod.esp"),
            &plugin_naming(&["Mod\\Here.nif", "Mod\\\\Away.nif", "Vanilla\\Gone.nif"]),
        );
        write(&instance.join("mods/Mod/meshes/mod/here.nif"), b"nif");
        // A disabled mod's files are not in the game.
        write(&instance.join("mods/Off/meshes/mod/away.nif"), b"nif");
        write(
            &instance.join("profiles/Default/modlist.txt"),
            b"# MO2\n+Mod\n-Off\n",
        );
        write(
            &instance.join("profiles/Default/plugins.txt"),
            b"# MO2\nOblivion.esm\nMod.esp\n",
        );

        let data = VirtualData::of_instance(&instance, "Default", &game).unwrap();
        let active = active_plugins(&instance, "Default").unwrap();
        let report = check(&active, &data).unwrap();
        assert_eq!(report.plugins_read, 2);
        assert_eq!(report.missing_in_game, 1);
        assert_eq!(report.missing.len(), 1);
        let (plugin, missing) = &report.missing[0];
        assert_eq!(plugin, "Mod.esp");
        assert_eq!(
            missing,
            &vec![Missing {
                path: "meshes/mod/away.nif".to_string(),
                record: "STAT 01000801 Thing1".to_string(),
            }]
        );
    }
}
