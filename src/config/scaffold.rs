//! Write a merge-only modlist from a bare list of plugin names.
//!
//! `mudcrab merge` needs very little: which mod folder each source plugin lives
//! in, and a load order to sort masters by. Both are already sitting in an MO2
//! instance, so asking someone to write them out by hand is asking them to
//! transcribe what the tool could read. This module reads it.
//!
//! It exists because the audience for the merge engine is people who cannot run
//! zEdit's GUI, which means they also have no `merges.json` to convert from.
//! A list of plugin names is the smallest thing such a person reliably has.

use crate::config::install::is_plugin_file;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A source plugin resolved to the mod folder that provides it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSource {
    pub mod_id: String,
    pub plugin: String,
}

/// Every visible plugin in the instance, by lowercased name.
///
/// Mod roots only, matching what MO2 exposes as `Data`, and `.mohidden` files
/// are not plugin files as far as `is_plugin_file` is concerned -- which is the
/// behaviour wanted here too. A hidden plugin has already been merged away by
/// something, and offering it as a fresh source would be a trap.
fn index_plugins(mods_dir: &Path) -> anyhow::Result<BTreeMap<String, Vec<(String, String)>>> {
    let mut index: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();

    let entries = std::fs::read_dir(mods_dir)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", mods_dir.display()))?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Some(mod_id) = entry.file_name().to_str().map(ToString::to_string) else {
            continue;
        };
        let Ok(files) = std::fs::read_dir(entry.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if !file.file_type().map(|kind| kind.is_file()).unwrap_or(false)
                || !is_plugin_file(&path)
            {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                index
                    .entry(name.to_ascii_lowercase())
                    .or_default()
                    .push((mod_id.clone(), name.to_string()));
            }
        }
    }
    Ok(index)
}

/// Resolve each requested plugin to the mod that provides it.
///
/// A request may be written `Some Mod/Plugin.esp` to name the provider
/// directly, which is the way out of an ambiguity rather than a second syntax
/// to learn: the error for an ambiguous name says to use it.
pub fn resolve_sources(mods_dir: &Path, requested: &[String]) -> anyhow::Result<Vec<ResolvedSource>> {
    let index = index_plugins(mods_dir)?;
    let mut out = Vec::with_capacity(requested.len());
    let mut seen: Vec<String> = Vec::new();

    for raw in requested {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }

        let (wanted_mod, wanted_plugin) = match raw.rsplit_once(['/', '\\']) {
            Some((owner, plugin)) => (Some(owner.trim()), plugin.trim()),
            None => (None, raw),
        };

        if seen.iter().any(|p| p.eq_ignore_ascii_case(wanted_plugin)) {
            anyhow::bail!("{wanted_plugin} is listed twice; a plugin can only be merged once");
        }

        let candidates = index.get(&wanted_plugin.to_ascii_lowercase()).ok_or_else(|| {
            anyhow::anyhow!(
                "no mod in {} provides {wanted_plugin}.\n\
                 If it is there but hidden as `{wanted_plugin}.mohidden`, it has already been \
                 merged away by something else -- unhide it first, or merge whatever produced it \
                 instead.",
                mods_dir.display()
            )
        })?;

        let chosen = match wanted_mod {
            Some(owner) => candidates
                .iter()
                .find(|(mod_id, _)| mod_id.eq_ignore_ascii_case(owner))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "mod {owner} does not provide {wanted_plugin}. Mods that do: {}",
                        candidates.iter().map(|(m, _)| m.as_str()).collect::<Vec<_>>().join(", ")
                    )
                })?,
            None if candidates.len() > 1 => anyhow::bail!(
                "{wanted_plugin} is provided by {} mods, so which one to merge is ambiguous: {}.\n\
                 Name the one you mean as `<mod>/{wanted_plugin}`.",
                candidates.len(),
                candidates.iter().map(|(m, _)| m.as_str()).collect::<Vec<_>>().join(", ")
            ),
            None => &candidates[0],
        };

        seen.push(chosen.1.clone());
        out.push(ResolvedSource {
            mod_id: chosen.0.clone(),
            plugin: chosen.1.clone(),
        });
    }

    if out.len() < 2 {
        anyhow::bail!("a merge needs at least two source plugins; {} given", out.len());
    }
    Ok(out)
}

/// Profile load-order files under an MO2 instance, given its `mods/` directory.
pub fn discover_load_orders(mods_dir: &Path) -> Vec<PathBuf> {
    let Some(instance) = mods_dir.parent() else {
        return Vec::new();
    };
    let Ok(profiles) = std::fs::read_dir(instance.join("profiles")) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = profiles
        .flatten()
        .map(|entry| entry.path().join("loadorder.txt"))
        .filter(|path| path.is_file())
        .collect();
    found.sort();
    found
}

