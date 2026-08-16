//! M6 gate: a merge declared in TOML survives compile -> query -> install.
//!
//! Three synthetic source mods, one merge mod. The interesting properties are
//! not "a file appeared" but the ones that are easy to get subtly wrong:
//! sources stay enabled while their plugins vanish from the load order, their
//! BSAs keep loading, a second install is a no-op rather than a failure, and
//! the whole thing can be undone.

#[path = "support/esp.rs"]
mod esp;

use assert_cmd::Command;
use mudcrab::plugin::{FormId, Plugin};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};
use zip::write::FileOptions;

/// A source plugin with one STAT record, named so the winner is identifiable.
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

fn write_mod_archive(path: &Path, plugin_name: &str, plugin: &[u8], bsa: Option<&str>) {
    let file = std::fs::File::create(path).expect("create zip");
    let mut zip = zip::ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default();

    zip.start_file(format!("Data/{plugin_name}"), options)
        .expect("plugin entry");
    zip.write_all(plugin).expect("plugin bytes");

    if let Some(bsa) = bsa {
        zip.start_file(format!("Data/{bsa}"), options)
            .expect("bsa entry");
        zip.write_all(b"fake-bsa").expect("bsa bytes");
    }

    zip.finish().expect("finalize zip");
}

struct Fixture {
    dir: TempDir,
    game_dir: PathBuf,
    plan: PathBuf,
    cache: PathBuf,
    instance: PathBuf,
}

impl Fixture {
    fn mods_dir(&self) -> PathBuf {
        self.instance.join("mods")
    }

    fn profile_dir(&self) -> PathBuf {
        self.instance.join("profiles").join("test-profile")
    }

    fn install(&self) {
        Command::cargo_bin("mudcrab")
            .expect("binary should build")
            .arg("install")
            .arg(&self.plan)
            .arg("--cache")
            .arg(&self.cache)
            .arg("--mo2-instance-dir")
            .arg(&self.instance)
            .arg("--profile-name")
            .arg("test-profile")
            .arg("--game-dir")
            .arg(&self.game_dir)
            .assert()
            .success();
    }
}

/// Alpha and Beta both claim object index 0x801, so one must be renumbered.
/// Gamma masters Alpha and overrides that same record, so after the merge it
/// must clobber Alpha's copy rather than appear alongside it.
fn build_fixture() -> Fixture {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir");
    std::fs::write(game_dir.join("Oblivion.ini"), "bFull Screen = 1\n").expect("game ini");

    write_mod_archive(
        &root.join("alpha.zip"),
        "Alpha.esp",
        &source_plugin(&["Oblivion.esm"], 0x0100_0801, "AlphaStat"),
        Some("AlphaAssets.bsa"),
    );
    write_mod_archive(
        &root.join("beta.zip"),
        "Beta.esp",
        &source_plugin(&["Oblivion.esm"], 0x0100_0801, "BetaStat"),
        None,
    );
    write_mod_archive(
        &root.join("gamma.zip"),
        "Gamma.esp",
        // mod index 1 == Alpha.esp in Gamma's master list
        &source_plugin(&["Oblivion.esm", "Alpha.esp"], 0x0100_0801, "GammaStat"),
        None,
    );

    let modlist = root.join("modlist.toml");
    std::fs::write(
        &modlist,
        format!(
            r#"
name = "Merge E2E"
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
id = "Gamma"
section = ["FORTS"]
[[mods.archives]]
path = "{gamma}"
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
  {{ mod = "Gamma", plugin = "Gamma.esp" }},
]
"#,
            alpha = root.join("alpha.zip").display(),
            beta = root.join("beta.zip").display(),
            gamma = root.join("gamma.zip").display(),
        ),
    )
    .expect("modlist");

    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    let cache = root.join("cache");
    let instance = root.join("mo2-instance");

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("compile")
        .arg(&modlist)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&plan)
        .arg("--headless")
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("download")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .assert()
        .success();

    Fixture {
        dir,
        game_dir,
        plan,
        cache,
        instance,
    }
}

