//! A merge is only rebuilt when something it is built from has changed.
//!
//! The author of a 700-mod list re-runs `install` dozens of times while working
//! through a section. Rebuilding an 86-source merge on each of those runs is
//! minutes of work to reproduce a file that is already on disk byte for byte,
//! so the run has to be able to tell that nothing it merges from has moved.

#[path = "support/esp.rs"]
mod esp;

use assert_cmd::Command;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};
use zip::write::FileOptions;

fn source_plugin(masters: &[&str], form_id: u32, editor_id: &str) -> Vec<u8> {
    esp::plugin(
        masters,
        &[esp::group(
            *b"STAT",
            0,
            &[esp::record(
                b"STAT",
                form_id,
                0,
                &[esp::field(b"EDID", &esp::zstring(editor_id))],
            )],
        )],
    )
}

fn write_mod_archive(path: &Path, plugin_name: &str, plugin: &[u8]) {
    let file = std::fs::File::create(path).expect("create zip");
    let mut zip = zip::ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    zip.start_file(format!("Data/{plugin_name}"), options)
        .expect("plugin entry");
    zip.write_all(plugin).expect("plugin bytes");
    zip.finish().expect("finalize zip");
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
        for escaped in chars.by_ref() {
            if escaped.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

struct Fixture {
    _dir: TempDir,
    game_dir: PathBuf,
    plan: PathBuf,
    cache: PathBuf,
    mods_dir: PathBuf,
}

impl Fixture {
    /// Run `install` and return what it logged.
    fn install(&self, extra: &[&str]) -> String {
        let assert = Command::cargo_bin("mudcrab")
            .expect("binary should build")
            .arg("install")
            .arg(&self.plan)
            .args(["--cache", self.cache.to_str().unwrap()])
            .args(["--mods-dir", self.mods_dir.to_str().unwrap()])
            .args(["--game-dir", self.game_dir.to_str().unwrap()])
            .args(extra)
            .assert()
            .success();
        strip_ansi(&String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8 stdout"))
    }

    fn merged_path(&self) -> PathBuf {
        self.mods_dir.join("Merged Plugins").join("Merged.esp")
    }

    /// A source plugin as it sits on disk after the merge hid it.
    fn hidden_source(&self, mod_id: &str, plugin: &str) -> PathBuf {
        self.mods_dir.join(mod_id).join(format!("{plugin}.mohidden"))
    }
}

fn build_fixture() -> Fixture {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir");

    write_mod_archive(
        &root.join("alpha.zip"),
        "Alpha.esp",
        &source_plugin(&["Oblivion.esm"], 0x0100_0801, "AlphaStat"),
    );
    write_mod_archive(
        &root.join("beta.zip"),
        "Beta.esp",
        &source_plugin(&["Oblivion.esm"], 0x0100_0802, "BetaStat"),
    );

    let modlist = root.join("modlist.toml");
    std::fs::write(
        &modlist,
        format!(
            r#"
name = "Merge Rebuild"
plugins = ["Oblivion.esm", "Merged.esp"]

[[mods]]
id = "Alpha"
section = ["FORTS"]
[[mods.archives]]
path = "{alpha}"
download_handler = "local"

[[mods]]
id = "Beta"
section = ["FORTS"]
[[mods.archives]]
path = "{beta}"
download_handler = "local"

[[mods]]
id = "Merged Plugins"
section = ["36 - zMERGED PLUGINS"]
type = "merge"
plugins = ["Merged.esp"]

[mods.merge]
output = "Merged.esp"
sources = [
  {{ mod = "Alpha", plugin = "Alpha.esp" }},
  {{ mod = "Beta",  plugin = "Beta.esp" }},
]
"#,
            alpha = root.join("alpha.zip").display(),
            beta = root.join("beta.zip").display(),
        ),
    )
    .expect("modlist");

    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    let cache = root.join("cache");

    for args in [
        vec![
            "compile".to_string(),
            modlist.display().to_string(),
            "--output".to_string(),
            compiled.display().to_string(),
        ],
        vec![
            "query".to_string(),
            compiled.display().to_string(),
            "--output".to_string(),
            plan.display().to_string(),
            "--headless".to_string(),
        ],
        vec![
            "download".to_string(),
            plan.display().to_string(),
            "--cache".to_string(),
            cache.display().to_string(),
        ],
    ] {
        Command::cargo_bin("mudcrab")
            .expect("binary should build")
            .args(&args)
            .assert()
            .success();
    }

    let mods_dir = root.join("mods");
    Fixture {
        _dir: dir,
        game_dir,
        plan,
        cache,
        mods_dir,
    }
}

#[test]
fn a_second_install_does_not_rebuild_an_unchanged_merge() {
    let fixture = build_fixture();

    let first = fixture.install(&[]);
    assert!(
        first.contains("merge: built"),
        "the first run has to build it:\n{first}"
    );

    let merged = fixture.merged_path();
    let bytes = std::fs::read(&merged).expect("merged plugin should exist");
    let built_at = std::fs::metadata(&merged)
        .and_then(|meta| meta.modified())
        .expect("merged plugin mtime");

    let second = fixture.install(&[]);
    assert!(
        second.contains("reason=\"inputs unchanged since the last build\""),
        "the second run should skip the rebuild:\n{second}"
    );
    assert!(
        !second.contains("merge: built"),
        "nothing should have been rebuilt:\n{second}"
    );
    assert_eq!(
        std::fs::metadata(&merged)
            .and_then(|meta| meta.modified())
            .expect("merged plugin mtime"),
        built_at,
        "the merged plugin should not have been rewritten at all"
    );
    assert_eq!(
        std::fs::read(&merged).expect("merged plugin should still be readable"),
        bytes
    );

    // Skipping the rebuild must not skip the hiding: the sources stay hidden,
    // which is what makes the merge take effect in the load order.
    assert!(fixture.hidden_source("Alpha", "Alpha.esp").exists());
    assert!(!fixture.mods_dir.join("Alpha").join("Alpha.esp").exists());
}

#[test]
fn changing_a_source_plugin_triggers_a_rebuild() {
    let fixture = build_fixture();
    fixture.install(&[]);

    let merged = fixture.merged_path();
    let before = std::fs::read(&merged).expect("merged plugin should exist");

    // The merge hid its own source, so the plugin now lives under .mohidden --
    // which is exactly the file the next merge will read.
    let source = fixture.hidden_source("Alpha", "Alpha.esp");
    std::fs::write(
        &source,
        source_plugin(&["Oblivion.esm"], 0x0100_0801, "AlphaStatRewritten"),
    )
    .expect("source plugin should be rewritable");

    let report = fixture.install(&[]);
    assert!(
        report.contains("merge: built"),
        "an edited source should force a rebuild:\n{report}"
    );

    let after = std::fs::read(&merged).expect("merged plugin should exist");
    assert_ne!(
        before, after,
        "the rebuilt merge should carry the edited record"
    );
    assert!(
        String::from_utf8_lossy(&after).contains("AlphaStatRewritten"),
        "the rebuild should have read the edited source"
    );
}

#[test]
fn force_merges_rebuilds_even_when_nothing_changed() {
    let fixture = build_fixture();
    fixture.install(&[]);

    let merged = fixture.merged_path();
    let bytes = std::fs::read(&merged).expect("merged plugin should exist");
    let built_at = std::fs::metadata(&merged)
        .and_then(|meta| meta.modified())
        .expect("merged plugin mtime");

    // Coarse filesystem timestamps would make "rewritten" and "left alone"
    // indistinguishable, so give the clock a moment to move on.
    std::thread::sleep(std::time::Duration::from_millis(20));

    let report = fixture.install(&["--force-merges"]);
    assert!(
        report.contains("merge: built"),
        "--force-merges should rebuild regardless:\n{report}"
    );
    assert!(
        !report.contains("reason=\"inputs unchanged since the last build\""),
        "--force-merges should not consult the recorded inputs:\n{report}"
    );
    assert_ne!(
        std::fs::metadata(&merged)
            .and_then(|meta| meta.modified())
            .expect("merged plugin mtime"),
        built_at,
        "the merged plugin should have been written again"
    );

    // Deterministic: forcing a rebuild reproduces the same bytes.
    assert_eq!(
        std::fs::read(&merged).expect("merged plugin should be readable"),
        bytes
    );
}

#[test]
fn a_deleted_merge_output_is_rebuilt_even_though_the_inputs_match() {
    let fixture = build_fixture();
    fixture.install(&[]);

    std::fs::remove_file(fixture.merged_path()).expect("merged plugin should be removable");

    let report = fixture.install(&[]);
    assert!(
        report.contains("merge: built"),
        "a matching hash over a missing output is not a completed build:\n{report}"
    );
    assert!(fixture.merged_path().is_file());
}
