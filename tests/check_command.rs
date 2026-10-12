//! `check`: what can be known before installing, with every archive in the
//! downloads folder.

use assert_cmd::Command;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;
use zip::write::FileOptions;

fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options: FileOptions<'_, ()> = FileOptions::default();
    for (name, bytes) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

/// A plugin that is only a TES4 header naming `masters`.
fn plugin(masters: &[&str]) -> Vec<u8> {
    let mut fields = Vec::new();
    let mut field = |kind: &[u8; 4], data: &[u8]| {
        fields.extend_from_slice(kind);
        fields.extend_from_slice(&(data.len() as u16).to_le_bytes());
        fields.extend_from_slice(data);
    };
    let mut hedr = Vec::new();
    hedr.extend_from_slice(&1.0f32.to_le_bytes());
    hedr.extend_from_slice(&0u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    field(b"HEDR", &hedr);
    for master in masters {
        let mut name = master.as_bytes().to_vec();
        name.push(0);
        field(b"MAST", &name);
        field(b"DATA", &0u64.to_le_bytes());
    }
    let mut out = b"TES4".to_vec();
    out.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(&fields);
    out
}

fn mudcrab(root: &Path) -> Command {
    let mut command = Command::cargo_bin("mudcrab").unwrap();
    command
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("LOCALAPPDATA", root.join("state"));
    command
}

#[test]
fn check_reads_inside_archives_and_checks_masters_against_the_load_order() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    make_zip(
        &downloads.join("Base-1-1-0.zip"),
        &[("Base.esm", &plugin(&["Oblivion.esm"]))],
    );
    make_zip(
        &downloads.join("Patch-2-1-0.zip"),
        &[(
            "Patch.esp",
            &plugin(&["Oblivion.esm", "Base.esm", "Elsewhere.esm"]),
        )],
    );
    let list = root.join("list.toml");
    std::fs::write(
        &list,
        r#"name = "L"
plugins = ["Patch.esp", "Base.esm"]

[[mods]]
id = "Base"
plugins = ["Base.esm"]
[[mods.archives]]
path = "nexus:oblivion/1/10"
file_name = "Base-1-1-0.zip"

[[mods]]
id = "Patch"
plugins = ["Patch.esp"]
[[mods.archives]]
path = "nexus:oblivion/2/20"
file_name = "Patch-2-1-0.zip"

[[mods]]
id = "Absent"
[[mods.archives]]
path = "nexus:oblivion/3/30"
file_name = "Absent-3-1-0.zip"
"#,
    )
    .unwrap();
    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    mudcrab(root)
        .args(["compile", list.to_str().unwrap(), "--output"])
        .arg(&compiled)
        .assert()
        .success();
    mudcrab(root)
        .args([
            "query",
            compiled.to_str().unwrap(),
            "--headless",
            "--output",
        ])
        .arg(&plan)
        .assert()
        .success();

    let output = mudcrab(root)
        .args(["check", plan.to_str().unwrap(), "--downloads"])
        .arg(&downloads)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        said.contains("Absent-3-1-0.zip is not in the downloads folder"),
        "{said}"
    );
    assert!(
        said.contains("Patch (Patch.esp): needs Base.esm, which loads after it"),
        "{said}"
    );
    assert!(
        said.contains("Patch (Patch.esp): needs Elsewhere.esm, which is not in the list"),
        "{said}"
    );
    assert!(
        !said.contains("Oblivion.esm"),
        "a game plugin is present: {said}"
    );
    assert!(said.contains("3 problem(s)"), "{said}");
}

/// Compile, query and check `list` against `downloads`; (success, stdout,
/// stdout and stderr together).
fn check_list(root: &Path, list: &str, downloads: &Path) -> (bool, String, String) {
    let path = root.join("list.toml");
    std::fs::write(&path, list).unwrap();
    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    mudcrab(root)
        .args(["compile", path.to_str().unwrap(), "--output"])
        .arg(&compiled)
        .assert()
        .success();
    mudcrab(root)
        .args([
            "query",
            compiled.to_str().unwrap(),
            "--headless",
            "--output",
        ])
        .arg(&plan)
        .assert()
        .success();
    let output = mudcrab(root)
        .args(["check", plan.to_str().unwrap(), "--downloads"])
        .arg(downloads)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    let all = format!("{out}{}", String::from_utf8_lossy(&output.stderr));
    (output.status.success(), out, all)
}

#[test]
fn check_compares_the_load_order_bsas_and_actions_with_what_the_mods_install() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    let plain = plugin(&["Oblivion.esm"]);
    // Ships A.esp, an unordered Extra.esp, a BSA no plugin loads, and a
    // texture for a prune to remove.
    make_zip(
        &downloads.join("A-1-1-0.zip"),
        &[
            ("A.esp", &plain),
            ("Extra.esp", &plain),
            ("Orphan.bsa", b"BSA"),
            ("A.bsa", b"BSA"),
            ("textures/a.dds", b"dds"),
        ],
    );
    // Ships Shared.esp, as does C.
    make_zip(&downloads.join("B-2-1-0.zip"), &[("Shared.esp", &plain)]);
    make_zip(&downloads.join("C-3-1-0.zip"), &[("Shared.esp", &plain)]);
    // Recorded with an MD5 it does not have.
    make_zip(&downloads.join("D.zip"), &[("readme.txt", b"d")]);
    let list = r#"name = "L"
plugins = ["A.esp", "Renamed.esp", "Shared.esp", "Ghost.esp"]

[[mods]]
id = "A"
plugins = ["A.esp", "Renamed.esp"]
[[mods.archives]]
path = "nexus:oblivion/1/10"
file_name = "A-1-1-0.zip"
exclude = ["nothing-here/*"]
[[mods.actions]]
action = "file_prune"
paths = ["textures/missing.dds"]

