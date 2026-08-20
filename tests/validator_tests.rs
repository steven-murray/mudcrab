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

#[test]
fn accepts_valid_modlist_with_merge() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"x\"\nplugins = [\"Oblivion.esm\", \"Merged.esp\"]\n\n\
         [[mods]]\nid = \"Alpha\"\n\
         [[mods.archives]]\npath = \"a.zip\"\ndownload_handler = \"local\"\n\n\
         [[mods]]\nid = \"Merged Plugins\"\nsection = [\"36 - zMERGED PLUGINS\"]\ntype = \"merge\"\n\n\
         [mods.merge]\noutput = \"Merged.esp\"\n\
         sources = [\n  { mod = \"Alpha\", plugin = \"Alpha.esp\" },\n]\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    validate(&source).expect("validation should pass");
}

#[test]
fn rejects_merge_type_with_no_merge_section() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"x\"\nplugins = [\"Oblivion.esm\"]\n\n\
         [[mods]]\nid = \"Merged Plugins\"\ntype = \"merge\"\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err.to_string().contains("no [mods.merge] section"));
}

#[test]
fn rejects_merge_section_on_mod_not_typed_merge() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"x\"\nplugins = [\"Oblivion.esm\", \"Merged.esp\"]\n\n\
         [[mods]]\nid = \"Alpha\"\n\
         [[mods.archives]]\npath = \"a.zip\"\ndownload_handler = \"local\"\n\n\
         [[mods]]\nid = \"Merged Plugins\"\n\n\
         [mods.merge]\noutput = \"Merged.esp\"\n\
         sources = [\n  { mod = \"Alpha\", plugin = \"Alpha.esp\" },\n]\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err.to_string().contains("but is not type = \"merge\""));
}

#[test]
fn rejects_merge_mod_that_also_declares_archives() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"x\"\nplugins = [\"Oblivion.esm\", \"Merged.esp\"]\n\n\
         [[mods]]\nid = \"Alpha\"\n\
         [[mods.archives]]\npath = \"a.zip\"\ndownload_handler = \"local\"\n\n\
         [[mods]]\nid = \"Merged Plugins\"\ntype = \"merge\"\n\
         [[mods.archives]]\npath = \"b.zip\"\ndownload_handler = \"local\"\n\n\
         [mods.merge]\noutput = \"Merged.esp\"\n\
         sources = [\n  { mod = \"Alpha\", plugin = \"Alpha.esp\" },\n]\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err
        .to_string()
        .contains("also declares archives or files"));
}

#[test]
fn rejects_merge_output_missing_from_load_order() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"x\"\nplugins = [\"Oblivion.esm\"]\n\n\
         [[mods]]\nid = \"Alpha\"\n\
         [[mods.archives]]\npath = \"a.zip\"\ndownload_handler = \"local\"\n\n\
         [[mods]]\nid = \"Merged Plugins\"\ntype = \"merge\"\n\n\
         [mods.merge]\noutput = \"Merged.esp\"\n\
         sources = [\n  { mod = \"Alpha\", plugin = \"Alpha.esp\" },\n]\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    let msg = err.to_string();
    assert!(msg.contains("produces"), "{msg}");
    assert!(msg.contains("missing from the global plugins load order"), "{msg}");
}

#[test]
fn rejects_merge_source_naming_unknown_mod() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"x\"\nplugins = [\"Oblivion.esm\", \"Merged.esp\"]\n\n\
         [[mods]]\nid = \"Merged Plugins\"\ntype = \"merge\"\n\n\
         [mods.merge]\noutput = \"Merged.esp\"\n\
         sources = [\n  { mod = \"Nonexistent\", plugin = \"Foo.esp\" },\n]\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err.to_string().contains("from unknown mod Nonexistent"));
}

#[test]
fn rejects_merge_source_plugin_still_in_global_load_order() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"x\"\nplugins = [\"Oblivion.esm\", \"Merged.esp\", \"Alpha.esp\"]\n\n\
         [[mods]]\nid = \"Alpha\"\n\
         [[mods.archives]]\npath = \"a.zip\"\ndownload_handler = \"local\"\n\n\
         [[mods]]\nid = \"Merged Plugins\"\ntype = \"merge\"\n\n\
         [mods.merge]\noutput = \"Merged.esp\"\n\
         sources = [\n  { mod = \"Alpha\", plugin = \"Alpha.esp\" },\n]\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err
        .to_string()
        .contains("still in the global plugins load order"));
}

