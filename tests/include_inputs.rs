//! Questions an included file asks: named for the include, asked of whoever
//! installs the list unless the list answers them with `set`.

use mudcrab::config::loader::load_modlist;
use mudcrab::config::pipeline::compile;
use mudcrab::config::query::{Overrides, build_plan};
use mudcrab::config::schema::InputValue;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

const PART: &str = r#"component = true

[inputs.hd]
query = "Use the HD textures?"
type = "bool"
default = false

[inputs.size]
query = "Which size?"
type = "choice"
choices = ["small", "large"]
default = "small"

[[mods]]
id = "Textures"
if = "hd"
archives = [{ path = "manual:hd.7z" }]

[[mods]]
id = "Sized"
archives = [
    { path = "manual:small.7z", if = "size == small" },
    { path = "manual:large.7z", if = "size == large" },
]

[[mods]]
id = "Asks the list"
if = "minimal"
archives = [{ path = "manual:m.7z" }]
"#;

fn list(dir: &Path, options: &str) -> PathBuf {
    std::fs::write(dir.join("part.toml"), PART).unwrap();
    let path = dir.join("list.toml");
    std::fs::write(
        &path,
        format!(
            "name = \"L\"\n\n[inputs.minimal]\nquery = \"Minimal?\"\ntype = \"bool\"\n\
             default = true\n\n[[mods]]\nid = \"Shared Part\"\ninclude = \"part.toml\"\n{options}\n"
        ),
    )
    .unwrap();
    path
}

fn planned(path: &Path, answers: &[(&str, &str)]) -> anyhow::Result<Vec<String>> {
    let compiled = compile(path, true)?.modlist;
    let answers: Overrides = answers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Ok(build_plan(&compiled, &answers, None)?
        .mods
        .into_iter()
        .map(|m| m.id)
        .collect())
}

#[test]
fn an_included_files_questions_are_named_for_its_include() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "");
    let source = load_modlist(&path).unwrap();
    let names: Vec<&str> = source.inputs.keys().map(String::as_str).collect();
    assert_eq!(names, ["minimal", "Shared_Part.hd", "Shared_Part.size"]);
    assert_eq!(source.mods[0].condition.as_deref(), Some("Shared_Part.hd"));
    assert_eq!(
        source.mods[1].archives[1].condition.as_deref(),
        Some("Shared_Part.size == large")
    );
    // A question the file does not declare is the including list's.
    assert_eq!(source.mods[2].condition.as_deref(), Some("minimal"));

    assert_eq!(planned(&path, &[]).unwrap(), ["Sized", "Asks the list"]);
    assert_eq!(
        planned(&path, &[("Shared_Part.hd", "true"), ("minimal", "false")]).unwrap(),
        ["Textures", "Sized"]
    );
}

#[test]
fn the_including_list_can_answer_them_for_everyone() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "\n[mods.set]\nhd = true\nsize = \"large\"");
    let source = load_modlist(&path).unwrap();
    assert!(matches!(
        source.inputs["Shared_Part.hd"].fixed,
        Some(InputValue::Bool(true))
    ));
    assert_eq!(
        planned(&path, &[]).unwrap(),
        ["Textures", "Sized", "Asks the list"]
    );
    let err = planned(&path, &[("Shared_Part.hd", "false")])
        .unwrap_err()
        .to_string();
    assert!(err.contains("answers Shared_Part.hd itself"), "{err}");

    // Nobody installing the list is asked them.
    let document = mudcrab::export::build_document(&source, None, None).unwrap();
    let asked: Vec<&str> = document.inputs.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(asked, ["minimal"]);
}

#[test]
fn an_answer_to_no_question_or_of_the_wrong_kind_is_refused() {
    let dir = tempdir().unwrap();
    let err = format!(
        "{:#}",
        load_modlist(&list(dir.path(), "\n[mods.set]\nnope = true")).unwrap_err()
    );
    assert!(
        err.contains("'nope', which is not one of the included file's questions"),
        "{err}"
    );
    let err = format!(
        "{:#}",
        load_modlist(&list(dir.path(), "\n[mods.set]\nsize = \"huge\"")).unwrap_err()
    );
    assert!(err.contains("not one of [\"small\", \"large\"]"), "{err}");
    let err = format!(
        "{:#}",
        load_modlist(&list(dir.path(), "\n[mods.set]\nhd = \"yes\"")).unwrap_err()
    );
    assert!(err.contains("hd is Bool"), "{err}");
}

#[test]
fn a_question_nested_twice_is_named_for_both_includes() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("part.toml"), PART).unwrap();
    std::fs::write(
        dir.path().join("outer.toml"),
        "component = true\n\n[[mods]]\nid = \"Inner\"\ninclude = \"part.toml\"\n",
    )
    .unwrap();
    let path = dir.path().join("list.toml");
    std::fs::write(
        &path,
        "name = \"L\"\n\n[inputs.minimal]\nquery = \"Minimal?\"\ntype = \"bool\"\n\n\
         [[mods]]\nid = \"Outer\"\ninclude = \"outer.toml\"\n\n[mods.set]\n\"Inner.hd\" = true\n",
    )
    .unwrap();
    let source = load_modlist(&path).unwrap();
    assert!(source.inputs["Outer.Inner.hd"].fixed.is_some());
    assert_eq!(source.mods[0].condition.as_deref(), Some("Outer.Inner.hd"));
    mudcrab::config::validator::validate(&source).unwrap();
}
