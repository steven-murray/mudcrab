//! A list copied to a new file: renamed, with its own docs file, and naming
//! the same included files from wherever the copy is.

use std::path::Path;
use tempfile::tempdir;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn a_copy_elsewhere_has_its_own_docs_and_the_same_mods() {
    let dir = tempdir().unwrap();
    let d = dir.path();
    let list = d.join("lists/mine.toml");
    write(
        &list,
        "name = \"Mine\"\ndocs_file = \"mine.docs.toml\"\n\n\
         [[mods]]\nid = \"Base\"\narchives = [{ path = \"manual:base.zip\" }]\n\n\
         [[mods]]\nid = \"part\"\ninclude = \"parts/part.toml\"\n",
    );
    write(
        &d.join("lists/mine.docs.toml"),
        "[mods.Base]\ntitle = \"The base\"\n",
    );
    write(
        &d.join("lists/parts/part.toml"),
        "name = \"Part\"\n\n[[mods]]\nid = \"Extra\"\narchives = [{ path = \"manual:extra.zip\" }]\n",
    );

    let copy = d.join("elsewhere/theirs.toml");
    let written = mudcrab::config::edit::clone_list(&list, &copy, "Theirs").unwrap();
    assert_eq!(written.len(), 2, "{written:?}");

    let text = std::fs::read_to_string(&copy).unwrap();
    assert!(text.contains("name = \"Theirs\""), "{text}");
    assert!(text.contains("docs_file = \"theirs.docs.toml\""), "{text}");
    assert!(
        text.contains("include = \"../lists/parts/part.toml\""),
        "{text}"
    );
    assert!(d.join("elsewhere/theirs.docs.toml").is_file());

    let loaded = mudcrab::config::loader::load_modlist_with_docs(&copy).unwrap();
    let ids: Vec<_> = loaded.mods.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert_eq!(ids[0], "Base");
    assert_eq!(
        loaded.mods[0].docs.title.as_deref(),
        Some("The base"),
        "the docs came along"
    );
    // The original is as it was.
    assert!(
        std::fs::read_to_string(&list)
            .unwrap()
            .contains("name = \"Mine\"")
    );
}

#[test]
fn a_copy_is_never_written_over_another_file() {
    let dir = tempdir().unwrap();
    let list = dir.path().join("a.toml");
    write(&list, "name = \"A\"\n");
    let taken = dir.path().join("b.toml");
    write(&taken, "name = \"B\"\n");
    assert!(mudcrab::config::edit::clone_list(&list, &taken, "B2").is_err());
    assert_eq!(std::fs::read_to_string(&taken).unwrap(), "name = \"B\"\n");
}
