use assert_cmd::Command;
use std::path::Path;
use tempfile::{TempDir, tempdir};

/// Write a file, creating its parent directories.
fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent directory should be created");
    }
    std::fs::write(&path, contents).expect("fixture file should be written");
}

/// Two mod trees plus the plan that describes ours, in one tempdir.
struct Fixture {
    dir: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempdir().expect("temp dir should be created");
        std::fs::create_dir_all(dir.path().join("mods")).expect("ours should be created");
        std::fs::create_dir_all(dir.path().join("oracle")).expect("oracle should be created");
        Self { dir }
    }

    fn ours(&self) -> std::path::PathBuf {
        self.dir.path().join("mods")
    }

    fn oracle(&self) -> std::path::PathBuf {
        self.dir.path().join("oracle")
    }

    fn plan_path(&self) -> std::path::PathBuf {
        self.dir.path().join("plan.json")
    }

    /// Write a personalized plan holding just the fields `diff` reads.
    fn write_plan(&self, mods: &str) {
        let plan = format!(
            r#"{{
  "schema_version": 1,
  "name": "Diff Test",
  "guide": {{ "published": "2025-03-01", "file_id": 1000040999 }},
  "responses": {{}},
  "mod_order": [],
  "selected_mods": [],
  "mods": [{mods}],
  "plugins": []
}}"#
        );
        std::fs::write(self.plan_path(), plan).expect("plan should be written");
    }

    /// `diff` told which guide to measure archives against, the way a run
    /// without a plan has to be. The age tests used to rely on the guide's date
    /// being a constant inside mudcrab.
    fn diff_against_guide(&self) -> Command {
        let mut command = self.diff();
        command
            .arg("--guide-date")
            .arg("2025-03-01")
            .arg("--guide-file-id")
            .arg("1000040999");
        command
    }

    fn diff(&self) -> Command {
        let mut command = Command::cargo_bin("mudcrab").expect("binary should build");
        command
            .arg("diff")
            .arg("--mods-dir")
            .arg(self.ours())
            .arg("--oracle")
            .arg(self.oracle());
        command
    }
}

/// A `[[mods]]` entry as the plan serializes it.
fn plan_mod(id: &str, section: &[&str], file_name: Option<&str>) -> String {
    let sections = section
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let archives = match file_name {
        Some(name) => format!(
            r#"{{"download_handler": "nexus", "file_name": "{name}", "layout": null, "data_folder": null, "target_subdir": null, "include": [], "exclude": []}}"#
        ),
        None => String::new(),
    };
    format!(
        r#"{{"id": "{id}", "section": [{sections}], "archives": [{archives}], "files": [], "actions": []}}"#
    )
}

/// The same entry, plus the `oracle_name` that says what the reference instance
/// calls this mod when our id is deliberately different.
fn aliased_plan_mod(id: &str, oracle_name: &str, section: &[&str]) -> String {
    let sections = section
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"{{"id": "{id}", "oracle_name": "{oracle_name}", "section": [{sections}], "archives": [], "files": [], "actions": []}}"#
    )
}

#[test]
fn identical_trees_report_no_differences_and_exit_zero() {
    let fixture = Fixture::new();
    for root in [fixture.ours(), fixture.oracle()] {
        write(&root, "Blockhead/obse/plugins/Blockhead.dll", "binary payload");
        write(&root, "Blockhead/readme.txt", "read me");
        write(&root, "Fast Exit/obse/plugins/FastExit.dll", "exit payload");
    }
    // Only the Oracle carries MO2's bookkeeping, which is exactly why it is
    // ignored: ours will never have one.
    write(
        &fixture.oracle(),
        "Blockhead/meta.ini",
        "[General]\nmodid=43752\nversion=11.1.0.0\ninstallationFile=Blockhead-43752-11-1-1640043918.7z\n",
    );

    let output = fixture.diff().assert().success().get_output().stdout.clone();
    let text = String::from_utf8(output).expect("report should be utf-8");

    assert!(
        text.contains("2 compared, 2 identical, 0 differing, 0 missing from ours, 0 extra in ours"),
        "unexpected report:\n{text}"
    );
    // An identical section is a couple of lines, not a file listing.
    assert!(!text.contains("content differs"), "unexpected report:\n{text}");
    assert!(!text.contains("only in"), "unexpected report:\n{text}");
}

