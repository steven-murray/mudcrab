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
    CreateDummyPluginAction, ExtractBsaAction, FileHideAction, FileMoveAction, FilePruneAction,
    ModAction, PackBsaAction,
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
            plan_mods: &[],
            active_plugins: &Default::default(),
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
        prune_packed: false,
    })
}

fn pack_and_prune(output: &str) -> ModAction {
    ModAction::PackBsa(PackBsaAction {
        output: output.to_string(),
        include: Vec::new(),
        exclude: Vec::new(),
        prune_packed: true,
    })
}

fn dummy(output: &str) -> ModAction {
    ModAction::CreateDummyPlugin(CreateDummyPluginAction {
        output: output.to_string(),
    })
}

fn hide(paths: &[&str]) -> ModAction {
    ModAction::FileHide(FileHideAction {
        conflicts_with: Vec::new(),
        under: None,
        paths: paths.iter().map(|s| s.to_string()).collect(),
    })
}

fn prune(paths: &[&str]) -> ModAction {
    ModAction::FilePrune(FilePruneAction {
        conflicts_with: Vec::new(),
        under: None,
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
            plan_mods: &[],
            active_plugins: &Default::default(),
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

/// Guides name folders the way the archive spells them; staging folds them to
/// lowercase. Matching case-sensitively means every capitalised folder in every
/// guide instruction fails, which is what Part 11's `NoMushroomStalks` did.
#[test]
fn file_prune_matches_a_folder_whose_case_staging_has_already_folded() {
    let (dir, target) = staged_mod();
    std::fs::create_dir_all(target.join("nomushroomstalks/meshes")).expect("mkdir");
    std::fs::write(target.join("nomushroomstalks/meshes/stalk.nif"), b"x").expect("write");

    // The guide's spelling, against the folded folder on disk.
    run(&[prune(&["NoMushroomStalks"])], dir.path(), &target).expect("prune");

    assert!(!target.join("nomushroomstalks").exists());
    assert!(target.join("readme.txt").exists(), "unrelated files survive");

    // And the reverse direction: a lowercase pattern against a folder that
    // kept its capitals, as a mod-supplied path can.
    let (dir, target) = staged_mod();
    std::fs::create_dir_all(target.join("Docs")).expect("mkdir");
    std::fs::write(target.join("Docs/readme.html"), b"x").expect("write");
    run(&[prune(&["docs"])], dir.path(), &target).expect("prune");
    assert!(!target.join("Docs").exists());
}

/// Guide 27 of Part 11: delete everything under `textures/rocks` *except* the
/// `underwater` folder. `*` must stop at the separator, or the exception is
/// swallowed -- which is what happened, silently, until the Oracle diff showed
/// ten files missing that the guide explicitly keeps.
#[test]
fn file_prune_star_does_not_reach_into_subfolders() {
    let (dir, target) = staged_mod();
    std::fs::create_dir_all(target.join("textures/rocks/underwater")).expect("mkdir");
    std::fs::write(target.join("textures/rocks/loose01.dds"), b"x").expect("write");
    std::fs::write(target.join("textures/rocks/underwater/keep.dds"), b"x").expect("write");

    run(&[prune(&["textures/rocks/*.dds"])], dir.path(), &target).expect("prune");

    assert!(!target.join("textures/rocks/loose01.dds").exists(), "direct children go");
    assert!(
        target.join("textures/rocks/underwater/keep.dds").exists(),
        "a file one level deeper is not a direct child and must survive"
    );

    // `**` still crosses, which is what a bare folder name expands to.
    run(&[prune(&["textures/rocks/underwater"])], dir.path(), &target).expect("prune");
    assert!(!target.join("textures/rocks/underwater").exists());
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
            plan_mods: &[],
            active_plugins: &Default::default(),
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
            plan_mods: &[],
            active_plugins: &Default::default(),
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
            plan_mods: &[],
            active_plugins: &Default::default(),
            owner: "Example",
            settings: &settings,
            mod_target: Some(&target),
        },
    )
    .expect("dry run");

    assert!(target.join("meshes/rocks/rock01.nif").exists());
}

// --- extract_bsa ------------------------------------------------------------

fn extract(archive: &str, keep: bool) -> ModAction {
    ModAction::ExtractBsa(ExtractBsaAction {
        archive: archive.to_string(),
        keep_archive: keep,
    })
}

fn moves(from: &str, to: &str) -> ModAction {
    ModAction::FileMove(FileMoveAction {
        from: from.to_string(),
        to: to.to_string(),
    })
}

#[test]
fn extract_bsa_unpacks_and_removes_the_archive() {
    let (dir, target) = staged_mod();
    // Pack, prune the loose originals, then extract: back where we started.
    run(
        &[
            pack("Example.bsa", &["meshes/**", "textures/**"], &[]),
            prune(&["meshes", "textures"]),
        ],
        dir.path(),
        &target,
    )
    .expect("pack and prune");
    assert!(!target.join("meshes").exists());

    run(&[extract("Example.bsa", false)], dir.path(), &target).expect("extract");

    assert_eq!(
        std::fs::read_to_string(target.join("meshes/rocks/rock01.nif")).unwrap(),
        "NIF DATA"
    );
    assert_eq!(
        std::fs::read_to_string(target.join("textures/rocks/rock01.dds")).unwrap(),
        "DDS DATA"
    );
    assert!(
        !target.join("Example.bsa").exists(),
        "the archive must go, or the next pack_bsa folds it into itself"
    );
}

#[test]
fn extract_bsa_can_keep_the_archive() {
    let (dir, target) = staged_mod();
    run(&[pack("Example.bsa", &["meshes/**"], &[])], dir.path(), &target).expect("pack");
    run(&[extract("Example.bsa", true)], dir.path(), &target).expect("extract");

    assert!(target.join("Example.bsa").exists());
}

#[test]
fn extract_bsa_round_trips_through_pack_bsa() {
    // The whole point of the pair: unpack an archive, add something, repack.
    // This is Part 9's OOO voice files in miniature.
    let (dir, target) = staged_mod();
    run(
        &[
            pack("Example.bsa", &["meshes/**"], &[]),
            prune(&["meshes"]),
        ],
        dir.path(),
        &target,
    )
    .expect("initial pack");

    std::fs::create_dir_all(target.join("sound/voice")).unwrap();
    std::fs::write(target.join("sound/voice/line.mp3"), b"VOICE").unwrap();

    run(
        &[
            extract("Example.bsa", false),
            pack("Example.bsa", &[], &["readme.txt"]),
            prune(&["meshes", "sound", "textures"]),
        ],
        dir.path(),
        &target,
    )
    .expect("repack");

    let bytes = std::fs::read(target.join("Example.bsa")).unwrap();
    let archive = Bsa::parse(&bytes).unwrap();
    let mut paths: Vec<String> = archive.paths().collect();
    paths.sort();
    assert!(
        paths.contains(&"sound\\voice\\line.mp3".to_string()),
        "the added file must be in the repacked archive: {paths:?}"
    );
    assert!(
        paths.contains(&"meshes\\rocks\\rock01.nif".to_string()),
        "and so must the originals: {paths:?}"
    );
}

#[test]
fn extract_bsa_fails_when_the_archive_is_not_there() {
    let (dir, target) = staged_mod();
    let err = run(&[extract("Missing.bsa", false)], dir.path(), &target).unwrap_err();
    assert!(format!("{err:#}").contains("found no archive at"), "{err:#}");
}

// --- file_move --------------------------------------------------------------

#[test]
fn file_move_relocates_a_plugin_into_optional() {
    let (dir, target) = staged_mod();
    std::fs::write(target.join("Thing_Optional.esp"), b"plugin").unwrap();

    run(
        &[moves("Thing_Optional.esp", "optional/Thing_Optional.esp")],
        dir.path(),
        &target,
    )
    .expect("move");

    assert!(!target.join("Thing_Optional.esp").exists());
    assert_eq!(
        std::fs::read(target.join("optional/Thing_Optional.esp")).unwrap(),
        b"plugin"
    );
}

#[test]
fn file_move_matches_the_source_case_insensitively() {
    let (dir, target) = staged_mod();
    run(
        &[moves("MESHES/rocks/ROCK01.nif", "optional/rock01.nif")],
        dir.path(),
        &target,
    )
    .expect("move");

    assert!(target.join("optional/rock01.nif").exists());
}

#[test]
fn file_move_fails_when_the_source_is_not_there() {
    let (dir, target) = staged_mod();
    let err = run(&[moves("nope.esp", "optional/nope.esp")], dir.path(), &target).unwrap_err();
    assert!(format!("{err:#}").contains("found no 'nope.esp'"), "{err:#}");
}

#[test]
fn pack_bsa_can_delete_exactly_what_it_packed() {
    // The alternative is a file_prune naming the top-level folders by hand,
    // which means guessing the archive's layout. Part 9's OOO row guessed
    // "menus", which is not in that archive, and guessed "sound" where the
    // staged tree had "Sound" -- one error each way in a single list.
    let (dir, target) = staged_mod();
    run(&[pack_and_prune("Example.bsa")], dir.path(), &target).expect("pack");

    assert!(target.join("Example.bsa").is_file());
    assert!(!target.join("meshes").exists(), "packed folders should be gone");
    assert!(!target.join("textures").exists());
    assert!(!target.join("sound").exists());
    assert!(
        target.join("readme.txt").is_file(),
        "a root-level file is never packed, so it must not be deleted either"
    );
}

#[test]
fn pack_bsa_prune_leaves_anything_it_did_not_pack() {
    let (dir, target) = staged_mod();
    run(
        &[ModAction::PackBsa(PackBsaAction {
            output: "Example.bsa".to_string(),
            include: vec!["meshes/**".to_string()],
            exclude: Vec::new(),
            prune_packed: true,
        })],
        dir.path(),
        &target,
    )
    .expect("pack");

    assert!(!target.join("meshes").exists(), "packed");
    assert!(target.join("textures/rocks/rock01.dds").is_file(), "not packed");
    assert!(target.join("sound/fx/thud.wav").is_file(), "not packed");
}

#[test]
fn pack_bsa_prune_is_idempotent() {
    let (dir, target) = staged_mod();
    let actions = [pack_and_prune("Example.bsa")];
    run(&actions, dir.path(), &target).expect("first");
    let first = std::fs::metadata(target.join("Example.bsa")).unwrap().len();

    // Second run has nothing loose left to pack, so it must fail loudly rather
    // than silently write an empty archive over a good one.
    let err = run(&actions, dir.path(), &target).unwrap_err();
    assert!(format!("{err:#}").contains("matched no files"), "{err:#}");
    assert_eq!(
        std::fs::metadata(target.join("Example.bsa")).unwrap().len(),
        first,
        "the existing archive must survive a failed re-pack"
    );
}

#[test]
fn pack_bsa_prune_deletes_regardless_of_how_the_tree_is_cased() {
    // A BSA stores names lowercased. Rejoining those to the staged folder finds
    // nothing where the tree is cased differently, which is how OOO's 1554
    // `Sound/` files survived a prune that reported 4406 deletions and left
    // them shadowing the archive that held them.
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("mods/Example");
    for path in ["Sound/Voice/Line.mp3", "MESHES/Rocks/Rock01.nif"] {
        let full = target.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, b"data").unwrap();
    }

    run(&[pack_and_prune("Example.bsa")], dir.path(), &target).expect("pack");

    assert!(!target.join("Sound").exists(), "mixed-case folders must go too");
    assert!(!target.join("MESHES").exists());
    assert!(target.join("Example.bsa").is_file());
}

