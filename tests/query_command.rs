use assert_cmd::Command;
use serde_json::Value;
use std::path::PathBuf;
use tempfile::tempdir;

/// Compile the simple fixture and run `query --headless` with `extra` args,
/// returning the plan.
fn headless_plan(extra: &[&str]) -> Value {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("modlists")
        .join("simple.toml");
    try_headless_plan(&fixture, extra).expect("query should succeed")
}

/// Like [`headless_plan`] for a modlist written from `src`.
fn plan_for(src: &str, extra: &[&str]) -> Result<Value, String> {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(&path, src).expect("fixture should be written");
    try_headless_plan(&path, extra)
}

/// Compile and query `fixture`; the plan, or the failing command's stderr.
fn try_headless_plan(fixture: &std::path::Path, extra: &[&str]) -> Result<Value, String> {
    let dir = tempdir().expect("temp dir should be created");
    let compiled = dir.path().join("compiled.json");
    let query_out = dir.path().join("plan.json");

    Command::cargo_bin("mudcrab")
        .map(|mut c| {
            c.env(
                "XDG_STATE_HOME",
                std::env::temp_dir().join("mudcrab-test-state"),
            )
            .env(
                "LOCALAPPDATA",
                std::env::temp_dir().join("mudcrab-test-state"),
            );
            c
        })
        .expect("binary should build")
        .arg("compile")
        .arg(fixture)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();
    let out = Command::cargo_bin("mudcrab")
        .map(|mut c| {
            c.env(
                "XDG_STATE_HOME",
                std::env::temp_dir().join("mudcrab-test-state"),
            )
            .env(
                "LOCALAPPDATA",
                std::env::temp_dir().join("mudcrab-test-state"),
            );
            c
        })
        .expect("binary should build")
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&query_out)
        .arg("--headless")
        .args(extra)
        .output()
        .expect("query should run");
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }

    Ok(
        serde_json::from_str(&std::fs::read_to_string(&query_out).expect("plan written"))
            .expect("plan should be valid json"),
    )
}

/// A mod gated on an input, a mod gated on that mod, and an action gated on
/// both: the readme's "include Mod B depending on whether Mod A is included".
const MOD_CONDITIONS: &str = r#"
name = "Mod conditions"

[inputs.hd]
type = "bool"
query = "HD textures?"
default = false

[[mods]]
id = "patch for hd"
if = "mod:\"hd pack\""
type = "empty"

[[mods]]
id = "hd pack"
if = "hd"
type = "empty"

[[mods]]
id = "ui"
type = "generated"

[[mods.actions]]
if = "mod:\"hd pack\" && !hd"
action = "write_file"
path = "never.txt"
content = "x"

[[mods.actions]]
if = "!mod:\"hd pack\" || hd"
action = "write_file"
path = "always.txt"
content = "x"
"#;

