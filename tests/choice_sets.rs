//! An archive's choice sets: the first whose `if` holds is installed, its
//! own choices when none does.

use mudcrab::config::pipeline::compile;
use mudcrab::config::query::{build_plan, parse_overrides};
use tempfile::tempdir;

const LIST: &str = r#"name = "Choices"

[inputs.map]
query = "Which map style?"
type = "choice"
choices = ["Elven", "Classic", "Plain"]
default = "Elven"

[[mods]]
id = "Dynamic Map"

[[mods.archives]]
path = "manual:map.zip"
layout = "bain"
bain_subpackages = ["00 Core", "01 Plain"]

[[mods.archives.choices]]
if = "map == Elven"
bain_subpackages = ["00 Core", "01 Elven"]

[[mods.archives.choices]]
if = "map == Classic"
bain_subpackages = ["00 Core", "01 Classic"]
"#;

fn packages(list: &std::path::Path, set: &[&str]) -> Vec<String> {
    let compiled = compile(list, false).unwrap();
    let sets: Vec<String> = set.iter().map(|s| s.to_string()).collect();
    let plan = build_plan(&compiled.modlist, &parse_overrides(&sets).unwrap(), None).unwrap();
    let archive = &plan.mods[0].archives[0];
    assert!(
        archive.choices.is_empty(),
        "the plan holds only what is chosen"
    );
    archive
        .bain_subpackages
        .iter()
        .map(|p| p.name().to_string())
        .collect()
}

#[test]
fn the_first_set_whose_if_holds_is_installed() {
    let dir = tempdir().unwrap();
    let list = dir.path().join("list.toml");
    std::fs::write(&list, LIST).unwrap();
    assert_eq!(packages(&list, &[]), ["00 Core", "01 Elven"]);
    assert_eq!(packages(&list, &["map=Classic"]), ["00 Core", "01 Classic"]);
    // None holds: the archive's own.
    assert_eq!(packages(&list, &["map=Plain"]), ["00 Core", "01 Plain"]);

    // A set's `if` is checked like any other.
    std::fs::write(&list, LIST.replace("map == Classic", "style == Classic")).unwrap();
    let err = compile(&list, false)
        .err()
        .expect("an undeclared input is refused");
    assert!(format!("{err:#}").contains("choice set 2"), "{err:#}");
}
