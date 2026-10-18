//! Lists built from other files: `include` in `[[mods]]`, and the edits
//! that change an included mod in the file it is written in.

use mudcrab::config::edit::{self, doc};
use mudcrab::config::loader::{load_modlist, load_modlist_with_docs};
use mudcrab::config::pipeline::compile;
use mudcrab::config::validator::validate;
use std::path::Path;
use tempfile::tempdir;

fn write(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, text).unwrap();
    path
}

const LIST: &str = r#"name = "Split"

plugins = [
    "Oblivion.esm",
    { include = "Core" },
    "Last.esp",
]

[inputs.big]
query = "Big textures?"
type = "bool"
default = false

[[mods]]
id = "First"
section = ["Start"]
archives = [{ path = "manual:first.7z" }]

[[mods]]
id = "Core"
section = ["Core"]
include = "parts/core.toml"

[[mods]]
id = "Last"
section = ["End"]
plugins = ["Last.esp"]
archives = [{ path = "manual:last.7z" }]
"#;

const CORE: &str = r#"component = true

plugins = [
    "A.esp",
    "B.esp",
]

[[mods]]
id = "A"
section = ["Patches"]
plugins = ["A.esp"]
archives = [{ path = "manual:a.7z" }]

[[mods]]
id = "B"
if = "big"
plugins = ["B.esp"]
archives = [{ path = "manual:b.7z" }]

[ini.Display]
iMaxFPS = 60

[sections.Patches]
summary = "The patches."
images = [{ path = "img/p.png" }]
"#;

/// The same list, written in one file.
const FLAT: &str = r#"name = "Split"

plugins = [
    "Oblivion.esm",
    "A.esp",
    "B.esp",
    "Last.esp",
]

[inputs.big]
query = "Big textures?"
type = "bool"
default = false

[[mods]]
id = "First"
section = ["Start"]
archives = [{ path = "manual:first.7z" }]

[[mods]]
id = "A"
section = ["Core", "Patches"]
plugins = ["A.esp"]
archives = [{ path = "manual:a.7z" }]

[[mods]]
id = "B"
section = ["Core"]
if = "big"
plugins = ["B.esp"]
archives = [{ path = "manual:b.7z" }]

[[mods]]
id = "Last"
section = ["End"]
plugins = ["Last.esp"]
archives = [{ path = "manual:last.7z" }]

[ini.Display]
iMaxFPS = 60
"#;

fn split(dir: &Path) -> std::path::PathBuf {
    write(dir, "parts/img/p.png", "");
    write(dir, "parts/core.toml", CORE);
    write(dir, "list.toml", LIST)
}

#[test]
fn an_included_file_compiles_as_if_written_in_place() {
    let dir = tempdir().unwrap();
    let list = split(dir.path());
    let flat = write(dir.path(), "flat.toml", FLAT);
    write(dir.path(), "parts/img/p.png", "");

    let mut split = serde_json::to_value(compile(&list, true).unwrap().modlist).unwrap();
    let flat = serde_json::to_value(compile(&flat, true).unwrap().modlist).unwrap();
    // Only where the mods came from differs.
    for entry in split["mods"].as_array_mut().unwrap() {
        let origin = entry.as_object_mut().unwrap().remove("origin");
        let id = entry["id"].as_str().unwrap();
        let expected = matches!(id, "A" | "B").then(|| "parts/core.toml".into());
        assert_eq!(origin, expected, "{id}");
    }
    assert_eq!(split, flat);
}

#[test]
fn an_included_files_section_docs_take_its_section_and_folder() {
    let dir = tempdir().unwrap();
    let list = split(dir.path());
    let source = load_modlist_with_docs(&list).unwrap();
    let docs = &source.sections["Core - Patches"];
    assert_eq!(docs.summary.as_deref(), Some("The patches."));
    assert_eq!(docs.images[0].path.as_deref(), Some("parts/img/p.png"));
    assert_eq!(source.included, [dir.path().join("parts/core.toml")]);
}

#[test]
fn included_plugins_the_load_order_does_not_place_are_refused() {
    let dir = tempdir().unwrap();
    write(dir.path(), "parts/core.toml", CORE);
    let list = write(
        dir.path(),
        "list.toml",
        &LIST.replace("    { include = \"Core\" },\n", ""),
    );
    let err = load_modlist(&list).unwrap_err().to_string();
    assert!(
        err.contains("include 'Core' has plugins (A.esp, B.esp)"),
        "{err}"
    );
    assert!(err.contains("{ include = \"Core\" }"), "{err}");
}

#[test]
fn placing_an_include_nothing_names_is_refused() {
    let dir = tempdir().unwrap();
    split(dir.path());
    let list = write(
        dir.path(),
        "list.toml",
        &LIST.replace("{ include = \"Core\" }", "{ include = \"Cor\" }"),
    );
    let err = load_modlist(&list).unwrap_err().to_string();
    assert!(err.contains("{ include = \"Cor\" }"), "{err}");
}

