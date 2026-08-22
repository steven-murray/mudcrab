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
    zip.start_file("Data/Core.esm", options)
        .expect("zip plugin entry should be created");
    zip.write_all(b"TES4")
        .expect("zip plugin payload should be written");
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

    let plugins = std::fs::read_to_string(profile_dir.join("plugins.txt"))
        .expect("plugins.txt should exist");
    assert_eq!(plugins, "Core.esm\n");

    // Oblivion's plugins.txt says what is active, not what order it loads in.
    // loadorder.txt is where MO2 keeps the order, and the plugin's own mtime is
    // where the game keeps it -- so the export has to write both.
    let loadorder = std::fs::read_to_string(profile_dir.join("loadorder.txt"))
        .expect("loadorder.txt should exist");
    assert_eq!(loadorder, plugins);

    let stamped = std::fs::metadata(instance_dir.join("mods").join("core").join("Core.esm"))
        .expect("the plugin should be staged")
        .modified()
        .expect("mtime")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after the epoch")
        .as_secs();
    assert_eq!(stamped, 946_684_800, "first in the load order gets the base stamp");

    let archives = std::fs::read_to_string(profile_dir.join("archives.txt"))
        .expect("archives.txt should exist");
    assert_eq!(archives, "CoreAssets.bsa\n");

    let profile_ini = std::fs::read_to_string(profile_dir.join("oblivion.ini"))
        .expect("profile oblivion.ini should exist");
    assert!(profile_ini.contains("bFull Screen = 0"));

    let game_ini = std::fs::read_to_string(game_dir.join("Oblivion.ini"))
        .expect("game oblivion.ini should still exist");
    assert!(game_ini.contains("bFull Screen = 1"));

    // A second run must not throw the first run's edits away.
    //
    // This used to copy the game's Oblivion.ini over the profile's on every
    // install, so a section-by-section build ended up with only the LAST
    // section's settings. Six of eighteen were wrong on disk when it was
    // found, including the DarNified font paths -- a broken UI.
    //
    // Simulated with a comment line no action would reproduce: if the file is
    // reseeded, the marker is gone. A comment rather than an assignment so it
    // cannot perturb the spacing heuristic and change what the assertion below
    // is really testing.
    let profile_ini_path = profile_dir.join("oblivion.ini");
    let edited = format!(
        "{}\n; sEarlierSectionMarker=kept\n",
        std::fs::read_to_string(&profile_ini_path).expect("read profile ini")
    );
    std::fs::write(&profile_ini_path, &edited).expect("write profile ini");

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

    let after = std::fs::read_to_string(&profile_ini_path).expect("read profile ini again");
    assert!(
        after.contains("; sEarlierSectionMarker=kept"),
        "a second install reseeded the profile INI and discarded earlier edits:\n{after}"
    );
    assert!(
        after.contains("bFull Screen = 0"),
        "this run's own ini_set should still be applied:\n{after}"
    );
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