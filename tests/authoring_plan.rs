//! The list planned for writing it: every mod, the ones the answers leave
//! out too, as each would install if it were in.

use mudcrab::config::pipeline::compile;
use mudcrab::config::query::{build_authoring_plan, build_plan, parse_overrides};
use tempfile::tempdir;

const LIST: &str = r#"name = "Authoring"

[inputs.res]
query = "Texture size?"
type = "choice"
choices = ["1k", "2k"]
default = "1k"

[[mods]]
id = "Core"
archives = [{ path = "manual:core.zip" }]

[[mods]]
id = "Birds"
optional = true
archives = [
  { path = "manual:birds-1k.zip", if = "res == 1k" },
  { path = "manual:birds-2k.zip", if = "res == 2k" },
]

[[mods]]
id = "Nests"
if = 'mod:Birds'
dependencies = ["Birds"]
archives = [{ path = "manual:nests.zip" }]

[[mods]]
id = "Big Only"
optional = true
archives = [{ path = "manual:big.zip", if = "res == 2k" }]
"#;

#[test]
fn left_out_mods_are_planned_as_if_they_were_in() {
    let dir = tempdir().unwrap();
    let list = dir.path().join("list.toml");
    std::fs::write(&list, LIST).unwrap();
    let compiled = compile(&list, false).unwrap();

    // The answers install only Core.
    let install = build_plan(&compiled.modlist, &Default::default(), None).unwrap();
    assert_eq!(install.selected_mods, ["Core"]);

    let authoring = build_authoring_plan(&compiled.modlist, &Default::default()).unwrap();
    let planned: Vec<&str> = authoring.plan.mods.iter().map(|m| m.id.as_str()).collect();
    // Birds and Nests as they would install; Big Only, whose one archive
    // these answers leave out, cannot be, and says why.
    assert_eq!(planned, ["Core", "Birds", "Nests"]);
    assert_eq!(authoring.plan.selected_mods, ["Core"]);
    assert_eq!(authoring.left_out, ["Birds", "Nests", "Big Only"]);
    assert!(
        authoring.notes.iter().any(|n| n.contains("mod Big Only:")),
        "{:?}",
        authoring.notes
    );
    // What a left-out mod installs still follows the answers.
    let birds = &authoring.plan.mods[1];
    assert_eq!(
        birds.archives[0].path.as_deref(),
        Some("manual:birds-1k.zip")
    );
    // Every archive is kept, as it would install, the 2k one too.
    let every: Vec<Option<&str>> = authoring.every_archive["Birds"]
        .iter()
        .map(|a| a.as_ref().and_then(|a| a.path.as_deref()))
        .collect();
    assert_eq!(
        every,
        [Some("manual:birds-1k.zip"), Some("manual:birds-2k.zip")]
    );
    assert_eq!(authoring.holds("res == 2k"), Some(false));
    assert_eq!(authoring.holds("mod:Core"), Some(true));
    let sets = parse_overrides(&["res=2k".to_string()]).unwrap();
    let big = build_authoring_plan(&compiled.modlist, &sets).unwrap();
    assert_eq!(
        big.plan.mods[1].archives[0].path.as_deref(),
        Some("manual:birds-2k.zip")
    );
    assert!(big.plan.mods.iter().any(|m| m.id == "Big Only"));
}

#[test]
fn what_would_stop_an_install_is_noted_instead() {
    let dir = tempdir().unwrap();
    let list = dir.path().join("list.toml");
    // Always in, but needing a mod only 2k answers bring in.
    let extra = "\n[[mods]]\nid = \"Fancy\"\nif = \"res == 2k\"\narchives = [{ path = \"manual:fancy.zip\" }]\n\n[[mods]]\nid = \"Uses Fancy\"\ndependencies = [\"Fancy\"]\narchives = [{ path = \"manual:uses.zip\" }]\n";
    std::fs::write(&list, format!("{LIST}{extra}")).unwrap();
    let compiled = compile(&list, false).unwrap();
    assert!(build_plan(&compiled.modlist, &Default::default(), None).is_err());
    let authoring = build_authoring_plan(&compiled.modlist, &Default::default()).unwrap();
    assert!(
        authoring
            .notes
            .iter()
            .any(|n| n.contains("Uses Fancy is included but depends on Fancy")),
        "{:?}",
        authoring.notes
    );
}
