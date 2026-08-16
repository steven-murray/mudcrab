//! A run that dies part way still records what it got done.
//!
//! The manifest used to be written once, after the whole loop. A 300-mod run
//! that failed on mod 250 returned `Err` with nothing written, so the 249 mods
//! it had just extracted were absent from the manifest and the next run
//! unpacked every one of them again -- which is most of an afternoon for a
//! failure that took one line to fix.

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
        zip.start_file(*name, options)
            .expect("zip file entry should be created");
        zip.write_all(bytes).expect("zip payload should be written");
    }
    zip.finish().expect("zip should finalize");
}

fn strip_ansi(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        for escaped in chars.by_ref() {
            if escaped.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

#[test]
fn a_run_that_fails_part_way_still_records_the_mods_that_succeeded() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let game_dir = root.join("game");
    let cache = root.join("cache");
    let mods_dir = root.join("mods");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    make_zip(&root.join("first.zip"), &[("Data/first.txt", b"first")]);
    make_zip(&root.join("second.zip"), &[("Data/second.txt", b"second")]);

    // `second` comes from a nexus descriptor with no api key and no search
    // path, so installing it fails: nothing can produce its archive.
    let modlist = root.join("modlist.toml");
    std::fs::write(
        &modlist,
        format!(
            r#"
name = "Partial Failure"

[[mods]]
id = "first"
dependencies = []
[[mods.archives]]
path = "{first}"
download_handler = "local"

[[mods]]
id = "second"
dependencies = []
[[mods.archives]]
path = "nexus:oblivion/1234/5678"
file_name = "second.zip"
"#,
            first = root.join("first.zip").display(),
        ),
    )
    .expect("modlist should be written");

    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .args(["compile", modlist.to_str().unwrap()])
        .args(["--output", compiled.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .args(["query", compiled.to_str().unwrap()])
        .args(["--output", plan.to_str().unwrap()])
        .arg("--headless")
        .assert()
        .success();
    // Only the first mod's archive is fetched.
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .args(["download", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--only", "first"])
        .assert()
        .success();

    let install = |extra: &[&str]| {
        Command::cargo_bin("mudcrab")
            .expect("binary should build")
            .env_remove("NEXUS_API_KEY")
            .args(["install", plan.to_str().unwrap()])
            .args(["--cache", cache.to_str().unwrap()])
            .args(["--mods-dir", mods_dir.to_str().unwrap()])
            .args(["--game-dir", game_dir.to_str().unwrap()])
            .args(extra)
            .assert()
    };

    install(&[]).failure();

    // The first mod is on disk, and the manifest says so.
    assert!(mods_dir.join("first").join("first.txt").is_file());
    let manifest_path = mods_dir.join("install_manifest.json");
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path).expect("manifest should have been written"),
    )
    .expect("manifest should be json");
    let recorded: Vec<&str> = manifest["installed_mods"]
        .as_array()
        .expect("installed_mods should be an array")
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();
    assert_eq!(
        recorded,
        vec!["first"],
        "the mod that succeeded should be recorded, and only that one"
    );

    // Give the failing mod what it needs and run again.
    let search_dir = root.join("downloads");
    make_zip(
        &search_dir.join("second.zip"),
        &[("Data/second.txt", b"second")],
    );

    let assert = install(&["--archive-search-path", search_dir.to_str().unwrap()]).success();
    let report =
        strip_ansi(&String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8 stdout"));

    assert!(
        report.contains("mod_id=first")
            && report.contains("reason=\"already installed and unchanged\""),
        "the mod recorded by the failed run should be skipped, not re-extracted:\n{report}"
    );
    assert!(mods_dir.join("second").join("second.txt").is_file());
}

#[test]
fn a_failed_reinstall_does_not_leave_the_mod_recorded_as_installed() {
    // The other half of the same problem: a mod whose definition changed is
    // cleared before it is re-extracted. If that extraction fails, the manifest
    // must stop claiming the mod, or the next run skips a folder that is empty.
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let game_dir = root.join("game");
    let cache = root.join("cache");
    let mods_dir = root.join("mods");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    let archive = root.join("core.zip");
    make_zip(&archive, &[("Data/core.txt", b"core")]);

    let write_plan = |suffix: &str| {
        let modlist = root.join(format!("modlist{suffix}.toml"));
        std::fs::write(
            &modlist,
            format!(
                "name = \"Reinstall Failure\"\n\n[[mods]]\nid = \"core\"\ndependencies = []\n\n\
                 [[mods.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n\
                 exclude = [\"nothing{suffix}\"]\n",
                archive.display()
            ),
        )
        .expect("modlist should be written");

        let compiled = root.join(format!("compiled{suffix}.json"));
        let plan = root.join(format!("plan{suffix}.json"));
        Command::cargo_bin("mudcrab")
            .expect("binary should build")
            .args(["compile", modlist.to_str().unwrap()])
            .args(["--output", compiled.to_str().unwrap()])
            .assert()
            .success();
        Command::cargo_bin("mudcrab")
            .expect("binary should build")
            .args(["query", compiled.to_str().unwrap()])
            .args(["--output", plan.to_str().unwrap()])
            .arg("--headless")
            .assert()
            .success();
        plan
    };

    let first_plan = write_plan("-a");
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .args(["download", first_plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .args(["install", first_plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--mods-dir", mods_dir.to_str().unwrap()])
        .args(["--game-dir", game_dir.to_str().unwrap()])
        .assert()
        .success();

    // A second plan whose definition differs, so the mod is cleared and
    // re-extracted -- but its archive is no longer anywhere to be found.
    let second_plan = write_plan("-b");
    std::fs::remove_file(&archive).expect("archive should be removable");
    for entry in std::fs::read_dir(&cache).expect("cache should be readable") {
        let path = entry.expect("cache entry").path();
        std::fs::remove_file(&path).expect("cache entry should be removable");
    }

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env_remove("NEXUS_API_KEY")
        .args(["install", second_plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--mods-dir", mods_dir.to_str().unwrap()])
        .args(["--game-dir", game_dir.to_str().unwrap()])
        .assert()
        .failure();

    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(mods_dir.join("install_manifest.json"))
            .expect("manifest should exist"),
    )
    .expect("manifest should be json");
    assert_eq!(
        manifest["installed_mods"]
            .as_array()
            .expect("installed_mods should be an array")
            .len(),
        0,
        "a mod that was cleared and then failed must not stay recorded as installed"
    );
}
