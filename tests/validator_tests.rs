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

    assert!(err.to_string().contains("also declares archives or files"));
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
    assert!(
        msg.contains("missing from the global plugins load order"),
        "{msg}"
    );
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

    assert!(
        err.to_string()
            .contains("still in the global plugins load order")
    );
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
///
/// A warning rather than an error, because a list mid-build legitimately runs
/// over while the merges that bring it back under are still unwritten.
#[test]
fn warns_about_a_load_order_over_the_plugin_limit() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    let plugins: Vec<String> = (0..256).map(|n| format!("  \"mod{n:03}.esp\",")).collect();
    std::fs::write(
        &path,
        format!("name = \"X\"\n\nplugins = [\n{}\n]\n", plugins.join("\n")),
    )
    .expect("fixture should be written");

    let source = load_modlist(&path).expect("modlist should parse");
    let warnings = validate(&source).expect("over the limit is buildable, just not playable");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let msg = &warnings[0];
    assert!(msg.contains("256"), "{msg}");
    assert!(msg.contains("255"), "{msg}");
    // The message has to say what it means, not only that it happened.
    assert!(msg.contains("merges"), "{msg}");
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
    let warnings = validate(&source).expect("255 is the limit, not one past it");
    assert!(warnings.is_empty(), "{warnings:?}");
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

fn validate_src(src: &str) -> anyhow::Result<Vec<String>> {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("modlist.toml");
    std::fs::write(&path, src).expect("fixture should be written");
    let source = load_modlist(&path)?;
    validate(&source)
}

const OPTIONAL_ACTION: &str = r#"
name = "Inputs"

[inputs.decompress]
type = "bool"
query = "Decompress?"
default = true

[[mods]]
id = "ui"

[[mods.actions]]
if = "decompress"
action = "dds_decompress"
paths = ["textures/menus"]
"#;

#[test]
fn accepts_an_action_conditioned_on_a_declared_input() {
    validate_src(OPTIONAL_ACTION).expect("should validate");
}

#[test]
fn rejects_an_action_condition_naming_an_undeclared_input() {
    let err = validate_src(&OPTIONAL_ACTION.replace("if = \"decompress\"", "if = \"!decompres\""))
        .expect_err("should fail");
    assert!(err.to_string().contains("'decompres'"), "{err}");
    assert!(err.to_string().contains("dds_decompress"), "{err}");
}

#[test]
fn rejects_a_mod_condition_naming_an_undeclared_input() {
    let err =
        validate_src(&OPTIONAL_ACTION.replace("id = \"ui\"", "id = \"ui\"\nif = \"nope == x\""))
            .expect_err("should fail");
    assert!(err.to_string().contains("'nope'"), "{err}");
}

#[test]
fn rejects_a_default_of_the_wrong_type() {
    let err = validate_src(&OPTIONAL_ACTION.replace("default = true", "default = \"yes\""))
        .expect_err("should fail");
    assert!(err.to_string().contains("default"), "{err}");
}

#[test]
fn rejects_a_choice_default_that_is_not_a_choice() {
    let src = r#"
name = "Inputs"

[inputs.res]
type = "choice"
query = "Resolution?"
choices = ["1k", "2k"]
default = "4k"
"#;
    let err = validate_src(src).expect_err("should fail");
    assert!(err.to_string().contains("\"4k\""), "{err}");
}

#[test]
fn a_generated_mod_needs_actions_and_no_archives() {
    let no_actions = "name = \"G\"\n\n[[mods]]\nid = \"g\"\ntype = \"generated\"\n";
    assert!(
        validate_src(no_actions)
            .unwrap_err()
            .to_string()
            .contains("no actions")
    );

    let with_archive = "name = \"G\"\n\n[[mods]]\nid = \"g\"\ntype = \"generated\"\n\n[[mods.archives]]\npath = \"x.zip\"\n\n[[mods.actions]]\naction = \"write_file\"\npath = \"a\"\ncontent = \"b\"\n";
    assert!(
        validate_src(with_archive)
            .unwrap_err()
            .to_string()
            .contains("archives")
    );
}