#[test]
fn a_declared_merge_installs_hides_its_sources_and_can_be_undone() {
    let fixture = build_fixture();
    fixture.install();

    let mods_dir = fixture.mods_dir();
    let profile_dir = fixture.profile_dir();

    // --- the merged plugin is real and correct ---------------------------
    let merged_path = mods_dir.join("Merged Plugins").join("Merged.esp");
    let merged = Plugin::read(&merged_path).expect("merged plugin should parse");

    assert_eq!(
        merged.masters.masters().len(),
        1,
        "Alpha is merged away, so only Oblivion.esm should remain a master"
    );

    let mut records: Vec<(FormId, String)> = merged
        .records()
        .map(|record| {
            let edid = record
                .fields()
                .iter()
                .find(|field| &field.signature == b"EDID")
                .map(|field| {
                    String::from_utf8_lossy(&field.data)
                        .trim_end_matches('\0')
                        .to_string()
                })
                .unwrap_or_default();
            (record.form_id, edid)
        })
        .collect();
    records.sort_by_key(|(form_id, _)| form_id.0);

    assert_eq!(
        records,
        vec![
            // Gamma's override wins over Alpha's original at the same FormID.
            (FormId(0x0100_0801), "GammaStat".to_string()),
            // Beta collided with Alpha and was renumbered.
            (FormId(0x0100_0802), "BetaStat".to_string()),
        ],
        "clobber order and renumbering"
    );

    // The zMerge-shaped sidecar, which the manual verification loop diffs.
    assert!(mods_dir
        .join("Merged Plugins")
        .join("merge - Merged Plugins")
        .join("map.json")
        .exists());

    // --- sources are hidden, not disabled --------------------------------
    for (mod_id, plugin) in [
        ("Alpha", "Alpha.esp"),
        ("Beta", "Beta.esp"),
        ("Gamma", "Gamma.esp"),
    ] {
        assert!(
            mods_dir.join(mod_id).join(format!("{plugin}.mohidden")).exists(),
            "{plugin} should be hidden"
        );
        assert!(
            !mods_dir.join(mod_id).join(plugin).exists(),
            "{plugin} should no longer be visible"
        );
    }

    let modlist_txt =
        std::fs::read_to_string(profile_dir.join("modlist.txt")).expect("modlist.txt");
    for mod_id in ["Alpha", "Beta", "Gamma", "Merged Plugins"] {
        assert!(
            modlist_txt.lines().any(|line| line == format!("+{mod_id}")),
            "{mod_id} must stay enabled so its assets still load; got:\n{modlist_txt}"
        );
    }

    // --- the load order names the merge, never its sources ---------------
    let plugins_txt =
        std::fs::read_to_string(profile_dir.join("plugins.txt")).expect("plugins.txt");
    assert_eq!(plugins_txt, "Oblivion.esm\nMerged.esp\n");

    // --- BSAs keep loading: hiding a plugin must not unload its archive ---
    let archives_txt =
        std::fs::read_to_string(profile_dir.join("archives.txt")).expect("archives.txt");
    assert_eq!(
        archives_txt, "AlphaAssets.bsa\n",
        "Alpha's BSA must survive its plugin being hidden"
    );

    // --- installing again is a no-op, not a failure ----------------------
    //
    // The regression this guards: the merge's own hiding step renames away the
    // files the merge reads, so a second run must find them under .mohidden.
    let before = std::fs::read(&merged_path).expect("read merged");
    fixture.install();
    let after = std::fs::read(&merged_path).expect("re-read merged");
    assert_eq!(before, after, "second install should reproduce the same bytes");
    assert!(mods_dir.join("Alpha").join("Alpha.esp.mohidden").exists());
    assert!(!mods_dir.join("Alpha").join("Alpha.esp").exists());

    // --- and it can all be undone ----------------------------------------
    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .arg("unhide-merges")
        .arg("--mo2-instance-dir")
        .arg(&fixture.instance)
        .arg("--profile-name")
        .arg("test-profile")
        .assert()
        .success();

    for (mod_id, plugin) in [
        ("Alpha", "Alpha.esp"),
        ("Beta", "Beta.esp"),
        ("Gamma", "Gamma.esp"),
    ] {
        assert!(
            mods_dir.join(mod_id).join(plugin).exists(),
            "{plugin} should be restored"
        );
        assert!(
            !mods_dir.join(mod_id).join(format!("{plugin}.mohidden")).exists(),
            "{plugin}.mohidden should be gone"
        );
    }

    drop(fixture.dir);
}
