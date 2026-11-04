//! `optional` on a mod: each group of optional mods asked as one `multi`
//! question of its mods, gathered from every file of the list.

use mudcrab::config::loader::load_modlist;
use mudcrab::config::pipeline::compile;
use mudcrab::config::query::{build_plan, parse_overrides};
use mudcrab::config::schema::{InputType, InputValue};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

const LIST: &str = r#"name = "Optional"

[inputs.big]
query = "Big textures?"
type = "bool"
default = true

[[mods]]
id = "Core"
archives = [{ path = "manual:core.zip" }]

[[mods]]
id = "Birds"
optional = true
archives = [{ path = "manual:birds.zip" }]

[[mods]]
id = "Sharp Rocks"
optional = { group = "Better textures", default = true }
archives = [{ path = "manual:rocks.zip" }]

[[mods]]
id = "Huge Trees"
optional = "Better textures"
if = "big"
archives = [{ path = "manual:trees.zip" }]

[[mods]]
id = "extra"
include = "extra.toml"
"#;

const EXTRA: &str = r#"name = "Extra"

[[mods]]
id = "Rats"
optional = true
archives = [{ path = "manual:rats.zip" }]
"#;

fn plan(list: &Path, sets: &[&str]) -> anyhow::Result<Vec<String>> {
    let compiled = compile(list, false)?;
    let sets: Vec<String> = sets.iter().map(|s| s.to_string()).collect();
    let plan = build_plan(&compiled.modlist, &parse_overrides(&sets)?, None)?;
    Ok(plan.mods.into_iter().map(|m| m.id).collect())
}

#[test]
fn each_group_is_one_question_of_its_mods() {
    let dir = tempdir().unwrap();
    let list = write(dir.path(), "list.toml", LIST);
    write(dir.path(), "extra.toml", EXTRA);

    let loaded = load_modlist(&list).unwrap();
    let own = &loaded.inputs["optional"];
    assert_eq!(own.input_type, InputType::Multi);
    // Gathered from the included file too.
    assert_eq!(own.choices, ["Birds", "Rats"]);
    let textures = &loaded.inputs["better_textures"];
    assert_eq!(textures.choices, ["Sharp Rocks", "Huge Trees"]);
    assert!(matches!(&textures.default, Some(InputValue::List(l)) if l == &["Sharp Rocks"]));
    assert!(
        textures.query.starts_with("Better textures"),
        "{}",
        textures.query
    );

    // Headless: what starts ticked.
    assert_eq!(plan(&list, &[]).unwrap(), ["Core", "Sharp Rocks"]);
    assert_eq!(
        plan(&list, &["optional=Rats", "better_textures=Huge Trees"]).unwrap(),
        ["Core", "Huge Trees", "Rats"]
    );
    // Picked, and its own `if` too.
    assert_eq!(
        plan(&list, &["big=false", "better_textures=Huge Trees"]).unwrap(),
        ["Core"]
    );
}

#[test]
fn a_group_may_be_asked_as_the_list_words_it() {
    let dir = tempdir().unwrap();
    let worded = LIST.replace(
        "[inputs.big]",
        "[inputs.optional]\nquery = \"Any extras?\"\ntype = \"multi\"\nchoices = [\"Owls\"]\n\n[inputs.big]",
    );
    let list = write(dir.path(), "list.toml", &worded);
    write(dir.path(), "extra.toml", EXTRA);
    let loaded = load_modlist(&list).unwrap();
    assert_eq!(loaded.inputs["optional"].query, "Any extras?");
    assert_eq!(loaded.inputs["optional"].choices, ["Owls", "Birds", "Rats"]);

    let wrong = LIST.replace(
        "[inputs.big]",
        "[inputs.optional]\nquery = \"Extras?\"\ntype = \"bool\"\n\n[inputs.big]",
    );
    let list = write(dir.path(), "list.toml", &wrong);
    let err = load_modlist(&list).unwrap_err();
    assert!(format!("{err:#}").contains("is a multi"), "{err:#}");
}

#[test]
fn the_guide_says_which_group() {
    let dir = tempdir().unwrap();
    let list = write(dir.path(), "list.toml", LIST);
    write(dir.path(), "extra.toml", EXTRA);
    let loaded = load_modlist(&list).unwrap();
    let doc = mudcrab::export::build_document(&loaded, None, None).unwrap();
    let json = serde_json::to_value(&doc).unwrap();
    let text = json.to_string();
    assert!(
        text.contains(
            r#""optional":{"default":true,"group":"Better textures","input":"better_textures""#
        ),
        "{text}"
    );
}
