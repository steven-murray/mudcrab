//! `export`: the document every exporter reads, and where its prose comes from.

use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;
use tempfile::tempdir;

fn mudcrab() -> Command {
    let mut cmd = Command::cargo_bin("mudcrab").expect("binary builds");
    let state = std::env::temp_dir().join("mudcrab-test-state");
    cmd.env("XDG_STATE_HOME", &state)
        .env("LOCALAPPDATA", &state);
    cmd
}

const MODLIST: &str = r#"
name = "Docs"
plugins = ["Rats.esp"]
docs_file = "docs.toml"

inputs.rats.type = "bool"
inputs.rats.query = "Install the rats?"
inputs.rats.default = true

[docs]
summary = "A tiny list."

[sections."1 - AUDIO"]
summary = "Sounds."
images = [{ path = "shots/audio.png", url = "https://example.com/audio.png", caption = "Ears" }]
mood = "loud"

[[mods]]
id = "Rats"
section = ["1 - AUDIO", "Creatures"]
if = "rats"
plugins = ["Rats.esp"]

[[mods.archives]]
path = "nexus:oblivion/52661/1000033714"
layout = "bain"
bain_subpackages = ["00 Core"]

[[mods.actions]]
action = "ini_set"
file = "ini/rats.ini"
key = "rats.swear"
value = 1

[[mods]]
id = "Bells"
section = ["1 - AUDIO"]

[[mods.archives]]
path = "manual:bells.7z"
"#;

const DOCS_FILE: &str = r#"
[mods."Rats"]
summary = "Rats that swear."
body = "Optional, as the guide says."
"#;

fn write_list(dir: &Path, modlist: &str, docs: &str) -> std::path::PathBuf {
    std::fs::create_dir_all(dir.join("shots")).unwrap();
    std::fs::write(dir.join("shots/audio.png"), b"png").unwrap();
    std::fs::write(dir.join("docs.toml"), docs).unwrap();
    let path = dir.join("modlist.toml");
    std::fs::write(&path, modlist).unwrap();
    path
}

fn export(path: &Path, extra: &[&str]) -> Result<Value, String> {
    let out = mudcrab()
        .arg("export")
        .arg(path)
        .args(extra)
        .output()
        .expect("runs");
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(serde_json::from_slice(&out.stdout).expect("json on stdout"))
}

#[test]
fn json_is_a_tree_of_sections_with_prose_and_words() {
    let dir = tempdir().unwrap();
    let doc = export(&write_list(dir.path(), MODLIST, DOCS_FILE), &[]).unwrap();

    assert_eq!(doc["docs"]["summary"], "A tiny list.");
    assert_eq!(doc["inputs"][0]["query"], "Install the rats?");
    assert_eq!(doc["mod_count"], 2);

    let audio = &doc["entries"][0];
    assert_eq!(audio["kind"], "section");
    assert_eq!(audio["anchor"], "1-audio");
    assert_eq!(
        audio["docs"]["mood"], "loud",
        "unknown keys reach templates"
    );
    assert_eq!(audio["docs"]["images"][0]["caption"], "Ears");

    // Nested section first, then the mod that follows it in install order.
    let creatures = &audio["entries"][0];
    assert_eq!(creatures["name"], "1 - AUDIO - Creatures");
    assert_eq!(creatures["title"], "Creatures");
    assert_eq!(creatures["depth"], 2);
    assert_eq!(audio["entries"][1]["id"], "Bells");

    let rats = &creatures["entries"][0];
    assert_eq!(
        rats["docs"]["summary"], "Rats that swear.",
        "from the docs file"
    );
    assert_eq!(
        rats["condition"]["text"],
        "you answer yes to “Install the rats?”"
    );
    assert_eq!(
        rats["nexus"]["mod_url"],
        "https://www.nexusmods.com/oblivion/mods/52661"
    );
    assert_eq!(
        rats["archives"][0]["instructions"][0],
        "In the BAIN installer, select `00 Core`."
    );
    assert_eq!(
        rats["steps"][0]["text"],
        "In `ini/rats.ini`, set `rats.swear` to `1`."
    );
    assert_eq!(doc["load_order"][0]["mod"], "Rats");
}

#[test]
fn a_plan_narrows_the_document_to_one_build() {
    let dir = tempdir().unwrap();
    let list = write_list(dir.path(), MODLIST, DOCS_FILE);
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    mudcrab()
        .args(["compile"])
        .arg(&list)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();
    mudcrab()
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&plan)
        .args(["--headless", "--set", "rats=false"])
        .assert()
        .success();

    let doc = export(&list, &["--plan", plan.to_str().unwrap()]).unwrap();
    assert_eq!(doc["mod_count"], 1);
    assert_eq!(doc["entries"][0]["entries"][0]["id"], "Bells");
    assert_eq!(doc["inputs"][0]["answer"], false);
    assert_eq!(doc["load_order"], serde_json::json!([]));
}

#[test]
fn docs_that_would_vanish_are_refused() {
    let dir = tempdir().unwrap();
    let cases = [
        (
            MODLIST.replace("[sections.\"1 - AUDIO\"]", "[sections.\"1 - AUDI\"]"),
            DOCS_FILE.to_string(),
            "no mod is in",
        ),
        (
            MODLIST.replace("shots/audio.png", "shots/missing.png"),
            DOCS_FILE.to_string(),
            "does not exist",
        ),
        (
            MODLIST.to_string(),
            DOCS_FILE.replace("[mods.\"Rats\"]", "[mods.\"Ratz\"]"),
            "does not have",
        ),
        (
            MODLIST.replace("if = \"rats\"", "if = \"rats\"\ndocs.summary = \"twice\""),
            DOCS_FILE.to_string(),
            "both the modlist and the docs file",
        ),
    ];
    for (modlist, docs, expected) in cases {
        let err = export(&write_list(dir.path(), &modlist, &docs), &[]).unwrap_err();
        assert!(err.contains(expected), "{expected}: {err}");
    }
}
