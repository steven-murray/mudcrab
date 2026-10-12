use assert_cmd::Command;
use std::path::Path;

fn mudcrab() -> Command {
    let mut command = Command::cargo_bin("mudcrab").expect("binary should build");
    command.env(
        "XDG_STATE_HOME",
        std::env::temp_dir().join("mudcrab-test-state"),
    );
    command
}

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

const LIST: &str = r#"
name = "Sweep"

[[mods]]
id = "Base"
section = ["Core"]
[[mods.archives]]
path = "nexus:oblivion/1/1"

[[mods]]
id = "Retexture"
section = ["Looks"]
[[mods.archives]]
path = "nexus:oblivion/2/2"

[[mods]]
id = "Patch"
section = ["Looks"]
[[mods.archives]]
path = "nexus:oblivion/3/3"

[[mods]]
id = "Alone"
section = ["Looks"]
[[mods.archives]]
path = "nexus:oblivion/4/4"
"#;

#[test]
fn a_sweep_names_every_mod_that_shares_files_and_who_wins() {
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("list.toml");
    write(&list, LIST);
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    mudcrab()
        .arg("compile")
        .arg(&list)
        .arg("--offline")
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();
    mudcrab()
        .arg("query")
        .arg(&compiled)
        .arg("--headless")
        .arg("--output")
        .arg(&plan)
        .assert()
        .success();

    let mods = dir.path().join("mods");
    for (id, files) in [
        (
            "Base",
            &["textures/a.dds", "textures/b.dds", "meshes/c.nif"][..],
        ),
        ("Retexture", &["Textures/A.dds", "textures/b.dds"][..]),
        ("Patch", &["textures/b.dds"][..]),
        ("Alone", &["textures/z.dds"][..]),
    ] {
        // MO2's meta.ini is in every mod and conflicts with nothing.
        write(&mods.join(id).join("meta.ini"), "[General]\n");
        for file in files {
            write(&mods.join(id).join(file), "x");
        }
    }

    let output = mudcrab()
        .arg("conflicts")
        .arg("--plan")
        .arg(&plan)
        .arg("--mods-dir")
        .arg(&mods)
        .output()
        .unwrap();
    assert!(output.status.success());
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(
        report.contains("Base  [Core]\n  loses 2 files to: Retexture (1), Patch (1)\n"),
        "{report}"
    );
    assert!(
        // `Textures/A.dds` and `textures/a.dds` are one file to the game.
        report.contains(
            "Retexture  [Looks]\n  wins 1 file over: Base (1)\n  loses 1 file to: Patch (1)\n"
        ),
        "{report}"
    );
    assert!(
        report.contains("Patch  [Looks]\n  wins 2 files over: Base (1), Retexture (1)\n"),
        "{report}"
    );
    assert!(!report.contains("Alone"), "{report}");
    let summary = String::from_utf8(output.stderr).unwrap();
    assert!(
        summary.contains("3 of 4 installed mods share files"),
        "{summary}"
    );

    // --only narrows the report, not the comparison.
    let output = mudcrab()
        .arg("conflicts")
        .arg("--plan")
        .arg(&plan)
        .arg("--mods-dir")
        .arg(&mods)
        .args(["--only", "Patch"])
        .output()
        .unwrap();
    let report = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        report,
        "\nPatch  [Looks]\n  wins 2 files over: Base (1), Retexture (1)\n"
    );
}
