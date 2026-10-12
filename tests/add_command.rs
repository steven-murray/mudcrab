use assert_cmd::Command;
use mudcrab::config::loader::load_modlist;
use mudcrab::config::validator::validate;
use predicates::prelude::PredicateBooleanExt;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

/// A modlist deliberately full of the things a parse-and-reemit would destroy:
/// a header comment, a comment inside a multi-line array, tab indentation,
/// comments glued to a block and comments trailing one, and a `[ini]` table
/// after the mods.
const FIXTURE: &str = "\
# Why this list exists. Do not reorder without reading the notes.
name = \"Add Test\"

plugins = [
\t\"Oblivion.esm\",
\t# Core.esp must load here; LOOT sorts it wrong.
\t\"Core.esp\",
]

[[mods]]
id = \"Alpha\"
section = [\"CORE\"]

# Alpha's archive is nested two levels deep.
[[mods.archives]]
path = \"nexus:oblivion/1/2\"

[[mods]]
id = \"Beta\"
section = [\"CORE\"]

[[mods.archives]]
path = \"nexus:oblivion/3/4\"
# Beta ships an old DLL; check it against OBSE 22 before updating.

[[mods]]
id = \"Gamma\"
section = [\"UI\"]

[[mods.archives]]
path = \"nexus:oblivion/5/6\"

[ini]
bUseThreadedAI = 1
";

fn fixture() -> (TempDir, PathBuf) {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(&path, FIXTURE).expect("fixture modlist should be written");
    (dir, path)
}

fn add(path: &Path) -> Command {
    let mut command = Command::cargo_bin("mudcrab")
        .map(|mut c| {
            c.env(
                "XDG_STATE_HOME",
                std::env::temp_dir().join("mudcrab-test-state"),
            )
            .env(
                "LOCALAPPDATA",
                std::env::temp_dir().join("mudcrab-test-state"),
            );
            c
        })
        .expect("binary should build");
    // --nexus looks the file's name up when a key is set; tests stay offline.
    command.env_remove("NEXUS_API_KEY");
    command.arg("add").arg(path);
    command
}

fn mod_ids(path: &Path) -> Vec<String> {
    let source = load_modlist(path).expect("modlist should parse");
    validate(&source).expect("modlist should validate");
    source.mods.iter().map(|entry| entry.id.clone()).collect()
}

/// Assert that `updated` is `original` with one contiguous run of bytes spliced
/// in: every other byte, comments and whitespace included, is untouched.
fn assert_pure_insertion(original: &str, updated: &str) {
    assert!(
        updated.len() > original.len(),
        "insertion should make the file longer"
    );

    let old = original.as_bytes();
    let new = updated.as_bytes();

    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    assert!(
        prefix + suffix >= old.len(),
        "file was not edited by pure insertion: only {prefix} leading and {suffix} trailing of \
         {} original bytes survived",
        old.len()
    );
}

/// An Oracle-shaped mods directory holding one mod folder.
fn oracle_mod(root: &Path, folder: &str, meta: &str) -> PathBuf {
    let mod_dir = root.join("oracle").join(folder);
    std::fs::create_dir_all(&mod_dir).expect("oracle mod dir should be created");
    std::fs::write(mod_dir.join("meta.ini"), meta).expect("meta.ini should be written");
    root.join("oracle")
}

const NEXUS_META: &str = "\
[General]
gameName=Oblivion
modid=43752
ignoredVersion=
version=11.1.0.0
category=\"7,\"
installationFile=Blockhead-43752-11-1-1640043918.7z
repository=Nexus
color=@Variant(\\0\\0\\0\\x43\\0\\xff\\xff\\0\\0\\0\\0\\0\\0\\0\\0)

[installedFiles]
size=1
1\\modid=43752
1\\fileid=1000029844

[Plugins]
Omod%20Installer\\omodsPendingPostInstall=@Invalid()
";

#[test]
fn inserts_at_the_end_of_the_named_section() {
    let (_dir, path) = fixture();

    add(&path)
        .args(["--nexus", "9/10", "--id", "Delta", "--section", "CORE"])
        .assert()
        .success();

    assert_eq!(
        mod_ids(&path),
        vec!["Alpha", "Beta", "Delta", "Gamma"],
        "the new mod belongs after the last CORE mod, not at the end of the file"
    );

    let updated = std::fs::read_to_string(&path).expect("modlist should be readable");
    assert!(
        updated.contains("# Beta ships an old DLL; check it against OBSE 22 before updating.\n\n[[mods]]\nid = \"Delta\""),
        "insert should follow Beta's trailing note, not displace it:\n{updated}"
    );
}

#[test]
fn a_new_section_is_appended_at_the_end_of_the_file() {
    let (_dir, path) = fixture();

    add(&path)
        .args(["--nexus", "9/10", "--id", "Delta", "--section", "BRAND NEW"])
        .assert()
        .success();

    assert_eq!(mod_ids(&path), vec!["Alpha", "Beta", "Gamma", "Delta"]);

    let updated = std::fs::read_to_string(&path).expect("modlist should be readable");
    assert!(
        updated.trim_end().ends_with("download_handler = \"nexus\""),
        "the block should be the last thing in the file:\n{updated}"
    );
}

#[test]
fn comments_and_whitespace_elsewhere_are_byte_identical() {
    let (_dir, path) = fixture();

    add(&path)
        .args(["--nexus", "9/10", "--id", "Delta", "--section", "CORE"])
        .assert()
        .success();

    let updated = std::fs::read_to_string(&path).expect("modlist should be readable");
    assert_pure_insertion(FIXTURE, &updated);

    for surviving in [
        "# Why this list exists. Do not reorder without reading the notes.",
        "\t# Core.esp must load here; LOOT sorts it wrong.",
        "# Alpha's archive is nested two levels deep.",
        "[ini]\nbUseThreadedAI = 1",
    ] {
        assert!(updated.contains(surviving), "lost: {surviving}");
    }
}

#[test]
fn repeated_inserts_into_one_section_stay_in_that_section() {
    let (_dir, path) = fixture();
    let mut previous = FIXTURE.to_string();

    for (index, id) in ["Delta", "Epsilon", "Zeta"].iter().enumerate() {
        add(&path)
            .args(["--nexus", &format!("{}/1", index + 10), "--id", id])
            .args(["--section", "CORE"])
            .assert()
            .success();

        let updated = std::fs::read_to_string(&path).expect("modlist should be readable");
        assert_pure_insertion(&previous, &updated);
        previous = updated;
    }

    assert_eq!(
        mod_ids(&path),
        vec!["Alpha", "Beta", "Delta", "Epsilon", "Zeta", "Gamma"]
    );
}

#[test]
fn refuses_a_duplicate_mod_id() {
    let (_dir, path) = fixture();

    add(&path)
        .args(["--nexus", "9/10", "--id", "Beta", "--section", "CORE"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "already contains a mod with id 'Beta'",
        ));

    assert_eq!(
        std::fs::read_to_string(&path).expect("modlist should be readable"),
        FIXTURE,
        "a refused add must not touch the file"
    );
}

#[test]
fn refuses_a_mod_id_that_is_not_a_safe_directory_name() {
    let (_dir, path) = fixture();

    add(&path)
        .args(["--nexus", "9/10", "--id", "bad/name", "--section", "CORE"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("path separator"));

    assert_eq!(
        std::fs::read_to_string(&path).expect("modlist should be readable"),
        FIXTURE
    );
}

#[test]
fn accepts_ids_containing_spaces_brackets_apostrophes_and_ampersands() {
    let (_dir, path) = fixture();
    let id = "Harvest [Flora] - Blue's Fixes & Tweaks";

    add(&path)
        .args(["--nexus", "9/10", "--id", id, "--section", "CORE"])
        .assert()
        .success();

    assert!(mod_ids(&path).iter().any(|entry| entry == id));
}

#[test]
fn from_mo2_reads_meta_ini_and_the_old_flag_name_still_works() {
    let (dir, path) = fixture();
    let oracle = oracle_mod(dir.path(), "Blockhead", NEXUS_META);

    add(&path)
        .arg("--from-oracle")
        .arg(&oracle)
        .args(["--mod", "Blockhead", "--section", "OBSE PLUGINS"])
        .assert()
        .success();

    let updated = std::fs::read_to_string(&path).expect("modlist should be readable");
    assert_pure_insertion(FIXTURE, &updated);

    // id defaults to the folder name.
    assert!(mod_ids(&path).iter().any(|entry| entry == "Blockhead"));
    assert!(updated.contains("path = \"nexus:oblivion/43752/1000029844\""));
    assert!(updated.contains("download_handler = \"nexus\""));
    assert!(updated.contains("file_name = \"Blockhead-43752-11-1-1640043918.7z\""));
    assert!(updated.contains("# version 11.1.0.0"));
}

#[test]
fn from_oracle_id_can_be_overridden() {
    let (dir, path) = fixture();
    let oracle = oracle_mod(dir.path(), "Blockhead", NEXUS_META);

    add(&path)
        .arg("--from-mo2")
        .arg(&oracle)
        .args(["--mod", "Blockhead", "--id", "Blockhead (OBSE)"])
        .args(["--section", "OBSE PLUGINS"])
        .assert()
        .success();

    assert!(
        mod_ids(&path)
            .iter()
            .any(|entry| entry == "Blockhead (OBSE)")
    );
}

#[test]
fn modid_zero_gets_a_todo_and_no_path() {
    let (dir, path) = fixture();
    let meta = "\
[General]
modid=0
version=1.3.2.0
installationFile=DarNified UI 132 FOMOD - Merged.7z
url=https://www.nexusmods.com/oblivion/mods/10763

[installedFiles]
size=1
1\\modid=0
1\\fileid=0
";
    let oracle = oracle_mod(dir.path(), "DarNified UI", meta);

    add(&path)
        .arg("--from-mo2")
        .arg(&oracle)
        .args(["--mod", "DarNified UI", "--section", "UI"])
        .assert()
        .success()
        .stderr(predicates::str::contains("did not come from Nexus"));

    let updated = std::fs::read_to_string(&path).expect("modlist should be readable");
    assert!(updated.contains("# TODO: non-Nexus source"));
    assert!(updated.contains("file_name = \"DarNified UI 132 FOMOD - Merged.7z\""));
    assert!(
        !updated.contains("nexus:oblivion/0/"),
        "no URL may be invented for a non-Nexus mod:\n{updated}"
    );

    let source = load_modlist(&path).expect("modlist should parse");
    validate(&source).expect("modlist should still validate");
    let entry = source
        .mods
        .iter()
        .find(|entry| entry.id == "DarNified UI")
        .expect("the new mod should be present");
    assert_eq!(entry.archives.len(), 1);
    assert!(entry.archives[0].path.is_none());
}

#[test]
fn reports_plugins_without_touching_the_load_order() {
    let (dir, path) = fixture();
    let oracle = oracle_mod(dir.path(), "Blockhead", NEXUS_META);
    let mod_dir = oracle.join("Blockhead");
    std::fs::write(mod_dir.join("Visible.esp"), b"x").expect("plugin should be written");
    std::fs::create_dir_all(mod_dir.join("nested")).expect("subdir should be created");
    std::fs::write(mod_dir.join("nested/Deep.esm"), b"x").expect("plugin should be written");
    std::fs::write(mod_dir.join("Disabled.esp.mohidden"), b"x").expect("hidden should be written");

    add(&path)
        .arg("--from-mo2")
        .arg(&oracle)
        .args(["--mod", "Blockhead", "--section", "OBSE PLUGINS"])
        .assert()
        .success()
        .stderr(predicates::str::contains("ships 2 plugins"))
        .stderr(predicates::str::contains("Visible.esp"))
        .stderr(predicates::str::contains("nested/Deep.esm"))
        .stderr(predicates::str::contains("Disabled.esp").not());

    let source = load_modlist(&path).expect("modlist should parse");
    // Nothing was guessed into the load order, and the block declares no
    // plugins, so the modlist still validates.
    validate(&source).expect("modlist should still validate");
    assert_eq!(source.plugins, vec!["Oblivion.esm", "Core.esp"]);
    let entry = source
        .mods
        .iter()
        .find(|entry| entry.id == "Blockhead")
        .expect("the new mod should be present");
    assert!(entry.plugins.is_empty());
}

#[test]
fn dry_run_prints_the_block_and_writes_nothing() {
    let (_dir, path) = fixture();

    add(&path)
        .args(["--nexus", "9/10", "--id", "Delta", "--section", "CORE"])
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "after the last mod in section CORE",
        ))
        .stdout(predicates::str::contains("id = \"Delta\""))
        .stdout(predicates::str::contains("path = \"nexus:oblivion/9/10\""));

    assert_eq!(
        std::fs::read_to_string(&path).expect("modlist should be readable"),
        FIXTURE,
        "--dry-run must not change the file"
    );
}

#[test]
fn omitting_the_section_warns_and_leaves_the_key_off() {
    let (dir, path) = fixture();
    let oracle = oracle_mod(dir.path(), "Blockhead", NEXUS_META);

    add(&path)
        .arg("--from-mo2")
        .arg(&oracle)
        .args(["--mod", "Blockhead"])
        .assert()
        .success()
        .stderr(predicates::str::contains("no --section given"));

    let source = load_modlist(&path).expect("modlist should parse");
    let entry = source
        .mods
        .iter()
        .find(|entry| entry.id == "Blockhead")
        .expect("the new mod should be present");
    assert!(entry.section.is_empty());
}

#[test]
fn a_nested_section_path_is_matched_level_by_level() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    let text = "name = \"Nested\"\n\n[[mods]]\nid = \"A\"\nsection = [\"OUTER\", \"Inner\"]\n\n[[mods]]\nid = \"B\"\nsection = [\"OUTER\"]\n";
    std::fs::write(&path, text).expect("fixture should be written");

    add(&path)
        .args(["--nexus", "1/2", "--id", "C"])
        .args(["--section", "OUTER", "--section", "Inner"])
        .assert()
        .success();

    assert_eq!(mod_ids(&path), vec!["A", "C", "B"]);

    let source = load_modlist(&path).expect("modlist should parse");
    let entry = source
        .mods
        .iter()
        .find(|entry| entry.id == "C")
        .expect("the new mod should be present");
    assert_eq!(entry.section, vec!["OUTER", "Inner"]);
}

#[test]
fn a_modid_with_no_installed_fileid_is_refused_rather_than_guessed() {
    let (dir, path) = fixture();
    let meta = "[General]\nmodid=52229\nversion=1.0\ninstallationFile=x.7z\n\n[installedFiles]\nsize=1\n1\\modid=0\n1\\fileid=0\n";
    let oracle = oracle_mod(dir.path(), "Script Patch", meta);

    add(&path)
        .arg("--from-mo2")
        .arg(&oracle)
        .args(["--mod", "Script Patch", "--section", "CORE"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--nexus 52229/<fileid>"));

    assert_eq!(
        std::fs::read_to_string(&path).expect("modlist should be readable"),
        FIXTURE
    );
}

#[test]
fn an_insert_that_would_break_validation_restores_the_original() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    // Already invalid before the edit (empty name), so the post-insert
    // validation is guaranteed to fail and the rollback path is exercised.
    let text = "name = \"\"\n\n[[mods]]\nid = \"A\"\nsection = [\"S\"]\n";
    std::fs::write(&path, text).expect("fixture should be written");

    add(&path)
        .args(["--nexus", "1/2", "--id", "B", "--section", "S"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("original file has been restored"));

    assert_eq!(
        std::fs::read_to_string(&path).expect("modlist should be readable"),
        text,
        "a failed validation must leave the author's file exactly as it was"
    );
}

#[test]
fn requires_a_source() {
    let (_dir, path) = fixture();

    add(&path)
        .args(["--id", "Delta", "--section", "CORE"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("give a source"));
}

#[test]
fn a_new_plugin_needs_a_place_in_the_load_order() {
    let (_dir, path) = fixture();
    let original = std::fs::read_to_string(&path).unwrap();
    add(&path)
        .args(["--nexus", "7/8", "--id", "Delta", "--section", "CORE"])
        .args(["--plugin", "Delta.esp"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--load-after"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);

    add(&path)
        .args(["--nexus", "7/8", "--id", "Delta", "--section", "CORE"])
        .args(["--plugin", "Delta.esp", "--load-after", "Oblivion.esm"])
        .assert()
        .success();
    let source = load_modlist(&path).unwrap();
    assert_eq!(source.plugins, ["Oblivion.esm", "Delta.esp", "Core.esp"]);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("\t\"Delta.esp\",\n\t# Core.esp must load here"),
        "the comment stays with Core.esp: {text}"
    );

    add(&path)
        .args(["--nexus", "9/10", "--id", "Epsilon", "--section", "UI"])
        .args(["--plugin", "Epsilon.esp", "--load-last"])
        .assert()
        .success();
    assert_eq!(
        load_modlist(&path)
            .unwrap()
            .plugins
            .last()
            .map(String::as_str),
        Some("Epsilon.esp")
    );
}

#[test]
fn an_archive_writes_its_own_entry() {
    let (dir, path) = fixture();
    let archive = dir.path().join("Delta-77-1-0.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for name in ["Delta Data/Delta.esp", "Delta Data/textures/a.dds"] {
        zip.start_file(name, options).unwrap();
        std::io::Write::write_all(&mut zip, b"x").unwrap();
    }
    zip.finish().unwrap();

    // With no `.meta` the source has to be given.
    add(&path)
        .arg("--archive")
        .arg(&archive)
        .args(["--id", "Delta", "--section", "CORE"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--nexus"));

    std::fs::write(
        dir.path().join("Delta-77-1-0.zip.meta"),
        "[General]\ngameName=Oblivion\nmodID=77\nfileID=99\n",
    )
    .unwrap();
    add(&path)
        .arg("--archive")
        .arg(&archive)
        .args(["--id", "Delta", "--section", "CORE"])
        .args(["--plugin", "Delta.esp", "--load-last"])
        .assert()
        .success();
    let source = load_modlist(&path).unwrap();
    let delta = source.mods.iter().find(|m| m.id == "Delta").unwrap();
    let archive = &delta.archives[0];
    assert_eq!(archive.path.as_deref(), Some("nexus:oblivion/77/99"));
    assert_eq!(archive.file_name.as_deref(), Some("Delta-77-1-0.zip"));
    assert_eq!(delta.plugins, ["Delta.esp"]);
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("# no layout")
    );
}

#[test]
fn a_bain_wizard_is_run_and_its_result_recorded() {
    let (dir, path) = fixture();
    let archive = dir.path().join("Wiz-88-1-0.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    let wizard = "\
DeSelectAll
SelectSubPackage \"00 Core\"
If !DataFileExists(\"Missing.esm\")
    DeSelectEspm \"Core - Patch.esp\"
EndIf
SelectOne \"Style\", \"Plain\", \"\", \"\", \"|Coloured\", \"\", \"\"
    Case \"Plain\"
        SelectSubPackage \"01 Plain\"
        Break
    Case \"Coloured\"
        SelectSubPackage \"01 Coloured\"
        EditINI('ini\\\\Wiz.ini', 'set', 'Wiz.bColour', 1)
        Break
EndSelect
";
    for (name, body) in [
        ("wizard.txt", wizard),
        ("00 Core/Core.esp", "x"),
        ("00 Core/Core - Patch.esp", "x"),
        ("01 Plain/textures/a.dds", "x"),
        ("01 Coloured/textures/a.dds", "x"),
    ] {
        zip.start_file(name, options).unwrap();
        std::io::Write::write_all(&mut zip, body.as_bytes()).unwrap();
    }
    zip.finish().unwrap();

    // Not a terminal, so the wizard runs on its defaults.
    add(&path)
        .arg("--archive")
        .arg(&archive)
        .args(["--nexus", "88/99", "--id", "Wiz", "--section", "CORE"])
        .args(["--plugin", "Core.esp", "--load-last"])
        .assert()
        .success();
    let source = load_modlist(&path).unwrap();
    let wiz = source.mods.iter().find(|m| m.id == "Wiz").unwrap();
    let archive = &wiz.archives[0];
    let subpackages: Vec<&str> = archive.bain_subpackages.iter().map(|s| s.name()).collect();
    assert_eq!(subpackages, ["00 Core", "01 Coloured"]);
    assert_eq!(archive.exclude, ["Core - Patch.esp"]);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("action = \"ini_tweak\"\nfile = \"Wiz.ini\"\nkey = \"Wiz.bColour\"\nvalue = \"1\"\nformat = \"set-to\""),
        "{text}"
    );
}
