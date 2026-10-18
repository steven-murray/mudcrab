//! Which mods provide the same files, and which of them wins: what `mudcrab
//! conflicts` prints and the GUI's conflicts view shows, from one place.
//!
//! `conflicts_with` decides which files a mod gives up, and the guide's rows
//! that use it delete hundreds at a time. That is not a thing to run blind, so
//! the same sets the actions act on are offered here to look at first.

use crate::config::filter::ModFilter;
use crate::config::install::InstallSettings;
use crate::config::install::index::staged_paths_detailed;
use crate::config::install::safe_mod_dir_name;
use crate::config::schema::{PersonalizedMod, PersonalizedPlan};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Every installed mod that shares files with another.
#[derive(Debug, Clone, Serialize)]
pub struct Sweep {
    /// In the plan's order, each with whom it shares files.
    pub mods: Vec<ModConflicts>,
    /// Mods compared: the plan's that are installed.
    pub installed: usize,
    /// Mods in the plan with no folder, so not compared.
    pub missing: usize,
}

/// One mod's shared files, by the mod on the other side.
#[derive(Debug, Clone, Serialize)]
pub struct ModConflicts {
    pub id: String,
    pub section: Vec<String>,
    /// Earlier mods it overrides, most files first.
    pub wins: Vec<Partner>,
    /// Later mods that override it, most files first.
    pub losses: Vec<Partner>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Partner {
    pub mod_id: String,
    pub files: usize,
}

/// Every installed mod's files, compared with every other's, in MO2's order:
/// a later mod wins a file an earlier one also provides. Reads only the
/// installed folders (`settings.mods_dir`), which is what MO2 sees; a mod
/// without one is counted as missing. `filter` limits which mods are
/// reported, not which are compared.
pub fn sweep(
    plan: &PersonalizedPlan,
    settings: &InstallSettings,
    filter: &ModFilter,
) -> anyhow::Result<Sweep> {
    let active = active_plugins(plan);
    // Each path, and the mods providing it in priority order.
    let mut providers: HashMap<String, Vec<usize>> = HashMap::new();
    let mut installed = Vec::new();
    let mut missing = 0usize;
    for (index, entry) in plan.mods.iter().enumerate() {
        let dir = settings.mods_dir.join(safe_mod_dir_name(&entry.id)?);
        if !dir.is_dir() {
            missing += 1;
            continue;
        }
        installed.push(index);
        let paths = staged_paths_detailed(entry, Some(&dir), settings, &active)?;
        for path in paths.visible {
            // MO2 writes a meta.ini into every mod; it is not in the game's
            // view, and would make every mod "conflict" with every other.
            if path == "meta.ini" {
                continue;
            }
            providers.entry(path).or_default().push(index);
        }
    }

    // Per mod: how many files it wins from each earlier mod, and loses to
    // each later one. A file three mods provide counts once per pair that
    // matters: the winner over each loser.
    let mut wins: BTreeMap<usize, BTreeMap<usize, usize>> = BTreeMap::new();
    let mut losses: BTreeMap<usize, BTreeMap<usize, usize>> = BTreeMap::new();
    for mods in providers.values().filter(|mods| mods.len() > 1) {
        let winner = *mods.last().expect("more than one");
        for &loser in &mods[..mods.len() - 1] {
            *wins.entry(winner).or_default().entry(loser).or_default() += 1;
            *losses.entry(loser).or_default().entry(winner).or_default() += 1;
        }
    }
    let partners = |counts: Option<&BTreeMap<usize, usize>>| -> Vec<Partner> {
        let mut list: Vec<(usize, usize)> = counts
            .into_iter()
            .flatten()
            .map(|(m, n)| (*m, *n))
            .collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        list.into_iter()
            .map(|(m, files)| Partner {
                mod_id: plan.mods[m].id.clone(),
                files,
            })
            .collect()
    };

    let mods = installed
        .iter()
        .filter(|&&index| {
            let entry = &plan.mods[index];
            filter.is_empty() || filter.matches(&entry.section, &entry.id)
        })
        .filter(|index| wins.contains_key(index) || losses.contains_key(index))
        .map(|&index| ModConflicts {
            id: plan.mods[index].id.clone(),
            section: plan.mods[index].section.clone(),
            wins: partners(wins.get(&index)),
            losses: partners(losses.get(&index)),
        })
        .collect();
    Ok(Sweep {
        mods,
        installed: installed.len(),
        missing,
    })
}

/// The files one mod shares with others.
#[derive(Debug, Clone, Serialize)]
pub struct Shared {
    /// How many files the mod provides.
    pub files: usize,
    /// Each other mod's file count, in the order asked.
    pub others: Vec<Partner>,
    /// The mod's files that one of the others also provides.
    pub overlap: Vec<String>,
    /// Further shared files the mod has hidden already (a `file_hide` that
    /// has run), so out of the virtual file system and not in `overlap`.
    pub already_hidden: usize,
}

/// What mod `id` provides that any of `with` also does, under the folder
/// `under` if given. A mod is read from its installed folder in
/// `settings.mods_dir` when it has one, otherwise from its archives (in
/// `settings.cache_dir`) as `install` would lay them out.
pub fn shared(
    plan: &PersonalizedPlan,
    settings: &InstallSettings,
    id: &str,
    with: &[String],
    under: Option<&str>,
) -> anyhow::Result<Shared> {
    let active = active_plugins(plan);
    let mine = paths_of(plan, settings, &active, id)?;
    let mut theirs = BTreeSet::new();
    let mut others = Vec::new();
    for other in with {
        let paths = paths_of(plan, settings, &active, other)?;
        others.push(Partner {
            mod_id: other.clone(),
            files: paths.visible.len(),
        });
        theirs.extend(paths.visible);
    }
    let under = under.map(|value| {
        format!(
            "{}/",
            value.replace('\\', "/").trim_matches('/').to_lowercase()
        )
    });
    let inside = |path: &&String| under.as_deref().is_none_or(|p| path.starts_with(p));
    Ok(Shared {
        files: mine.visible.len(),
        others,
        overlap: mine
            .visible
            .iter()
            .filter(inside)
            .filter(|path| theirs.contains(*path))
            .cloned()
            .collect(),
        already_hidden: mine
            .hidden
            .iter()
            .filter(inside)
            .filter(|path| theirs.contains(*path))
            .count(),
    })
}

/// Every file mod `id` provides, lowercased and `/`-separated: from its
/// installed folder, or its archives.
pub fn files_of(
    plan: &PersonalizedPlan,
    settings: &InstallSettings,
    id: &str,
) -> anyhow::Result<BTreeSet<String>> {
    Ok(paths_of(plan, settings, &active_plugins(plan), id)?.visible)
}

fn paths_of(
    plan: &PersonalizedPlan,
    settings: &InstallSettings,
    active: &HashSet<String>,
    id: &str,
) -> anyhow::Result<crate::config::install::index::StagedPaths> {
    let entry = find(plan, id)?;
    staged_paths_detailed(
        entry,
        Some(&settings.mods_dir.join(safe_mod_dir_name(&entry.id)?)),
        settings,
        active,
    )
}

fn find<'a>(plan: &'a PersonalizedPlan, id: &str) -> anyhow::Result<&'a PersonalizedMod> {
    plan.mods
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("no mod '{id}' in the plan"))
}

