use assert_cmd::Command;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn download_copies_local_archives_into_cache() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let archive = repo_root
        .join("tests")
        .join("fixtures")
        .join("archives")
        .join("local_test_archive.bin");

    let dir = tempdir().expect("temp dir should be created");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    let source = format!(
        "name = \"Local Download Test\"\n\n[modlist.core]\ndependencies = []\n\n[[modlist.core.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n",
        archive.display()
    );
    std::fs::write(&modlist, source).expect("fixture modlist should be written");

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("compile")
        .arg(&modlist)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&plan)
        .arg("--headless")
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("download")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .assert()
        .success();

    let files = std::fs::read_dir(&cache)
        .expect("cache should exist")
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    assert_eq!(files.len(), 1);
}
