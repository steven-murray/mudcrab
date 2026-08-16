//! Detectors for the things this merge deliberately does not handle.
//!
//! M0's recon established three scoping decisions -- no script bytecode
//! patching, no asset copying, no BSA handling -- by measuring the real
//! corpus rather than assuming (`MOFAM-test/notes/merge-recon.md`). Each held
//! for all six MOFAM merges.
//!
//! These detectors exist so those decisions are **self-invalidating rather
//! than silent**. If a modlist ever violates one, the merge stops and says
//! which assumption broke and what the evidence for it was. That is the whole
//! point: a wrong assumption should surface as a refusal, not as a plugin that
//! loads and then misbehaves in game.
//!
//! They are not implementations, and they are deliberately not opt-out. A flag
//! to skip them would just restore the silent failure they exist to prevent.

use super::rewrite::Remapper;
use crate::plugin::{FormId, Plugin, PluginName};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error(
        "{plugin}: record {record} has compiled script bytecode containing the FormID \
         {form_id} at byte offset {offset}, and this merge renumbers that FormID.\n\
         mudcrab renumbers SCRO entries in place and does not patch bytecode, because \
         Oblivion scripts reference forms by 16-bit index into the record's SCRO list \
         (MOFAM-test/notes/merge-recon.md section 2 -- zero occurrences across the whole \
         752-mod corpus). This plugin is the counter-example that assumption needed.\n\
         Merging it would leave the script pointing at the wrong form."
    )]
    ScriptFormId {
        plugin: PluginName,
        record: FormId,
        form_id: FormId,
        offset: usize,
    },

    #[error(
        "{plugin}: {kind} at {}\n\
         These assets are looked up by plugin name or FormID, so merging the plugin \
         away breaks the lookup and mudcrab does not rewrite them \
         (MOFAM-test/notes/merge-recon.md section 3 -- no merged MOFAM plugin has any).\n\
         Either exclude this plugin from the merge, or handle the assets by hand.",
        .path.display()
    )]
    KeyedAssets {
        plugin: PluginName,
        kind: &'static str,
        path: PathBuf,
    },
}

/// Does any compiled script embed a FormID this merge is about to change?
///
/// Oblivion bytecode addresses forms through the record's SCRO list, not by
/// inline FormID, so renumbering SCRO in place is enough. The naive check --
/// "does any SCRO value appear as bytes in SCDA" -- produces only false
/// positives: small integers like 20 that happen to equal `Oblivion.esm`'s
/// Player FormID `0x00000014`, mostly unaligned.
///
/// So this asks the question that actually matters: does an SCRO value appear
/// **4-byte aligned** in the bytecode *and* does this merge change it? Values
/// in `Oblivion.esm` never change, which is why the naive check's hits are
/// harmless. Measured zero across all six merges.
pub fn audit_scripts(
    plugin_name: &PluginName,
    plugin: &Plugin,
    remapper: &Remapper<'_>,
) -> Result<(), AuditError> {
    for record in plugin.records() {
        let mut script_refs: BTreeSet<u32> = BTreeSet::new();
        let mut bytecode: Option<&[u8]> = None;

        for field in record.fields() {
            match &field.signature {
                b"SCRO" if field.data.len() == 4 => {
                    script_refs.insert(u32::from_le_bytes(
                        field.data[..4].try_into().expect("checked length"),
                    ));
                }
                b"SCDA" => bytecode = Some(&field.data),
                _ => {}
            }
        }

        let (Some(bytecode), false) = (bytecode, script_refs.is_empty()) else {
            continue;
        };

        // Only FormIDs this merge actually moves can do harm.
        let changed: BTreeSet<u32> = script_refs
            .into_iter()
            .filter(|raw| {
                let old = FormId(*raw);
                remapper.probe(old).map(|new| new != old).unwrap_or(false)
            })
            .collect();
        if changed.is_empty() {
            continue;
        }

        for offset in (0..bytecode.len().saturating_sub(3)).step_by(4) {
            let word = u32::from_le_bytes(
                bytecode[offset..offset + 4].try_into().expect("in bounds"),
            );
            if changed.contains(&word) {
                return Err(AuditError::ScriptFormId {
                    plugin: plugin_name.clone(),
                    record: record.form_id,
                    form_id: FormId(word),
                    offset,
                });
            }
        }
    }

    Ok(())
}

/// Assets whose lookup key contains the plugin name or a FormID.
///
/// That is the whole criterion, and it is why these three and not others:
/// merging a plugin away changes its name and renumbers its FormIDs, so any
/// asset addressed through either stops resolving. Assets addressed by path
/// alone -- meshes, textures, the overwhelming majority -- are unaffected, and
/// keep loading because merged source mods stay enabled in MO2.
const KEYED_ASSET_DIRS: &[(&str, &str)] = &[
    // Sound/Voice/<plugin>.esp/<topic>/<INFO FormID>_<n>.mp3
    ("Sound/Voice", "voice data"),
    // Oblivion keeps FaceGen inside the NPC_ record, so a loose directory
    // means something non-standard is in play.
    ("Textures/Characters/FaceGen", "loose FaceGen textures"),
    ("Meshes/Characters/FaceGen", "loose FaceGen meshes"),
];