#[test]
fn a_file_that_differs_only_in_content_is_detected() {
    let fixture = Fixture::new();
    // Same size, different bytes: nothing but reading them can tell these apart.
    write(&fixture.ours(), "Blockhead/data/config.ini", "bLoadModded=1");
    write(&fixture.oracle(), "Blockhead/data/config.ini", "bLoadModded=0");

    let assert = fixture.diff().assert().failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");

    assert!(
        text.contains("1 compared, 0 identical, 1 differing"),
        "unexpected report:\n{text}"
    );
    assert!(text.contains("~ Blockhead"), "unexpected report:\n{text}");
    assert!(
        text.contains("content differs (1):"),
        "unexpected report:\n{text}"
    );
    assert!(
        text.contains("data/config.ini") && text.contains("same size 13 B"),
        "unexpected report:\n{text}"
    );
    // Same-size differences are described by digest, so the two are named.
    assert!(text.contains("sha256"), "unexpected report:\n{text}");
}

#[test]
fn a_file_that_differs_in_size_is_reported_without_hashing() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Blockhead/data/config.ini", "short");
    write(
        &fixture.oracle(),
        "Blockhead/data/config.ini",
        "a much longer configuration file",
    );

    let assert = fixture.diff().assert().failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");

    assert!(
        text.contains("(ours 5 B, oracle 32 B)"),
        "unexpected report:\n{text}"
    );
    // Differing sizes settle it, so no digest was computed for either side.
    assert!(!text.contains("sha256"), "unexpected report:\n{text}");
}

#[test]
fn files_present_on_one_side_only_are_reported_on_that_side() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Blockhead/shared.txt", "same");
    write(&fixture.oracle(), "Blockhead/shared.txt", "same");
    write(&fixture.ours(), "Blockhead/extra-of-ours.txt", "ours");
    write(&fixture.oracle(), "Blockhead/textures/only-theirs.dds", "theirs");

    let assert = fixture.diff().assert().failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");

    assert!(
        text.contains("only in ours (1):") && text.contains("extra-of-ours.txt"),
        "unexpected report:\n{text}"
    );
    assert!(
        text.contains("only in the Oracle (1):") && text.contains("textures/only-theirs.dds"),
        "unexpected report:\n{text}"
    );
}

#[test]
fn a_mod_folder_on_one_side_only_is_reported_as_missing_or_extra() {
    let fixture = Fixture::new();
    write(&fixture.oracle(), "Not Built Yet/data/thing.esp", "plugin");
    write(&fixture.ours(), "Ours Alone/data/thing.esp", "plugin");

    let assert = fixture.diff().assert().failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");

    assert!(
        text.contains("2 compared, 0 identical, 0 differing, 1 missing from ours, 1 extra in ours"),
        "unexpected report:\n{text}"
    );
    assert!(
        text.contains("- Not Built Yet  (missing from ours)"),
        "unexpected report:\n{text}"
    );
    assert!(
        text.contains("+ Ours Alone  (not in the Oracle)"),
        "unexpected report:\n{text}"
    );
}

#[test]
fn paths_are_matched_case_insensitively_and_across_separators() {
    let fixture = Fixture::new();
    // These trees come from Windows-authored archives: the same file routinely
    // arrives with different capitalisation on each side.
    write(&fixture.ours(), "Blockhead/Textures/Armor/Foo.DDS", "texture");
    write(&fixture.oracle(), "Blockhead/textures/armor/foo.dds", "texture");

    let output = fixture.diff().assert().success().get_output().stdout.clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(
        text.contains("1 compared, 1 identical, 0 differing"),
        "unexpected report:\n{text}"
    );
}

#[test]
fn a_mohidden_plugin_is_the_same_file_but_not_the_same_state() {
    let fixture = Fixture::new();
    // The Oracle hid this plugin to make room for a merge. The rename is not a
    // content difference -- the file is matched to its unhidden twin and the
    // bytes are never reported as differing -- but it *is* a difference in what
    // the game loads, so it is reported on its own line.
    write(&fixture.ours(), "Fort Aurus/Fort Aurus.esp", "plugin bytes");
    write(
        &fixture.oracle(),
        "Fort Aurus/Fort Aurus.esp.mohidden",
        "plugin bytes",
    );

    let output = fixture.diff().assert().failure().get_output().stdout.clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(
        text.contains("hidden on one side only (1):"),
        "unexpected report:\n{text}"
    );
    assert!(
        text.contains("Fort Aurus.esp  (hidden in the Oracle)"),
        "unexpected report:\n{text}"
    );
    assert!(
        !text.contains("only in ours") && !text.contains("only in the Oracle"),
        "the file must still be matched to its twin, not reported as unpaired:\n{text}"
    );
    assert!(
        !text.contains("content differs"),
        "the bytes are identical:\n{text}"
    );
}

