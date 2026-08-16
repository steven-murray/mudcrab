//! The three BSA-related install actions: `pack_bsa`, `create_dummy_plugin`
//! and `file_prune`.
//!
//! The composition test is the important one: these three exist to be run in
//! sequence, and the ordering guarantee they depend on comes from `apply_all`
//! iterating the declared list rather than from any machinery of their own.

use mudcrab::bsa::Bsa;
use mudcrab::config::actions::{apply_all, ActionCx};
use mudcrab::config::install::InstallSettings;
use mudcrab::config::schema::{
    CreateDummyPluginAction, FileHideAction, FilePruneAction, ModAction, PackBsaAction,
};
use mudcrab::plugin::Plugin;
use std::path::{Path, PathBuf};

fn settings(root: &Path) -> InstallSettings {
    InstallSettings {
        cache_dir: root.join("cache"),
        mods_dir: root.join("mods"),
        mo2_instance_dir: None,
        profile_name: "Default".to_string(),
        game_dir: None,
        game_root_dir: None,
        execute_actions: true,
        dry_run: false,
        tools: Default::default(),
        filter: Default::default(),
        archive_search_paths: Vec::new(),
        force_merges: false,
        force: false,
    }
}

/// A staged mod with a few loose asset folders.
fn staged_mod() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("mods/Example");
    for (path, contents) in [
        ("meshes/rocks/rock01.nif", "NIF DATA"),
        ("meshes/rocks/rock02.nif", "MORE NIF"),
        ("textures/rocks/rock01.dds", "DDS DATA"),
        ("sound/fx/thud.wav", "RIFF DATA"),
        ("readme.txt", "docs"),
    ] {
        let full = target.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, contents).unwrap();
    }
    (dir, target)
}

fn run(actions: &[ModAction], root: &Path, target: &Path) -> anyhow::Result<()> {
    let settings = settings(root);
    apply_all(
        actions,
        &ActionCx {
            owner: "Example",
            settings: &settings,
            mod_target: Some(target),
        },
    )
}

fn pack(output: &str, include: &[&str], exclude: &[&str]) -> ModAction {
    ModAction::PackBsa(PackBsaAction {
        output: output.to_string(),
        include: include.iter().map(|s| s.to_string()).collect(),
        exclude: exclude.iter().map(|s| s.to_string()).collect(),
    })
}

fn dummy(output: &str) -> ModAction {
    ModAction::CreateDummyPlugin(CreateDummyPluginAction {
        output: output.to_string(),
    })
}

fn hide(paths: &[&str]) -> ModAction {
    ModAction::FileHide(FileHideAction {
        paths: paths.iter().map(|s| s.to_string()).collect(),
    })
}

fn prune(paths: &[&str]) -> ModAction {
    ModAction::FilePrune(FilePruneAction {
        paths: paths.iter().map(|s| s.to_string()).collect(),
    })
}

// --- pack_bsa ---------------------------------------------------------------

#[test]
fn pack_bsa_produces_a_readable_archive_of_the_staged_files() {
    let (dir, target) = staged_mod();
    run(&[pack("Example.bsa", &[], &[])], dir.path(), &target).expect("pack");

    let bytes = std::fs::read(target.join("Example.bsa")).expect("archive written");
    let archive = Bsa::parse(&bytes).expect("parse");

    let mut paths: Vec<String> = archive.paths().collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "meshes\\rocks\\rock01.nif".to_string(),
            "meshes\\rocks\\rock02.nif".to_string(),
            "sound\\fx\\thud.wav".to_string(),
            "textures\\rocks\\rock01.dds".to_string(),
        ],
        "readme.txt sits at the archive root, so it stays loose"
    );

    let (folder, file) = archive
        .files()
        .find(|(_, f)| f.name == "rock01.nif")
        .expect("rock01.nif");
    assert_eq!(
        file.data(&file.path_in(folder)).unwrap().as_ref(),
        b"NIF DATA"
    );
}

#[test]
fn pack_bsa_honours_include_and_exclude_globs() {
    let (dir, target) = staged_mod();
    run(
        &[pack("Example.bsa", &["meshes/**"], &["**/rock02.nif"])],
        dir.path(),
        &target,
    )
    .expect("pack");

    let bytes = std::fs::read(target.join("Example.bsa")).unwrap();
    let archive = Bsa::parse(&bytes).unwrap();
    assert_eq!(
        archive.paths().collect::<Vec<_>>(),
        vec!["meshes\\rocks\\rock01.nif".to_string()]
    );
}

