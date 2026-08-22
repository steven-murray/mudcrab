//! Applying a load order to an Oblivion instance.
//!
//! Oblivion has no load-order file. **The load order is the plugins' file
//! modification times**, oldest first, and every tool that appears to manage it
//! -- MO2, Wrye Bash, LOOT -- is really just restamping those files.
//!
//! That makes `plugins.txt` alone insufficient, which is the trap this module
//! exists to close. Oblivion's `plugins.txt` records *which* plugins are
//! active, not the order they load in; writing it in the declared order looks
//! right and changes nothing. MO2 then opens the profile, reads the real order
//! off the timestamps, finds it disagrees with `loadorder.txt`, and rewrites
//! `loadorder.txt` to match the files -- so the modlist's order is not merely
//! ignored, it is overwritten by whatever order the archives happened to be
//! extracted in.
//!
//! So the order is applied twice, to the two places that hold it:
//!
//! - `loadorder.txt`, which is what MO2 reads and displays.
//! - the plugin files' mtimes, which is what the game reads.
//!
//! Both are written from the same list, so there is nothing for MO2 to
//! reconcile and no run of the GUI is needed to make the order real.

use crate::config::install::is_plugin_file;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 2000-01-01T00:00:00Z. Oblivion only compares these to each other, so the
/// base is arbitrary; a date before the game shipped keeps a mudcrab-stamped
/// plugin visibly distinct from one carrying a real timestamp.
const BASE_SECS: u64 = 946_684_800;

/// A minute per plugin. Large enough that no filesystem's timestamp
/// granularity can collapse two neighbours into a tie, which would leave their
/// relative order down to whatever the game does with equal stamps.
const STEP_SECS: u64 = 60;

/// The declared order, split by whether a file exists to carry it.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct LoadOrderPlan {
    /// Plugins with at least one file, in declared order, each with every copy
    /// of that name. Copies rather than the one MO2 would pick: a plugin
    /// shipped by two mods has a winner decided by mod priority, and stamping
    /// both means the answer does not depend on this module agreeing with MO2
    /// about which that is. The loser is not loaded, so its stamp costs
    /// nothing.
    pub present: Vec<(String, Vec<PathBuf>)>,
    /// Declared plugins with no file anywhere: nothing to stamp, so they hold
    /// no position. Expected for one the list pre-declares and a later step
    /// writes -- `Bashed Patch, 0.esp` -- and unavoidable in a staging install
    /// with no game directory, where the base masters are not visible from
    /// here. Reported rather than treated as an error for that reason.
    pub missing: Vec<String>,
}

/// Work out which declared plugins can actually hold a position.
pub(crate) fn plan_load_order(
    plugins: &[String],
    mods_dir: &Path,
    game_dir: Option<&Path>,
) -> anyhow::Result<LoadOrderPlan> {
    let mut copies = plugin_copies(mods_dir, game_dir)?;
    let mut plan = LoadOrderPlan::default();

    for plugin in plugins {
        match copies.remove(&plugin.trim().to_ascii_lowercase()) {
            Some(paths) => plan.present.push((plugin.clone(), paths)),
            None => plan.missing.push(plugin.clone()),
        }
    }

    Ok(plan)
}

/// Stamp every copy of every plugin, in declared order.
pub(crate) fn stamp_plugin_order(plan: &LoadOrderPlan) -> anyhow::Result<usize> {
    let mut stamped = 0;
    for (index, (_, paths)) in plan.present.iter().enumerate() {
        let stamp = UNIX_EPOCH + Duration::from_secs(BASE_SECS + index as u64 * STEP_SECS);
        for path in paths {
            set_modified(path, stamp)?;
            stamped += 1;
        }
    }
    Ok(stamped)
}

fn set_modified(path: &Path, time: SystemTime) -> anyhow::Result<()> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|err| anyhow::anyhow!("failed to open {} to set its load order: {err}", path.display()))?;
    file.set_modified(time)
        .map_err(|err| anyhow::anyhow!("failed to stamp {}: {err}", path.display()))
}

/// Every plugin file MO2 can see, by lowercased name.
///
/// Mod roots only, not a recursive walk: MO2 exposes a mod's root as `Data`,
/// so a plugin in a subfolder -- `optional/`, a leftover `docs/` -- is not part
/// of the load order and stamping it would say otherwise.
fn plugin_copies(
    mods_dir: &Path,
    game_dir: Option<&Path>,
) -> anyhow::Result<HashMap<String, Vec<PathBuf>>> {
    let mut out: HashMap<String, Vec<PathBuf>> = HashMap::new();

    if mods_dir.is_dir() {
        for entry in read_dir(mods_dir)? {
            let mod_dir = entry?.path();
            if mod_dir.is_dir() {
                collect_root_plugins(&mod_dir, &mut out)?;
            }
        }
    }

    // Base game masters belong to no mod.
    if let Some(data_dir) = game_dir.map(|dir| dir.join("Data"))
        && data_dir.is_dir()
    {
        collect_root_plugins(&data_dir, &mut out)?;
    }

    Ok(out)
}

