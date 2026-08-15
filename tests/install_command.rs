use assert_cmd::Command;
use std::io::Write;
use tempfile::tempdir;
use zip::write::FileOptions;

#[test]
fn install_unpacks_zip_archives_for_each_selected_mod() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("core.zip");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let mods_dir = dir.path().join("mods");
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    let zip_file = std::fs::File::create(&archive).expect("zip fixture should be created");
    let mut zip = zip::ZipWriter::new(zip_file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    zip.start_file("Data/test.txt", options)
        .expect("zip file entry should be created");
    zip.write_all(b"hello from archive")
        .expect("zip payload should be written");
    zip.finish().expect("zip should finalize");

    let source = format!(
        "name = \"Install Test\"\n\n[modlist.core]\ndependencies = []\n\n[[modlist.core.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n",
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
        .env("GAME_DIR", &game_dir)
        .arg("install")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .arg("--mods-dir")
        .arg(&mods_dir)
        .arg("--game-dir")
        .arg(&game_dir)
        .assert()
        .success();

    let extracted = mods_dir.join("core").join("test.txt");
    assert!(extracted.exists());
    let payload = std::fs::read_to_string(&extracted).expect("extracted file should be readable");
    assert_eq!(payload, "hello from archive");
    assert!(mods_dir.join("install_manifest.json").exists());
}

#[test]
fn install_extracts_game_root_files_to_game_root_dir_and_excludes_from_mod_folder() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("core.zip");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let mods_dir = dir.path().join("mods");
    let game_root_dir = dir.path().join("game-root");
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    // Archive contains two files:
    //   game_loader.exe  -> should land in game-root/, not in the mod folder
    //   Data/mod.txt     -> should land in the mod folder only
    let zip_file = std::fs::File::create(&archive).expect("zip fixture should be created");
    let mut zip = zip::ZipWriter::new(zip_file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
    zip.start_file("game_loader.exe", options)
        .expect("exe entry should be created");
    zip.write_all(b"fake-loader-exe")
        .expect("exe payload should be written");
    zip.start_file("Data/mod.txt", options)
        .expect("data entry should be created");
    zip.write_all(b"mod data")
        .expect("data payload should be written");
    zip.finish().expect("zip should finalize");

    let source = format!(
        "name = \"Game Root Test\"\n\n[modlist.core]\ndependencies = []\n\n[[modlist.core.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\ngame_root_files = [\"*.exe\"]\n",
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
        .env("GAME_DIR", &game_dir)
        .arg("install")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .arg("--mods-dir")
        .arg(&mods_dir)
        .arg("--game-dir")
        .arg(&game_dir)
        .arg("--game-root-dir")
        .arg(&game_root_dir)
        .assert()
        .success();

    // Exe landed in game-root
    let exe_in_root = game_root_dir.join("game_loader.exe");
    assert!(exe_in_root.exists(), "game_loader.exe should be in game-root dir");
    let exe_payload = std::fs::read(&exe_in_root).expect("game-root exe should be readable");
    assert_eq!(exe_payload, b"fake-loader-exe");

    // Exe was NOT placed in the mod folder
    let exe_in_mod = mods_dir.join("core").join("game_loader.exe");
    assert!(!exe_in_mod.exists(), "game_loader.exe should not appear in mod folder");

    // Data file landed in mod folder as normal (top-level Data is auto-normalized)
    let data_in_mod = mods_dir.join("core").join("mod.txt");
    assert!(data_in_mod.exists(), "mod.txt should be in mod folder");
}

#[test]
fn install_fails_when_ini_set_target_file_is_missing() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("core.zip");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let mods_dir = dir.path().join("mods");
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    let zip_file = std::fs::File::create(&archive).expect("zip fixture should be created");
    let mut zip = zip::ZipWriter::new(zip_file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    zip.start_file("Data/test.txt", options)
        .expect("zip file entry should be created");
    zip.write_all(b"hello from archive")
        .expect("zip payload should be written");
    zip.finish().expect("zip should finalize");

    let source = format!(
        "name = \"Install Missing Ini Target Test\"\n\n[modlist.core]\ndependencies = []\n\n[[modlist.core.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n\n[[modlist.core.actions]]\naction = \"ini_set\"\nfile = \"Data/missing.ini\"\nkey = \"bFull Screen\"\nvalue = \"0\"\n",
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
        .env("GAME_DIR", &game_dir)
        .arg("install")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .arg("--mods-dir")
        .arg(&mods_dir)
        .arg("--game-dir")
        .arg(&game_dir)
        .assert()
        .failure();
}

#[test]
fn install_auto_detects_supported_archive_layouts_without_arguments() {
    let dir = tempdir().expect("temp dir should be created");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let mods_dir = dir.path().join("mods");
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    let root_zip = dir.path().join("root.zip");
    let data_zip = dir.path().join("data.zip");
    let mod_zip = dir.path().join("mod.zip");
    let mod_data_zip = dir.path().join("moddata.zip");

    make_zip_with_files(&root_zip, &[("plugin_root.esp", b"root"), ("textures/a.dds", b"tex")]);
    make_zip_with_files(&data_zip, &[("Data/plugin_data.esp", b"data"), ("Data/meshes/a.nif", b"mesh")]);
    make_zip_with_files(&mod_zip, &[("modonly/plugin_mod.esp", b"mod"), ("modonly/sound/a.wav", b"wav")]);
    make_zip_with_files(
        &mod_data_zip,
        &[("moddata/Data/plugin_mod_data.esp", b"moddata"), ("moddata/Data/menus/a.xml", b"xml")],
    );

    let source = format!(
        "name = \"Auto Layout Test\"\n\n[modlist.root]\ndependencies = []\n\n[[modlist.root.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n\n[modlist.data]\ndependencies = []\n\n[[modlist.data.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n\n[modlist.modonly]\ndependencies = []\n\n[[modlist.modonly.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n\n[modlist.moddata]\ndependencies = []\n\n[[modlist.moddata.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n",
        root_zip.display(),
        data_zip.display(),
        mod_zip.display(),
        mod_data_zip.display()
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
        .env("GAME_DIR", &game_dir)
        .arg("install")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .arg("--mods-dir")
        .arg(&mods_dir)
        .arg("--game-dir")
        .arg(&game_dir)
        .assert()
        .success();

    assert!(mods_dir.join("root").join("plugin_root.esp").exists());
    assert!(mods_dir.join("data").join("plugin_data.esp").exists());
    assert!(mods_dir.join("modonly").join("plugin_mod.esp").exists());
    assert!(mods_dir.join("moddata").join("plugin_mod_data.esp").exists());
}

#[test]
fn install_rejects_noncanonical_plugin_layouts() {
    let dir = tempdir().expect("temp dir should be created");
    let archive = dir.path().join("bad.zip");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let mods_dir = dir.path().join("mods");
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    make_zip_with_files(&archive, &[("foo/bar/plugin_bad.esp", b"bad")]);

    let source = format!(
        "name = \"Bad Layout Test\"\n\n[modlist.bad]\ndependencies = []\n\n[[modlist.bad.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n",
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
        .env("GAME_DIR", &game_dir)
        .arg("install")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .arg("--mods-dir")
        .arg(&mods_dir)
        .arg("--game-dir")
        .arg(&game_dir)
        .assert()
        .failure();
}

fn make_zip_with_files(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let zip_file = std::fs::File::create(path).expect("zip fixture should be created");
    let mut zip = zip::ZipWriter::new(zip_file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    for (name, bytes) in entries {
        zip.start_file(name, options)
            .expect("zip file entry should be created");
        zip.write_all(bytes)
            .expect("zip payload should be written");
    }
    zip.finish().expect("zip should finalize");
}