fn selected(plan: &Value) -> Vec<&str> {
    plan["selected_mods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}

fn ui_actions(plan: &Value) -> usize {
    plan["mods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == "ui")
        .unwrap()["actions"]
        .as_array()
        .unwrap()
        .len()
}

#[test]
fn a_mod_can_follow_another_mod_declared_after_it() {
    let off = plan_for(MOD_CONDITIONS, &[]).unwrap();
    assert_eq!(selected(&off), ["ui"]);
    assert_eq!(ui_actions(&off), 1);

    let on = plan_for(MOD_CONDITIONS, &["--set", "hd=true"]).unwrap();
    assert_eq!(selected(&on), ["patch for hd", "hd pack", "ui"]);
    assert_eq!(ui_actions(&on), 1);
}

#[test]
fn a_dependency_left_out_by_its_condition_is_an_error() {
    let src = MOD_CONDITIONS.replace(
        "id = \"patch for hd\"\nif = \"mod:\\\"hd pack\\\"\"",
        "id = \"patch for hd\"\ndependencies = [\"hd pack\"]",
    );
    assert_ne!(src, MOD_CONDITIONS);
    let err = plan_for(&src, &[]).unwrap_err();
    assert!(err.contains("depends on hd pack"), "{err}");
    assert!(err.contains("if = 'mod:\"hd pack\"'"), "{err}");
}

#[test]
fn set_overrides_the_declared_default() {
    let plan = headless_plan(&["--set", "use_hd_textures=true"]);
    assert_eq!(plan["responses"]["use_hd_textures"], true);
    assert_eq!(plan["selected_mods"].as_array().unwrap().len(), 2);
}

#[test]
fn query_headless_writes_personalized_plan() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("modlists")
        .join("simple.toml");

    let dir = tempdir().expect("temp dir should be created");
    let compiled = dir.path().join("compiled.json");
    let query_out = dir.path().join("plan.json");
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    let mut compile_cmd = Command::cargo_bin("mudcrab")
        .map(|mut c| {
            c.env(
                "XDG_STATE_HOME",
                std::env::temp_dir().join("mudcrab-test-state"),
            )
            .env(
                "LOCALAPPDATA",
                std::env::temp_dir().join("mudcrab-test-state"),
            );
            c
        })
        .expect("binary should build");
    compile_cmd
        .env("GAME_DIR", &game_dir)
        .arg("compile")
        .arg(&fixture)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();

    let mut query_cmd = Command::cargo_bin("mudcrab")
        .map(|mut c| {
            c.env(
                "XDG_STATE_HOME",
                std::env::temp_dir().join("mudcrab-test-state"),
            )
            .env(
                "LOCALAPPDATA",
                std::env::temp_dir().join("mudcrab-test-state"),
            );
            c
        })
        .expect("binary should build");
    query_cmd
        .env("GAME_DIR", &game_dir)
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&query_out)
        .arg("--headless")
        .assert()
        .success();

    let raw = std::fs::read_to_string(&query_out).expect("query output should be written");
    let json: Value = serde_json::from_str(&raw).expect("query output should be valid json");

    assert_eq!(json["name"], "Simple Skyrim Core");
    assert_eq!(json["responses"]["use_hd_textures"], false);

    let selected_mods = json["selected_mods"]
        .as_array()
        .expect("selected_mods should be an array");
    assert_eq!(selected_mods.len(), 1);
    assert_eq!(selected_mods[0], "skyui");
}

/// One "resolution" answer reaching an archive, a FOMOD row and a BAIN
/// subpackage -- B2's "1K or 2K textures?".
const ARCHIVE_CONDITIONS: &str = r#"
name = "Archive conditions"

[inputs.res]
type = "choice"
query = "Texture resolution?"
choices = ["1k", "2k"]
default = "1k"

[[mods]]
id = "textures"

[[mods.archives]]
path = "manual:textures-1k.7z"

[[mods.archives]]
if = "res == 2k"
path = "manual:textures-2k.7z"

[[mods]]
id = "fomod mod"

[[mods.archives]]
path = "manual:fomod.7z"
layout = "fomod"

[[mods.archives.fomod_selections]]
if = "res == 1k"
step = "Textures"
group = "Resolution"
options = ["1K"]

[[mods.archives.fomod_selections]]
if = "res == 2k"
step = "Textures"
group = "Resolution"
options = ["2K"]

[[mods]]
id = "bain mod"

[[mods.archives]]
path = "manual:bain.7z"
layout = "bain"
bain_subpackages = ["00 Core", { name = "01 2K Textures", if = "res == 2k" }]
"#;

fn mod_entry<'a>(plan: &'a Value, id: &str) -> &'a Value {
    plan["mods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == id)
        .unwrap()
}

