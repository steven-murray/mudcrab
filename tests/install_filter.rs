//! Installing one section of a modlist at a time.
//!
//! A 700-mod list cannot be built in a single run, so `--section` and `--only`
//! narrow what an install touches. The properties worth pinning down are the
//! ones that are easy to get subtly wrong: the MO2 profile must describe what
//! is actually on disk rather than the whole list, and -- the dangerous one --
//! a narrow run must not make everything it skipped look uninstalled.

use assert_cmd::Command;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};
use zip::write::FileOptions;

/// One mod's archive: a plugin plus a marker file, enough to tell installs apart.
fn write_mod_archive(path: &Path, plugin_name: &str) {
    let file = std::fs::File::create(path).expect("create zip");
    let mut zip = zip::ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default();

    zip.start_file(format!("Data/{plugin_name}"), options)
        .expect("plugin entry");
    zip.write_all(b"TES4").expect("plugin bytes");

    zip.start_file("Data/marker.txt", options).expect("marker entry");
    zip.write_all(plugin_name.as_bytes()).expect("marker bytes");

    zip.finish().expect("finalize zip");
}

struct Fixture {
    _dir: TempDir,
    game_dir: PathBuf,
    plan: PathBuf,
    cache: PathBuf,
    instance: PathBuf,
}

impl Fixture {
    fn mods_dir(&self) -> PathBuf {
        self.instance.join("mods")
    }

    fn profile_dir(&self) -> PathBuf {
        self.instance.join("profiles").join("test-profile")
    }

    fn read_profile_file(&self, name: &str) -> String {
        std::fs::read_to_string(self.profile_dir().join(name))
            .unwrap_or_else(|err| panic!("{name} should exist: {err}"))
    }

    fn manifest_ids(&self) -> Vec<String> {
        let raw = std::fs::read_to_string(self.profile_dir().join("install_manifest.json"))
            .expect("install manifest should exist");
        let parsed: serde_json::Value =
            serde_json::from_str(&raw).expect("install manifest should be valid JSON");
        let mut ids: Vec<String> = parsed["installed_mods"]
            .as_array()
            .expect("installed_mods should be an array")
            .iter()
            .map(|entry| entry["id"].as_str().expect("id should be a string").to_string())
            .collect();
        ids.sort();
        ids
    }

    /// Run `install`, passing through whatever filter flags the test wants.
    fn install(&self, filter_args: &[&str]) {
        let mut command = Command::cargo_bin("mudcrab").expect("binary should build");
        command
            .arg("install")
            .arg(&self.plan)
            .arg("--cache")
            .arg(&self.cache)
            .arg("--mo2-instance-dir")
            .arg(&self.instance)
            .arg("--profile-name")
            .arg("test-profile")
            .arg("--game-dir")
            .arg(&self.game_dir);
        for arg in filter_args {
            command.arg(arg);
        }
        command.assert().success();
    }
}

/// Three sections, one of them nested, so both "which section" and "which level
/// of the section path" have something to bite on.
fn build_fixture() -> Fixture {
    let dir = tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();

    let game_dir = root.join("game");
    std::fs::create_dir_all(game_dir.join("Data")).expect("game data dir");
    std::fs::write(game_dir.join("Oblivion.ini"), "bFull Screen = 1\n").expect("game ini");
    // A base-game master: it belongs to no mod, so it must survive a filtered
    // export that drops the plugins of mods which are not installed yet.
    std::fs::write(game_dir.join("Data").join("Oblivion.esm"), b"TES4").expect("game master");

    for (archive, plugin) in [
        ("a1.zip", "A1.esp"),
        ("b1.zip", "B1.esp"),
        ("b2.zip", "B2.esp"),
        ("c1.zip", "C1.esp"),
    ] {
        write_mod_archive(&root.join(archive), plugin);
    }

    let modlist = root.join("modlist.toml");
    std::fs::write(
        &modlist,
        format!(
            r#"
name = "Section Filter"
plugins = ["Oblivion.esm", "A1.esp", "B1.esp", "B2.esp", "C1.esp"]

[[mods]]
id = "a1"
section = ["A"]
[[mods.archives]]
path = "{a1}"
download_handler = "local"

[[mods]]
id = "b1"
section = ["B"]
[[mods.archives]]
path = "{b1}"
download_handler = "local"

[[mods]]
id = "b2"
section = ["B", "Nested"]
[[mods.archives]]
path = "{b2}"
download_handler = "local"

[[mods]]
id = "c1"
section = ["C"]
[[mods.archives]]
path = "{c1}"
download_handler = "local"
"#,
            a1 = root.join("a1.zip").display(),
            b1 = root.join("b1.zip").display(),
            b2 = root.join("b2.zip").display(),
            c1 = root.join("c1.zip").display(),
        ),
    )
    .expect("modlist");

    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    let cache = root.join("cache");
    let instance = root.join("mo2-instance");

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("compile")
        .arg(&modlist)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&plan)
        .arg("--headless")
        .assert()
        .success();

    // Downloading is deliberately unfiltered: every later filtered install
    // should find its archives already cached.
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("download")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .assert()
        .success();

    Fixture {
        _dir: dir,
        game_dir,
        plan,
        cache,
        instance,
    }
}

