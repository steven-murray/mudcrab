//! An MO2 instance whose ModOrganizer.ini sits with MO2 itself while its
//! mods and profiles are elsewhere, as `base_directory` puts them -- usual
//! on Windows. install reads where things are from the ini.

use assert_cmd::Command;
use std::io::Write;
use tempfile::tempdir;
use zip::write::FileOptions;

fn mudcrab() -> Command {
    let mut command = Command::cargo_bin("mudcrab").unwrap();
    let state = std::env::temp_dir().join("mudcrab-test-state");
    command
        .env("XDG_STATE_HOME", &state)
        .env("LOCALAPPDATA", &state);
    command
}

#[test]
fn install_puts_mods_and_the_profile_where_the_ini_says() {
    let dir = tempdir().unwrap();
    let d = dir.path();
    std::fs::create_dir_all(d.join("game")).unwrap();
    std::fs::write(d.join("game/Oblivion.ini"), "[General]\n").unwrap();
    std::fs::create_dir_all(d.join("downloads")).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(d.join("downloads/a.zip")).unwrap());
    zip.start_file("Data/a.txt", FileOptions::<'_, ()>::default())
        .unwrap();
    zip.write_all(b"a").unwrap();
    zip.finish().unwrap();
    let list = d.join("list.toml");
    std::fs::write(
        &list,
        "name = \"L\"\n\n[[mods]]\nid = \"a\"\narchives = [{ path = \"manual:a.zip\" }]\n",
    )
    .unwrap();

    // MO2 in one folder, its data in another.
    let program = d.join("Mod Organizer 2");
    let data = d.join("modding/sampler");
    std::fs::create_dir_all(&program).unwrap();
    std::fs::create_dir_all(data.join("mods")).unwrap();
    std::fs::create_dir_all(data.join("profiles/Default")).unwrap();
    let base = data.display().to_string().replace('\\', "/");
    let base = if cfg!(windows) {
        base
    } else {
        format!("Z:{base}")
    };
    std::fs::write(
        program.join("ModOrganizer.ini"),
        format!("[General]\n\n[Settings]\nbase_directory={base}\n"),
    )
    .unwrap();

    let compiled = d.join("compiled.json");
    let plan = d.join("plan.json");
    mudcrab()
        .arg("compile")
        .arg(&list)
        .arg("-o")
        .arg(&compiled)
        .assert()
        .success();
    mudcrab()
        .arg("query")
        .arg(&compiled)
        .args(["--headless", "-o"])
        .arg(&plan)
        .assert()
        .success();

    // Given the data folder, install says which folder to give instead.
    let refused = mudcrab()
        .arg("install")
        .arg(&plan)
        .arg("--downloads")
        .arg(d.join("downloads"))
        .arg("--mo2-instance-dir")
        .arg(&data)
        .arg("--game-dir")
        .arg(d.join("game"))
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("not its ModOrganizer.ini"));

    mudcrab()
        .arg("install")
        .arg(&plan)
        .arg("--downloads")
        .arg(d.join("downloads"))
        .arg("--mo2-instance-dir")
        .arg(&program)
        .arg("--game-dir")
        .arg(d.join("game"))
        .assert()
        .success();
    assert!(data.join("mods/a/a.txt").is_file());
    assert!(data.join("profiles/Default/modlist.txt").is_file());
    assert!(!program.join("mods").exists());
    assert!(!program.join("profiles").exists());
    // MO2's own settings stay in its ini.
    let ini = std::fs::read_to_string(program.join("ModOrganizer.ini")).unwrap();
    assert!(ini.contains("download_directory="), "{ini}");
    assert!(!data.join("ModOrganizer.ini").exists());
}
