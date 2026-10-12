use assert_cmd::Command;

#[test]
fn writes_what_it_finds_where_xdg_config_home_says() {
    let home = tempfile::tempdir().unwrap();
    let instance = home.path().join("Games/My List");
    std::fs::create_dir_all(instance.join("downloads")).unwrap();
    std::fs::write(instance.join("ModOrganizer.ini"), "[General]\n").unwrap();
    let xedit = home.path().join("Games/tools/xEdit/TES4Edit.exe");
    std::fs::create_dir_all(xedit.parent().unwrap()).unwrap();
    std::fs::write(&xedit, b"MZ").unwrap();
    let list = home.path().join("list.toml");
    std::fs::write(
        &list,
        "name = \"L\"\n\n[[mods]]\nid = \"M\"\n\n[[mods.archives]]\npath = \"manual:m.7z\"\n\n[[mods.actions]]\naction = \"qac\"\nplugins = [\"m.esp\"]\n",
    )
    .unwrap();
    let config = home.path().join("config");

    let output = Command::cargo_bin("mudcrab")
        .unwrap()
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", &config)
        .env("XDG_STATE_HOME", home.path().join("state"))
        .arg("setup-tools")
        .arg(&list)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let said = String::from_utf8(output.stdout).unwrap();
    assert!(said.contains("XDG_CONFIG_HOME is"), "{said}");

    let written = std::fs::read_to_string(config.join("mudcrab/tools.toml")).unwrap();
    assert!(
        written.contains(&format!(
            "downloads_dir = \"{}\"",
            instance.join("downloads").display()
        )),
        "{written}"
    );
    assert!(
        written.contains(&format!("exe = \"{}\"", xedit.display())),
        "{written}"
    );
}