#[test]
fn compile_and_query_carry_the_section_path_through_to_the_plan() {
    let fixture = build_fixture();

    let plan: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture.plan).expect("plan"))
            .expect("plan should be valid JSON");

    let b2 = plan["mods"]
        .as_array()
        .expect("mods array")
        .iter()
        .find(|entry| entry["id"] == "b2")
        .expect("b2 should be in the plan");

    assert_eq!(
        b2["section"],
        serde_json::json!(["B", "Nested"]),
        "the plan must carry the section path, not just flattened separator names"
    );
}

#[test]
fn section_filter_installs_only_that_sections_mods() {
    let fixture = build_fixture();
    fixture.install(&["--section", "B"]);

    let mods_dir = fixture.mods_dir();
    assert!(mods_dir.join("b1").join("marker.txt").exists(), "b1 should install");
    assert!(mods_dir.join("b2").join("marker.txt").exists(), "b2 should install");
    assert!(!mods_dir.join("a1").exists(), "a1 is in section A");
    assert!(!mods_dir.join("c1").exists(), "c1 is in section C");

    // The profile describes what is on disk: section B's mods, section B's
    // separators, and nothing from A or C.
    assert_eq!(
        fixture.read_profile_file("modlist.txt"),
        "+b2\n-B - Nested_separator\n+b1\n-B_separator\n"
    );
    assert!(!mods_dir.join("A_separator").exists(), "A has nothing under it yet");
    assert!(!mods_dir.join("C_separator").exists(), "C has nothing under it yet");
    assert!(mods_dir.join("B_separator").exists());
    assert!(mods_dir.join("B - Nested_separator").exists());

    // Base game masters stay; plugins from uninstalled sections do not.
    assert_eq!(
        fixture.read_profile_file("plugins.txt"),
        "Oblivion.esm\nB1.esp\nB2.esp\n"
    );

    assert_eq!(fixture.manifest_ids(), vec!["b1", "b2"]);
}

#[test]
fn section_matching_is_case_insensitive_and_matches_a_nested_level() {
    let fixture = build_fixture();
    // "nEsTeD" is the second level of b2's path and appears in no other mod's,
    // so matching it proves both the case folding and that every level of the
    // path is searched rather than just the first.
    fixture.install(&["--section", "nEsTeD"]);

    let mods_dir = fixture.mods_dir();
    assert!(mods_dir.join("b2").join("marker.txt").exists());
    for other in ["a1", "b1", "c1"] {
        assert!(!mods_dir.join(other).exists(), "{other} should not install");
    }

    // Both levels of b2's path get a separator, the parent included.
    assert_eq!(
        fixture.read_profile_file("modlist.txt"),
        "+b2\n-B - Nested_separator\n-B_separator\n"
    );
}

#[test]
fn section_filter_matching_a_parent_level_takes_the_whole_subtree() {
    let fixture = build_fixture();
    // Lowercased, to pin the case-insensitivity of a parent-level match too.
    fixture.install(&["--section", "b"]);

    let mods_dir = fixture.mods_dir();
    assert!(mods_dir.join("b1").exists());
    assert!(
        mods_dir.join("b2").exists(),
        "asking for a section means the whole subtree under it"
    );
    assert!(!mods_dir.join("a1").exists());
}

#[test]
fn only_installs_exactly_the_named_mod() {
    let fixture = build_fixture();
    fixture.install(&["--only", "b1"]);

    let mods_dir = fixture.mods_dir();
    assert!(mods_dir.join("b1").join("marker.txt").exists());
    for other in ["a1", "b2", "c1"] {
        assert!(!mods_dir.join(other).exists(), "{other} should not install");
    }

    assert_eq!(fixture.read_profile_file("modlist.txt"), "+b1\n-B_separator\n");
    assert_eq!(
        fixture.read_profile_file("plugins.txt"),
        "Oblivion.esm\nB1.esp\n"
    );
    assert_eq!(fixture.manifest_ids(), vec!["b1"]);
}

#[test]
fn section_and_only_union_rather_than_intersect() {
    let fixture = build_fixture();
    fixture.install(&["--section", "C", "--only", "a1"]);

    let mods_dir = fixture.mods_dir();
    assert!(mods_dir.join("a1").exists(), "--only adds to the selection");
    assert!(mods_dir.join("c1").exists(), "--section adds to the selection");
    assert!(!mods_dir.join("b1").exists());
    assert!(!mods_dir.join("b2").exists());

    assert_eq!(fixture.manifest_ids(), vec!["a1", "c1"]);
}

