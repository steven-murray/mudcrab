use assert_cmd::Command;
use std::io::Write;
use tempfile::tempdir;
use zip::write::FileOptions;

#[test]
fn install_exports_modorganizer2_instance_structure() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("core.zip");
    let game_dir = dir.path().join("game");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let instance_dir = dir.path().join("mo2-instance");

    std::fs::create_dir_all(&game_dir).expect("game dir should be created");
    std::fs::write(game_dir.join("Oblivion.ini"), "bFull Screen = 1\n")
        .expect("game ini should be written");

    let zip_file = std::fs::File::create(&archive).expect("zip fixture should be created");
    let mut zip = zip::ZipWriter::new(zip_file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    zip.start_file("Data/test.txt", options)
        .expect("zip file entry should be created");
    zip.write_all(b"hello from archive")
        .expect("zip payload should be written");
    zip.start_file("Data/CoreAssets.bsa", options)
        .expect("zip bsa entry should be created");
    zip.write_all(b"fake-bsa")
        .expect("zip bsa payload should be written");
    zip.finish().expect("zip should finalize");

    let source = format!(
        "name = \"Install MO2 Test\"\nplugins = [\"Core.esm\"]\n\n[[mods]]\nid = \"core\"\ndependencies = []\nplugins = [\"Core.esm\"]\n\n[[mods.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n\n[[mods.actions]]\naction = \"ini_set\"\nscope = \"game\"\nfile = \"OBLIVION.INI\"\nkey = \"bFull Screen\"\nvalue = \"0\"\n",
        archive.display()
    );
    std::fs::write(&modlist, source).expect("fixture modlist should be written");

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("compile")
        .arg(&modlist)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&plan)
        .arg("--headless")
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("download")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("install")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .arg("--mo2-instance-dir")
        .arg(&instance_dir)
        .arg("--profile-name")
        .arg("test-profile")
        .arg("--game-dir")
        .arg(&game_dir)
        .assert()
        .success();

    let extracted = instance_dir.join("mods").join("core").join("test.txt");
    assert!(extracted.exists());

    let downloads = std::fs::read_dir(instance_dir.join("downloads"))
        .expect("downloads dir should exist")
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    assert_eq!(downloads.len(), 1);

    let profile_dir = instance_dir.join("profiles").join("test-profile");
    let modlist_txt = std::fs::read_to_string(profile_dir.join("modlist.txt"))
        .expect("modlist.txt should exist");
    assert_eq!(modlist_txt, "+core\n");

    assert!(!profile_dir.join("loadorder.txt").exists());

    let plugins = std::fs::read_to_string(profile_dir.join("plugins.txt"))
        .expect("plugins.txt should exist");
    assert_eq!(plugins, "Core.esm\n");

    let archives = std::fs::read_to_string(profile_dir.join("archives.txt"))
        .expect("archives.txt should exist");
    assert_eq!(archives, "CoreAssets.bsa\n");

    let profile_ini = std::fs::read_to_string(profile_dir.join("oblivion.ini"))
        .expect("profile oblivion.ini should exist");
    assert!(profile_ini.contains("bFull Screen = 0"));

    let game_ini = std::fs::read_to_string(game_dir.join("Oblivion.ini"))
        .expect("game oblivion.ini should still exist");
    assert!(game_ini.contains("bFull Screen = 1"));
}

#[test]
fn install_applies_top_level_ini_section_to_mo2_profile() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("core.zip");
    let game_dir = dir.path().join("game");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let instance_dir = dir.path().join("mo2-instance");

    std::fs::create_dir_all(&game_dir).expect("game dir should be created");
    std::fs::write(game_dir.join("Oblivion.ini"), "[Display]\nbFull Screen = 1\n")
        .expect("game ini should be written");

    let zip_file = std::fs::File::create(&archive).expect("zip fixture should be created");
    let mut zip = zip::ZipWriter::new(zip_file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    zip.start_file("Data/test.txt", options)
        .expect("zip file entry should be created");
    zip.write_all(b"hello from archive")
        .expect("zip payload should be written");
    zip.finish().expect("zip should finalize");

    let source = format!(
        "name = \"Install MO2 Top Level Ini Test\"\nplugins = [\"Core.esm\"]\n\n[ini]\n\"bFull Screen\" = 0\n\"iSize W\" = 1920\n\n[[mods]]\nid = \"core\"\ndependencies = []\nplugins = [\"Core.esm\"]\n\n[[mods.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n",
        archive.display()
    );
    std::fs::write(&modlist, source).expect("fixture modlist should be written");

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("compile")
        .arg(&modlist)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&plan)
        .arg("--headless")
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("download")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("install")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .arg("--mo2-instance-dir")
        .arg(&instance_dir)
        .arg("--profile-name")
        .arg("test-profile")
        .arg("--game-dir")
        .arg(&game_dir)
        .assert()
        .success();

    let profile_ini = std::fs::read_to_string(instance_dir.join("profiles").join("test-profile").join("oblivion.ini"))
        .expect("profile oblivion.ini should exist");
    assert!(profile_ini.contains("bFull Screen = 0"));
    assert!(profile_ini.contains("iSize W = 1920"));

    let game_ini = std::fs::read_to_string(game_dir.join("Oblivion.ini"))
        .expect("game oblivion.ini should still exist");
    assert!(game_ini.contains("bFull Screen = 1"));
    assert!(!game_ini.contains("iSize W = 1920"));
}

