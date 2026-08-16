use assert_cmd::Command;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// Compile then query a source modlist, leaving a personalized plan at `plan`.
fn build_plan(game_dir: &Path, modlist: &Path, compiled: &Path, plan: &Path) {
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", game_dir)
        .args(["compile", modlist.to_str().unwrap()])
        .args(["--output", compiled.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", game_dir)
        .args(["query", compiled.to_str().unwrap()])
        .args(["--output", plan.to_str().unwrap()])
        .arg("--headless")
        .assert()
        .success();
}

/// One dead link must not hide the next one.
///
/// `download` used to abort on its first failure, so finding the bad sources in
/// a section meant one full round trip per bad source. `check` already reports
/// them together; this is the same shape.
#[test]
fn download_reports_every_failure_rather_than_stopping_at_the_first() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let game_dir = root.join("game");
    let cache = root.join("cache");
    let modlist = root.join("modlist.toml");
    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    // Two dead sources with a good one between them, so a run that stopped
    // early would also be visible as the good one never being fetched.
    let good = root.join("good.zip");
    std::fs::write(&good, b"payload").expect("good archive should be written");

    std::fs::write(
        &modlist,
        format!(
            r#"
name = "Accumulating Failures"

[[mods]]
id = "dead-first"
dependencies = []
[[mods.archives]]
path = "{missing_a}"
download_handler = "local"

[[mods]]
id = "alive"
dependencies = []
[[mods.archives]]
path = "{good}"
download_handler = "local"

[[mods]]
id = "dead-second"
dependencies = []
[[mods.archives]]
path = "{missing_b}"
download_handler = "local"
"#,
            missing_a = root.join("nowhere-a.zip").display(),
            good = good.display(),
            missing_b = root.join("nowhere-b.zip").display(),
        ),
    )
    .expect("modlist should be written");
    build_plan(&game_dir, &modlist, &compiled, &plan);

    let assert = Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .args(["download", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf-8 stderr");
    assert!(
        stderr.contains("download failed for 2 archive(s)"),
        "both failures should be counted:\n{stderr}"
    );
    assert!(
        stderr.contains("dead-first") && stderr.contains("dead-second"),
        "both failures should be named:\n{stderr}"
    );

    // The mod between them was still fetched: a failure is reported, not fatal.
    let cached: Vec<String> = std::fs::read_dir(&cache)
        .expect("cache should exist")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        cached.iter().any(|name| name.starts_with("alive_0_")),
        "the good archive should have been fetched anyway, got {cached:?}"
    );
}

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
        "name = \"Local Download Test\"\n\n[[mods]]\nid = \"core\"\ndependencies = []\n\n[[mods.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n",
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