#[test]
fn no_filter_still_installs_everything() {
    let fixture = build_fixture();
    fixture.install(&[]);

    let mods_dir = fixture.mods_dir();
    for mod_id in ["a1", "b1", "b2", "c1"] {
        assert!(mods_dir.join(mod_id).join("marker.txt").exists(), "{mod_id}");
    }

    assert_eq!(
        fixture.read_profile_file("modlist.txt"),
        "+c1\n-C_separator\n+b2\n-B - Nested_separator\n+b1\n-B_separator\n+a1\n-A_separator\n"
    );
    // Unfiltered, the load order is passed through exactly as declared.
    assert_eq!(
        fixture.read_profile_file("plugins.txt"),
        "Oblivion.esm\nA1.esp\nB1.esp\nB2.esp\nC1.esp\n"
    );
}

#[test]
fn a_filtered_install_does_not_drop_previously_installed_mods_from_the_manifest() {
    let fixture = build_fixture();
    fixture.install(&[]);
    assert_eq!(fixture.manifest_ids(), vec!["a1", "b1", "b2", "c1"]);

    // Re-run scoped to one mod. The other three were not touched, so they are
    // still on disk -- forgetting them would reinstall them from scratch next
    // time and, worse, delete them from the MO2 profile in the meantime.
    fixture.install(&["--only", "b1"]);

    assert_eq!(
        fixture.manifest_ids(),
        vec!["a1", "b1", "b2", "c1"],
        "a narrow run must not uninstall the rest of the list"
    );

    let mods_dir = fixture.mods_dir();
    for mod_id in ["a1", "b1", "b2", "c1"] {
        assert!(mods_dir.join(mod_id).join("marker.txt").exists(), "{mod_id}");
    }

    // And the profile still describes all of it, because all of it is installed.
    assert_eq!(
        fixture.read_profile_file("modlist.txt"),
        "+c1\n-C_separator\n+b2\n-B - Nested_separator\n+b1\n-B_separator\n+a1\n-A_separator\n"
    );
    assert_eq!(
        fixture.read_profile_file("plugins.txt"),
        "Oblivion.esm\nA1.esp\nB1.esp\nB2.esp\nC1.esp\n"
    );
}

#[test]
fn sections_installed_one_at_a_time_accumulate_in_the_profile() {
    let fixture = build_fixture();

    fixture.install(&["--section", "A"]);
    assert_eq!(fixture.manifest_ids(), vec!["a1"]);

    fixture.install(&["--section", "C"]);

    assert_eq!(
        fixture.manifest_ids(),
        vec!["a1", "c1"],
        "building section by section is the whole point; each run adds to the last"
    );
    assert_eq!(
        fixture.read_profile_file("modlist.txt"),
        "+c1\n-C_separator\n+a1\n-A_separator\n"
    );
    assert_eq!(
        fixture.read_profile_file("plugins.txt"),
        "Oblivion.esm\nA1.esp\nC1.esp\n"
    );
}

#[test]
fn download_and_check_take_the_same_flags_as_install() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();

    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir");
    write_mod_archive(&root.join("b1.zip"), "B1.esp");

    // a1 points at an archive that does not exist, so it is only harmless while
    // the filter is actually excluding it.
    let modlist = root.join("modlist.toml");
    std::fs::write(
        &modlist,
        format!(
            r#"
name = "Filtered Download"

[[mods]]
id = "a1"
section = ["A"]
[[mods.archives]]
path = "{missing}"
download_handler = "local"

[[mods]]
id = "b1"
section = ["B"]
[[mods.archives]]
path = "{b1}"
download_handler = "local"
"#,
            missing = root.join("does-not-exist.zip").display(),
            b1 = root.join("b1.zip").display(),
        ),
    )
    .expect("modlist");

    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    let cache = root.join("cache");

    for args in [
        vec!["compile", modlist.to_str().unwrap(), "--output", compiled.to_str().unwrap()],
        vec![
            "query",
            compiled.to_str().unwrap(),
            "--output",
            plan.to_str().unwrap(),
            "--headless",
        ],
    ] {
        Command::cargo_bin("mudcrab")
            .expect("binary should build")
            .env("GAME_DIR", &game_dir)
            .args(args)
            .assert()
            .success();
    }

    // Unfiltered, a1's missing archive fails the download.
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .args(["download", plan.to_str().unwrap(), "--cache", cache.to_str().unwrap()])
        .assert()
        .failure();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .args([
            "download",
            plan.to_str().unwrap(),
            "--cache",
            cache.to_str().unwrap(),
            "--section",
            "b",
        ])
        .assert()
        .success();

    // check sees the same subset: b1 is cached, a1 was never fetched.
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .args(["check", plan.to_str().unwrap(), "--cache", cache.to_str().unwrap()])
        .assert()
        .failure();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .args([
            "check",
            plan.to_str().unwrap(),
            "--cache",
            cache.to_str().unwrap(),
            "--only",
            "b1",
        ])
        .assert()
        .success();
}