#[test]
fn pack_bsa_does_not_pack_the_archive_into_itself() {
    let (dir, target) = staged_mod();
    let actions = [pack("Example.bsa", &["**/*.nif"], &[])];

    run(&actions, dir.path(), &target).expect("first pack");
    let first = std::fs::metadata(target.join("Example.bsa")).unwrap().len();

    // Re-running must be idempotent. Without the self-exclusion the previous
    // archive would be folded into the new one.
    run(&actions, dir.path(), &target).expect("second pack");
    let second = std::fs::metadata(target.join("Example.bsa")).unwrap().len();

    assert_eq!(first, second, "packing twice must not grow the archive");
    let bytes = std::fs::read(target.join("Example.bsa")).unwrap();
    let archive = Bsa::parse(&bytes).unwrap();
    assert!(
        !archive.paths().any(|p| p.ends_with(".bsa")),
        "the archive must not contain itself"
    );
}

#[test]
fn pack_bsa_refuses_a_path_escaping_the_mod_folder() {
    let (dir, target) = staged_mod();
    let err = run(&[pack("../escaped.bsa", &[], &[])], dir.path(), &target).unwrap_err();
    assert!(err.to_string().contains("escaped.bsa") || err.chain().any(|c| c.to_string().contains("invalid relative path")), "{err:#}");
    assert!(!dir.path().join("mods/escaped.bsa").exists());
}

#[test]
fn pack_bsa_fails_when_nothing_matches() {
    let (dir, target) = staged_mod();
    let err = run(
        &[pack("Example.bsa", &["nothing/**"], &[])],
        dir.path(),
        &target,
    )
    .unwrap_err();
    assert!(format!("{err:#}").contains("matched no files"), "{err:#}");
}

#[test]
fn pack_bsa_writes_nothing_in_a_dry_run() {
    let (dir, target) = staged_mod();
    let mut settings = settings(dir.path());
    settings.dry_run = true;

    apply_all(
        &[pack("Example.bsa", &[], &[])],
        &ActionCx {
            owner: "Example",
            settings: &settings,
            mod_target: Some(&target),
        },
    )
    .expect("dry run");

    assert!(!target.join("Example.bsa").exists());
}

// --- create_dummy_plugin ----------------------------------------------------

#[test]
fn create_dummy_plugin_writes_a_valid_empty_plugin() {
    let (dir, target) = staged_mod();
    run(&[dummy("Example.esp")], dir.path(), &target).expect("dummy");

    let bytes = std::fs::read(target.join("Example.esp")).expect("plugin written");
    let plugin = Plugin::parse(&bytes).expect("parses as a TES4 plugin");

    assert_eq!(&plugin.header.signature, b"TES4");
    // Oblivion.esm and nothing else. Nothing here references it -- there are no
    // records at all -- but a plugin declaring no masters is unusual enough that
    // tools treat it as suspect, so the dummy looks like every other plugin.
    assert_eq!(
        plugin
            .masters
            .masters()
            .iter()
            .map(|master| master.as_str())
            .collect::<Vec<_>>(),
        ["Oblivion.esm"]
    );
    assert_eq!(plugin.records().count(), 0);
    assert_eq!(plugin.record_and_group_count(), 0);
    assert!(plugin.header.field(b"HEDR").is_some());

    // and it round-trips through the plugin writer like any other plugin
    assert_eq!(plugin.to_bytes(), bytes);
}

#[test]
fn create_dummy_plugin_refuses_a_path_escaping_the_mod_folder() {
    let (dir, target) = staged_mod();
    let err = run(&[dummy("../escaped.esp")], dir.path(), &target).unwrap_err();
    assert!(err.chain().any(|c| c.to_string().contains("invalid relative path")), "{err:#}");
    assert!(!dir.path().join("mods/escaped.esp").exists());
}

// --- file_prune -------------------------------------------------------------

#[test]
fn file_prune_deletes_only_matching_paths() {
    let (dir, target) = staged_mod();
    run(&[prune(&["meshes/**"])], dir.path(), &target).expect("prune");

    assert!(!target.join("meshes").exists(), "emptied folders are removed");
    assert!(target.join("textures/rocks/rock01.dds").exists());
    assert!(target.join("sound/fx/thud.wav").exists());
    assert!(target.join("readme.txt").exists());
}

#[test]
fn file_prune_removes_folders_left_empty() {
    let (dir, target) = staged_mod();
    run(&[prune(&["**/*.nif", "**/*.dds", "**/*.wav"])], dir.path(), &target)
        .expect("prune");

    // Nested empties are cleaned all the way up, not just the leaf.
    assert!(!target.join("meshes").exists());
    assert!(!target.join("textures").exists());
    assert!(!target.join("sound").exists());
    assert!(target.join("readme.txt").exists());
}

