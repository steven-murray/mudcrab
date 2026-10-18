//! Including files from git repositories: here, repositories made on disk
//! for each test, so nothing goes near the network.

use mudcrab::config::edit::{self, doc};
use mudcrab::config::git;
use mudcrab::config::loader::load_modlist;
use mudcrab::config::validator::validate;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

/// One cache for the whole run: each test's repository has its own URL, so
/// they do not meet in it.
fn cache() {
    static CACHE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let dir = CACHE.get_or_init(|| tempfile::tempdir().unwrap().keep());
    git::set_cache_dir(dir.clone());
}

fn run(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
        ])
        .args(args)
        .current_dir(repo)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// A repository holding `files`, committed on `main` and tagged `v1`.
fn repo(dir: &Path, files: &[(&str, &str)]) -> PathBuf {
    let repo = dir.join("common");
    std::fs::create_dir_all(&repo).unwrap();
    run(&repo, &["init", "--quiet"]);
    commit(&repo, files, "first");
    run(&repo, &["tag", "v1"]);
    repo
}

fn commit(repo: &Path, files: &[(&str, &str)], message: &str) {
    for (name, text) in files {
        let path = repo.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    run(repo, &["add", "."]);
    run(repo, &["commit", "--quiet", "-m", message]);
}

const CORE: &str = r#"component = true

[[mods]]
id = "A"
section = ["Patches"]
archives = [{ path = "manual:a.7z" }]
"#;

fn list(dir: &Path, include: &str) -> PathBuf {
    let path = dir.join("list.toml");
    std::fs::write(
        &path,
        format!(
            "name = \"L\"\n\n[[mods]]\nid = \"Own\"\narchives = [{{ path = \"manual:own.7z\" }}]\n\n\
             [[mods]]\nid = \"Common\"\nsection = [\"Core\"]\ninclude = {include:?}\n"
        ),
    )
    .unwrap();
    path
}

fn ids(list: &Path) -> Vec<String> {
    load_modlist(list)
        .unwrap()
        .mods
        .into_iter()
        .map(|m| m.id)
        .collect()
}

#[test]
fn a_file_is_included_from_a_tag_and_the_commit_locked() {
    cache();
    let dir = tempdir().unwrap();
    let repo = repo(dir.path(), &[("core.toml", CORE)]);
    let list = list(
        dir.path(),
        &format!("git+{}?tag=v1#core.toml", repo.display()),
    );

    let source = load_modlist(&list).unwrap();
    let a = source.mods.iter().find(|m| m.id == "A").unwrap();
    assert_eq!(a.section, ["Core", "Patches"]);
    let base = source.base_dir.clone().unwrap();
    let origin = mudcrab::config::compiler::origin_name(&base, a.origin.as_ref().unwrap());
    assert!(
        origin.starts_with("common@") && origin.ends_with(":core.toml"),
        "{origin}"
    );
    let lock = std::fs::read_to_string(git::lock_path(&list)).unwrap();
    assert!(lock.contains("ref = \"tag=v1\""), "{lock}");
    assert!(source.includes[0].commit.is_some());
}

#[test]
fn the_lock_holds_a_branch_until_update_moves_it() {
    cache();
    let dir = tempdir().unwrap();
    let repo = repo(dir.path(), &[("core.toml", CORE)]);
    let list = list(
        dir.path(),
        &format!("git+{}?branch=main#core.toml", repo.display()),
    );
    assert_eq!(ids(&list), ["Own", "A"]);
    let warnings = validate(&load_modlist(&list).unwrap()).unwrap();
    assert!(
        warnings.iter().any(|w| w.contains("follows branch main")),
        "{warnings:?}"
    );

    commit(
        &repo,
        &[(
            "core.toml",
            &format!("{CORE}\n[[mods]]\nid = \"B\"\narchives = [{{ path = \"manual:b.7z\" }}]\n"),
        )],
        "second",
    );
    // Still the locked commit.
    assert_eq!(ids(&list), ["Own", "A"]);

    let (update, mut lock) = git::plan_update(&list, &[]).unwrap();
    assert_eq!(update.moved.len(), 1);
    assert_eq!(update.added, ["B"]);
    assert!(update.removed.is_empty() && update.changed.is_empty());
    assert_eq!(ids(&list), ["Own", "A"], "planning writes nothing");
    lock.save().unwrap();
    assert_eq!(ids(&list), ["Own", "A", "B"]);
}

#[test]
fn offline_the_lock_and_cache_are_enough_and_nothing_else_is() {
    cache();
    let dir = tempdir().unwrap();
    let repo = repo(dir.path(), &[("core.toml", CORE)]);
    let list = list(
        dir.path(),
        &format!("git+{}?tag=v1#core.toml", repo.display()),
    );
    assert_eq!(ids(&list), ["Own", "A"]);
    // The repository gone: the cache still has the commit.
    std::fs::remove_dir_all(&repo).unwrap();
    assert_eq!(ids(&list), ["Own", "A"]);

    // A tag the lock does not hold cannot be looked up.
    let other = list_named(
        dir.path(),
        "other.toml",
        &format!("git+{}?tag=v2#core.toml", repo.display()),
    );
    let mut lock = git::Lock::read(&other).unwrap();
    lock.offline = true;
    let err = format!(
        "{:#}",
        mudcrab::config::loader::load_locked(&other, false, &Default::default(), &mut lock)
            .unwrap_err()
    );
    assert!(err.contains("offline"), "{err}");
}

fn list_named(dir: &Path, name: &str, include: &str) -> PathBuf {
    let path = list(dir, include);
    let named = dir.join(name);
    std::fs::rename(&path, &named).unwrap();
    named
}

#[test]
fn an_included_file_includes_its_neighbours_from_the_same_commit() {
    cache();
    let dir = tempdir().unwrap();
    let repo = repo(
        dir.path(),
        &[
            (
                "groups/all.toml",
                "component = true\n\n[[mods]]\nid = \"Core\"\ninclude = \"../core.toml\"\n",
            ),
            ("core.toml", CORE),
            (
                "escape.toml",
                "component = true\n\n[[mods]]\nid = \"Out\"\ninclude = \"../../list.toml\"\n",
            ),
        ],
    );
    let list = list(
        dir.path(),
        &format!("git+{}?tag=v1#groups/all.toml", repo.display()),
    );
    assert_eq!(ids(&list), ["Own", "A"]);

    let escape = list_named(
        dir.path(),
        "escape.toml",
        &format!("git+{}?tag=v1#escape.toml", repo.display()),
    );
    let err = format!("{:#}", load_modlist(&escape).unwrap_err());
    assert!(err.contains("outside its repository"), "{err}");
}

#[test]
fn a_mod_from_a_git_include_is_not_edited_in_the_cache() {
    cache();
    let dir = tempdir().unwrap();
    let repo = repo(dir.path(), &[("core.toml", CORE)]);
    let list = list(
        dir.path(),
        &format!("git+{}?tag=v1#core.toml", repo.display()),
    );
    let pairs = doc::parse_pairs(&["oracle_name=\"X\"".to_string()]).unwrap();
    let err = edit::plan_mod(&list, "A", |text, _, _| {
        doc::set_fields(text, "A", None, &pairs, &[])
    })
    .unwrap_err()
    .to_string();
    assert!(err.contains("from a git repository"), "{err}");
    let err = format!("{:#}", edit::plan_remove(&list, "A").unwrap_err());
    assert!(err.contains("from a git repository"), "{err}");
}

#[test]
fn a_tag_the_repository_lacks_is_named() {
    cache();
    let dir = tempdir().unwrap();
    let repo = repo(dir.path(), &[("core.toml", CORE)]);
    let list = list(
        dir.path(),
        &format!("git+{}?tag=v9#core.toml", repo.display()),
    );
    let err = format!("{:#}", load_modlist(&list).unwrap_err());
    assert!(err.contains("has no tag v9"), "{err}");

    let list = list_named(
        dir.path(),
        "missing.toml",
        &format!("git+{}?tag=v1#nope.toml", repo.display()),
    );
    let err = format!("{:#}", load_modlist(&list).unwrap_err());
    assert!(err.contains("has no file nope.toml"), "{err}");
}