#[test]
fn a_file_hidden_the_same_way_on_both_sides_is_identical() {
    let fixture = Fixture::new();
    write(
        &fixture.ours(),
        "Beast Races/textures/khajiit.mohidden/head.dds",
        "texture bytes",
    );
    write(
        &fixture.oracle(),
        "Beast Races/textures/khajiit.mohidden/head.dds",
        "texture bytes",
    );

    let output = fixture.diff().assert().success().get_output().stdout.clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(
        text.contains("1 compared, 1 identical, 0 differing"),
        "unexpected report:\n{text}"
    );
}

#[test]
fn meta_ini_is_ignored_at_the_mod_root() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Blockhead/readme.txt", "read me");
    write(&fixture.oracle(), "Blockhead/readme.txt", "read me");
    write(
        &fixture.oracle(),
        "Blockhead/meta.ini",
        "[General]\nmodid=43752\ninstallationFile=Blockhead-43752-11-1-1640043918.7z\n",
    );
    // A meta.ini deeper in the tree is a mod's own file, not MO2 bookkeeping,
    // so it must still be compared.
    write(&fixture.ours(), "Blockhead/ini/meta.ini", "ours");
    write(&fixture.oracle(), "Blockhead/ini/meta.ini", "theirs");

    let assert = fixture.diff().assert().failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");

    assert!(
        text.contains("content differs (1):") && text.contains("ini/meta.ini"),
        "unexpected report:\n{text}"
    );
    assert!(
        !text.contains("only in the Oracle"),
        "root meta.ini leaked into the report:\n{text}"
    );
}

#[test]
fn mo2_separator_folders_are_not_treated_as_mods() {
    let fixture = Fixture::new();
    write(&fixture.oracle(), "1 - MASTERS_separator/meta.ini", "[General]\n");
    write(&fixture.ours(), "Blockhead/readme.txt", "read me");
    write(&fixture.oracle(), "Blockhead/readme.txt", "read me");

    let output = fixture.diff().assert().success().get_output().stdout.clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(
        text.contains("1 compared, 1 identical"),
        "separator folder was compared as a mod:\n{text}"
    );
}

#[test]
fn a_post_guide_archive_timestamp_is_flagged_without_gating_the_run() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Newer Mod/readme.txt", "same");
    write(&fixture.oracle(), "Newer Mod/readme.txt", "same");
    // 1750000000 is 2025-06-15, well after the guide's March 2025 publication.
    write(
        &fixture.oracle(),
        "Newer Mod/meta.ini",
        "[General]\nmodid=1234\nversion=2.0\ninstallationFile=Newer Mod-1234-2-0-1750000000.7z\n",
    );

    // The files reproduce the Oracle, so the run is clean; the drift is a note
    // about the reference's own archive, not a fault in our copy of it.
    let output = fixture.diff_against_guide().assert().success().get_output().stdout.clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(text.contains("version notes:"), "unexpected report:\n{text}");
    assert!(text.contains("POST-GUIDE (1)"), "unexpected report:\n{text}");
    assert!(
        text.contains("! Newer Mod  dated 2025-06-15"),
        "unexpected report:\n{text}"
    );
    assert!(text.contains("version 2.0"), "unexpected report:\n{text}");
}

#[test]
fn a_pre_guide_archive_timestamp_produces_no_version_note() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Older Mod/readme.txt", "same");
    write(&fixture.oracle(), "Older Mod/readme.txt", "same");
    // 1647873144 is 2022-03-21.
    write(
        &fixture.oracle(),
        "Older Mod/meta.ini",
        "[General]\nmodid=50682\ninstallationFile=Better Fort Aurus-50682-1-1-1647873144.7z\n",
    );

    let output = fixture.diff_against_guide().assert().success().get_output().stdout.clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(!text.contains("version notes:"), "unexpected report:\n{text}");
    assert!(!text.contains("POST-GUIDE"), "unexpected report:\n{text}");
}

