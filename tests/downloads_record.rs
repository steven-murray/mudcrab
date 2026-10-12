use assert_cmd::Command;

#[test]
fn records_a_hand_downloads_size_and_md5_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let downloads = dir.path().join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    std::fs::write(downloads.join("Hand.7z"), b"payload").unwrap();
    let list = dir.path().join("list.toml");
    let text = "name = \"R\"\n\n[[mods]]\nid = \"Hand\"\n\n[[mods.archives]]\n# from a forum post\npath = \"manual:Hand.7z\"\nlayout = \"simple\"\n\n[[mods]]\nid = \"Gone\"\n\n[[mods.archives]]\npath = \"manual:Gone.7z\"\n";
    std::fs::write(&list, text).unwrap();

    let output = Command::cargo_bin("mudcrab")
        .unwrap()
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .args(["downloads", "record"])
        .arg(&list)
        .arg("--downloads")
        .arg(&downloads)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("not on disk, so not recorded: Gone"),
        "{stderr}"
    );

    let md5 = format!("{:x}", md5::compute(b"payload"));
    assert_eq!(
        std::fs::read_to_string(&list).unwrap(),
        text.replace(
            "path = \"manual:Hand.7z\"\n",
            &format!("path = \"manual:Hand.7z\"\nsize = 7\nmd5 = \"{md5}\"\n")
        ),
        "two lines added after the path, nothing else touched"
    );
}

#[test]
fn records_the_file_name_of_a_nexus_archive_found_by_its_meta() {
    let dir = tempfile::tempdir().unwrap();
    let downloads = dir.path().join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    std::fs::write(downloads.join("Mod-12-1-0.7z"), b"x").unwrap();
    std::fs::write(
        downloads.join("Mod-12-1-0.7z.meta"),
        "[General]\ngameName=Oblivion\nmodID=12\nfileID=34\n",
    )
    .unwrap();
    let list = dir.path().join("list.toml");
    let text = "name = \"R\"\n\n[[mods]]\nid = \"M\"\n\n[[mods.archives]]\npath = \"nexus:oblivion/12/34\"\n";
    std::fs::write(&list, text).unwrap();

    Command::cargo_bin("mudcrab")
        .unwrap()
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .args(["downloads", "record"])
        .arg(&list)
        .arg("--downloads")
        .arg(&downloads)
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(&list).unwrap(),
        text.replace(
            "path = \"nexus:oblivion/12/34\"\n",
            "path = \"nexus:oblivion/12/34\"\nfile_name = \"Mod-12-1-0.7z\"\n"
        )
    );
}
