use mudcrab::config::loader::load_modlist;
use mudcrab::config::validator::validate;
use tempfile::tempdir;

#[test]
fn rejects_empty_name() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"\"\n\n[modlist.core]\ndependencies = []\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err.to_string().contains("name must not be empty"));
}

#[test]
fn rejects_cycle() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"Cycle Test\"\n\n[modlist.a]\ndependencies = [\"b\"]\n\n[modlist.b]\ndependencies = [\"a\"]\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err.to_string().contains("dependency cycle"));
}

#[test]
fn accepts_minimal_valid_modlist() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"Valid\"\n\n[modlist.core]\ndependencies = []\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    validate(&source).expect("validation should pass");
}

#[test]
fn accepts_nested_mod_sections() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"Nested Sections\"\n\n[modlist.foundation.base]\ndependencies = []\n\n[modlist.gameplay.overhaul]\ndependencies = [\"base\"]\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    validate(&source).expect("validation should pass");
}

#[test]
fn rejects_duplicate_mod_ids_across_sections() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"Duplicate IDs\"\n\n[modlist.first.core]\ndependencies = []\n\n[modlist.second.core]\ndependencies = []\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err.to_string().contains("duplicate mod id core"));
}