#[test]
fn install_exports_modlist_txt_in_reverse_order() {
    let dir = tempdir().expect("temp dir should be created");
    let archive_a = dir.path().join("a.zip");
    let archive_b = dir.path().join("b.zip");
    let game_dir = dir.path().join("game");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let instance_dir = dir.path().join("mo2-instance");

    std::fs::create_dir_all(&game_dir).expect("game dir should be created");
    std::fs::write(game_dir.join("Oblivion.ini"), "bFull Screen = 1\n")
        .expect("game ini should be written");

    for archive in [&archive_a, &archive_b] {
        let zip_file = std::fs::File::create(archive).expect("zip fixture should be created");
        let mut zip = zip::ZipWriter::new(zip_file);
        let options: FileOptions<'_, ()> = FileOptions::default();
        zip.start_file("Data/test.txt", options)
            .expect("zip file entry should be created");
        zip.write_all(b"hello from archive")
            .expect("zip payload should be written");
        zip.finish().expect("zip should finalize");
    }

    let source = format!(
        "name = \"Install MO2 Reverse Order Test\"\n\n[[mods]]\nid = \"alpha\"\ndependencies = []\n\n[[mods.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n\n[[mods]]\nid = \"beta\"\ndependencies = [\"alpha\"]\n\n[[mods.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n",
        archive_a.display(),
        archive_b.display()
    );
    std::fs::write(&modlist, source).expect("fixture modlist should be written");

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("compile")
        .arg(&modlist)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&plan)
        .arg("--headless")
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("download")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("install")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .arg("--mo2-instance-dir")
        .arg(&instance_dir)
        .arg("--profile-name")
        .arg("test-profile")
        .arg("--game-dir")
        .arg(&game_dir)
        .assert()
        .success();

    let profile_dir = instance_dir.join("profiles").join("test-profile");
    let modlist_txt = std::fs::read_to_string(profile_dir.join("modlist.txt"))
        .expect("modlist.txt should exist");
    assert_eq!(modlist_txt, "+beta\n+alpha\n");
}

#[test]
fn install_exports_mo2_sections_and_flattens_nested_section_names() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("core.zip");
    let game_dir = dir.path().join("game");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let instance_dir = dir.path().join("mo2-instance");

    std::fs::create_dir_all(&game_dir).expect("game dir should be created");
    std::fs::write(game_dir.join("Oblivion.ini"), "bFull Screen = 1\n")
        .expect("game ini should be written");

    let zip_file = std::fs::File::create(&archive).expect("zip fixture should be created");
    let mut zip = zip::ZipWriter::new(zip_file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    zip.start_file("Data/test.txt", options)
        .expect("zip file entry should be created");
    zip.write_all(b"hello from archive")
        .expect("zip payload should be written");
    zip.finish().expect("zip should finalize");

    let source = format!(
        "name = \"Install MO2 Sections Test\"\n\n[[mods]]\nid = \"alpha\"\nsection = [\"OBSE PLUGINS\", \"Core\"]\ndependencies = []\n\n[[mods.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n",
        archive.display()
    );
    std::fs::write(&modlist, source).expect("fixture modlist should be written");

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("compile")
        .arg(&modlist)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&plan)
        .arg("--headless")
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("download")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("install")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .arg("--mo2-instance-dir")
        .arg(&instance_dir)
        .arg("--profile-name")
        .arg("test-profile")
        .arg("--game-dir")
        .arg(&game_dir)
        .assert()
        .success();

    let profile_dir = instance_dir.join("profiles").join("test-profile");
    let modlist_txt = std::fs::read_to_string(profile_dir.join("modlist.txt"))
        .expect("modlist.txt should exist");
    assert_eq!(
        modlist_txt,
        "+alpha\n-OBSE PLUGINS - Core_separator\n-OBSE PLUGINS_separator\n"
    );

    assert!(
        instance_dir
            .join("mods")
            .join("OBSE PLUGINS_separator")
            .exists()
    );
    assert!(
        instance_dir
            .join("mods")
            .join("OBSE PLUGINS - Core_separator")
            .exists()
    );
}