#[test]
fn an_unparseable_archive_name_is_reported_as_unknown_rather_than_assumed_fine() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Anvil Morning Glory/readme.txt", "same");
    write(&fixture.oracle(), "Anvil Morning Glory/readme.txt", "same");
    // Trailing 19039 is a Nexus mod id, not a Unix timestamp.
    write(
        &fixture.oracle(),
        "Anvil Morning Glory/meta.ini",
        "[General]\nmodid=19039\ninstallationFile=Anvil Morning Glory-19039.7z\n",
    );

    let output = fixture.diff_against_guide().assert().success().get_output().stdout.clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(text.contains("UNKNOWN AGE (1)"), "unexpected report:\n{text}");
    assert!(
        text.contains("? Anvil Morning Glory  no Unix timestamp in 'Anvil Morning Glory-19039.7z'"),
        "unexpected report:\n{text}"
    );
    assert!(!text.contains("POST-GUIDE"), "unexpected report:\n{text}");
}

#[test]
fn a_plan_archive_that_is_not_the_oracles_is_a_difference() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Blockhead/readme.txt", "same");
    write(&fixture.oracle(), "Blockhead/readme.txt", "same");
    write(
        &fixture.oracle(),
        "Blockhead/meta.ini",
        "[General]\nmodid=43752\ninstallationFile=Blockhead-43752-11-1-1640043918.7z\n",
    );
    fixture.write_plan(&plan_mod(
        "Blockhead",
        &["OBSE PLUGINS"],
        Some("Blockhead-43752-10-0-1600000000.7z"),
    ));

    let assert = fixture
        .diff()
        .arg("--plan")
        .arg(fixture.plan_path())
        .assert()
        .failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");

    assert!(
        text.contains("archive mismatch: plan has 'Blockhead-43752-10-0-1600000000.7z', Oracle installed 'Blockhead-43752-11-1-1640043918.7z'"),
        "unexpected report:\n{text}"
    );
    assert!(
        text.contains("0 identical, 1 differing"),
        "unexpected report:\n{text}"
    );
}

#[test]
fn section_filtering_narrows_the_run_to_the_planned_section() {
    let fixture = Fixture::new();
    // In scope and clean.
    write(&fixture.ours(), "Blockhead/readme.txt", "same");
    write(&fixture.oracle(), "Blockhead/readme.txt", "same");
    // Out of scope and broken: it must not be compared, or a section could
    // never be signed off while a later one is still half-built.
    write(&fixture.ours(), "Fort Aurus/thing.esp", "ours");
    write(&fixture.oracle(), "Fort Aurus/thing.esp", "theirs");

    fixture.write_plan(&format!(
        "{}, {}",
        plan_mod("Blockhead", &["OBSE PLUGINS"], None),
        plan_mod("Fort Aurus", &["NEW LOCATIONS"], None)
    ));

    let output = fixture
        .diff()
        .arg("--plan")
        .arg(fixture.plan_path())
        .arg("--section")
        .arg("obse plugins")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(
        text.contains("1 compared, 1 identical, 0 differing"),
        "unexpected report:\n{text}"
    );
    assert!(text.contains("[OBSE PLUGINS]"), "unexpected report:\n{text}");
    assert!(!text.contains("Fort Aurus"), "unexpected report:\n{text}");

    // The whole-tree run over the same fixture does see the broken mod, which
    // is what proves the filter narrowed rather than the mod being clean.
    fixture.diff().assert().failure();
}

/// Write the pair of folders that our id and the Oracle's disagree about.
///
/// One file matches and one differs, so a run that matched them up has
/// something to say beyond "both sides exist".
fn write_aliased_pair(fixture: &Fixture) {
    write(&fixture.ours(), "Cleaned DLC Masters/shared.esp", "plugin bytes");
    write(&fixture.oracle(), "Clean ESM/shared.esp", "plugin bytes");
    write(&fixture.ours(), "Cleaned DLC Masters/notes.txt", "ours");
    write(&fixture.oracle(), "Clean ESM/notes.txt", "theirs");
}

