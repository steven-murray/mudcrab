//! A game-scoped `ini_set` is re-applied on every install.
//!
//! Actions are normally applied once and latched in the manifest: a mod whose
//! definition is unchanged does not need its staged folder rebuilt, so it does
//! not need its actions run again. That reasoning breaks for an action whose
//! target is outside the mod folder. MO2 recreates a profile's `Oblivion.ini`
//! whenever it likes, so a game-scoped edit can be undone without anything
//! about the mod changing -- and the latch then means it is never restored.
//!
//! Found the hard way: DarNified UI's five `SFontFile` lines were applied once,
//! MO2 later replaced the profile INI with a fresh copy, and every subsequent
//! install skipped the mod as unchanged. The game came up with vanilla fonts
//! over a UI built for DarN's, which is unplayable.

use assert_cmd::Command;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;
use zip::write::FileOptions;

fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("archive parent should be created");
    }
    let file = std::fs::File::create(path).expect("zip fixture should be created");
    let mut zip = zip::ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    for (name, bytes) in entries {
        zip.start_file(*name, options).expect("zip entry");
        zip.write_all(bytes).expect("zip payload");
    }
    zip.finish().expect("zip should finalize");
}

#[test]
fn a_game_scoped_ini_edit_survives_the_ini_being_replaced() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let game_dir = root.join("game");
    let cache = root.join("cache");
    let mods_dir = root.join("mods");
    std::fs::create_dir_all(&game_dir).expect("game dir");
    // The game INI exists before any install, as it does in a real setup.
    std::fs::write(
        game_dir.join("Oblivion.ini"),
        "[Fonts]\nSFontFile_2=Data\\Fonts\\Vanilla.fnt\n",
    )
    .expect("seed ini");

    make_zip(&root.join("ui.zip"), &[("Data/menus/main.xml", b"<menu/>")]);

    let modlist = root.join("modlist.toml");
    std::fs::write(
        &modlist,
        format!(
            r#"
name = "Game Ini Reapply"

[[mods]]
id = "ui"
dependencies = []

[[mods.archives]]
path = "{ui}"
download_handler = "local"

[[mods.actions]]
action = "ini_set"
scope = "game"
file = "Oblivion.ini"
key = "SFontFile_2"
value = "Data\\Fonts\\DarN_Kingthings_Petrock_14.fnt"

[[mods.actions]]
action = "ini_set"
file = "menus/main.xml"
key = "staged"
value = "1"
"#,
            ui = root.join("ui.zip").display(),
        ),
    )
    .expect("modlist written");

    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    let run = |args: &[&str]| {
        Command::cargo_bin("mudcrab")
            .expect("binary builds")
            .args(args)
            .assert()
            .success();
    };
    run(&["compile", modlist.to_str().unwrap(), "--output", compiled.to_str().unwrap()]);
    run(&["query", compiled.to_str().unwrap(), "--output", plan.to_str().unwrap(), "--headless"]);
    run(&["download", plan.to_str().unwrap(), "--cache", cache.to_str().unwrap()]);

    let install = || {
        Command::cargo_bin("mudcrab")
            .expect("binary builds")
            .args(["install", plan.to_str().unwrap()])
            .args(["--cache", cache.to_str().unwrap()])
            .args(["--mods-dir", mods_dir.to_str().unwrap()])
            .args(["--game-dir", game_dir.to_str().unwrap()])
            .assert()
            .success();
    };

    install();

    let game_ini = game_dir.join("Oblivion.ini");
    let first = std::fs::read_to_string(&game_ini).expect("game ini written");
    assert!(
        first.contains("DarN_Kingthings_Petrock_14.fnt"),
        "first install should set the font:\n{first}"
    );

    // MO2 replaces the profile INI with a fresh copy, losing the edit. Nothing
    // about the mod has changed, so the next install will skip it.
    std::fs::write(&game_ini, "[Fonts]\nSFontFile_2=Data\\Fonts\\Vanilla.fnt\n")
        .expect("ini reset");

    install();

    let second = std::fs::read_to_string(&game_ini).expect("game ini readable");
    assert!(
        second.contains("DarN_Kingthings_Petrock_14.fnt"),
        "a skipped mod must still re-apply its game-scoped edits:\n{second}"
    );
    assert!(
        !second.contains("Vanilla.fnt"),
        "the stale value should have been replaced:\n{second}"
    );
}
