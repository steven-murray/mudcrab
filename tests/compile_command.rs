use assert_cmd::Command;
use serde_json::Value;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn compile_writes_compiled_artifact() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("modlists")
        .join("simple.toml");

    let dir = tempdir().expect("temp dir should be created");
    let out = dir.path().join("compiled.json");
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    let mut cmd = Command::cargo_bin("mudcrab").expect("binary should build");
    cmd.env("GAME_DIR", &game_dir)
        .arg("compile")
        .arg(&fixture)
        .arg("--output")
        .arg(&out);

    cmd.assert().success();

    let raw = std::fs::read_to_string(&out).expect("output should be written");
    let json: Value = serde_json::from_str(&raw).expect("output should be valid json");

    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["name"], "Simple Skyrim Core");
    assert_eq!(json["mod_count"], 2);
    assert_eq!(json["plugin_count"], 1);
}

#[test]
fn compile_flattens_nested_mod_sections() {
    let dir = tempdir().expect("temp dir should be created");
    let modlist = dir.path().join("nested.toml");
    let out = dir.path().join("compiled.json");
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    std::fs::write(
        &modlist,
        "name = \"Nested Compile\"\n\n[modlist.foundation.base]\ndependencies = []\n\n[modlist.gameplay.combat]\ndependencies = [\"base\"]\n\n[modlist.gameplay.magic]\ndependencies = [\"base\"]\n",
    )
    .expect("fixture should be written");

    let mut cmd = Command::cargo_bin("mudcrab").expect("binary should build");
    cmd.env("GAME_DIR", &game_dir)
        .arg("compile")
        .arg(&modlist)
        .arg("--output")
        .arg(&out);

    cmd.assert().success();

    let raw = std::fs::read_to_string(&out).expect("output should be written");
    let json: Value = serde_json::from_str(&raw).expect("output should be valid json");

    assert_eq!(json["mod_count"], 3);
    assert_eq!(json["mods"][0]["id"], "base");
    assert_eq!(json["mods"][1]["id"], "combat");
    assert_eq!(json["mods"][2]["id"], "magic");
}
