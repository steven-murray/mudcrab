use assert_cmd::Command;
use serde_json::Value;
use std::path::PathBuf;
use tempfile::tempdir;

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

    let mut compile_cmd = Command::cargo_bin("mudcrab").expect("binary should build");
    compile_cmd
        .env("GAME_DIR", &game_dir)
        .arg("compile")
        .arg(&fixture)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();

    let mut query_cmd = Command::cargo_bin("mudcrab").expect("binary should build");
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