#[test]
fn file_prune_treats_a_bare_directory_name_as_the_whole_folder() {
    // What the guide means every time it says "delete the loose meshes &
    // textures folders". As a raw glob, `meshes` matches only a file called
    // `meshes` -- so this silently deleted nothing until it was expanded.
    let (dir, target) = staged_mod();
    run(&[prune(&["meshes", "textures"])], dir.path(), &target).expect("prune");

    assert!(!target.join("meshes").exists(), "the meshes folder should be gone");
    assert!(!target.join("textures").exists(), "the textures folder should be gone");
    assert!(target.join("sound/fx/thud.wav").exists(), "unrelated folders survive");
    assert!(target.join("readme.txt").exists());
}

#[test]
fn file_prune_accepts_a_directory_name_with_a_trailing_slash() {
    let (dir, target) = staged_mod();
    run(&[prune(&["meshes/"])], dir.path(), &target).expect("prune");

    assert!(!target.join("meshes").exists());
    assert!(target.join("textures/rocks/rock01.dds").exists());
}

#[test]
fn file_prune_fails_when_a_pattern_matches_nothing() {
    // A prune that deletes nothing leaves the loose files it was meant to
    // remove shadowing whatever was packed, and the install still reports
    // success. It has to be loud.
    let (dir, target) = staged_mod();
    let err = run(&[prune(&["meshes", "Meshes"])], dir.path(), &target)
        .expect_err("a pattern matching nothing should fail");

    let message = format!("{err:#}");
    assert!(message.contains("'Meshes'"), "the failing pattern must be named: {message}");
    assert!(!message.contains("'meshes'"), "the matching pattern must not be: {message}");
}

#[test]
fn file_prune_refuses_traversal_patterns() {
    let (dir, target) = staged_mod();
    let sibling = dir.path().join("mods/Other");
    std::fs::create_dir_all(&sibling).unwrap();
    std::fs::write(sibling.join("victim.esp"), b"x").unwrap();

    for pattern in ["../Other/**", "/etc/*", "..\\Other\\*"] {
        let err = run(&[prune(&[pattern])], dir.path(), &target).unwrap_err();
        let message = format!("{err:#}");
        assert!(
            message.contains("inside the mod folder") || message.contains("must be relative"),
            "pattern {pattern} was not rejected: {message}"
        );
    }

    assert!(sibling.join("victim.esp").exists(), "nothing outside was touched");
    assert!(target.join("meshes/rocks/rock01.nif").exists());
}

#[test]
fn file_prune_requires_at_least_one_pattern() {
    // An empty list would compile to a glob set matching everything.
    let (dir, target) = staged_mod();
    let err = run(&[prune(&[])], dir.path(), &target).unwrap_err();
    assert!(format!("{err:#}").contains("at least one"), "{err:#}");
    assert!(target.join("meshes/rocks/rock01.nif").exists());
}

#[test]
fn file_prune_deletes_nothing_in_a_dry_run() {
    let (dir, target) = staged_mod();
    let mut settings = settings(dir.path());
    settings.dry_run = true;

    apply_all(
        &[prune(&["meshes/**"])],
        &ActionCx {
            owner: "Example",
            settings: &settings,
            mod_target: Some(&target),
        },
    )
    .expect("dry run");

    assert!(target.join("meshes/rocks/rock01.nif").exists());
}

// --- composition ------------------------------------------------------------

#[test]
fn the_three_actions_compose_in_declaration_order() {
    // The whole point of file_prune: the loose files must survive long enough
    // to be packed, then be deleted. Ordering comes from apply_all iterating
    // the declared list, with no extra machinery.
    let (dir, target) = staged_mod();
    std::fs::remove_file(target.join("readme.txt")).unwrap();

    run(
        &[
            pack("Example.bsa", &[], &[]),
            dummy("Example.esp"),
            prune(&["meshes/**", "textures/**", "sound/**"]),
        ],
        dir.path(),
        &target,
    )
    .expect("compose");

    let mut left: Vec<String> = std::fs::read_dir(&target)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        vec!["Example.bsa".to_string(), "Example.esp".to_string()],
        "only the archive and its plugin should remain"
    );

    // The archive still holds everything the prune removed.
    let bytes = std::fs::read(target.join("Example.bsa")).unwrap();
    let archive = Bsa::parse(&bytes).unwrap();
    let mut paths: Vec<String> = archive.paths().collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "meshes\\rocks\\rock01.nif".to_string(),
            "meshes\\rocks\\rock02.nif".to_string(),
            "sound\\fx\\thud.wav".to_string(),
            "textures\\rocks\\rock01.dds".to_string(),
        ]
    );

    // and the plugin beside it is a real, empty plugin
    let plugin = Plugin::parse(&std::fs::read(target.join("Example.esp")).unwrap()).unwrap();
    assert_eq!(plugin.records().count(), 0);
}

