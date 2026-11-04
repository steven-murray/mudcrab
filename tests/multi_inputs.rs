//! `multi` questions: any number of their choices, asked about with
//! `"choice" in key`.

use mudcrab::config::pipeline::compile;
use mudcrab::config::query::{build_plan, parse_overrides};
use std::path::Path;
use tempfile::tempdir;

const LIST: &str = r#"name = "Multi"

[inputs.extras]
query = "Which extras?"
type = "multi"
choices = ["Birds", "Better Water", "Rats"]
default = ["Birds"]

[[mods]]
id = "Birds"
if = "Birds in extras"
archives = [{ path = "manual:birds.zip" }]

[[mods]]
id = "Water"
if = "\"Better Water\" in extras"
archives = [{ path = "manual:water.zip" }]

[[mods]]
id = "NoRats"
if = "!Rats in extras"
archives = [{ path = "manual:norats.zip" }]
"#;

fn selected(dir: &Path, list: &str, sets: &[&str]) -> anyhow::Result<Vec<String>> {
    let path = dir.join("list.toml");
    std::fs::write(&path, list).unwrap();
    let compiled = compile(&path, false)?;
    let sets: Vec<String> = sets.iter().map(|s| s.to_string()).collect();
    let plan = build_plan(&compiled.modlist, &parse_overrides(&sets)?, None)?;
    Ok(plan.mods.into_iter().map(|m| m.id).collect())
}

#[test]
fn the_default_and_given_answers_pick_mods() {
    let dir = tempdir().unwrap();
    assert_eq!(
        selected(dir.path(), LIST, &[]).unwrap(),
        ["Birds", "NoRats"]
    );
    assert_eq!(
        selected(dir.path(), LIST, &["extras=Rats, Better Water"]).unwrap(),
        ["Water"]
    );
    assert_eq!(
        selected(dir.path(), LIST, &["extras="]).unwrap(),
        ["NoRats"],
        "none picked"
    );
    let err = selected(dir.path(), LIST, &["extras=Bats"]).unwrap_err();
    assert!(format!("{err:#}").contains("Bats is not one of"), "{err:#}");
}

#[test]
fn a_condition_must_fit_the_question() {
    let dir = tempdir().unwrap();
    for (bad, said) in [
        ("Bats in extras", "not one of its choices"),
        ("extras == Birds", "is a multi input"),
    ] {
        let list = LIST.replace("Birds in extras", bad);
        let err = selected(dir.path(), &list, &[]).unwrap_err();
        assert!(format!("{err:#}").contains(said), "{bad}: {err:#}");
    }
    let list = LIST.replace("default = [\"Birds\"]", "default = [\"Cats\"]");
    assert!(selected(dir.path(), &list, &[]).is_err());
    let list = LIST.replace("\"Rats\"]", "\"Rats, big\"]");
    let err = selected(dir.path(), &list, &[]).unwrap_err();
    assert!(format!("{err:#}").contains("comma"), "{err:#}");
}

#[test]
fn an_in_term_reads_back_as_written() {
    use mudcrab::config::condition::Condition;
    for text in [
        "Birds in extras",
        "\"Better Water\" in extras && !Rats in extras",
    ] {
        assert_eq!(Condition::parse(text).unwrap().to_text(), text);
    }
}
