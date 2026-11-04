//! Questions asked in order, each perhaps only when its `if` holds on the
//! answers before it, with defaults that may follow those answers.

use mudcrab::config::pipeline::compile;
use mudcrab::config::query::{build_plan, parse_overrides, question_states};
use mudcrab::config::schema::InputValue;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

const LIST: &str = r#"name = "Guided"

[inputs.enb]
query = "Use an ENB?"
type = "bool"
default = true

[inputs.potato]
query = "A slow computer?"
type = "bool"
default = false

[inputs.preset]
query = "Which ENB preset?"
type = "choice"
choices = ["low", "high", "ultra"]
default = "high"
if = "enb"
default_when = [{ if = "potato", value = "low" }]

[[mods]]
id = "Light ENB"
if = "preset == low && enb"
archives = [{ path = "manual:light.zip" }]

[[mods]]
id = "Heavy ENB"
if = "preset == ultra && enb"
archives = [{ path = "manual:heavy.zip" }]
"#;

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn responses(
    list: &Path,
    sets: &[&str],
) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
    let compiled = compile(list, false)?;
    let sets: Vec<String> = sets.iter().map(|s| s.to_string()).collect();
    Ok(build_plan(&compiled.modlist, &parse_overrides(&sets)?, None)?.responses)
}

fn text(value: &InputValue) -> String {
    match value {
        InputValue::Bool(b) => b.to_string(),
        InputValue::Text(t) => t.clone(),
        InputValue::List(l) => l.join(","),
    }
}

#[test]
fn a_default_follows_an_earlier_answer() {
    let dir = tempdir().unwrap();
    let list = write(dir.path(), "list.toml", LIST);
    assert_eq!(text(&responses(&list, &[]).unwrap()["preset"]), "high");
    assert_eq!(
        text(&responses(&list, &["potato=true"]).unwrap()["preset"]),
        "low"
    );
    assert_eq!(
        text(&responses(&list, &["potato=true", "preset=ultra"]).unwrap()["preset"]),
        "ultra",
        "an answer given beats a default"
    );
}

#[test]
fn a_question_whose_if_fails_is_not_asked_and_takes_its_default() {
    let dir = tempdir().unwrap();
    let list = write(dir.path(), "list.toml", LIST);
    // Not asked: --set does not reach it.
    let got = responses(&list, &["enb=false", "preset=ultra"]).unwrap();
    assert_eq!(text(&got["preset"]), "high");

    // Asked in the order declared, and only when it applies.
    let compiled = compile(&list, false).unwrap();
    for (enb, expected) in [
        (
            true,
            vec!["Use an ENB?", "A slow computer?", "Which ENB preset?"],
        ),
        (false, vec!["Use an ENB?", "A slow computer?"]),
    ] {
        let mut asked = Vec::new();
        let mut ask = |spec: &mudcrab::config::schema::InputSpec| {
            asked.push(spec.query.clone());
            Ok(match spec.input_type {
                mudcrab::config::schema::InputType::Bool => InputValue::Bool(enb),
                _ => spec.default.clone().unwrap(),
            })
        };
        build_plan(&compiled.modlist, &Default::default(), Some(&mut ask)).unwrap();
        assert_eq!(asked, expected);
    }
}

#[test]
fn a_front_end_sees_which_questions_are_asked() {
    let dir = tempdir().unwrap();
    let list = write(dir.path(), "list.toml", LIST);
    let loaded = mudcrab::config::loader::load_modlist(&list).unwrap();
    let states = question_states(
        &loaded.inputs,
        &parse_overrides(&["enb=true".into(), "potato=true".into()]).unwrap(),
    )
    .unwrap();
    let preset = states.iter().find(|q| q.id == "preset").unwrap();
    assert!(preset.asked);
    assert_eq!(text(preset.default.as_ref().unwrap()), "low");
    let states = question_states(
        &loaded.inputs,
        &parse_overrides(&["enb=false".into()]).unwrap(),
    )
    .unwrap();
    assert!(!states.iter().find(|q| q.id == "preset").unwrap().asked);
}

#[test]
fn a_question_asks_only_about_those_before_it() {
    let dir = tempdir().unwrap();
    for (from, to, said) in [
        ("if = \"enb\"", "if = \"later\"", "not declared before it"),
        (
            "if = \"enb\"",
            "if = \"mod:\\\"Light ENB\\\"\"",
            "asks about mod",
        ),
        ("default = \"high\"\nif", "if", "needs a default"),
        ("value = \"low\"", "value = \"medium\"", "does not fit"),
    ] {
        let text = LIST.replace(from, to).replacen(
            "[[mods]]",
            "[inputs.later]\nquery = \"Later?\"\ntype = \"bool\"\ndefault = false\n\n[[mods]]",
            1,
        );
        let list = write(dir.path(), "list.toml", &text);
        let Err(err) = compile(&list, false) else {
            panic!("{to}: compiled")
        };
        assert!(format!("{err:#}").contains(said), "{to}: {err:#}");
    }
}

#[test]
fn an_included_files_question_asks_about_its_own_questions() {
    let dir = tempdir().unwrap();
    write(dir.path(), "part.toml", LIST);
    let list = write(
        dir.path(),
        "list.toml",
        "name = \"Outer\"\n\n[[mods]]\nid = \"part\"\ninclude = \"part.toml\"\n",
    );
    let got = responses(&list, &["part.potato=true"]).unwrap();
    assert_eq!(text(&got["part.preset"]), "low");
    let got = responses(&list, &["part.enb=false", "part.preset=ultra"]).unwrap();
    assert_eq!(text(&got["part.preset"]), "high");
}
