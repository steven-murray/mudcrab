//! One group of mods, written once and included by two lists that want
//! different things from it: the case composability exists for.

use mudcrab::config::loader::load_modlist;
use mudcrab::config::pipeline::compile;
use mudcrab::config::query::build_plan;
use mudcrab::config::validator::validate;
use std::path::Path;
use tempfile::tempdir;

const GROUP: &str = r#"component = true

[inputs.hud]
query = "Install the HUD changes?"
type = "bool"
default = true

[[mods]]
id = "Script Extender"
archives = [{ path = "manual:se.7z" }]

[[mods]]
id = "Menu Fix"
dependencies = ["Script Extender"]
archives = [{ path = "manual:menu.7z" }]

[[mods]]
id = "HUD"
if = "hud && mod:\"Menu Fix\""
archives = [{ path = "manual:hud.7z" }]
"#;

fn write(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

fn planned(path: &Path) -> Vec<String> {
    let compiled = compile(path, true).unwrap().modlist;
    build_plan(&compiled, &Default::default(), None)
        .unwrap()
        .mods
        .into_iter()
        .map(|m| m.id)
        .collect()
}

#[test]
fn two_lists_take_what_they_want_from_one_group() {
    let dir = tempdir().unwrap();
    write(dir.path(), "shared/core.toml", GROUP);
    // One list takes the group as it is, and asks its question.
    let full = write(
        dir.path(),
        "full/list.toml",
        "name = \"Full\"\n\n[[mods]]\nid = \"Core\"\nsection = [\"Core\"]\n\
         include = \"../shared/core.toml\"\n",
    );
    // The other renames it, answers the question, and leaves out a mod.
    let lean = write(
        dir.path(),
        "lean/list.toml",
        "name = \"Lean\"\n\n[[mods]]\nid = \"Core\"\ninclude = \"../shared/core.toml\"\n\
         prefix = \"Core: \"\n\n[mods.set]\nhud = false\n\n[mods.override.\"Menu Fix\"]\n\
         oracle_name = \"MenuFix\"\n",
    );
    validate(&load_modlist(&full).unwrap()).unwrap();
    validate(&load_modlist(&lean).unwrap()).unwrap();

    assert_eq!(planned(&full), ["Script Extender", "Menu Fix", "HUD"]);
    assert_eq!(planned(&lean), ["Core: Script Extender", "Core: Menu Fix"]);
    let lean = load_modlist(&lean).unwrap();
    assert_eq!(lean.mods[1].dependencies, ["Core: Script Extender"]);
    assert_eq!(lean.mods[1].oracle_name.as_deref(), Some("MenuFix"));
    assert!(lean.inputs["Core.hud"].fixed.is_some());
}