fn active_plugins(plan: &PersonalizedPlan) -> HashSet<String> {
    plan.plugins
        .iter()
        .map(|plugin| plugin.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(mods: &std::path::Path) -> InstallSettings {
        InstallSettings {
            cache_dir: mods.join("downloads"),
            mods_dir: mods.to_path_buf(),
            mo2_instance_dir: None,
            profile_name: String::new(),
            game_dir: None,
            execute_actions: false,
            dry_run: true,
            tools: Default::default(),
            filter: Default::default(),
            force_merges: false,
            force: false,
            redo_actions: Vec::new(),
        }
    }

    #[test]
    fn a_later_mod_wins_the_files_both_provide() {
        let root = tempfile::tempdir().unwrap();
        let mods = root.path();
        for (id, files) in [
            (
                "A",
                &["textures/rock.dds", "textures/a.dds", "meta.ini"][..],
            ),
            ("B", &["textures/rock.dds", "meta.ini"][..]),
            ("C", &["textures/a.dds.mohidden", "textures/c.dds"][..]),
        ] {
            for file in files {
                let path = mods.join(id).join(file);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, b"").unwrap();
            }
        }
        let plan: PersonalizedPlan = serde_json::from_value(serde_json::json!({
            "schema_version": 1, "name": "L", "responses": {}, "mod_order": [],
            "selected_mods": [], "plugins": [],
            "mods": [
                {"id": "A", "archives": []},
                {"id": "B", "archives": []},
                {"id": "C", "archives": []},
                {"id": "Gone", "archives": []}
            ]
        }))
        .unwrap();
        let settings = settings(mods);

        let found = sweep(&plan, &settings, &ModFilter::default()).unwrap();
        assert_eq!((found.installed, found.missing), (3, 1));
        // meta.ini is MO2's, and a hidden file is out of the game's view.
        let ids: Vec<&str> = found.mods.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["A", "B"]);
        assert_eq!(found.mods[0].losses[0].mod_id, "B");
        assert_eq!(found.mods[1].wins[0].files, 1);

        let shared = shared(&plan, &settings, "C", &["A".to_string()], None).unwrap();
        assert!(shared.overlap.is_empty());
        assert_eq!(shared.already_hidden, 1, "C has hidden its copy of a.dds");
        let shared = shared_under(&plan, &settings);
        assert_eq!(shared.overlap, ["textures/rock.dds"]);
    }

    fn shared_under(plan: &PersonalizedPlan, settings: &InstallSettings) -> Shared {
        shared(plan, settings, "A", &["B".to_string()], Some("Textures\\")).unwrap()
    }
}