#[test]
fn an_mo2_plugin_needs_an_archive_and_takes_no_actions() {
    let no_archive = "name = \"P\"\n\n[[mods]]\nid = \"rootbuilder\"\ntype = \"mo2-plugin\"\n";
    assert!(
        validate_src(no_archive)
            .unwrap_err()
            .to_string()
            .contains("mo2-plugin")
    );

    let ok = "name = \"P\"\n\n[[mods]]\nid = \"rootbuilder\"\ntype = \"mo2-plugin\"\n\n[[mods.archives]]\npath = \"x.zip\"\nlayout = \"custom-data-folder\"\ndata_folder = \"rootbuilder\"\n";
    validate_src(ok).expect("should validate");
}

#[test]
fn a_simple_layout_with_a_data_folder_is_rejected() {
    let src = "name = \"S\"\n\n[[mods]]\nid = \"m\"\n\n[[mods.archives]]\npath = \"x.zip\"\nlayout = \"simple\"\ndata_folder = \"sub\"\n";
    assert!(
        validate_src(src)
            .unwrap_err()
            .to_string()
            .contains("custom-data-folder")
    );
}

const MOD_CONDITION: &str = r#"
name = "Mod conditions"

[[mods]]
id = "base"
type = "empty"

[[mods]]
id = "patch"
if = "mod:base"
type = "empty"
"#;

#[test]
fn accepts_a_condition_naming_another_mod() {
    validate_src(MOD_CONDITION).expect("should validate");
}

#[test]
fn rejects_a_condition_naming_an_unknown_mod() {
    let err =
        validate_src(&MOD_CONDITION.replace("mod:base", "mod:bsae")).expect_err("should fail");
    assert!(err.to_string().contains("'bsae'"), "{err}");
}

#[test]
fn rejects_a_mod_condition_asking_about_itself() {
    let err =
        validate_src(&MOD_CONDITION.replace("mod:base", "!mod:patch")).expect_err("should fail");
    assert!(err.to_string().contains("the mod itself"), "{err}");
}

#[test]
fn rejects_mod_conditions_that_need_each_other() {
    let src = MOD_CONDITION.replace("id = \"base\"", "id = \"base\"\nif = \"!mod:patch\"");
    let err = validate_src(&src).expect_err("should fail");
    assert!(err.to_string().contains("base -> patch -> base"), "{err}");
}

#[test]
fn an_action_may_ask_about_its_own_mod() {
    // Only a mod's own `if` decides whether it is in; an action asking about
    // its mod is merely redundant, not a loop.
    let src = MOD_CONDITION.replace(
        "if = \"mod:base\"\ntype = \"empty\"",
        "type = \"generated\"\n\n[[mods.actions]]\nif = \"mod:patch\"\naction = \"write_file\"\npath = \"a\"\ncontent = \"b\"",
    );
    assert_ne!(src, MOD_CONDITION);
    validate_src(&src).expect("should validate");
}

#[test]
fn rejects_a_malformed_condition() {
    let err =
        validate_src(&MOD_CONDITION.replace("mod:base", "Some Mod")).expect_err("should fail");
    assert!(format!("{err:#}").contains("mod:\"Some Mod\""), "{err:#}");
}

const ARCHIVE_CONDITION: &str = r#"
name = "Archive conditions"

[inputs.hd]
type = "bool"
query = "HD?"
default = false

[[mods]]
id = "m"

[[mods.archives]]
path = "manual:m.7z"
layout = "bain"
bain_subpackages = ["00 Core", { name = "01 HD", if = "hd" }]

[[mods.archives]]
if = "hd"
path = "manual:m-hd.7z"
layout = "fomod"

[[mods.archives.fomod_selections]]
if = "hd"
step = "S"
group = "G"
options = ["O"]
"#;

#[test]
fn accepts_conditions_on_archives_fomod_rows_and_bain_subpackages() {
    validate_src(ARCHIVE_CONDITION).expect("should validate");
}

#[test]
fn rejects_undeclared_inputs_in_archive_level_conditions() {
    for (from, site) in [
        ("{ name = \"01 HD\", if = \"hd\" }", "BAIN subpackage 01 HD"),
        (
            "if = \"hd\"\npath = \"manual:m-hd.7z\"",
            "archive manual:m-hd.7z",
        ),
        ("if = \"hd\"\nstep", "FOMOD selection S / G"),
    ] {
        let src = ARCHIVE_CONDITION.replacen(from, &from.replacen("\"hd\"", "\"hdd\"", 1), 1);
        assert_ne!(src, ARCHIVE_CONDITION, "{from}");
        let err = validate_src(&src).expect_err("should fail").to_string();
        assert!(err.contains("'hdd'") && err.contains(site), "{site}: {err}");
    }
}