[[mods]]
id = "B"
plugins = ["Shared.esp"]
[[mods.archives]]
path = "nexus:oblivion/2/20"
file_name = "B-2-1-0.zip"

[[mods]]
id = "C"
[[mods.archives]]
path = "nexus:oblivion/3/30"
file_name = "C-3-1-0.zip"

[[mods]]
id = "D"
[[mods.archives]]
path = "manual:D.zip"
size = 1
md5 = "00000000000000000000000000000000"
"#;
    let (ok, out, err) = check_list(root, list, &downloads);
    assert!(!ok);
    // Errors.
    assert!(
        err.contains("Would fail at install") && err.contains("textures/missing.dds"),
        "{err}"
    );
    assert!(
        err.contains("Ghost.esp is in the load order, but no mod installs it"),
        "{err}"
    );
    assert!(err.contains("is not the file the list records"), "{err}");
    // A's rehearsal failed, so its folder is not concluded from.
    assert!(!err.contains("Renamed.esp"), "{err}");

    // Without the bad prune, A's folder is known.
    let list = list.replace(
        "[[mods.actions]]\naction = \"file_prune\"\npaths = [\"textures/missing.dds\"]\n",
        "",
    );
    let (ok, out2, err) = check_list(root, &list, &downloads);
    assert!(!ok);
    assert!(
        err.contains("A: declares Renamed.esp, which it does not install"),
        "{err}"
    );
    assert!(
        err.contains("A: orphan.bsa -- no active plugin's name begins it"),
        "{err}"
    );
    assert!(!err.contains("a.bsa,"), "A.esp loads A.bsa: {err}");
    // Warnings, which do not fail the check by themselves.
    assert!(out2.contains("extra.esp (A)"), "{out2}");
    assert!(out2.contains("Shared.esp: C > B"), "{out2}");
    assert!(
        out2.contains("A: exclude 'nothing-here/*' leaves nothing out"),
        "{out2}"
    );
    let _ = out;
}

#[test]
fn a_plugin_from_a_mod_that_moves_it_is_traced_through_the_move() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    make_zip(
        &downloads.join("M-1-1-0.zip"),
        &[(
            "optional/Moved.esp",
            &plugin(&["Oblivion.esm", "Nowhere.esm"]),
        )],
    );
    let list = r#"name = "L"
plugins = ["Moved.esp"]

[[mods]]
id = "M"
plugins = ["Moved.esp"]
[[mods.archives]]
path = "nexus:oblivion/1/10"
file_name = "M-1-1-0.zip"
layout = "simple"
[[mods.actions]]
action = "file_move"
from = "optional/Moved.esp"
to = "Moved.esp"
"#;
    let (ok, _, err) = check_list(root, list, &downloads);
    assert!(!ok);
    assert!(
        err.contains("M (Moved.esp): needs Nowhere.esm, which is not in the list"),
        "the moved plugin's masters come from its archive: {err}"
    );
    assert!(!err.contains("declares Moved.esp"), "{err}");
}

#[test]
fn a_declared_conflict_that_overlaps_nothing_fails_as_install_would() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    make_zip(&downloads.join("X-1-1-0.zip"), &[("textures/x.dds", b"x")]);
    make_zip(&downloads.join("Y-2-1-0.zip"), &[("textures/y.dds", b"y")]);
    let list = r#"name = "L"

[[mods]]
id = "X"
[[mods.archives]]
path = "nexus:oblivion/1/10"
file_name = "X-1-1-0.zip"
[[mods.actions]]
action = "file_hide"
conflicts_with = ["Y"]

[[mods]]
id = "Y"
[[mods.archives]]
path = "nexus:oblivion/2/20"
file_name = "Y-2-1-0.zip"
"#;
    let (ok, _, err) = check_list(root, list, &downloads);
    assert!(!ok);
    assert!(
        err.contains("Would fail at install") && err.contains("selected no files"),
        "{err}"
    );
}

/// A plan from before mods' declared plugins were carried into it cannot say
/// who installs a plugin `check` cannot read; it says so rather than report
/// every such plugin as installed by nothing.
#[test]
fn an_older_plan_skips_the_load_order_comparison_and_says_why() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    // Its content is inside a second archive, so its plugin cannot be read.
    make_zip(&downloads.join("Wrapped-1-1-0.zip"), &[("inner.7z", b"7z")]);
    let list = root.join("list.toml");
    std::fs::write(
        &list,
        r#"name = "L"
plugins = ["Wrapped.esp"]

[[mods]]
id = "Wrapped"
plugins = ["Wrapped.esp"]
[[mods.archives]]
path = "nexus:oblivion/1/10"
file_name = "Wrapped-1-1-0.zip"
inner_archive = "inner.7z"
"#,
    )
    .unwrap();
    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    mudcrab(root)
        .args(["compile", list.to_str().unwrap(), "--output"])
        .arg(&compiled)
        .assert()
        .success();
    mudcrab(root)
        .args([
            "query",
            compiled.to_str().unwrap(),
            "--headless",
            "--output",
        ])
        .arg(&plan)
        .assert()
        .success();
    // As an older mudcrab wrote it: no `plugins` on the mods.
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&plan).unwrap()).unwrap();
    for entry in json["mods"].as_array_mut().unwrap() {
        entry.as_object_mut().unwrap().remove("plugins");
    }
    std::fs::write(&plan, json.to_string()).unwrap();

    let output = mudcrab(root)
        .args(["check", plan.to_str().unwrap(), "--downloads"])
        .arg(&downloads)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{said}");
    assert!(said.contains("made by an older mudcrab"), "{said}");
    assert!(!said.contains("no mod installs it"), "{said}");
}
