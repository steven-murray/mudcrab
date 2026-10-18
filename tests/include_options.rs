//! What an including list can do to what it includes: `only`, `except`,
//! `prefix`, `if`, and `override` with `skip` and `append`.

use mudcrab::config::edit::{self, doc};
use mudcrab::config::loader::load_modlist;
use mudcrab::config::pipeline::compile;
use mudcrab::config::query::{Overrides, build_plan};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

const PART: &str = r#"component = true

plugins = ["A.esp", "B.esp"]

[[mods]]
id = "A"
plugins = ["A.esp"]
archives = [{ path = "manual:a.7z" }]

[[mods]]
id = "B"
dependencies = ["A"]
if = "mod:A || hd"
plugins = ["B.esp"]
archives = [{ path = "manual:b.7z" }]

[[mods.actions]]
action = "file_hide"
paths = ["textures/b.dds"]

[[mods]]
id = "C"
archives = [{ path = "manual:c.7z" }]
"#;

/// A list including `PART` as "Part", with `options` on the include entry.
fn list(dir: &Path, options: &str) -> PathBuf {
    std::fs::write(dir.join("part.toml"), PART).unwrap();
    let path = dir.join("list.toml");
    std::fs::write(
        &path,
        format!(
            "name = \"L\"\nplugins = [{{ include = \"Part\" }}]\n\n[inputs.hd]\nquery = \"HD?\"\n\
             type = \"bool\"\ndefault = false\n\n[[mods]]\nid = \"Part\"\nsection = [\"P\"]\n\
             include = \"part.toml\"\n{options}\n"
        ),
    )
    .unwrap();
    path
}

fn ids(path: &Path) -> Vec<String> {
    load_modlist(path)
        .unwrap()
        .mods
        .into_iter()
        .map(|m| m.id)
        .collect()
}

fn error(path: &Path) -> String {
    format!("{:#}", load_modlist(path).unwrap_err())
}

#[test]
fn only_and_except_take_some_of_the_mods_and_their_plugins() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "only = [\"A\", \"C\"]");
    assert_eq!(ids(&path), ["A", "C"]);
    assert_eq!(load_modlist(&path).unwrap().plugins, ["A.esp"]);

    let path = list(dir.path(), "except = [\"B\"]");
    assert_eq!(ids(&path), ["A", "C"]);
    assert_eq!(load_modlist(&path).unwrap().plugins, ["A.esp"]);
}

#[test]
fn an_override_can_skip_a_mod() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "[mods.override.C]\nskip = true");
    assert_eq!(ids(&path), ["A", "B"]);
}

#[test]
fn an_option_naming_a_mod_the_file_lacks_is_refused() {
    let dir = tempdir().unwrap();
    for options in [
        "only = [\"Z\"]",
        "except = [\"Z\"]",
        "[mods.override.Z]\nskip = true",
    ] {
        let err = error(&list(dir.path(), options));
        assert!(
            err.contains("'Z', which is not one of the included file's mods"),
            "{options}: {err}"
        );
    }
}

#[test]
fn an_override_replaces_fields_and_appends_to_lists() {
    let dir = tempdir().unwrap();
    let path = list(
        dir.path(),
        "[mods.override.B]\noracle_name = \"Bee\"\nactions.append = [{ action = \"file_hide\", \
         paths = [\"textures/c.dds\"] }]\n\n[mods.override.C]\ndocs = { summary = \"Mine.\" }",
    );
    let source = load_modlist(&path).unwrap();
    let b = &source.mods[1];
    assert_eq!(b.oracle_name.as_deref(), Some("Bee"));
    assert_eq!(b.actions.len(), 2);
    // Untouched fields are the file's.
    assert_eq!(b.dependencies, ["A"]);
    assert_eq!(b.origin, Some(dir.path().join("part.toml")));
    assert_eq!(source.mods[2].docs.summary.as_deref(), Some("Mine."));
}

#[test]
fn an_override_of_the_id_or_of_no_field_is_refused() {
    let dir = tempdir().unwrap();
    let err = error(&list(dir.path(), "[mods.override.A]\nid = \"Other\""));
    assert!(err.contains("`id` cannot be overridden"), "{err}");
    let err = error(&list(dir.path(), "[mods.override.A]\ncolour = \"red\""));
    assert!(
        err.contains("override of 'A'") && err.contains("colour"),
        "{err}"
    );
    let err = error(&list(
        dir.path(),
        "[mods.override.A]\nplugins.append = \"X.esp\"",
    ));
    assert!(err.contains("takes a list"), "{err}");
}

#[test]
fn a_prefix_renames_the_mods_and_what_they_say_about_each_other() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "prefix = \"Part: \"");
    let source = load_modlist(&path).unwrap();
    let ids: Vec<&str> = source.mods.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["Part: A", "Part: B", "Part: C"]);
    let b = &source.mods[1];
    assert_eq!(b.dependencies, ["Part: A"]);
    assert_eq!(b.condition.as_deref(), Some("mod:\"Part: A\" || hd"));
    assert_eq!(b.written_id.as_deref(), Some("B"));
    mudcrab::config::validator::validate(&source).unwrap();
}

#[test]
fn a_prefixed_mod_is_edited_by_the_id_its_file_has() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "prefix = \"Part: \"");
    let pairs = doc::parse_pairs(&["oracle_name=\"Bee\"".to_string()]).unwrap();
    edit::plan_mod(&path, "Part: B", |text, _, id| {
        assert_eq!(id, "B");
        doc::set_fields(text, id, None, &pairs, &[])
    })
    .unwrap()
    .write()
    .unwrap();
    let source = load_modlist(&path).unwrap();
    assert_eq!(source.mods[1].oracle_name.as_deref(), Some("Bee"));

    for edit in edit::plan_enabled(&path, "Part: C", false, None).unwrap() {
        edit.write().unwrap();
    }
    assert!(!load_modlist(&path).unwrap().mods[2].enabled);
}

#[test]
fn a_field_an_override_sets_is_not_edited_in_the_file() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "[mods.override.A]\noracle_name = \"Ay\"");
    let err = edit::refuse_overridden(&path, "A", None, &["oracle_name".to_string()])
        .unwrap_err()
        .to_string();
    assert!(err.contains("overrides its `oracle_name`"), "{err}");
    edit::refuse_overridden(&path, "A", None, &["dependencies".to_string()]).unwrap();
}

#[test]
fn an_includes_if_holds_for_every_mod_it_brings() {
    let dir = tempdir().unwrap();
    let path = list(dir.path(), "if = \"!hd\"");
    let source = load_modlist(&path).unwrap();
    assert_eq!(source.mods[0].condition.as_deref(), Some("!hd"));
    assert_eq!(
        source.mods[1].condition.as_deref(),
        Some("!hd && mod:A || !hd && hd")
    );

    let compiled = compile(&path, true).unwrap().modlist;
    let planned = |hd: bool| -> Vec<String> {
        let answers = Overrides::from([("hd".to_string(), hd.to_string())]);
        build_plan(&compiled, &answers, None)
            .unwrap()
            .mods
            .into_iter()
            .map(|m| m.id)
            .collect()
    };
    assert_eq!(planned(false), ["A", "B", "C"]);
    assert!(planned(true).is_empty());
}

#[test]
fn a_mod_with_an_includes_options_is_refused() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("list.toml");
    std::fs::write(
        &path,
        "name = \"L\"\n\n[[mods]]\nid = \"A\"\nprefix = \"x\"\narchives = [{ path = \"manual:a.7z\" }]\n",
    )
    .unwrap();
    assert!(error(&path).contains("mod 'A' has prefix, which only an include has"));
}
