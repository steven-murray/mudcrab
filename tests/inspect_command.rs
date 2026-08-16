//! `inspect` reads an archive and says what its modlist entry has to declare.
//!
//! The point of the command is that the author never has to extract an archive
//! and read `fomod/ModuleConfig.xml` by hand, so what is asserted here is that
//! the names they would have transcribed -- steps, groups, options, subpackage
//! folders -- come out verbatim, and that a big archive stays readable.

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

fn inspect(archive: &Path, extra: &[&str]) -> String {
    let assert = Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("inspect")
        .arg(archive)
        .args(extra)
        .assert()
        .success();
    String::from_utf8(assert.get_output().stdout.clone()).expect("stdout should be utf-8")
}

const MODULE_CONFIG: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<config>
  <moduleName>Fort Aurus Overhaul</moduleName>
  <requiredInstallFiles>
    <file source="base.txt" destination="base.txt" />
  </requiredInstallFiles>
  <installSteps order="Explicit">
    <installStep name="Main">
      <optionalFileGroups order="Explicit">
        <group name="Core Files" type="SelectExactlyOne">
          <plugins order="Explicit">
            <plugin name="Full">
              <files>
                <folder source="full" destination="" priority="0" />
              </files>
              <typeDescriptor><type name="Recommended" /></typeDescriptor>
            </plugin>
            <plugin name="Lite">
              <files>
                <folder source="lite" destination="" priority="0" />
              </files>
              <typeDescriptor><type name="Optional" /></typeDescriptor>
            </plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
    <installStep name="Extras">
      <optionalFileGroups order="Explicit">
        <group name="Optional Patches" type="SelectAny">
          <plugins order="Explicit">
            <plugin name="UOP Patch">
              <files>
                <file source="uop.esp" destination="uop.esp" priority="0" />
              </files>
              <typeDescriptor><type name="Optional" /></typeDescriptor>
            </plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
  </installSteps>
</config>"#;

#[test]
fn inspect_prints_every_fomod_step_group_and_option() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("Fort Aurus FOMOD-1-0.zip");
    make_zip(
        &archive,
        &[
            ("Fort Aurus/fomod/ModuleConfig.xml", MODULE_CONFIG),
            ("Fort Aurus/base.txt", b"base"),
            ("Fort Aurus/full/Meshes/full.nif", b"full"),
            ("Fort Aurus/lite/Meshes/lite.nif", b"lite"),
            ("Fort Aurus/uop.esp", b"TES4"),
        ],
    );

    let report = inspect(&archive, &[]);

    assert!(report.contains("layout guess: FOMOD"), "{report}");
    assert!(
        report.contains("Fort Aurus/fomod/ModuleConfig.xml"),
        "the installer's location should be named:\n{report}"
    );
    assert!(
        report.contains("Fort Aurus Overhaul"),
        "the module name should be printed:\n{report}"
    );

    for expected in [
        "step \"Main\"",
        "group \"Core Files\" (SelectExactlyOne)",
        "\"Full\"",
        "\"Lite\"",
        "step \"Extras\"",
        "group \"Optional Patches\" (SelectAny)",
        "\"UOP Patch\"",
    ] {
        assert!(
            report.contains(expected),
            "expected {expected:?} in the report:\n{report}"
        );
    }

    // The option types decide what a group installs when nothing is declared,
    // so they are part of what the author needs to see.
    assert!(report.contains("Recommended"), "{report}");

    // The snippet is the whole reason for the command: it has to be pasteable
    // TOML naming the same step and group the installer will look up.
    assert!(report.contains("layout = \"fomod\""), "{report}");
    assert!(
        report.contains("[[mods.archives.fomod_selections]]"),
        "{report}"
    );
    assert!(report.contains("step = \"Main\""), "{report}");
    assert!(report.contains("group = \"Core Files\""), "{report}");
    assert!(
        report.contains("options = [\"Full\"]"),
        "the recommended option is what install picks, so it should be pre-filled:\n{report}"
    );

    // The plugin has to go into the load order by hand, so it is called out.
    assert!(report.contains("Fort Aurus/uop.esp"), "{report}");
}