#[test]
fn an_oracle_name_matches_the_differently_named_oracle_folder() {
    let fixture = Fixture::new();
    write_aliased_pair(&fixture);
    fixture.write_plan(&aliased_plan_mod(
        "Cleaned DLC Masters",
        "Clean ESM",
        &["MASTERS"],
    ));

    let assert = fixture
        .diff()
        .arg("--plan")
        .arg(fixture.plan_path())
        .assert()
        .failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");

    // One mod, not two: the Oracle's folder was claimed by our id rather than
    // reported as a mod of its own.
    assert!(
        text.contains("1 compared, 0 identical, 1 differing, 0 missing from ours, 0 extra in ours"),
        "unexpected report:\n{text}"
    );
    // Reported under our id, naming the Oracle folder it was compared against.
    assert!(
        text.contains("~ Cleaned DLC Masters  (oracle: Clean ESM)"),
        "unexpected report:\n{text}"
    );
    // Matching them up is only useful if the files were then compared.
    assert!(
        text.contains("content differs (1):") && text.contains("notes.txt"),
        "unexpected report:\n{text}"
    );
    assert!(!text.contains("shared.esp"), "unexpected report:\n{text}");
}

#[test]
fn without_an_oracle_name_the_same_pair_is_a_missing_and_an_extra() {
    let fixture = Fixture::new();
    write_aliased_pair(&fixture);
    fixture.write_plan(&plan_mod("Cleaned DLC Masters", &["MASTERS"], None));

    let assert = fixture
        .diff()
        .arg("--plan")
        .arg(fixture.plan_path())
        .assert()
        .failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");

    assert!(
        text.contains("2 compared, 0 identical, 0 differing, 1 missing from ours, 1 extra in ours"),
        "unexpected report:\n{text}"
    );
    assert!(
        text.contains("- Clean ESM  (missing from ours)"),
        "unexpected report:\n{text}"
    );
    assert!(
        text.contains("+ Cleaned DLC Masters  (not in the Oracle)"),
        "unexpected report:\n{text}"
    );
    // No file was read for either side, so the real difference stays buried --
    // which is the whole reason `oracle_name` exists.
    assert!(!text.contains("content differs"), "unexpected report:\n{text}");
}

#[test]
fn an_aliased_mod_we_have_not_built_is_reported_under_our_id() {
    let fixture = Fixture::new();
    write(&fixture.oracle(), "Clean ESM/shared.esp", "plugin bytes");
    fixture.write_plan(&aliased_plan_mod(
        "Cleaned DLC Masters",
        "Clean ESM",
        &["MASTERS"],
    ));

    let assert = fixture
        .diff()
        .arg("--plan")
        .arg(fixture.plan_path())
        .assert()
        .failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");

    assert!(
        text.contains("1 compared, 0 identical, 0 differing, 1 missing from ours, 0 extra in ours"),
        "unexpected report:\n{text}"
    );
    // Our tree has no folder to take a name from, so the plan's id stands in;
    // the Oracle's own name would be the one thing we know is not ours.
    assert!(
        text.contains("- Cleaned DLC Masters  (oracle: Clean ESM)  (missing from ours)"),
        "unexpected report:\n{text}"
    );
}

#[test]
fn only_selects_a_single_mod_without_a_plan() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Blockhead/readme.txt", "same");
    write(&fixture.oracle(), "Blockhead/readme.txt", "same");
    write(&fixture.ours(), "Fort Aurus/thing.esp", "ours");
    write(&fixture.oracle(), "Fort Aurus/thing.esp", "theirs");

    let output = fixture
        .diff()
        .arg("--only")
        .arg("Blockhead")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(
        text.contains("1 compared, 1 identical, 0 differing"),
        "unexpected report:\n{text}"
    );
}

#[test]
fn section_filtering_without_a_plan_is_refused_rather_than_matching_nothing() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Blockhead/readme.txt", "same");
    write(&fixture.oracle(), "Blockhead/readme.txt", "same");

    fixture
        .diff()
        .arg("--section")
        .arg("OBSE PLUGINS")
        .assert()
        .failure()
        .stderr(predicates::str::contains("--section needs --plan"));
}

