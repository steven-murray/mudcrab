//! `set`, `remove`, `plugins`, `files`, `action` and `input`: each edits the
//! list in place, keeps what it does not touch byte for byte, and writes
//! nothing that would not validate.

use assert_cmd::Command;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

const LIST: &str = r#"# The list's own notes.
name = "Edit Test"
inputs.big.type = "bool"
inputs.big.query = "Big textures?"
inputs.big.default = false

plugins = [
    "Oblivion.esm",
    # Core
    "Core.esp", # loads early
    "Extra.esp",
]

# Core's notes.
[[mods]]
id = "Core"
section = ["CORE"]
plugins = ["Core.esp"]

[[mods.archives]]
path = "manual:Core.zip"
layout = "simple" # checked by hand

[[mods]]
id = "Extra"
section = ["CORE"]
plugins = ["Extra.esp"]

[[mods.archives]]
path = "manual:Extra.zip"

[[mods]]
id = "Textures"
section = ["LOOKS"]
if = "big"

[[mods.archives]]
path = "manual:Textures.zip"
"#;

fn mudcrab(root: &Path) -> Command {
    let mut command = Command::cargo_bin("mudcrab").unwrap();
    command
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("LOCALAPPDATA", root.join("state"))
        .env("APPDATA", root.join("config"))
        .env("NO_COLOR", "1");
    command
}

fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().unwrap();
    let list = dir.path().join("list.toml");
    std::fs::write(&list, LIST).unwrap();
    (dir, list)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn set_changes_one_line_and_keeps_its_comment() {
    let (dir, list) = setup();
    mudcrab(dir.path())
        .args([
            "set",
            list.to_str().unwrap(),
            "--mod",
            "Core",
            "--archive",
            "1",
        ])
        .args(["layout=bain", "data_folder=Data"])
        .assert()
        .success();
    assert_eq!(
        read(&list),
        LIST.replace(
            "layout = \"simple\" # checked by hand\n",
            "layout = \"bain\" # checked by hand\ndata_folder = \"Data\"\n"
        )
    );
    mudcrab(dir.path())
        .args([
            "set",
            list.to_str().unwrap(),
            "--mod",
            "Textures",
            "--unset",
            "if",
        ])
        .assert()
        .success();
    assert!(!read(&list).contains("if = \"big\""));
}

