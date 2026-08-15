use mudcrab::config::loader::load_modlist;
use mudcrab::config::validator::validate;
use tempfile::tempdir;

#[test]
fn rejects_empty_name() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"\"\n\n[[mods]]\nid = \"core\"\ndependencies = []\n",
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
        "name = \"Cycle Test\"\n\n[[mods]]\nid = \"a\"\ndependencies = [\"b\"]\n\n[[mods]]\nid = \"b\"\ndependencies = [\"a\"]\n",
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
        "name = \"Valid\"\n\n[[mods]]\nid = \"core\"\ndependencies = []\n",
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
        "name = \"Nested Sections\"\n\n[[mods]]\nid = \"base\"\nsection = [\"foundation\"]\ndependencies = []\n\n[[mods]]\nid = \"overhaul\"\nsection = [\"gameplay\"]\ndependencies = [\"base\"]\n",
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
        "name = \"Duplicate IDs\"\n\n[[mods]]\nid = \"core\"\nsection = [\"first\"]\ndependencies = []\n\n[[mods]]\nid = \"core\"\nsection = [\"second\"]\ndependencies = []\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err.to_string().contains("duplicate mod id core"));
}

#[test]
fn rejects_unknown_field_naming_it_and_the_valid_alternatives() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"X\"\n\n[[mods]]\nid = \"a\"\ndependencie = []\n",
    )
    .expect("fixture should be written");

    let err = load_modlist(&path).expect_err("typo should be rejected");
    let msg = err.to_string();
    // The old nested/untagged schema reported only "data did not match any
    // variant of untagged enum ModNode", with no field or line information.
    assert!(msg.contains("dependencie"), "{msg}");
    assert!(msg.contains("dependencies"), "{msg}");
}

#[test]
fn rejects_unknown_archive_field() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"X\"\n\n[[mods]]\nid = \"a\"\n\n[[mods.archives]]\npath = \"p\"\ndata_folders = \"Data\"\n",
    )
    .expect("fixture should be written");

    let msg = load_modlist(&path)
        .expect_err("typo should be rejected")
        .to_string();
    assert!(msg.contains("data_folders"), "{msg}");
}

#[test]
fn rejects_unknown_layout_value() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"X\"\n\n[[mods]]\nid = \"a\"\n\n[[mods.archives]]\npath = \"p\"\nlayout = \"bane\"\n",
    )
    .expect("fixture should be written");

    let msg = load_modlist(&path)
        .expect_err("bad layout should be rejected")
        .to_string();
    assert!(msg.contains("bane") && msg.contains("bain"), "{msg}");
}

#[test]
fn section_is_always_a_list_and_each_level_becomes_a_separator() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"X\"\n\n\
         [[mods]]\nid = \"a\"\nsection = [\"OBSE PLUGINS\", \"Core\"]\n\n\
         [[mods]]\nid = \"b\"\nsection = [\"OBSE PLUGINS\", \"Core\"]\n\n\
         [[mods]]\nid = \"c\"\nsection = [\"OBSE PLUGINS\", \"Extra\"]\n",
    )
    .expect("fixture should be written");

    let modlist = load_modlist(&path).expect("modlist should load");
    let entries = modlist
        .mo2_modlist_entries()
        .expect("entries should be derived");

    let rendered: Vec<String> = entries
        .iter()
        .map(|e| match e {
            mudcrab::config::schema::Mo2ModlistEntry::Section { name } => format!("#{name}"),
            mudcrab::config::schema::Mo2ModlistEntry::Mod { id } => id.clone(),
        })
        .collect();

    // A separator per level, emitted only when that level changes.
    assert_eq!(
        rendered,
        vec![
            "#OBSE PLUGINS",
            "#OBSE PLUGINS - Core",
            "a",
            "b",
            "#OBSE PLUGINS - Extra",
            "c",
        ]
    );
}
