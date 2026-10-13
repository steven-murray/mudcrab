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
        // Where Windows keeps it, and its logs.
        .env("APPDATA", &config)
        .env("LOCALAPPDATA", home.path().join("state"))
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
    if !cfg!(windows) {
        assert!(said.contains("XDG_CONFIG_HOME is"), "{said}");
    }

    let path = config.join("mudcrab").join("tools.toml");
    let written = mudcrab::config::tools::ToolsConfig::load(&path).unwrap();
    assert_eq!(written.downloads_dir, Some(instance.join("downloads")));
    assert_eq!(written.tes4edit.map(|t| t.exe), Some(xedit));
}

/// A second tools.toml, for a test, without touching XDG_CONFIG_HOME (which
/// moves every program's settings, browsers included).
#[test]
fn mudcrab_tools_config_and_the_flag_name_another_tools_toml() {
    let home = tempfile::tempdir().unwrap();
    let list = home.path().join("list.toml");
    std::fs::write(
        &list,
        "name = \"L\"\n\n[[mods]]\nid = \"M\"\n\n[[mods.archives]]\npath = \"manual:m.7z\"\n",
    )
    .unwrap();
    let config = home.path().join("config");
    let other = home.path().join("test-tools.toml");
    let run = |args: &[&str], env: Option<&std::path::Path>| {
        let mut command = Command::cargo_bin("mudcrab").unwrap();
        command
            .env("HOME", home.path())
            .env("XDG_CONFIG_HOME", &config)
            .env("APPDATA", &config)
            .env("LOCALAPPDATA", home.path().join("state"))
            .env("XDG_STATE_HOME", home.path().join("state"))
            .env_remove("MUDCRAB_TOOLS_CONFIG")
            .args(args);
        if let Some(path) = env {
            command.env("MUDCRAB_TOOLS_CONFIG", path);
        }
        command.output().unwrap()
    };

    let output = run(&["setup-tools", list.to_str().unwrap()], Some(&other));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(other.exists());
    assert!(!config.join("mudcrab/tools.toml").exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains("MUDCRAB_TOOLS_CONFIG"));

    // Every command reads it: here, downloads_dir.
    let downloads = home.path().join("from-test-tools");
    std::fs::write(
        &other,
        format!("downloads_dir = '{}'\n", downloads.display()),
    )
    .unwrap();
    let compiled = home.path().join("compiled.json");
    let plan = home.path().join("plan.json");
    for args in [
        vec![
            "compile",
            list.to_str().unwrap(),
            "--output",
            compiled.to_str().unwrap(),
        ],
        vec![
            "query",
            compiled.to_str().unwrap(),
            "--headless",
            "--output",
            plan.to_str().unwrap(),
        ],
    ] {
        assert!(run(&args, None).status.success());
    }
    for (args, env) in [
        (vec!["check", plan.to_str().unwrap()], Some(other.as_path())),
        (
            vec![
                "check",
                plan.to_str().unwrap(),
                "--tools-config",
                other.to_str().unwrap(),
            ],
            None,
        ),
    ] {
        let output = run(&args, env);
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            said.contains(&format!("using {}", other.display())),
            "{said}"
        );
        assert!(said.contains("from-test-tools"), "{said}");
    }
}