/// A prune or hide carrying neither `paths` nor `conflicts_with` selects
/// everything or nothing depending on which way the glob machinery is read,
/// and both answers are wrong. `paths` used to be mandatory in the schema; a
/// conflict selection names no paths, so the rule moved here.
#[test]
fn a_prune_or_hide_with_nothing_to_act_on_is_rejected() {
    let (dir, target) = staged_mod();

    for action in [
        ModAction::FilePrune(FilePruneAction {
            paths: Vec::new(),
            conflicts_with: Vec::new(),
            under: None,
        }),
        ModAction::FileHide(FileHideAction {
            paths: Vec::new(),
            conflicts_with: Vec::new(),
            under: None,
        }),
    ] {
        let name = action.name();
        let err = run(&[action], dir.path(), &target).expect_err("should reject");
        // `apply_all` wraps every failure in the action's name, so the reason
        // is one link down the chain.
        let reported = format!("{err:#}");
        assert!(reported.contains("conflicts_with"), "{name}: {reported}");
    }

    // Nothing was touched on the way to refusing.
    assert!(target.join("meshes/rocks/rock01.nif").exists());
}

/// `conflicts_with` naming a mod the plan does not have is a typo, and the
/// guide's own names for the mods in the row that needs this were wrong three
/// separate ways. An empty conflict list looks exactly like "no conflicts".
#[test]
fn conflicts_with_naming_an_unknown_mod_is_rejected() {
    let (dir, target) = staged_mod();

    let err = run(
        &[ModAction::FilePrune(FilePruneAction {
            paths: Vec::new(),
            conflicts_with: vec!["No Such Mod".to_string()],
            under: None,
        })],
        dir.path(),
        &target,
    )
    .expect_err("should reject");

    let reported = format!("{err:#}");
    assert!(reported.contains("No Such Mod"), "{reported}");
    assert!(target.join("meshes/rocks/rock01.nif").exists());
}