#[test]
fn json_format_carries_the_same_findings_structured() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Blockhead/data/config.ini", "bLoadModded=1");
    write(&fixture.oracle(), "Blockhead/data/config.ini", "bLoadModded=0");
    write(&fixture.oracle(), "Blockhead/extra.txt", "theirs");
    write(
        &fixture.oracle(),
        "Blockhead/meta.ini",
        "[General]\nmodid=1234\nversion=2.0\ninstallationFile=Newer Mod-1234-2-0-1750000000.7z\n",
    );

    let assert = fixture
        .diff_against_guide()
        .arg("--format")
        .arg("json")
        .assert()
        .failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");
    let report: serde_json::Value =
        serde_json::from_str(&text).expect("json report should parse");

    assert_eq!(report["summary"]["mods_compared"], 1);
    assert_eq!(report["summary"]["differing"], 1);
    assert_eq!(report["summary"]["post_guide"], 1);

    let entry = &report["sections"][0]["mods"][0];
    assert_eq!(entry["id"], "Blockhead");
    assert_eq!(entry["presence"], "both");
    assert_eq!(entry["only_in_oracle"][0], "extra.txt");

    let content = &entry["content_differs"][0];
    assert_eq!(content["path"], "data/config.ini");
    assert_eq!(content["ours_size"], 13);
    assert_eq!(content["oracle_size"], 13);
    assert!(content["ours_sha256"].is_string());
    assert_ne!(content["ours_sha256"], content["oracle_sha256"]);

    assert_eq!(entry["version"]["guide_age"]["status"], "post_guide");
    assert_eq!(entry["version"]["guide_age"]["date"], "2025-06-15");
    assert_eq!(entry["version"]["oracle_version"], "2.0");
}

#[test]
fn an_empty_scope_says_so_instead_of_claiming_success_quietly() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Blockhead/readme.txt", "same");
    write(&fixture.oracle(), "Blockhead/readme.txt", "same");

    let output = fixture
        .diff()
        .arg("--only")
        .arg("Nothing With This Name")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(
        text.contains("nothing in scope"),
        "unexpected report:\n{text}"
    );
}

#[test]
fn a_missing_mods_directory_is_a_section_not_yet_built_not_an_error() {
    let fixture = Fixture::new();
    std::fs::remove_dir_all(fixture.ours()).expect("ours should be removable");
    write(&fixture.oracle(), "Blockhead/readme.txt", "same");

    let assert = fixture.diff().assert().failure();
    let text = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");

    assert!(
        text.contains("1 compared, 0 identical, 0 differing, 1 missing from ours"),
        "unexpected report:\n{text}"
    );
}

/// A modlist that is not transcribing a guide has nothing to call an archive
/// older or newer than, and `diff` should say so rather than inventing a date.
///
/// This is the behaviour that used to be impossible: the guide's publication
/// date was a constant in the binary, so every list was measured against
/// MOFAM's March 2025 whether it followed MOFAM or not.
#[test]
fn a_modlist_with_no_guide_reports_no_archive_age() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Newer Mod/readme.txt", "same");
    write(&fixture.oracle(), "Newer Mod/readme.txt", "same");
    write(
        &fixture.oracle(),
        "Newer Mod/meta.ini",
        "[General]\nmodid=1234\nversion=2.0\ninstallationFile=Newer Mod-1234-2-0-1750000000.7z\n",
    );
    // Same plan, minus the [guide] table.
    let plan = r#"{
  "schema_version": 1,
  "name": "Diff Test",
  "responses": {},
  "mod_order": [],
  "selected_mods": [],
  "mods": [],
  "plugins": []
}"#;
    std::fs::write(fixture.plan_path(), plan).expect("plan should be written");

    let output = fixture
        .diff()
        .arg("--plan")
        .arg(fixture.plan_path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).expect("utf-8");

    assert!(
        !text.contains("POST-GUIDE"),
        "a list with no guide cannot have a post-guide archive:\n{text}"
    );
}

/// A `[guide]` whose date will not parse disables every age check in the
/// report. Failing loudly beats a report that quietly stops checking.
#[test]
fn an_unparseable_guide_date_is_an_error() {
    let fixture = Fixture::new();
    write(&fixture.ours(), "Mod/readme.txt", "same");
    write(&fixture.oracle(), "Mod/readme.txt", "same");
    let plan = r#"{
  "schema_version": 1,
  "name": "Diff Test",
  "guide": { "published": "March 2025" },
  "responses": {},
  "mod_order": [],
  "selected_mods": [],
  "mods": [],
  "plugins": []
}"#;
    std::fs::write(fixture.plan_path(), plan).expect("plan should be written");

    let output = fixture
        .diff()
        .arg("--plan")
        .arg(fixture.plan_path())
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let text = String::from_utf8(output).expect("utf-8");
    assert!(text.contains("YYYY-MM-DD"), "unexpected error:\n{text}");
}