#[test]
fn one_answer_reaches_archives_fomod_rows_and_bain_subpackages() {
    let low = plan_for(ARCHIVE_CONDITIONS, &[]).unwrap();
    let archives = mod_entry(&low, "textures")["archives"].as_array().unwrap();
    assert_eq!(archives.len(), 1);
    assert_eq!(archives[0]["path"], "manual:textures-1k.7z");
    let selections = &mod_entry(&low, "fomod mod")["archives"][0]["fomod_selections"];
    assert_eq!(selections.as_array().unwrap().len(), 1);
    assert_eq!(selections[0]["options"][0], "1K");
    assert_eq!(
        mod_entry(&low, "bain mod")["archives"][0]["bain_subpackages"],
        serde_json::json!(["00 Core"])
    );

    let high = plan_for(ARCHIVE_CONDITIONS, &["--set", "res=2k"]).unwrap();
    assert_eq!(
        mod_entry(&high, "textures")["archives"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        mod_entry(&high, "fomod mod")["archives"][0]["fomod_selections"][0]["options"][0],
        "2K"
    );
    assert_eq!(
        mod_entry(&high, "bain mod")["archives"][0]["bain_subpackages"],
        serde_json::json!(["00 Core", "01 2K Textures"])
    );

    // Nothing downstream of query sees a condition.
    let text = high.to_string();
    assert!(!text.contains("\"if\""), "{text}");
}

#[test]
fn conditions_that_leave_a_mod_with_no_archive_are_an_error() {
    let src = ARCHIVE_CONDITIONS.replace(
        "[[mods.archives]]\npath = \"manual:textures-1k.7z\"",
        "[[mods.archives]]\nif = \"res == 2k\"\npath = \"manual:textures-1k.7z\"",
    );
    assert_ne!(src, ARCHIVE_CONDITIONS);
    let err = plan_for(&src, &[]).unwrap_err();
    assert!(
        err.contains("mod textures") && err.contains("on the mod instead"),
        "{err}"
    );

    let src = ARCHIVE_CONDITIONS.replace(
        "[\"00 Core\", ",
        "[{ name = \"00 Core\", if = \"res == 2k\" }, ",
    );
    assert_ne!(src, ARCHIVE_CONDITIONS);
    let err = plan_for(&src, &[]).unwrap_err();
    assert!(
        err.contains("BAIN subpackage") && err.contains("manual:bain.7z"),
        "{err}"
    );
}

const PLUGINS: &str = r#"
name = "Plugins"
plugins = ["Base.esp", "Rats.esp", "Merged.esp"]

[inputs.rats]
type = "bool"
query = "Rats?"
default = false

[[mods]]
id = "base"
plugins = ["Base.esp"]

[[mods.archives]]
path = "manual:base.7z"

[[mods]]
id = "rats"
if = "rats"
plugins = ["Rats.esp"]

[[mods.archives]]
path = "manual:rats.7z"
"#;

#[test]
fn a_left_out_mod_takes_its_plugins_out_of_the_load_order() {
    let src = PLUGINS.replace(", \"Merged.esp\"", "");
    let off = plan_for(&src, &[]).unwrap();
    assert_eq!(off["plugins"], serde_json::json!(["Base.esp"]));
    let on = plan_for(&src, &["--set", "rats=true"]).unwrap();
    assert_eq!(on["plugins"], serde_json::json!(["Base.esp", "Rats.esp"]));
}

#[test]
fn a_merge_using_a_left_out_mods_plugin_is_refused() {
    // Merge sources leave the load order; the merged plugin takes their place.
    let base = PLUGINS.replace("\"Base.esp\", \"Rats.esp\", ", "");
    let src = format!(
        "{base}\n[[mods]]\nid = \"merged\"\ntype = \"merge\"\n\n[mods.merge]\noutput = \"Merged.esp\"\n\
         method = \"clobber\"\nsources = [{{ mod = \"base\", plugin = \"Base.esp\" }}, {{ mod = \"rats\", plugin = \"Rats.esp\" }}]\n"
    );
    let err = plan_for(&src, &[]).unwrap_err();
    assert!(
        err.contains("merge merged takes Rats.esp from mod rats"),
        "{err}"
    );

    // Gated the same way, both go together, merged plugin included.
    let gated = src.replace("id = \"merged\"", "id = \"merged\"\nif = \"mod:rats\"");
    let off = plan_for(&gated, &[]).unwrap();
    assert_eq!(off["plugins"], serde_json::json!([]));
    let on = plan_for(&gated, &["--set", "rats=true"]).unwrap();
    assert_eq!(on["plugins"], serde_json::json!(["Merged.esp"]));
}