#[test]
fn a_file_including_itself_is_refused() {
    let dir = tempdir().unwrap();
    write(
        dir.path(),
        "parts/core.toml",
        "component = true\n\n[[mods]]\nid = \"Again\"\ninclude = \"../list.toml\"\n",
    );
    let list = write(dir.path(), "list.toml", LIST);
    let err = format!("{:#}", load_modlist(&list).unwrap_err());
    assert!(err.contains("includes itself"), "{err}");
}

#[test]
fn an_include_entry_with_a_mods_fields_is_refused() {
    let dir = tempdir().unwrap();
    write(dir.path(), "parts/core.toml", CORE);
    let list = write(
        dir.path(),
        "list.toml",
        &LIST.replace(
            "include = \"parts/core.toml\"",
            "include = \"parts/core.toml\"\nplugins = [\"X.esp\"]",
        ),
    );
    let err = load_modlist(&list).unwrap_err().to_string();
    assert!(err.contains("only `id`, `section` and `include`"), "{err}");
    assert!(err.contains("plugins"), "{err}");
}

#[test]
fn questions_in_an_included_file_are_refused_for_now() {
    let dir = tempdir().unwrap();
    write(
        dir.path(),
        "parts/core.toml",
        &format!("{CORE}\n[inputs.hd]\nquery = \"HD?\"\ntype = \"bool\"\n"),
    );
    let list = write(dir.path(), "list.toml", LIST);
    let err = format!("{:#}", load_modlist(&list).unwrap_err());
    assert!(err.contains("questions ([inputs])"), "{err}");
}

#[test]
fn an_ini_key_set_two_ways_is_refused() {
    let dir = tempdir().unwrap();
    write(dir.path(), "parts/core.toml", CORE);
    let list = write(
        dir.path(),
        "list.toml",
        &format!("{LIST}\n[ini.Display]\niMaxFPS = 90\n"),
    );
    let err = format!("{:#}", load_modlist(&list).unwrap_err());
    assert!(err.contains("[ini.Display] iMaxFPS"), "{err}");
}

#[test]
fn a_validation_error_names_the_file_the_mod_is_in() {
    let dir = tempdir().unwrap();
    write(dir.path(), "parts/img/p.png", "");
    write(
        dir.path(),
        "parts/core.toml",
        &CORE.replace("id = \"A\"", "id = \"A\"\ndependencies = [\"Nope\"]"),
    );
    let list = write(dir.path(), "list.toml", LIST);
    let err = validate(&load_modlist(&list).unwrap())
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown dependency Nope"), "{err}");
    assert!(err.contains("(mod A is in parts/core.toml)"), "{err}");
}

#[test]
fn a_component_validates_alone_but_is_not_planned() {
    let dir = tempdir().unwrap();
    write(dir.path(), "img/p.png", "");
    let core = write(dir.path(), "core.toml", CORE);
    // Its `if` asks the including list's question.
    let compiled = compile(&core, true).unwrap().modlist;
    let err = mudcrab::config::query::build_plan(&compiled, &Default::default(), None)
        .unwrap_err()
        .to_string();
    assert!(err.contains("is a component"), "{err}");
}

#[test]
fn an_included_mods_fields_are_set_in_its_own_file() {
    let dir = tempdir().unwrap();
    let list = split(dir.path());
    let pairs = doc::parse_pairs(&["oracle_name=\"Alpha\"".to_string()]).unwrap();
    let edit = edit::plan_mod(&list, "A", |text, _| {
        doc::set_fields(text, "A", None, &pairs, &[])
    })
    .unwrap();
    assert_eq!(edit.path, dir.path().join("parts/core.toml"));
    edit.write().unwrap();
    assert_eq!(std::fs::read_to_string(&list).unwrap(), LIST);
    let source = load_modlist(&list).unwrap();
    let a = source.mods.iter().find(|m| m.id == "A").unwrap();
    assert_eq!(a.oracle_name.as_deref(), Some("Alpha"));
}

#[test]
fn an_edit_to_an_included_file_is_checked_against_the_whole_list() {
    let dir = tempdir().unwrap();
    let list = split(dir.path());
    // Valid in the file alone, but the including list already places Last.esp.
    let file = dir.path().join("parts/core.toml");
    let err = edit::plan_in(&list, &file, |text, _| {
        Ok(text.replace("    \"B.esp\",\n", "    \"B.esp\",\n    \"Last.esp\",\n"))
    })
    .unwrap_err();
    assert!(
        format!("{err:#}").contains("duplicate plugin Last.esp"),
        "{err:#}"
    );
    assert!(
        format!("{err:#}").contains("nothing was written"),
        "{err:#}"
    );
}

#[test]
fn removing_an_included_mod_edits_its_file() {
    let dir = tempdir().unwrap();
    let list = split(dir.path());
    let edits = edit::plan_remove(&list, "B").unwrap();
    assert_eq!(edits.len(), 1);
    edits[0].write().unwrap();
    let core = std::fs::read_to_string(dir.path().join("parts/core.toml")).unwrap();
    assert!(!core.contains("id = \"B\""), "{core}");
    assert!(!core.contains("\"B.esp\""), "{core}");
    assert_eq!(
        load_modlist(&list).unwrap().plugins,
        ["Oblivion.esm", "A.esp", "Last.esp"]
    );
}