fn collect_root_plugins(dir: &Path, out: &mut HashMap<String, Vec<PathBuf>>) -> anyhow::Result<()> {
    for entry in read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        // `is_plugin_file` reads the extension, so a `.mohidden` merge source
        // is not one -- which is exactly right: hidden plugins have no place in
        // the load order and must keep whatever mtime their merge hashed.
        if entry.file_type().map(|kind| kind.is_file()).unwrap_or(false)
            && is_plugin_file(&path)
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
        {
            out.entry(name.to_ascii_lowercase()).or_default().push(path);
        }
    }
    Ok(())
}

fn read_dir(dir: &Path) -> anyhow::Result<std::fs::ReadDir> {
    std::fs::read_dir(dir).map_err(|err| anyhow::anyhow!("failed to read {}: {err}", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"TES4").unwrap();
    }

    fn mtime(path: &Path) -> u64 {
        std::fs::metadata(path)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn names(plan: &LoadOrderPlan) -> Vec<String> {
        plan.present.iter().map(|(name, _)| name.clone()).collect()
    }

    fn run(plugins: &[&str], mods: &Path) -> LoadOrderPlan {
        let names: Vec<String> = plugins.iter().map(ToString::to_string).collect();
        let plan = plan_load_order(&names, mods, None).unwrap();
        stamp_plugin_order(&plan).unwrap();
        plan
    }

    #[test]
    fn stamps_in_declared_order_regardless_of_the_order_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        touch(&mods.join("B/Second.esp"));
        touch(&mods.join("A/First.esm"));

        let plan = run(&["First.esm", "Second.esp"], &mods);

        assert_eq!(names(&plan), vec!["First.esm", "Second.esp"]);
        assert!(plan.missing.is_empty());
        assert_eq!(mtime(&mods.join("A/First.esm")), BASE_SECS);
        assert_eq!(mtime(&mods.join("B/Second.esp")), BASE_SECS + STEP_SECS);
    }

    /// The Nobody-Goes-into-the-Mountains shape: two mods, one plugin name.
    #[test]
    fn every_copy_of_a_name_gets_the_same_stamp() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        touch(&mods.join("Base/Shared.esp"));
        touch(&mods.join("Compat/Shared.esp"));

        let plan = run(&["Shared.esp"], &mods);

        assert_eq!(plan.present.len(), 1);
        assert_eq!(plan.present[0].1.len(), 2);
        assert_eq!(mtime(&mods.join("Base/Shared.esp")), BASE_SECS);
        assert_eq!(mtime(&mods.join("Compat/Shared.esp")), BASE_SECS);
    }

    /// Merge sources are hidden, and their mtimes are part of the merge's input
    /// hash. Stamping one would rebuild every merge on the next run.
    #[test]
    fn a_hidden_plugin_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        let hidden = mods.join("Source/Merged Away.esp.mohidden");
        touch(&hidden);
        let before = mtime(&hidden);

        let plan = run(&["Merged Away.esp"], &mods);

        assert!(plan.present.is_empty());
        assert_eq!(plan.missing, vec!["Merged Away.esp".to_string()]);
        assert_eq!(mtime(&hidden), before);
    }

    /// A plugin in a subfolder is not in MO2's Data view, so it holds no
    /// position and does not belong in the profile.
    #[test]
    fn a_plugin_below_the_mod_root_is_not_in_the_load_order() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        touch(&mods.join("Example/optional/Extra.esp"));

        let plan = run(&["Extra.esp"], &mods);

        assert!(plan.present.is_empty());
        assert_eq!(plan.missing, vec!["Extra.esp".to_string()]);
    }

    /// `Bashed Patch, 0.esp` is declared before Wrye Bash writes it. It takes
    /// no slot in the stamping, so the plugins after it close up rather than
    /// leaving a minute-wide gap in the timestamps.
    #[test]
    fn a_plugin_no_mod_ships_does_not_hold_a_slot() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        touch(&mods.join("A/First.esp"));
        touch(&mods.join("B/Last.esp"));

        let plan = run(&["First.esp", "Bashed Patch, 0.esp", "Last.esp"], &mods);

        assert_eq!(names(&plan), vec!["First.esp", "Last.esp"]);
        assert_eq!(plan.missing, vec!["Bashed Patch, 0.esp".to_string()]);
        assert_eq!(mtime(&mods.join("A/First.esp")), BASE_SECS);
        assert_eq!(mtime(&mods.join("B/Last.esp")), BASE_SECS + STEP_SECS);
    }
}
