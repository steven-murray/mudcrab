//! Installing from archives that are already on this machine.
//!
//! The motivating case is a modlist whose archives already sit in an MO2 or
//! Wabbajack downloads folder: re-fetching tens of gigabytes is slow, and
//! keeping a second copy of them is worse than slow on a nearly full disk.

use assert_cmd::Command;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;
use zip::write::FileOptions;

fn make_zip_with_files(path: &Path, entries: &[(&str, &[u8])]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("archive parent should be created");
    }
    let zip_file = std::fs::File::create(path).expect("zip fixture should be created");
    let mut zip = zip::ZipWriter::new(zip_file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    for (name, bytes) in entries {
        zip.start_file(*name, options)
            .expect("zip file entry should be created");
        zip.write_all(bytes).expect("zip payload should be written");
    }
    zip.finish().expect("zip should finalize");
}

/// Compile then query a source modlist, leaving a personalized plan at `plan`.
fn build_plan(game_dir: &Path, modlist: &Path, compiled: &Path, plan: &Path) {
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", game_dir)
        .args(["compile", modlist.to_str().unwrap()])
        .args(["--output", compiled.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", game_dir)
        .args(["query", compiled.to_str().unwrap()])
        .args(["--output", plan.to_str().unwrap()])
        .arg("--headless")
        .assert()
        .success();
}

/// Drop the ANSI colour codes the tracing subscriber interleaves through its
/// output, so assertions can match the field names it prints.
fn strip_ansi(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // CSI sequences run until a letter; that is all fmt emits.
        for escaped in chars.by_ref() {
            if escaped.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

/// Every file in a directory, sorted, for asserting a search path is untouched.
fn dir_listing(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(path)
        .expect("directory should be readable")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn install_adopts_an_archive_from_a_search_path_without_downloading() {
    let dir = tempdir().expect("temp dir should be created");
    let game_dir = dir.path().join("game");
    let search_dir = dir.path().join("existing-downloads");
    let cache = dir.path().join("cache");
    let mods_dir = dir.path().join("mods");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");
    std::fs::create_dir_all(&search_dir).expect("search dir should be created");

    let existing = search_dir.join("Really Big Mod-4321-1-0.zip");
    make_zip_with_files(&existing, &[("Data/test.txt", b"hello from a local archive")]);

    // A nexus descriptor with no api key available: if anything tries to
    // download this, the command fails.
    let source = "name = \"Search Path Test\"\n\n[[mods]]\nid = \"core\"\ndependencies = []\n\n[[mods.archives]]\npath = \"nexus:oblivion/4321/9999\"\nfile_name = \"Really Big Mod-4321-1-0.zip\"\n";
    std::fs::write(&modlist, source).expect("fixture modlist should be written");
    build_plan(&game_dir, &modlist, &compiled, &plan);

    // Note there is no `download` run at all: install resolves the archive itself.
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env_remove("NEXUS_API_KEY")
        .args(["install", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--mods-dir", mods_dir.to_str().unwrap()])
        .args(["--game-dir", game_dir.to_str().unwrap()])
        .args(["--archive-search-path", search_dir.to_str().unwrap()])
        .assert()
        .success();

    let extracted = mods_dir.join("core").join("test.txt");
    assert!(extracted.exists(), "the mod should have been installed");
    assert_eq!(
        std::fs::read_to_string(&extracted).expect("extracted file should be readable"),
        "hello from a local archive"
    );

    // The archive landed in the cache under its derived cache name, not its
    // original one, with the original recorded in the sidecar beside it.
    let cached = cache.join("core_0_9999.zip");
    assert!(cached.is_file(), "archive should be adopted into the cache");
    assert_eq!(
        std::fs::read_to_string(cache.join("core_0_9999.zip.orig-name"))
            .expect("orig-name sidecar should exist"),
        "Really Big Mod-4321-1-0.zip"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let cached_meta = std::fs::metadata(&cached).expect("cached archive metadata");
        let source_meta = std::fs::metadata(&existing).expect("search path archive metadata");
        assert_eq!(
            cached_meta.ino(),
            source_meta.ino(),
            "the adopted archive should be a hard link, not a second copy"
        );
    }

    // The search path is a read-only source.
    assert_eq!(dir_listing(&search_dir), vec!["Really Big Mod-4321-1-0.zip"]);
}

#[test]
fn download_resolves_from_a_search_path_instead_of_fetching() {
    let dir = tempdir().expect("temp dir should be created");
    let game_dir = dir.path().join("game");
    let search_dir = dir.path().join("existing-downloads");
    let cache = dir.path().join("cache");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    make_zip_with_files(
        &search_dir.join("Local Copy-1-0.zip"),
        &[("Data/test.txt", b"local")],
    );

    let source = "name = \"Download Search Path Test\"\n\n[[mods]]\nid = \"core\"\ndependencies = []\n\n[[mods.archives]]\npath = \"nexus:oblivion/1/2\"\nfile_name = \"Local Copy-1-0.zip\"\n";
    std::fs::write(&modlist, source).expect("fixture modlist should be written");
    build_plan(&game_dir, &modlist, &compiled, &plan);

    // Without a search path there is no api key, so the nexus handler fails.
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env_remove("NEXUS_API_KEY")
        .env("GAME_DIR", &game_dir)
        .args(["download", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .assert()
        .failure();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env_remove("NEXUS_API_KEY")
        .env("GAME_DIR", &game_dir)
        .args(["download", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--archive-search-path", search_dir.to_str().unwrap()])
        .assert()
        .success();

    assert!(cache.join("core_0_2.zip").is_file());
}

#[test]
fn search_paths_are_tried_in_order_and_the_first_hit_wins() {
    let dir = tempdir().expect("temp dir should be created");
    let game_dir = dir.path().join("game");
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    // The same filename in both directories, with contents that identify which.
    make_zip_with_files(&first.join("Shared.zip"), &[("Data/which.txt", b"first")]);
    make_zip_with_files(&second.join("Shared.zip"), &[("Data/which.txt", b"second")]);

    let source = "name = \"Search Order Test\"\n\n[[mods]]\nid = \"core\"\ndependencies = []\n\n[[mods.archives]]\npath = \"nexus:oblivion/1/2\"\nfile_name = \"Shared.zip\"\n";
    std::fs::write(&modlist, source).expect("fixture modlist should be written");
    build_plan(&game_dir, &modlist, &compiled, &plan);

    for (index, (a, b, expected)) in [
        (&first, &second, "first"),
        (&second, &first, "second"),
    ]
    .iter()
    .enumerate()
    {
        let cache = dir.path().join(format!("cache-{index}"));
        let mods_dir = dir.path().join(format!("mods-{index}"));

        Command::cargo_bin("mudcrab")
            .expect("binary should build")
            .env_remove("NEXUS_API_KEY")
            .args(["install", plan.to_str().unwrap()])
            .args(["--cache", cache.to_str().unwrap()])
            .args(["--mods-dir", mods_dir.to_str().unwrap()])
            .args(["--game-dir", game_dir.to_str().unwrap()])
            .args(["--archive-search-path", a.to_str().unwrap()])
            .args(["--archive-search-path", b.to_str().unwrap()])
            .assert()
            .success();

        let installed = std::fs::read_to_string(mods_dir.join("core").join("which.txt"))
            .expect("installed file should be readable");
        assert_eq!(
            installed, *expected,
            "the search path listed first should win"
        );
    }
}

#[test]
fn search_path_filename_matching_ignores_case() {
    let dir = tempdir().expect("temp dir should be created");
    let game_dir = dir.path().join("game");
    let search_dir = dir.path().join("downloads");
    let cache = dir.path().join("cache");
    let mods_dir = dir.path().join("mods");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    // On disk in one casing...
    make_zip_with_files(
        &search_dir.join("MiXeD CaSe ArChIvE-99-1-0.ZIP"),
        &[("Data/test.txt", b"case insensitive")],
    );

    // ...declared in the modlist in another. Mod pages are transcribed by hand.
    let source = "name = \"Case Test\"\n\n[[mods]]\nid = \"core\"\ndependencies = []\n\n[[mods.archives]]\npath = \"nexus:oblivion/99/7\"\nfile_name = \"mixed case archive-99-1-0.zip\"\n";
    std::fs::write(&modlist, source).expect("fixture modlist should be written");
    build_plan(&game_dir, &modlist, &compiled, &plan);

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env_remove("NEXUS_API_KEY")
        .args(["install", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--mods-dir", mods_dir.to_str().unwrap()])
        .args(["--game-dir", game_dir.to_str().unwrap()])
        .args(["--archive-search-path", search_dir.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(mods_dir.join("core").join("test.txt"))
            .expect("extracted file should be readable"),
        "case insensitive"
    );
}

#[test]
fn an_archive_without_a_file_name_still_uses_the_ordinary_download_path() {
    let dir = tempdir().expect("temp dir should be created");
    let game_dir = dir.path().join("game");
    let search_dir = dir.path().join("downloads");
    let cache = dir.path().join("cache");
    let mods_dir = dir.path().join("mods");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    // A search path holding an archive whose name matches nothing declared.
    make_zip_with_files(
        &search_dir.join("unrelated.zip"),
        &[("Data/wrong.txt", b"should never be installed")],
    );

    // `plain` has no file_name and is fetched the old way; `local` is adopted.
    let plain_archive = dir.path().join("plain.zip");
    make_zip_with_files(&plain_archive, &[("Data/plain.txt", b"downloaded normally")]);
    make_zip_with_files(
        &search_dir.join("Adopted-1-0.zip"),
        &[("Data/adopted.txt", b"adopted locally")],
    );

    let source = format!(
        "name = \"Fallthrough Test\"\n\n[[mods]]\nid = \"plain\"\ndependencies = []\n\n[[mods.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n\n[[mods]]\nid = \"adopted\"\ndependencies = []\n\n[[mods.archives]]\npath = \"nexus:oblivion/5/6\"\nfile_name = \"Adopted-1-0.zip\"\n",
        plain_archive.display()
    );
    std::fs::write(&modlist, source).expect("fixture modlist should be written");
    build_plan(&game_dir, &modlist, &compiled, &plan);

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env_remove("NEXUS_API_KEY")
        .env("GAME_DIR", &game_dir)
        .args(["download", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--archive-search-path", search_dir.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env_remove("NEXUS_API_KEY")
        .args(["install", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--mods-dir", mods_dir.to_str().unwrap()])
        .args(["--game-dir", game_dir.to_str().unwrap()])
        .args(["--archive-search-path", search_dir.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(mods_dir.join("plain").join("plain.txt"))
            .expect("normally downloaded file should be readable"),
        "downloaded normally"
    );
    assert_eq!(
        std::fs::read_to_string(mods_dir.join("adopted").join("adopted.txt"))
            .expect("adopted file should be readable"),
        "adopted locally"
    );
    assert!(
        !mods_dir.join("plain").join("wrong.txt").exists(),
        "an unrelated archive in a search path must never be picked up"
    );
}

#[test]
fn mo2_export_skips_a_download_that_is_already_there_at_the_same_size() {
    let dir = tempdir().expect("temp dir should be created");
    let game_dir = dir.path().join("game");
    let search_dir = dir.path().join("downloads");
    let cache = dir.path().join("cache");
    let instance_dir = dir.path().join("mo2-instance");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");
    std::fs::write(game_dir.join("Oblivion.ini"), "bFull Screen = 1\n")
        .expect("game ini should be written");

    make_zip_with_files(
        &search_dir.join("Exported-7-1-0.zip"),
        &[("Data/test.txt", b"payload")],
    );

    let source = "name = \"Export Skip Test\"\n\n[[mods]]\nid = \"core\"\ndependencies = []\n\n[[mods.archives]]\npath = \"nexus:oblivion/7/8\"\nfile_name = \"Exported-7-1-0.zip\"\n";
    std::fs::write(&modlist, source).expect("fixture modlist should be written");
    build_plan(&game_dir, &modlist, &compiled, &plan);

    let install = || {
        Command::cargo_bin("mudcrab")
            .expect("binary should build")
            .env_remove("NEXUS_API_KEY")
            .args(["install", plan.to_str().unwrap()])
            .args(["--cache", cache.to_str().unwrap()])
            .args(["--mo2-instance-dir", instance_dir.to_str().unwrap()])
            .args(["--game-dir", game_dir.to_str().unwrap()])
            .args(["--archive-search-path", search_dir.to_str().unwrap()])
            .assert()
            .success();
    };

    install();

    // The declared file_name, not the derived cache name, is what MO2 sees.
    let exported = instance_dir.join("downloads").join("Exported-7-1-0.zip");
    assert!(exported.is_file(), "archive should be exported into downloads/");
    let original = std::fs::read(&exported).expect("exported archive should be readable");

    // Stand in a same-sized sentinel. A re-export would overwrite it; a skip
    // leaves it alone. (Removing first so the sentinel is not written through a
    // hard link into the cache.)
    std::fs::remove_file(&exported).expect("exported archive should be removable");
    std::fs::write(&exported, vec![b'X'; original.len()]).expect("sentinel should be written");

    install();

    assert_eq!(
        std::fs::read(&exported).expect("exported archive should be readable"),
        vec![b'X'; original.len()],
        "an existing download of the same size should not be re-exported"
    );

    // A different size is not the same file, so it is replaced.
    std::fs::remove_file(&exported).expect("exported archive should be removable");
    std::fs::write(&exported, b"truncated").expect("short sentinel should be written");

    install();

    assert_eq!(
        std::fs::read(&exported).expect("exported archive should be readable"),
        original,
        "a download of a different size should be re-exported"
    );
}

#[test]
fn check_classifies_archives_as_cached_local_or_needing_download() {
    let dir = tempdir().expect("temp dir should be created");
    let game_dir = dir.path().join("game");
    let search_dir = dir.path().join("downloads");
    let cache = dir.path().join("cache");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    let cached_archive = dir.path().join("cached.zip");
    make_zip_with_files(&cached_archive, &[("Data/cached.txt", b"cached")]);
    make_zip_with_files(
        &search_dir.join("Findable-2-1-0.zip"),
        &[("Data/local.txt", b"local")],
    );

    let source = format!(
        "name = \"Check Classification Test\"\n\n[[mods]]\nid = \"cached\"\ndependencies = []\n\n[[mods.archives]]\npath = \"{}\"\ndownload_handler = \"local\"\n\n[[mods]]\nid = \"local\"\ndependencies = []\n\n[[mods.archives]]\npath = \"nexus:oblivion/2/3\"\nfile_name = \"Findable-2-1-0.zip\"\n\n[[mods]]\nid = \"missing\"\ndependencies = []\n\n[[mods.archives]]\npath = \"nexus:oblivion/4/5\"\nfile_name = \"Nowhere-4-1-0.zip\"\n",
        cached_archive.display()
    );
    std::fs::write(&modlist, source).expect("fixture modlist should be written");
    build_plan(&game_dir, &modlist, &compiled, &plan);

    // Only the first mod is actually fetched, so the other two stay unfetched.
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .args(["download", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--only", "cached"])
        .assert()
        .success();

    let all = Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .args(["check", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--archive-search-path", search_dir.to_str().unwrap()])
        .assert()
        .failure();

    let report = strip_ansi(
        &String::from_utf8(all.get_output().stdout.clone()).expect("stdout should be utf-8"),
    );
    assert!(
        report.contains("mod_id=cached") && report.contains("availability=\"cached\""),
        "the fetched archive should be reported as cached:\n{report}"
    );
    assert!(
        report.contains("availability=\"resolvable locally\""),
        "the search-path archive should be reported as resolvable locally:\n{report}"
    );
    assert!(
        report.contains("availability=\"MUST BE DOWNLOADED\""),
        "the unfetched archive should be reported as needing a download:\n{report}"
    );
    assert!(
        report.contains("cached=1")
            && report.contains("resolvable_locally=1")
            && report.contains("must_be_downloaded=1"),
        "a summary count should be printed even when the check fails:\n{report}"
    );

    // With the unavailable mod excluded, everything is satisfied and check passes.
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .args(["check", plan.to_str().unwrap()])
        .args(["--cache", cache.to_str().unwrap()])
        .args(["--archive-search-path", search_dir.to_str().unwrap()])
        .args(["--only", "cached", "--only", "local"])
        .assert()
        .success();

    // check is a report, not a fetch: it must not have adopted anything.
    assert!(
        !cache.join("local_0_3.zip").exists(),
        "check must not write the locally resolvable archive into the cache"
    );
}