/// Plugin names from a `loadorder.txt` or `plugins.txt`.
pub fn read_load_order(path: &Path) -> anyhow::Result<Vec<String>> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        // `*` marks an active plugin in the plugins.txt of later games. Harmless
        // here, and stripping it means either file can be handed over.
        .map(|line| line.strip_prefix('*').unwrap_or(line).trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToString::to_string)
        .collect())
}

/// The load order as it will be *after* the merge: sources removed, output put
/// where the first of them was.
///
/// Not cosmetic. The merge orders its masters by this list, so it should
/// describe the world the merged plugin will load into rather than the one it
/// replaces.
pub fn post_merge_load_order(
    load_order: &[String],
    sources: &[ResolvedSource],
    output: &str,
) -> Vec<String> {
    let merged: Vec<String> = sources.iter().map(|s| s.plugin.to_ascii_lowercase()).collect();
    let is_source = |name: &str| merged.iter().any(|m| m == &name.to_ascii_lowercase());

    let mut out = Vec::with_capacity(load_order.len());
    let mut placed = false;
    for name in load_order {
        if is_source(name) {
            if !placed {
                out.push(output.to_string());
                placed = true;
            }
            continue;
        }
        if name.eq_ignore_ascii_case(output) {
            continue;
        }
        out.push(name.clone());
    }
    if !placed {
        out.push(output.to_string());
    }
    out
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Render the merge-only modlist.
pub fn render(
    name: &str,
    output_plugin: &str,
    sources: &[ResolvedSource],
    load_order: &[String],
) -> String {
    let mut out = String::new();
    out.push_str(
        "# Generated by `mudcrab new-merge`. Build it with:\n\
         #\n\
         #   mudcrab merge <this file> --mods-dir <mods> --output <somewhere>\n\
         #\n\
         # Nothing outside --output is written: the source mods are read only.\n\n",
    );
    out.push_str(&format!("name = {}\n\n", toml_string(name)));

    out.push_str(
        "# The load order the merge is built against, with the sources replaced by\n\
         # the merged plugin. Masters are ordered by this list.\nplugins = [\n",
    );
    for plugin in load_order {
        out.push_str(&format!("  {},\n", toml_string(plugin)));
    }
    out.push_str("]\n\n");

    out.push_str("# Source mods: bare ids, because a merge only ever reads their folders.\n");
    let mut declared: Vec<&str> = Vec::new();
    for source in sources {
        if declared.contains(&source.mod_id.as_str()) {
            continue;
        }
        declared.push(&source.mod_id);
        out.push_str(&format!("[[mods]]\nid = {}\n\n", toml_string(&source.mod_id)));
    }

    out.push_str(&format!(
        "[[mods]]\nid = {}\ntype = \"merge\"\n\n  [mods.merge]\n  output = {}\n",
        toml_string(name),
        toml_string(output_plugin)
    ));
    out.push_str(
        "  # Left false so building this changes nothing in the instance. Set it\n\
         \x20 # true only once the merge is verified and you want MO2 to stop loading\n\
         \x20 # the originals.\n  hide_sources = false\n",
    );
    out.push_str("  # Ordered: this decides clobber precedence and FormID allocation.\n  sources = [\n");
    // Pad after the comma, not before it: aligning the plugin column must not
    // leave a gap where the comma belongs.
    let width = sources.iter().map(|s| toml_string(&s.mod_id).len()).max().unwrap_or(0);
    for source in sources {
        let id = toml_string(&source.mod_id);
        let pad = " ".repeat(width - id.len());
        out.push_str(&format!(
            "    {{ mod = {id},{pad} plugin = {} }},\n",
            toml_string(&source.plugin),
        ));
    }
    out.push_str("  ]\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(dir: &Path, mods: &[(&str, &[&str])]) -> PathBuf {
        let mods_dir = dir.join("mods");
        for (mod_id, plugins) in mods {
            let folder = mods_dir.join(mod_id);
            std::fs::create_dir_all(&folder).unwrap();
            for plugin in *plugins {
                std::fs::write(folder.join(plugin), b"TES4").unwrap();
            }
        }
        mods_dir
    }

    #[test]
    fn resolves_each_plugin_to_the_mod_that_ships_it() {
        let dir = tempfile::tempdir().unwrap();
        let mods = instance(dir.path(), &[("Mod A", &["A.esp"]), ("Mod B", &["B.esp"])]);

        let got = resolve_sources(&mods, &["A.esp".into(), "B.esp".into()]).unwrap();

        assert_eq!(got[0].mod_id, "Mod A");
        assert_eq!(got[1].mod_id, "Mod B");
    }

    /// Order follows the request, not the directory: it decides clobber
    /// precedence, so it is the user's to choose.
    #[test]
    fn source_order_follows_the_request() {
        let dir = tempfile::tempdir().unwrap();
        let mods = instance(dir.path(), &[("Mod A", &["A.esp"]), ("Mod B", &["B.esp"])]);

        let got = resolve_sources(&mods, &["B.esp".into(), "A.esp".into()]).unwrap();

        assert_eq!(got[0].plugin, "B.esp");
        assert_eq!(got[1].plugin, "A.esp");
    }

    #[test]
    fn two_mods_shipping_one_name_is_an_error_that_says_which() {
        let dir = tempfile::tempdir().unwrap();
        let mods = instance(dir.path(), &[("Base", &["Shared.esp"]), ("Compat", &["Shared.esp"]), ("Other", &["O.esp"])]);

        let err = resolve_sources(&mods, &["Shared.esp".into(), "O.esp".into()]).unwrap_err().to_string();

        assert!(err.contains("ambiguous"), "{err}");
        assert!(err.contains("Base") && err.contains("Compat"), "{err}");
        assert!(err.contains("<mod>/Shared.esp"), "the escape hatch must be in the message: {err}");
    }

    #[test]
    fn naming_the_mod_resolves_the_ambiguity() {
        let dir = tempfile::tempdir().unwrap();
        let mods = instance(dir.path(), &[("Base", &["Shared.esp"]), ("Compat", &["Shared.esp"]), ("Other", &["O.esp"])]);

        let got = resolve_sources(&mods, &["Compat/Shared.esp".into(), "O.esp".into()]).unwrap();

        assert_eq!(got[0], ResolvedSource { mod_id: "Compat".into(), plugin: "Shared.esp".into() });
    }

    /// A `.mohidden` plugin has already been merged away. Offering it as a
    /// source would produce a merge of something the game does not load.
    #[test]
    fn an_already_hidden_plugin_is_not_a_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let mods = instance(dir.path(), &[("Merged Away", &["Gone.esp.mohidden"]), ("A", &["A.esp"]), ("B", &["B.esp"])]);

        let err = resolve_sources(&mods, &["Gone.esp".into(), "A.esp".into()]).unwrap_err().to_string();

        assert!(err.contains("mohidden"), "the message must explain why it is invisible: {err}");
    }

    #[test]
    fn one_source_is_not_a_merge() {
        let dir = tempfile::tempdir().unwrap();
        let mods = instance(dir.path(), &[("Mod A", &["A.esp"])]);

        let err = resolve_sources(&mods, &["A.esp".into()]).unwrap_err().to_string();
        assert!(err.contains("at least two"), "{err}");
    }

    #[test]
    fn the_merged_plugin_takes_the_place_of_the_first_source() {
        let order: Vec<String> = ["Oblivion.esm", "A.esp", "Keep.esp", "B.esp", "Last.esp"]
            .iter().map(ToString::to_string).collect();
        let sources = vec![
            ResolvedSource { mod_id: "Mod A".into(), plugin: "A.esp".into() },
            ResolvedSource { mod_id: "Mod B".into(), plugin: "B.esp".into() },
        ];

        let got = post_merge_load_order(&order, &sources, "Merged.esp");

        assert_eq!(got, vec!["Oblivion.esm", "Merged.esp", "Keep.esp", "Last.esp"]);
    }

    #[test]
    fn an_output_already_in_the_load_order_is_not_duplicated() {
        let order: Vec<String> = ["A.esp", "Merged.esp", "B.esp"].iter().map(ToString::to_string).collect();
        let sources = vec![
            ResolvedSource { mod_id: "Mod A".into(), plugin: "A.esp".into() },
            ResolvedSource { mod_id: "Mod B".into(), plugin: "B.esp".into() },
        ];

        let got = post_merge_load_order(&order, &sources, "Merged.esp");

        assert_eq!(got, vec!["Merged.esp"]);
    }

    #[test]
    fn plugin_names_with_toml_metacharacters_survive_the_round_trip() {
        // `Harvest [Flora].esp` and friends: brackets, quotes, ampersands.
        let sources = vec![
            ResolvedSource { mod_id: "A \"B\"".into(), plugin: "Harvest [Flora].esp".into() },
            ResolvedSource { mod_id: "C & D".into(), plugin: "Waalx Animals & Creatures.esm".into() },
        ];
        let rendered = render("My Merge", "My Merge.esp", &sources, &["Oblivion.esm".into()]);

        let parsed: toml::Value = toml::from_str(&rendered).expect("rendered TOML must parse");
        let mods = parsed["mods"].as_array().unwrap();
        assert_eq!(mods[0]["id"].as_str().unwrap(), "A \"B\"");
        let merge = mods.last().unwrap()["merge"].as_table().unwrap();
        let first = &merge["sources"].as_array().unwrap()[0];
        assert_eq!(first["plugin"].as_str().unwrap(), "Harvest [Flora].esp");
    }
}