#[test]
fn rejects_same_source_plugin_claimed_by_two_merges() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"x\"\nplugins = [\"Oblivion.esm\", \"MergedA.esp\", \"MergedB.esp\"]\n\n\
         [[mods]]\nid = \"Alpha\"\n\
         [[mods.archives]]\npath = \"a.zip\"\ndownload_handler = \"local\"\n\n\
         [[mods]]\nid = \"Merged A\"\ntype = \"merge\"\n\n\
         [mods.merge]\noutput = \"MergedA.esp\"\n\
         sources = [\n  { mod = \"Alpha\", plugin = \"Alpha.esp\" },\n]\n\n\
         [[mods]]\nid = \"Merged B\"\ntype = \"merge\"\n\n\
         [mods.merge]\noutput = \"MergedB.esp\"\n\
         sources = [\n  { mod = \"Alpha\", plugin = \"Alpha.esp\" },\n]\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err.to_string().contains("is merged by both"));
}

#[test]
fn rejects_merge_with_empty_sources() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"x\"\nplugins = [\"Oblivion.esm\", \"Merged.esp\"]\n\n\
         [[mods]]\nid = \"Merged Plugins\"\ntype = \"merge\"\n\n\
         [mods.merge]\noutput = \"Merged.esp\"\nsources = []\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let err = validate(&source).expect_err("validation should fail");

    assert!(err.to_string().contains("lists no source plugins"));
}

/// Oblivion indexes a plugin's FormIDs by one byte, so a load order longer than
/// 255 does not fail loudly: the game loads the first 255 and the rest are
/// simply absent, which reads as a mod that failed to install.
#[test]
fn rejects_a_load_order_over_the_plugin_limit() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    let plugins: Vec<String> = (0..256).map(|n| format!("  \"mod{n:03}.esp\",")).collect();
    std::fs::write(
        &path,
        format!(
            "name = \"X\"\n\nplugins = [\n{}\n]\n",
            plugins.join("\n")
        ),
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let msg = validate(&source)
        .expect_err("256 plugins is one too many")
        .to_string();
    assert!(msg.contains("256"), "{msg}");
    assert!(msg.contains("255"), "{msg}");
    // The message has to say what to do about it, not only that it happened.
    assert!(msg.contains("Merge"), "{msg}");
}

/// And exactly 255 is fine, which is the number a finished list sits at.
#[test]
fn accepts_a_load_order_at_the_plugin_limit() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    let plugins: Vec<String> = (0..255).map(|n| format!("  \"mod{n:03}.esp\",")).collect();
    std::fs::write(
        &path,
        format!("name = \"X\"\n\nplugins = [\n{}\n]\n", plugins.join("\n")),
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    validate(&source).expect("255 is the limit, not one past it");
}

/// A merge with `hide_sources` takes its sources out of the load order, so the
/// mods that ship them cannot also be required to have them there. Both rules
/// are right; they just have to know about each other.
#[test]
fn a_mod_may_declare_a_plugin_a_merge_consumes() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        r#"
name = "Merge Test"
plugins = ["Merged.esp"]

[[mods]]
id = "source-mod"
plugins = ["Source.esp"]

[[mods]]
id = "the-merge"
type = "merge"

  [mods.merge]
  output = "Merged.esp"
  method = "clobber"
  hide_sources = true
  sources = [{ mod = "source-mod", plugin = "Source.esp" }]
"#,
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    validate(&source).expect("the merge accounts for Source.esp");
}

/// And a plugin no merge consumes is still required to be in the load order --
/// otherwise the mod ships something the game will never load, silently.
#[test]
fn a_mod_declaring_an_unmerged_plugin_outside_the_load_order_is_an_error() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(
        &path,
        "name = \"X\"\n\nplugins = []\n\n[[mods]]\nid = \"lonely\"\nplugins = [\"Orphan.esp\"]\n",
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let msg = validate(&source)
        .expect_err("nothing consumes Orphan.esp")
        .to_string();
    assert!(msg.contains("Orphan.esp"), "{msg}");
    assert!(msg.contains("no merge consumes it"), "{msg}");
}
