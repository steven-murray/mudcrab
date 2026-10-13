//! A mod's actions, run against a stand-in of its folder.
//!
//! The actions that change which files a mod has -- `file_prune`, `file_hide`,
//! `file_move`, `pack_bsa`, `extract_bsa`, `create_dummy_plugin`,
//! `write_file` -- are run for real, by the same code `install` uses, on a
//! folder of placeholder files laid out as the archives would be. Each
//! placeholder holds the path it was planned at, so a file can be traced
//! through a move to where it came from. A selector that matches nothing, a
//! path that is not there, an `except` nothing picks: each fails here exactly
//! as it would at install, and what is left is the mod's finished folder.

use crate::config::actions::{self, ActionCx};
use crate::config::install::InstallSettings;
use crate::config::install::index::staged_paths;
use crate::config::schema::{ModAction, PersonalizedMod};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A file in the finished folder.
pub(super) struct Finished {
    /// Lowercased, `/`-separated.
    pub(super) path: String,
    /// The path it was planned at; `None` for a file an action made.
    pub(super) origin: Option<String>,
    /// For a plugin an action made, its masters.
    pub(super) masters: Option<Vec<String>>,
}

/// What is left once the actions have run.
pub(super) enum Rehearsal {
    /// The finished folder.
    Done(Vec<Finished>),
    /// An action would fail at install.
    Fails(anyhow::Error),
    /// The result cannot be known here, and why.
    Unknown(String),
}

/// Whether each partner's files can be known from its archives, worked out
/// once per `check` and shared between its threads.
pub(super) type PartnerCache = std::sync::Mutex<HashMap<String, Result<(), String>>>;

/// Lay out `planned` (destination, and for a file whose real bytes matter --
/// a BSA an `extract_bsa` unpacks -- where those bytes are) under `root`, run
/// the mod's file actions on it, and read back what is left.
pub(super) fn rehearse(
    mod_entry: &PersonalizedMod,
    planned: &[(String, Option<PathBuf>)],
    root: &Path,
    settings: &InstallSettings,
    plan_mods: &[PersonalizedMod],
    active_plugins: &HashSet<String>,
    partners: &PartnerCache,
) -> anyhow::Result<Rehearsal> {
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", root.display()))?;
    for (destination, real) in planned {
        let path = root.join(destination);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
        }
        match real {
            Some(bytes_at) => std::fs::copy(bytes_at, &path).map(|_| ()),
            None => std::fs::write(&path, placeholder(destination)),
        }
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))?;
    }

    let cx = ActionCx {
        owner: &mod_entry.id,
        settings,
        mod_target: Some(root),
        plan_mods,
        active_plugins,
    };
    for action in mod_entry
        .actions
        .iter()
        .filter(|a| actions::changes_files(a))
    {
        // A conflict selection compares against what another mod provides;
        // that has to be knowable from its archives alone.
        for partner in conflicts_with(action) {
            let cached = partners
                .lock()
                .map_err(|_| anyhow::anyhow!("a check thread panicked"))?
                .get(partner)
                .cloned();
            // Worked out without the lock held, so other threads carry on; two
            // that ask at once both work it out, which is harmless.
            let known = match cached {
                Some(known) => known,
                None => {
                    let known = match plan_mods.iter().find(|m| &m.id == partner) {
                        Some(other) => staged_paths(other, None, settings, active_plugins)
                            .map(|_| ())
                            .map_err(|err| format!("{err:#}")),
                        None => Ok(()),
                    };
                    partners
                        .lock()
                        .map_err(|_| anyhow::anyhow!("a check thread panicked"))?
                        .insert(partner.clone(), known.clone());
                    known
                }
            };
            if let Err(why) = known {
                tracing::info!(mod_id = %mod_entry.id, partner = %partner, why, "check: partner's files unknown");
                return Ok(Rehearsal::Unknown(format!(
                    "its {} compares with {partner}, whose files are only known once it is \
                     installed",
                    action.name()
                )));
            }
        }
        if let Err(err) = actions::apply_action(action, &cx) {
            return Ok(Rehearsal::Fails(err));
        }
    }

    let mut out = Vec::new();
    for relative in crate::config::install::layout::bain::list_relative_paths(root)? {
        let lower = relative.to_lowercase();
        if lower.ends_with(".mohidden") {
            continue;
        }
        let bytes = std::fs::read(root.join(&relative)).unwrap_or_default();
        let origin = origin(&bytes);
        let masters = origin
            .is_none()
            .then(|| crate::plugin::Plugin::masters_of(&bytes).ok())
            .flatten();
        out.push(Finished {
            path: lower,
            origin,
            masters,
        });
    }
    Ok(Rehearsal::Done(out))
}

const MARK: &[u8] = b"mudcrab-check-placeholder:";

fn placeholder(destination: &str) -> Vec<u8> {
    let mut bytes = MARK.to_vec();
    bytes.extend_from_slice(destination.as_bytes());
    bytes
}

/// The planned path a placeholder stands for; `None` for a file an action
/// wrote.
fn origin(bytes: &[u8]) -> Option<String> {
    bytes
        .strip_prefix(MARK)
        .map(|rest| String::from_utf8_lossy(rest).into_owned())
}

fn conflicts_with(action: &ModAction) -> &[String] {
    match action {
        ModAction::FilePrune(spec) => &spec.conflicts_with,
        ModAction::FileHide(spec) => &spec.conflicts_with,
        _ => &[],
    }
}

/// The paths named by `extract_bsa` actions, lowercased: those placeholders
/// need the real archive.
pub(super) fn bsas_to_extract(mod_entry: &PersonalizedMod) -> BTreeSet<String> {
    mod_entry
        .actions
        .iter()
        .filter_map(|action| match action {
            ModAction::ExtractBsa(spec) => Some(
                spec.archive
                    .replace('\\', "/")
                    .trim_matches('/')
                    .to_lowercase(),
            ),
            _ => None,
        })
        .collect()
}