#[test]
fn pruning_before_packing_leaves_an_archive_without_the_pruned_files() {
    // The inverse ordering, to show the order is genuinely what decides the
    // outcome rather than the actions coincidentally doing the right thing.
    let (dir, target) = staged_mod();
    run(
        &[
            prune(&["meshes/**"]),
            pack("Example.bsa", &[], &[]),
        ],
        dir.path(),
        &target,
    )
    .expect("compose");

    let bytes = std::fs::read(target.join("Example.bsa")).unwrap();
    let archive = Bsa::parse(&bytes).unwrap();
    let paths: Vec<String> = archive.paths().collect();
    assert!(
        !paths.iter().any(|p| p.starts_with("meshes")),
        "meshes were pruned before the pack ran: {paths:?}"
    );
}

#[test]
fn all_three_are_only_valid_as_per_mod_actions() {
    // mod_target is None for install-wide actions, which is what makes a
    // staged-folder action invalid there.
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());

    for action in [
        pack("Example.bsa", &[], &[]),
        dummy("Example.esp"),
        prune(&["meshes/**"]),
    ] {
        let name = action.name();
        let err = apply_all(
            std::slice::from_ref(&action),
            &ActionCx {
                owner: "plan",
                settings: &settings,
                mod_target: None,
            },
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("per-mod action"),
            "{name} should be rejected without a mod target: {err:#}"
        );
    }
}

// --- file_hide --------------------------------------------------------------

#[test]
fn file_hide_renames_a_file_the_way_mo2_does() {
    let (dir, target) = staged_mod();
    run(&[hide(&["meshes/rocks/rock01.nif"])], dir.path(), &target).expect("hide");

    assert!(!target.join("meshes/rocks/rock01.nif").exists());
    assert!(target.join("meshes/rocks/rock01.nif.mohidden").exists());
    assert!(
        target.join("meshes/rocks/rock02.nif").exists(),
        "its neighbour is untouched"
    );
}

#[test]
fn file_hide_hides_a_whole_folder_in_one_rename() {
    // What MO2 does when you hide a directory: the folder is renamed and
    // everything below it goes with it, rather than each file being renamed.
    let (dir, target) = staged_mod();
    run(&[hide(&["meshes/rocks"])], dir.path(), &target).expect("hide");

    assert!(target.join("meshes/rocks.mohidden/rock01.nif").exists());
    assert!(target.join("meshes/rocks.mohidden/rock02.nif").exists());
    assert!(!target.join("meshes/rocks").exists());
}

#[test]
fn file_hide_matches_each_segment_case_insensitively() {
    // Archives are built on Windows and the guide transcribes paths by eye:
    // "Textures > Characters > Nuska > Hair" has to find `textures/...`.
    let (dir, target) = staged_mod();
    run(&[hide(&["Meshes/Rocks/ROCK01.nif"])], dir.path(), &target).expect("hide");

    assert!(target.join("meshes/rocks/rock01.nif.mohidden").exists());
}

#[test]
fn file_hide_is_idempotent() {
    let (dir, target) = staged_mod();
    let actions = [hide(&["meshes/rocks"])];

    run(&actions, dir.path(), &target).expect("first hide");
    run(&actions, dir.path(), &target).expect("second hide");

    assert!(target.join("meshes/rocks.mohidden/rock01.nif").exists());
    assert!(
        !target.join("meshes/rocks.mohidden.mohidden").exists(),
        "hiding twice must not double the suffix"
    );
}

#[test]
fn file_hide_fails_when_a_path_is_not_there() {
    // These are literal paths the guide named, not globs. One that is missing
    // means the archive changed or the entry has a typo, and either way the
    // install would keep a file it was told to remove.
    let (dir, target) = staged_mod();
    let err = run(&[hide(&["meshes/rocks", "meshes/nope"])], dir.path(), &target)
        .expect_err("a missing path should fail");

    let message = format!("{err:#}");
    assert!(message.contains("'meshes/nope'"), "{message}");
    assert!(!message.contains("'meshes/rocks'"), "{message}");
}

#[test]
fn file_hide_refuses_a_path_escaping_the_mod_folder() {
    let (dir, target) = staged_mod();
    let err = run(&[hide(&["../escaped"])], dir.path(), &target).unwrap_err();
    assert!(
        err.chain().any(|c| c.to_string().contains("invalid relative path")),
        "{err:#}"
    );
}

#[test]
fn file_hide_changes_nothing_in_a_dry_run() {
    let (dir, target) = staged_mod();
    let mut settings = settings(dir.path());
    settings.dry_run = true;

    apply_all(
        &[hide(&["meshes/rocks"])],
        &ActionCx {
            owner: "Example",
            settings: &settings,
            mod_target: Some(&target),
        },
    )
    .expect("dry run");

    assert!(target.join("meshes/rocks/rock01.nif").exists());
}
