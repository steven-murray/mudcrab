//! Shims: a mod defined once, in a file of its own, and taken with `use` by
//! any list that has the mod, its own fields written over it.

use assert_cmd::Command;
use mudcrab::config::edit::{self, doc};
use mudcrab::config::loader::{load_modlist, load_modlist_with_docs};
use mudcrab::config::validator::validate;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// A mod packaged badly, and the fix for it.
const SHIM: &str = r#"component = true

[[mods]]
id = "Trees"
section = ["Landscape"]
plugins = ["Trees.esp"]
docs = { summary = "Trees, installed properly." }

[[mods.archives]]
path = "nexus:oblivion/1234/5678"
layout = "custom-data-folder"
data_folder = "Trees/Data"

[[mods.actions]]
action = "file_hide"
paths = ["meshes/stray.nif"]
"#;

fn list(dir: &Path, entry: &str) -> PathBuf {
    std::fs::create_dir_all(dir.join("shims")).unwrap();
    std::fs::write(dir.join("shims/trees.toml"), SHIM).unwrap();
    let path = dir.join("list.toml");
    std::fs::write(
        &path,
        format!("name = \"L\"\nplugins = [\"Oblivion.esm\", \"Trees.esp\"]\n\n[[mods]]\n{entry}\n"),
    )
    .unwrap();
    path
}

#[test]
fn a_use_entry_takes_the_shims_mod() {
    let dir = tempdir().unwrap();
    let path = list(
        dir.path(),
        "id = \"Oblivion Trees\"\nsection = [\"13 - Realm\"]\nuse = \"shims/trees.toml\"",
    );
    let source = load_modlist(&path).unwrap();
    validate(&source).unwrap();
    let trees = &source.mods[0];
    assert_eq!(trees.id, "Oblivion Trees");
    assert_eq!(trees.section, ["13 - Realm"]);
    assert_eq!(trees.plugins, ["Trees.esp"]);
    assert_eq!(trees.archives[0].data_folder.as_deref(), Some("Trees/Data"));
    assert_eq!(
        trees.docs.summary.as_deref(),
        Some("Trees, installed properly.")
    );
    // Written in the list: edits change the entry there.
    assert!(trees.origin.is_none());
    assert_eq!(trees.used, Some(dir.path().join("shims/trees.toml")));
    assert!(source.includes[0].uses);
}

#[test]
fn the_entrys_fields_win_and_its_actions_are_added() {
    let dir = tempdir().unwrap();
    let path = list(
        dir.path(),
        "id = \"Trees\"\nuse = \"shims/trees.toml\"\nenabled = false\nplugins = []\n\n\
         [[mods.actions]]\naction = \"file_hide\"\npaths = [\"textures/extra.dds\"]",
    );
    std::fs::write(
        &path,
        std::fs::read_to_string(&path)
            .unwrap()
            .replace(", \"Trees.esp\"", ""),
    )
    .unwrap();
    let source = load_modlist(&path).unwrap();
    validate(&source).unwrap();
    let trees = &source.mods[0];
    assert!(!trees.enabled);
    assert!(trees.plugins.is_empty());
    assert_eq!(
        trees.section,
        ["Landscape"],
        "the shim's, as the entry has none"
    );
    assert_eq!(trees.actions.len(), 2);
}

#[test]
fn the_lists_docs_file_can_replace_a_shims_prose() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "id = \"Trees\"\nuse = \"shims/trees.toml\"");
    std::fs::write(
        dir.path().join("docs.toml"),
        "[mods.Trees]\nsummary = \"Ours.\"\n",
    )
    .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("docs_file = \"docs.toml\"\n{text}")).unwrap();
    let source = load_modlist_with_docs(&path).unwrap();
    assert_eq!(source.mods[0].docs.summary.as_deref(), Some("Ours."));
}

#[test]
fn a_shim_with_more_than_one_mod_or_questions_is_refused() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "id = \"Trees\"\nuse = \"shims/trees.toml\"");
    std::fs::write(
        dir.path().join("shims/trees.toml"),
        format!("{SHIM}\n[[mods]]\nid = \"Other\"\n"),
    )
    .unwrap();
    let err = format!("{:#}", load_modlist(&path).unwrap_err());
    assert!(err.contains("a shim holds one mod, and it has 2"), "{err}");

    std::fs::write(
        dir.path().join("shims/trees.toml"),
        format!("{SHIM}\n[inputs.hd]\nquery = \"HD?\"\ntype = \"bool\"\n"),
    )
    .unwrap();
    let err = format!("{:#}", load_modlist(&path).unwrap_err());
    assert!(err.contains("one mod and nothing else"), "{err}");

    let path = list(
        dir.path(),
        "id = \"Trees\"\nuse = \"shims/trees.toml\"\ninclude = \"shims/trees.toml\"",
    );
    let err = format!("{:#}", load_modlist(&path).unwrap_err());
    assert!(err.contains("both `use` and `include`"), "{err}");
}

#[test]
fn a_use_entry_is_edited_in_the_list() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "id = \"Trees\"\nuse = \"shims/trees.toml\"");
    let pairs = doc::parse_pairs(&["oracle_name=\"Old Trees\"".to_string()]).unwrap();
    let edit = edit::plan_mod(&path, "Trees", |text, _, id| {
        doc::set_fields(text, id, None, &pairs, &[])
    })
    .unwrap();
    assert_eq!(edit.path, path);
    edit.write().unwrap();
    assert_eq!(
        load_modlist(&path).unwrap().mods[0].oracle_name.as_deref(),
        Some("Old Trees")
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("shims/trees.toml")).unwrap(),
        SHIM
    );
}

#[test]
fn add_use_writes_the_entry_and_places_the_shims_plugins() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("shims")).unwrap();
    std::fs::write(dir.path().join("shims/trees.toml"), SHIM).unwrap();
    let path = dir.path().join("list.toml");
    std::fs::write(
        &path,
        "name = \"L\"\nplugins = [\n    \"Oblivion.esm\",\n]\n",
    )
    .unwrap();
    Command::cargo_bin("mudcrab")
        .unwrap()
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .env("LOCALAPPDATA", dir.path().join("state"))
        .args(["add"])
        .arg(&path)
        .args([
            "--use",
            "shims/trees.toml",
            "--section",
            "Realm",
            "--load-last",
        ])
        .assert()
        .success();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("use = \"shims/trees.toml\""), "{text}");
    assert!(!text.contains("[[mods.archives]]"), "{text}");
    let source = load_modlist(&path).unwrap();
    validate(&source).unwrap();
    assert_eq!(source.mods[0].id, "Trees");
    assert_eq!(source.plugins, ["Oblivion.esm", "Trees.esp"]);
}