/// Check the mod folder holding a source plugin for keyed assets.
///
/// `plugin_path` is the plugin itself; its parent is the mod's data root.
pub fn audit_assets(plugin_name: &PluginName, plugin_path: &Path) -> Result<(), AuditError> {
    let Some(data_root) = plugin_path.parent() else {
        return Ok(());
    };

    for (relative, kind) in KEYED_ASSET_DIRS {
        let Some(dir) = resolve_ci(data_root, relative) else {
            continue;
        };
        // Voice trees are per-plugin: only this plugin's own subdirectory
        // matters. A mod folder shared with unmerged plugins routinely holds
        // voice data for those, and merging must not object to it.
        if *kind == "voice data" {
            if let Some(mine) = child_ci(&dir, plugin_name.as_str()) {
                return Err(AuditError::KeyedAssets {
                    plugin: plugin_name.clone(),
                    kind,
                    path: mine,
                });
            }
            continue;
        }
        return Err(AuditError::KeyedAssets {
            plugin: plugin_name.clone(),
            kind,
            path: dir,
        });
    }

    // A config file named after the plugin, read by OBSE plugins and scripts.
    let stem = plugin_name.as_str();
    for candidate in [format!("{stem}.ini"), replace_extension_with_ini(stem)] {
        if let Some(path) = child_ci(data_root, &candidate) {
            return Err(AuditError::KeyedAssets {
                plugin: plugin_name.clone(),
                kind: "a plugin-name-keyed INI",
                path,
            });
        }
    }

    Ok(())
}

fn replace_extension_with_ini(plugin: &str) -> String {
    match plugin.rfind('.') {
        Some(dot) => format!("{}.ini", &plugin[..dot]),
        None => format!("{plugin}.ini"),
    }
}

fn child_ci(parent: &Path, name: &str) -> Option<PathBuf> {
    crate::util::fs::find_child_case_insensitive(parent, name)
}

/// Resolve a `a/b/c` relative path one component at a time, case-insensitively.
fn resolve_ci(root: &Path, relative: &str) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    for component in relative.split('/') {
        current = child_ci(&current, component)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn a_plugins_own_voice_directory_is_refused() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("Sound/Voice/Talkative.esp/GREETING"))
            .expect("voice tree");
        std::fs::write(root.join("Talkative.esp"), b"TES4").expect("plugin");

        let err = audit_assets(&PluginName::new("Talkative.esp"), &root.join("Talkative.esp"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("voice data"), "{err}");
        assert!(err.contains("merge-recon.md"), "the error must cite its evidence: {err}");
    }

    #[test]
    fn voice_data_belonging_to_another_plugin_is_ignored() {
        // Mod folders routinely hold several plugins, only some of them
        // merged. Objecting to a neighbour's voice tree would block merges
        // that are perfectly safe.
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("Sound/Voice/Chatty.esp/GREETING")).expect("voice tree");
        std::fs::write(root.join("Quiet.esp"), b"TES4").expect("plugin");

        audit_assets(&PluginName::new("Quiet.esp"), &root.join("Quiet.esp"))
            .expect("another plugin's voice data is not our problem");
    }

    #[test]
    fn a_plugin_name_keyed_ini_is_refused() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        std::fs::write(root.join("Configured.esp"), b"TES4").expect("plugin");
        std::fs::write(root.join("Configured.ini"), b"[General]\n").expect("ini");

        let err = audit_assets(
            &PluginName::new("Configured.esp"),
            &root.join("Configured.esp"),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("INI"), "{err}");
    }

    #[test]
    fn lookups_are_case_insensitive() {
        // Mod archives are Windows-authored; casing on disk rarely matches.
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("sound/voice/TALKATIVE.ESP")).expect("voice tree");
        std::fs::write(root.join("Talkative.esp"), b"TES4").expect("plugin");

        assert!(
            audit_assets(&PluginName::new("Talkative.esp"), &root.join("Talkative.esp")).is_err()
        );
    }

    #[test]
    fn an_ordinary_mod_folder_passes() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("Meshes/Architecture")).expect("meshes");
        std::fs::create_dir_all(root.join("Textures/Trees")).expect("textures");
        std::fs::write(root.join("Fort.esp"), b"TES4").expect("plugin");

        audit_assets(&PluginName::new("Fort.esp"), &root.join("Fort.esp")).expect("no keyed assets");
    }
}
