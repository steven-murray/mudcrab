//! `--dry-run` must not touch anything.
//!
//! It previously ran both plan-level and per-mod actions, so a dry run would
//! rewrite Oblivion.ini and shell out to xEdit for `qac` -- making it more
//! destructive than a real install, which at least only writes into the mods
//! directory. These tests pin the property the flag actually promises.

use assert_cmd::Command;
use std::io::Write;
use tempfile::tempdir;
use zip::write::FileOptions;

/// Compile + query a one-mod modlist that carries a game-scoped ini_set.
fn fixture(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let archive = dir.join("core.zip");
    let file = std::fs::File::create(&archive).expect("create zip");
    let mut zip = zip::ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    zip.start_file("Data/thing.txt", options).expect("entry");
    zip.write_all(b"payload").expect("payload");
    zip.finish().expect("finish");

    let game_dir = dir.join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir");
    std::fs::write(game_dir.join("Oblivion.ini"), "bFull Screen = 1\n").expect("ini");

    let modlist = dir.join("modlist.toml");
    std::fs::write(
        &modlist,
        format!(
            r#"
name = "Dry Run Test"
plugins = []

[ini]
"bFull Screen" = 0

[[mods]]
id = "core"
section = ["A"]
[[mods.archives]]
path = "{}"
download_handler = "local"
"#,
            archive.display()
        ),
    )
    .expect("modlist");

    let compiled = dir.join("compiled.json");
    let plan = dir.join("plan.json");
    for args in [
        vec!["compile", modlist.to_str().unwrap(), "--output", compiled.to_str().unwrap()],
        vec!["query", compiled.to_str().unwrap(), "--output", plan.to_str().unwrap(), "--headless"],
    ] {
        Command::cargo_bin("mudcrab")
            .expect("binary")
            .args(args)
            .assert()
            .success();
    }
    (plan, game_dir)
}

#[test]
fn a_dry_run_does_not_apply_ini_actions() {
    let dir = tempdir().expect("tempdir");
    let (plan, game_dir) = fixture(dir.path());
    let ini = game_dir.join("Oblivion.ini");
    let before = std::fs::read_to_string(&ini).expect("read ini");

    Command::cargo_bin("mudcrab")
        .expect("binary")
        .args([
            "install",
            plan.to_str().unwrap(),
            "--cache",
            dir.path().join("cache").to_str().unwrap(),
            "--mods-dir",
            dir.path().join("mods").to_str().unwrap(),
            "--game-dir",
            game_dir.to_str().unwrap(),
            "--dry-run",
        ])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(&ini).expect("re-read ini"),
        before,
        "a dry run must leave the game INI untouched"
    );
    assert!(
        !dir.path().join("mods").join("core").exists(),
        "a dry run must not stage any mod"
    );
}

#[test]
fn a_dry_run_reports_uncached_archives_instead_of_aborting() {
    // With an empty cache every archive is missing. Aborting on the first one
    // would hide the rest of the plan, which is the opposite of previewing.
    let dir = tempdir().expect("tempdir");
    let (plan, game_dir) = fixture(dir.path());

    Command::cargo_bin("mudcrab")
        .expect("binary")
        .args([
            "install",
            plan.to_str().unwrap(),
            "--cache",
            dir.path().join("empty-cache").to_str().unwrap(),
            "--mods-dir",
            dir.path().join("mods").to_str().unwrap(),
            "--game-dir",
            game_dir.to_str().unwrap(),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("not cached"));
}
