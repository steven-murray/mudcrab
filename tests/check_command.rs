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
    let said = String::from_utf8_lossy(&output.stderr);
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
    assert!(said.contains("3 issue(s)"), "{said}");
}