#[test]
fn a_plugin_moves_within_the_file_that_places_it() {
    let dir = tempdir().unwrap();
    let list = split(dir.path());
    edit::plan_plugin_move(&list, "B.esp", &doc::PluginPosition::First)
        .unwrap()
        .write()
        .unwrap();
    assert_eq!(
        load_modlist(&list).unwrap().plugins,
        ["Oblivion.esm", "B.esp", "A.esp", "Last.esp"]
    );

    let err = edit::plan_plugin_move(
        &list,
        "A.esp",
        &doc::PluginPosition::After("Last.esp".to_string()),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("moves only within the file"), "{err}");
}

#[test]
fn a_mod_moves_within_its_file_and_keeps_its_included_section() {
    let dir = tempdir().unwrap();
    let list = split(dir.path());
    let placement = mudcrab::config::add::Placement {
        before_mod: Some("A".to_string()),
        before_section: None,
    };
    let section = ["Core".to_string(), "Patches".to_string()];
    let (edit, _) = edit::plan_mod_move(&list, "B", Some(&section), &placement).unwrap();
    edit.write().unwrap();
    let source = load_modlist(&list).unwrap();
    let ids: Vec<&str> = source.mods.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["First", "B", "A", "Last"]);
    assert_eq!(source.mods[1].section, ["Core", "Patches"]);
    let core = std::fs::read_to_string(dir.path().join("parts/core.toml")).unwrap();
    assert!(core.contains("section = [\"Patches\"]"), "{core}");

    let placement = mudcrab::config::add::Placement {
        before_mod: Some("Last".to_string()),
        before_section: None,
    };
    let err = edit::plan_mod_move(&list, "A", None, &placement)
        .unwrap_err()
        .to_string();
    assert!(err.contains("moves only within the file"), "{err}");

    let err = edit::plan_mod_move(&list, "A", Some(&["End".to_string()]), &Default::default())
        .unwrap_err()
        .to_string();
    assert!(err.contains("is under section Core"), "{err}");
}

#[test]
fn a_mod_added_before_an_included_one_goes_in_its_file() {
    let dir = tempdir().unwrap();
    let list = split(dir.path());
    let place = edit::place_beside(&list, Some("B")).unwrap();
    assert_eq!(place.file, dir.path().join("parts/core.toml"));
    assert_eq!(place.sections, ["Core"]);
    assert_eq!(
        place.local(&["Core".to_string()]).unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(edit::place_beside(&list, None).unwrap().file, list);
}

/// MOFAM's way of splitting: each Part in its own file, the load order all
/// in the list's.
fn split_order_kept(dir: &Path) -> std::path::PathBuf {
    write(dir, "parts/img/p.png", "");
    write(
        dir,
        "parts/core.toml",
        &CORE.replace("plugins = [\n    \"A.esp\",\n    \"B.esp\",\n]\n", ""),
    );
    write(
        dir,
        "list.toml",
        &LIST.replace("{ include = \"Core\" }", "\"A.esp\",\n    \"B.esp\""),
    )
}

#[test]
fn an_included_mods_plugins_can_be_placed_by_the_list() {
    let dir = tempdir().unwrap();
    let list = split_order_kept(dir.path());
    validate(&load_modlist(&list).unwrap()).unwrap();

    let edits = edit::plan_remove(&list, "B").unwrap();
    let files: Vec<_> = edits.iter().map(|e| e.path.clone()).collect();
    assert_eq!(files, [dir.path().join("parts/core.toml"), list.clone()]);
    for edit in &edits {
        edit.write().unwrap();
    }
    assert_eq!(
        load_modlist(&list).unwrap().plugins,
        ["Oblivion.esm", "A.esp", "Last.esp"]
    );
}

#[test]
fn disabling_and_enabling_an_included_mod_changes_the_list_that_places_its_plugins() {
    let dir = tempdir().unwrap();
    let list = split_order_kept(dir.path());
    for edit in edit::plan_enabled(&list, "A", false, None).unwrap() {
        edit.write().unwrap();
    }
    let source = load_modlist(&list).unwrap();
    assert_eq!(source.plugins, ["Oblivion.esm", "B.esp", "Last.esp"]);
    assert!(!source.mods.iter().find(|m| m.id == "A").unwrap().enabled);

    // No mod before it has a plugin, so it goes last, as in a list in one
    // file.
    for edit in edit::plan_enabled(&list, "A", true, None).unwrap() {
        edit.write().unwrap();
    }
    assert_eq!(
        load_modlist(&list).unwrap().plugins,
        ["Oblivion.esm", "B.esp", "Last.esp", "A.esp"]
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("parts/core.toml")).unwrap(),
        CORE.replace("plugins = [\n    \"A.esp\",\n    \"B.esp\",\n]\n", "")
    );
}