#[test]
fn an_edit_that_would_not_validate_writes_nothing() {
    let (dir, list) = setup();
    mudcrab(dir.path())
        .args([
            "set",
            list.to_str().unwrap(),
            "--mod",
            "Core",
            "if=no_such_input",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("nothing was written"));
    mudcrab(dir.path())
        .args(["input", "remove", list.to_str().unwrap(), "big"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("nothing was written"));
    mudcrab(dir.path())
        .args([
            "set",
            list.to_str().unwrap(),
            "--mod",
            "Core",
            "section=[\"X\"]",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("mudcrab move"));
    assert_eq!(read(&list), LIST);
}

#[test]
fn dry_run_prints_the_diff_and_writes_nothing() {
    let (dir, list) = setup();
    mudcrab(dir.path())
        .args([
            "set",
            list.to_str().unwrap(),
            "--mod",
            "Extra",
            "if=big",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("+if = \"big\""))
        .stdout(predicates::str::contains("nothing written"));
    assert_eq!(read(&list), LIST);
}

#[test]
fn remove_takes_the_block_and_its_plugins() {
    let (dir, list) = setup();
    mudcrab(dir.path())
        .args(["remove", list.to_str().unwrap(), "--mod", "Extra"])
        .assert()
        .success();
    let text = read(&list);
    assert!(!text.contains("Extra"), "{text}");
    assert!(
        text.contains("    \"Core.esp\", # loads early\n]"),
        "{text}"
    );
    assert!(
        text.contains("# Core's notes.\n[[mods]]\nid = \"Core\""),
        "{text}"
    );
}

#[test]
fn plugins_list_and_move() {
    let (dir, list) = setup();
    mudcrab(dir.path())
        .args(["plugins", "list", list.to_str().unwrap()])
        .assert()
        .success()
        .stdout("Oblivion.esm\t(the game)\nCore.esp\tCore\nExtra.esp\tExtra\n");
    mudcrab(dir.path())
        .args(["plugins", "move", list.to_str().unwrap(), "Extra.esp"])
        .args(["--before", "Core.esp"])
        .assert()
        .success();
    assert!(
        read(&list).contains("    # Core\n    \"Extra.esp\",\n    \"Core.esp\", # loads early\n]"),
        "{}",
        read(&list)
    );
    mudcrab(dir.path())
        .args([
            "plugins",
            "move",
            list.to_str().unwrap(),
            "Extra.esp",
            "--last",
        ])
        .assert()
        .success();
    assert_eq!(read(&list), LIST);
}

#[test]
fn actions_add_list_and_remove() {
    let (dir, list) = setup();
    mudcrab(dir.path())
        .args([
            "action",
            "add",
            list.to_str().unwrap(),
            "--mod",
            "Core",
            "file_hide",
        ])
        .args(["paths=[\"textures/a.dds\"]", "--if", "big"])
        .assert()
        .success();
    mudcrab(dir.path())
        .args(["action", "list", list.to_str().unwrap(), "--mod", "Core"])
        .assert()
        .success()
        .stdout("1. action = \"file_hide\", if = \"big\", paths = [\"textures/a.dds\"]\n");
    // A field the action does not have is refused by validation.
    mudcrab(dir.path())
        .args([
            "action",
            "add",
            list.to_str().unwrap(),
            "--mod",
            "Core",
            "file_hide",
        ])
        .args(["pathz=[\"x\"]"])
        .assert()
        .failure();
    mudcrab(dir.path())
        .args([
            "action",
            "remove",
            list.to_str().unwrap(),
            "--mod",
            "Core",
            "1",
        ])
        .assert()
        .success();
    assert_eq!(read(&list), LIST);
}

#[test]
fn inputs_add_set_list_and_remove() {
    let (dir, list) = setup();
    mudcrab(dir.path())
        .args(["input", "add", list.to_str().unwrap(), "size"])
        .args(["--type", "choice", "--query", "Which size?"])
        .args(["--choice", "1k", "--choice", "2k", "--default", "1k"])
        .assert()
        .success();
    mudcrab(dir.path())
        .args(["input", "set", list.to_str().unwrap(), "size", "default=2k"])
        .assert()
        .success();
    mudcrab(dir.path())
        .args(["input", "list", list.to_str().unwrap()])
        .assert()
        .success()
        .stdout("big\tbool\tfalse\tBig textures?\nsize\tchoice [1k, 2k]\t2k\tWhich size?\n");
    mudcrab(dir.path())
        .args(["input", "remove", list.to_str().unwrap(), "size"])
        .assert()
        .success();
    assert_eq!(read(&list), LIST);
}

#[test]
fn files_shows_what_an_archive_installs_and_what_a_filter_changes() {
    let (dir, list) = setup();
    let downloads = dir.path().join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(downloads.join("Core.zip")).unwrap());
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
    for name in ["Core.esp", "textures/a.dds", "readme.txt"] {
        zip.start_file(name, options).unwrap();
        zip.write_all(b"x").unwrap();
    }
    zip.finish().unwrap();

    mudcrab(dir.path())
        .args([
            "files",
            list.to_str().unwrap(),
            "--mod",
            "Core",
            "--downloads",
        ])
        .arg(&downloads)
        .assert()
        .success()
        .stdout("Core.esp\nreadme.txt\ntextures/a.dds\n");
    mudcrab(dir.path())
        .args([
            "files",
            list.to_str().unwrap(),
            "--mod",
            "Core",
            "--downloads",
        ])
        .arg(&downloads)
        .args(["--exclude", "*.txt"])
        .assert()
        .success()
        .stdout(predicates::str::contains("- readme.txt"))
        .stdout(predicates::str::contains("2 file(s) taken, was 3"));
    assert!(read(&list).contains("exclude = [\"*.txt\"]"));
}

#[test]
fn files_chooses_a_bain_package_as_wrye_bash_ticks_one() {
    let dir = tempdir().unwrap();
    let list = dir.path().join("list.toml");
    std::fs::write(
        &list,
        "name = \"B\"\n\n[[mods]]\nid = \"Pack\"\n\n[[mods.archives]]\npath = \"manual:Pack.zip\"\nlayout = \"bain\"\nbain_subpackages = [\"00 Core\"]\n",
    )
    .unwrap();
    let downloads = dir.path().join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(downloads.join("Pack.zip")).unwrap());
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
    for name in ["00 Core/textures/a.dds", "10 Extra/textures/b.dds"] {
        zip.start_file(name, options).unwrap();
        zip.write_all(b"x").unwrap();
    }
    zip.finish().unwrap();

    let files = |extra: &[&str]| {
        let mut cmd = mudcrab(dir.path());
        cmd.args([
            "files",
            list.to_str().unwrap(),
            "--mod",
            "Pack",
            "--downloads",
        ])
        .arg(&downloads)
        .args(extra);
        cmd
    };
    files(&["--package", "10 Extra"])
        .assert()
        .success()
        .stdout(predicates::str::contains("+ textures/b.dds"));
    assert!(read(&list).contains("bain_subpackages = [\"00 Core\", \"10 Extra\"]"));
    files(&["--drop-package", "00 Core"])
        .assert()
        .success()
        .stdout(predicates::str::contains("- textures/a.dds"));
    assert!(read(&list).contains("bain_subpackages = [\"10 Extra\"]"));
}

#[test]
fn docs_writes_a_mods_notes_where_they_already_are() {
    let dir = tempdir().unwrap();
    // Inline: a list with no docs file.
    let inline = dir.path().join("inline.toml");
    std::fs::write(&inline, "name = \"I\"\n\n[[mods]]\nid = \"A\"\n").unwrap();
    mudcrab(dir.path())
        .args(["docs", inline.to_str().unwrap(), "--mod", "A"])
        .args(["--summary", "One line.", "--body", "First.\n\nSecond.\n"])
        .assert()
        .success();
    let text = read(&inline);
    assert!(text.contains("docs.summary = \"One line.\""), "{text}");
    assert!(
        text.contains("docs.body = '''\nFirst.\n\nSecond.\n'''"),
        "{text}"
    );

    // In the docs file, for a list that names one; removing the last field
    // removes the mod's table.
    let list = dir.path().join("list.toml");
    let docs = dir.path().join("list.docs.toml");
    std::fs::write(
        &list,
        "name = \"D\"\ndocs_file = \"list.docs.toml\"\n\n[[mods]]\nid = \"A\"\n",
    )
    .unwrap();
    std::fs::write(&docs, "[mods.\"A\"]\ntitle = \"The A\"\n").unwrap();
    mudcrab(dir.path())
        .args([
            "docs",
            list.to_str().unwrap(),
            "--mod",
            "A",
            "--summary",
            "Why.",
        ])
        .assert()
        .success();
    assert_eq!(
        read(&docs),
        "[mods.\"A\"]\ntitle = \"The A\"\nsummary = \"Why.\"\n"
    );
    assert!(!read(&list).contains("Why."));
    mudcrab(dir.path())
        .args(["docs", list.to_str().unwrap(), "--mod", "A"])
        .args(["--unset", "title", "--unset", "summary"])
        .assert()
        .success();
    assert_eq!(read(&docs), "");
    // A mod the list does not have is refused.
    mudcrab(dir.path())
        .args([
            "docs",
            list.to_str().unwrap(),
            "--mod",
            "Nope",
            "--summary",
            "x",
        ])
        .assert()
        .failure();
}

#[test]
fn choose_walks_an_archives_installer_again_and_writes_its_choices() {
    let dir = tempdir().unwrap();
    let config = r#"<config><moduleName>M</moduleName><installSteps order="Explicit">
      <installStep name="Main"><optionalFileGroups order="Explicit">
        <group name="Size" type="SelectExactlyOne"><plugins order="Explicit">
          <plugin name="Big"><files><folder source="big" destination="" /></files>
            <typeDescriptor><type name="Recommended" /></typeDescriptor></plugin>
          <plugin name="Small"><files><folder source="small" destination="" /></files>
            <typeDescriptor><type name="Optional" /></typeDescriptor></plugin>
        </plugins></group>
      </optionalFileGroups></installStep></installSteps></config>"#;
    let downloads = dir.path().join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(downloads.join("M.zip")).unwrap());
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
    for (name, bytes) in [
        ("fomod/ModuleConfig.xml", config.as_bytes()),
        ("big/a.dds", b"x"),
        ("small/a.dds", b"x"),
    ] {
        zip.start_file(name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
    let list = dir.path().join("list.toml");
    let entry = "name = \"F\"\n\n[[mods]]\nid = \"M\"\n\n[[mods.archives]]\npath = \"manual:M.zip\"\nlayout = \"fomod\"\n";
    let choose = || {
        let mut cmd = mudcrab(dir.path());
        cmd.args(["choose", list.to_str().unwrap(), "--mod", "M", "--defaults"])
            .arg("--downloads")
            .arg(&downloads);
        cmd
    };

    // No choices yet: the installer's own.
    std::fs::write(&list, entry).unwrap();
    choose().assert().success();
    assert!(
        read(&list).contains("step = \"Main\"\ngroup = \"Size\"\noptions = [\"Big\"]"),
        "{}",
        read(&list)
    );
    // A choice the list has made is the default, and kept.
    std::fs::write(
        &list,
        format!("{entry}\n[[mods.archives.fomod_selections]]\nstep = \"Main\"\ngroup = \"Size\"\noptions = [\"Small\"]\n"),
    )
    .unwrap();
    choose().assert().success();
    assert!(
        read(&list).contains("options = [\"Small\"]"),
        "{}",
        read(&list)
    );
}