#[test]
fn inspect_lists_bain_subpackages_for_a_numbered_archive() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("Book Jackets BAIN-1-0.zip");
    make_zip(
        &archive,
        &[
            ("00 Core/Textures/core.dds", b"core"),
            ("01a ESP Vanilla/Books.esp", b"TES4"),
            ("01b ESP Filtered/Books.esp", b"TES4"),
            ("02 Golden/Textures/gold.dds", b"gold"),
        ],
    );

    let report = inspect(&archive, &[]);

    assert!(report.contains("layout guess: BAIN"), "{report}");
    assert!(report.contains("layout = \"bain\""), "{report}");
    for subpackage in ["00 Core", "01a ESP Vanilla", "01b ESP Filtered", "02 Golden"] {
        assert!(
            report.contains(&format!("\"{subpackage}\",")),
            "{subpackage} should appear in the bain_subpackages snippet:\n{report}"
        );
    }
    // `01a` and `01b` are alternatives, which the report must not pretend to
    // resolve -- it lists them and says so.
    assert!(
        report.contains("alternatives to each other"),
        "the report should warn that the list needs pruning:\n{report}"
    );
}

#[test]
fn inspect_guesses_a_plain_archive_needs_nothing_declared() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("Simple Mod-1-0.zip");
    make_zip(
        &archive,
        &[
            ("Data/Meshes/thing.nif", b"mesh"),
            ("Data/Thing.esp", b"TES4"),
        ],
    );

    let report = inspect(&archive, &[]);

    assert!(report.contains("layout guess: plain data folder"), "{report}");
    assert!(
        report.contains("no layout, data_folder or target_subdir needed"),
        "{report}"
    );
    assert!(
        !report.contains("layout = "),
        "a plain archive should not be told to declare a layout:\n{report}"
    );
    assert!(report.contains("Data/Thing.esp"), "{report}");
}

#[test]
fn inspect_names_a_data_folder_the_auto_layout_would_not_find() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("Nested-1-0.zip");
    // Two top-level entries, so nothing is unwrapped automatically; the
    // content sits under one of them.
    make_zip(
        &archive,
        &[
            ("Docs/readme.txt", b"read me"),
            ("Install This/Textures/thing.dds", b"texture"),
        ],
    );

    let report = inspect(&archive, &[]);

    assert!(report.contains("layout guess: nested data folder"), "{report}");
    assert!(
        report.contains("data_folder = \"Install This\""),
        "{report}"
    );
}

#[test]
fn the_file_listing_is_opt_in() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("Big-1-0.zip");
    let entries: Vec<(String, Vec<u8>)> = (0..250)
        .map(|idx| (format!("Textures/tile{idx:03}.dds"), b"dds".to_vec()))
        .collect();
    let borrowed: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    make_zip(&archive, &borrowed);

    // A texture pack's default report is a summary, not its contents.
    let summary = inspect(&archive, &[]);
    assert!(
        !summary.contains("Textures/tile042.dds"),
        "the default report must not list files:\n{summary}"
    );
    assert!(
        summary.contains("250 files in the archive; pass --files to list them."),
        "{summary}"
    );
    assert!(
        summary.lines().count() < 30,
        "a 250-file archive should still produce a short report, got {} lines:\n{summary}",
        summary.lines().count()
    );

    let listed = inspect(&archive, &["--files"]);
    assert!(listed.contains("Textures/tile042.dds"), "{listed}");
    assert!(listed.contains("Textures/tile249.dds"), "{listed}");
}

#[test]
fn json_format_carries_the_same_findings_structured() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("Fort Aurus FOMOD-1-0.zip");
    make_zip(
        &archive,
        &[
            ("Fort Aurus/fomod/ModuleConfig.xml", MODULE_CONFIG),
            ("Fort Aurus/base.txt", b"base"),
            ("Fort Aurus/full/Meshes/full.nif", b"full"),
            ("Fort Aurus/lite/Meshes/lite.nif", b"lite"),
        ],
    );

    let raw = inspect(&archive, &["--format", "json"]);
    let value: serde_json::Value = serde_json::from_str(&raw).expect("output should be json");

    assert_eq!(value["layout"]["kind"], "fomod");
    let steps = value["fomod"]["steps"]
        .as_array()
        .expect("steps should be an array");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["name"], "Main");
    assert_eq!(steps[0]["groups"][0]["name"], "Core Files");
    assert_eq!(steps[0]["groups"][0]["group_type"], "SelectExactlyOne");
    assert_eq!(steps[0]["groups"][0]["options"][0]["name"], "Full");
    assert_eq!(
        steps[0]["groups"][0]["options"][0]["selected_by_default"],
        true
    );
    assert_eq!(
        steps[0]["groups"][0]["options"][1]["selected_by_default"],
        false
    );
    assert!(value["files"].is_null(), "--files was not passed");
}

#[test]
fn inspect_refuses_a_path_that_is_not_an_archive() {
    let dir = tempdir().expect("temp dir should be created");
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("inspect")
        .arg(dir.path().join("nope.zip"))
        .assert()
        .failure();
}
