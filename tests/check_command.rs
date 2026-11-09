//! `check`: what can be known before installing, with every archive in the
//! downloads folder.

use assert_cmd::Command;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;
use zip::write::FileOptions;

fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options: FileOptions<'_, ()> = FileOptions::default();
    for (name, bytes) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

/// A plugin that is only a TES4 header naming `masters`.
fn plugin(masters: &[&str]) -> Vec<u8> {
    let mut fields = Vec::new();
    let mut field = |kind: &[u8; 4], data: &[u8]| {
        fields.extend_from_slice(kind);
        fields.extend_from_slice(&(data.len() as u16).to_le_bytes());
        fields.extend_from_slice(data);
    };
    let mut hedr = Vec::new();
    hedr.extend_from_slice(&1.0f32.to_le_bytes());
    hedr.extend_from_slice(&0u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    field(b"HEDR", &hedr);
    for master in masters {
        let mut name = master.as_bytes().to_vec();
        name.push(0);
        field(b"MAST", &name);
        field(b"DATA", &0u64.to_le_bytes());
    }
    let mut out = b"TES4".to_vec();
    out.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(&fields);
    out
}

fn mudcrab(root: &Path) -> Command {
    let mut command = Command::cargo_bin("mudcrab").unwrap();
    command
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("LOCALAPPDATA", root.join("state"));
    command
}

/// Base, and Patch needing Base (ordered after it) and Elsewhere (not in
/// the list); Absent's archive is not downloaded.
fn masters_fixture(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let downloads = root.join("downloads");
    make_zip(
        &downloads.join("Base-1-1-0.zip"),
        &[("Base.esm", &plugin(&["Oblivion.esm"]))],
    );
    make_zip(
        &downloads.join("Patch-2-1-0.zip"),
        &[(
            "Patch.esp",
            &plugin(&["Oblivion.esm", "Base.esm", "Elsewhere.esm"]),
        )],
    );
    let list = root.join("list.toml");
    std::fs::write(
        &list,
        r#"name = "L"
plugins = ["Patch.esp", "Base.esm"]

[[mods]]
id = "Base"
plugins = ["Base.esm"]
[[mods.archives]]
path = "nexus:oblivion/1/10"
file_name = "Base-1-1-0.zip"

[[mods]]
id = "Patch"
plugins = ["Patch.esp"]
[[mods.archives]]
path = "nexus:oblivion/2/20"
file_name = "Patch-2-1-0.zip"

[[mods]]
id = "Absent"
[[mods.archives]]
path = "nexus:oblivion/3/30"
file_name = "Absent-3-1-0.zip"
"#,
    )
    .unwrap();
    (list, downloads)
}

#[test]
fn check_as_a_library_returns_its_findings_and_reports_them_as_events() {
    use mudcrab::events::{Collect, Event, Tone};
    let dir = tempdir().unwrap();
    let (list, downloads) = masters_fixture(dir.path());
    let compiled = mudcrab::config::pipeline::compile(&list, false).unwrap();
    let plan =
        mudcrab::config::query::build_plan(&compiled.modlist, &Default::default(), None).unwrap();
    let sink = std::sync::Arc::new(Collect::default());
    mudcrab::events::set_sink(sink.clone());

    let report = mudcrab::config::check::check_all(
        &plan,
        &mudcrab::config::check::CheckSettings {
            search: Vec::new(),
            cache_dir: downloads,
            filter: Default::default(),
            masters_only: false,
            resources: None,
        },
    )
    .unwrap();

    assert_eq!(report.problems, 3);
    let masters: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.kind == mudcrab::config::check::Kind::Masters)
        .collect();
    assert_eq!(masters.len(), 2);
    assert!(
        masters
            .iter()
            .all(|f| f.item.mod_id.as_deref() == Some("Patch"))
    );
    let events = sink.events();
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Section { tone: Tone::Problem, heading, items }
            if heading.starts_with("Plugins whose masters") && items.len() == 2
    )));
    // The sink is the process's, and other tests check side by side: so
    // any status, not the last event, is this one's.
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Status {
            tone: Tone::Problem,
            ..
        }
    )));
}

#[test]
fn check_reads_inside_archives_and_checks_masters_against_the_load_order() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (list, downloads) = masters_fixture(root);
    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    mudcrab(root)
        .args(["compile", list.to_str().unwrap(), "--output"])
        .arg(&compiled)
        .assert()
        .success();
    mudcrab(root)
        .args([
            "query",
            compiled.to_str().unwrap(),
            "--headless",
            "--output",
        ])
        .arg(&plan)
        .assert()
        .success();

    let output = mudcrab(root)
        .args(["check", plan.to_str().unwrap(), "--downloads"])
        .arg(&downloads)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        said.contains("Absent-3-1-0.zip is not in the downloads folder"),
        "{said}"
    );
    assert!(
        said.contains("Patch (Patch.esp): needs Base.esm, which loads after it"),
        "{said}"
    );
    assert!(
        said.contains("Patch (Patch.esp): needs Elsewhere.esm, which is not in the list"),
        "{said}"
    );
    assert!(
        !said.contains("Oblivion.esm"),
        "a game plugin is present: {said}"
    );
    assert!(said.contains("3 problem(s)"), "{said}");
}

/// Compile, query and check `list` against `downloads`; (success, stdout,
/// stdout and stderr together).
fn check_list(root: &Path, list: &str, downloads: &Path) -> (bool, String, String) {
    let path = root.join("list.toml");
    std::fs::write(&path, list).unwrap();
    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    mudcrab(root)
        .args(["compile", path.to_str().unwrap(), "--output"])
        .arg(&compiled)
        .assert()
        .success();
    mudcrab(root)
        .args([
            "query",
            compiled.to_str().unwrap(),
            "--headless",
            "--output",
        ])
        .arg(&plan)
        .assert()
        .success();
    let output = mudcrab(root)
        .args(["check", plan.to_str().unwrap(), "--downloads"])
        .arg(downloads)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    let all = format!("{out}{}", String::from_utf8_lossy(&output.stderr));
    (output.status.success(), out, all)
}

#[test]
fn check_compares_the_load_order_bsas_and_actions_with_what_the_mods_install() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    let plain = plugin(&["Oblivion.esm"]);
    // Ships A.esp, an unordered Extra.esp, a BSA no plugin loads, and a
    // texture for a prune to remove.
    make_zip(
        &downloads.join("A-1-1-0.zip"),
        &[
            ("A.esp", &plain),
            ("Extra.esp", &plain),
            ("Orphan.bsa", b"BSA"),
            ("A.bsa", b"BSA"),
            ("textures/a.dds", b"dds"),
        ],
    );
    // Ships Shared.esp, as does C.
    make_zip(&downloads.join("B-2-1-0.zip"), &[("Shared.esp", &plain)]);
    make_zip(&downloads.join("C-3-1-0.zip"), &[("Shared.esp", &plain)]);
    // Recorded with an MD5 it does not have.
    make_zip(&downloads.join("D.zip"), &[("readme.txt", b"d")]);
    let list = r#"name = "L"
plugins = ["A.esp", "Renamed.esp", "Shared.esp", "Ghost.esp"]

[[mods]]
id = "A"
plugins = ["A.esp", "Renamed.esp"]
[[mods.archives]]
path = "nexus:oblivion/1/10"
file_name = "A-1-1-0.zip"
exclude = ["nothing-here/*"]
[[mods.actions]]
action = "file_prune"
paths = ["textures/missing.dds"]

[[mods]]
id = "B"
plugins = ["Shared.esp"]
[[mods.archives]]
path = "nexus:oblivion/2/20"
file_name = "B-2-1-0.zip"

[[mods]]
id = "C"
[[mods.archives]]
path = "nexus:oblivion/3/30"
file_name = "C-3-1-0.zip"

[[mods]]
id = "D"
[[mods.archives]]
path = "manual:D.zip"
size = 1
md5 = "00000000000000000000000000000000"
"#;
    let (ok, out, err) = check_list(root, list, &downloads);
    assert!(!ok);
    // Errors.
    assert!(
        err.contains("Would fail at install") && err.contains("textures/missing.dds"),
        "{err}"
    );
    assert!(
        err.contains("Ghost.esp is in the load order, but no mod installs it"),
        "{err}"
    );
    assert!(err.contains("is not the file the list records"), "{err}");
    // A's rehearsal failed, so its folder is not concluded from.
    assert!(!err.contains("Renamed.esp"), "{err}");

    // Without the bad prune, A's folder is known.
    let list = list.replace(
        "[[mods.actions]]\naction = \"file_prune\"\npaths = [\"textures/missing.dds\"]\n",
        "",
    );
    let (ok, out2, err) = check_list(root, &list, &downloads);
    assert!(!ok);
    assert!(
        err.contains("A: declares Renamed.esp, which it does not install"),
        "{err}"
    );
    assert!(
        err.contains("A: orphan.bsa -- no active plugin's name begins it"),
        "{err}"
    );
    assert!(!err.contains("a.bsa,"), "A.esp loads A.bsa: {err}");
    // Warnings, which do not fail the check by themselves.
    assert!(out2.contains("extra.esp (A)"), "{out2}");
    assert!(out2.contains("Shared.esp: C > B"), "{out2}");
    assert!(
        out2.contains("A: exclude 'nothing-here/*' leaves nothing out"),
        "{out2}"
    );
    let _ = out;
}

#[test]
fn a_plugin_from_a_mod_that_moves_it_is_traced_through_the_move() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    make_zip(
        &downloads.join("M-1-1-0.zip"),
        &[(
            "optional/Moved.esp",
            &plugin(&["Oblivion.esm", "Nowhere.esm"]),
        )],
    );
    let list = r#"name = "L"
plugins = ["Moved.esp"]

[[mods]]
id = "M"
plugins = ["Moved.esp"]
[[mods.archives]]
path = "nexus:oblivion/1/10"
file_name = "M-1-1-0.zip"
layout = "simple"
[[mods.actions]]
action = "file_move"
from = "optional/Moved.esp"
to = "Moved.esp"
"#;
    let (ok, _, err) = check_list(root, list, &downloads);
    assert!(!ok);
    assert!(
        err.contains("M (Moved.esp): needs Nowhere.esm, which is not in the list"),
        "the moved plugin's masters come from its archive: {err}"
    );
    assert!(!err.contains("declares Moved.esp"), "{err}");
}

#[test]
fn a_declared_conflict_that_overlaps_nothing_fails_as_install_would() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    make_zip(&downloads.join("X-1-1-0.zip"), &[("textures/x.dds", b"x")]);
    make_zip(&downloads.join("Y-2-1-0.zip"), &[("textures/y.dds", b"y")]);
    let list = r#"name = "L"

[[mods]]
id = "X"
[[mods.archives]]
path = "nexus:oblivion/1/10"
file_name = "X-1-1-0.zip"
[[mods.actions]]
action = "file_hide"
conflicts_with = ["Y"]

[[mods]]
id = "Y"
[[mods.archives]]
path = "nexus:oblivion/2/20"
file_name = "Y-2-1-0.zip"
"#;
    let (ok, _, err) = check_list(root, list, &downloads);
    assert!(!ok);
    assert!(
        err.contains("Would fail at install") && err.contains("selected no files"),
        "{err}"
    );
}

/// A plan from before mods' declared plugins were carried into it cannot say
/// who installs a plugin `check` cannot read; it says so rather than report
/// every such plugin as installed by nothing.
#[test]
fn an_older_plan_skips_the_load_order_comparison_and_says_why() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    // Its content is inside a second archive, so its plugin cannot be read.
    make_zip(&downloads.join("Wrapped-1-1-0.zip"), &[("inner.7z", b"7z")]);
    let list = root.join("list.toml");
    std::fs::write(
        &list,
        r#"name = "L"
plugins = ["Wrapped.esp"]

[[mods]]
id = "Wrapped"
plugins = ["Wrapped.esp"]
[[mods.archives]]
path = "nexus:oblivion/1/10"
file_name = "Wrapped-1-1-0.zip"
inner_archive = "inner.7z"
"#,
    )
    .unwrap();
    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    mudcrab(root)
        .args(["compile", list.to_str().unwrap(), "--output"])
        .arg(&compiled)
        .assert()
        .success();
    mudcrab(root)
        .args([
            "query",
            compiled.to_str().unwrap(),
            "--headless",
            "--output",
        ])
        .arg(&plan)
        .assert()
        .success();
    // As an older mudcrab wrote it: no `plugins` on the mods.
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&plan).unwrap()).unwrap();
    for entry in json["mods"].as_array_mut().unwrap() {
        entry.as_object_mut().unwrap().remove("plugins");
    }
    std::fs::write(&plan, json.to_string()).unwrap();

    let output = mudcrab(root)
        .args(["check", plan.to_str().unwrap(), "--downloads"])
        .arg(&downloads)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{said}");
    assert!(said.contains("made by an older mudcrab"), "{said}");
    assert!(!said.contains("no mod installs it"), "{said}");
}

#[test]
fn masters_only_reports_masters_and_rereads_an_archive_that_changed() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (list, downloads) = masters_fixture(root);
    let compiled = root.join("compiled.json");
    let plan = root.join("plan.json");
    let run = |args: &[&str]| {
        let output = mudcrab(root)
            .env("XDG_CACHE_HOME", root.join("cache"))
            .args(args)
            .output()
            .unwrap();
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    };
    run(&[
        "compile",
        list.to_str().unwrap(),
        "--output",
        compiled.to_str().unwrap(),
    ]);
    run(&[
        "query",
        compiled.to_str().unwrap(),
        "--headless",
        "--output",
        plan.to_str().unwrap(),
    ]);
    let check = || {
        run(&[
            "check",
            plan.to_str().unwrap(),
            "--masters-only",
            "--downloads",
            downloads.to_str().unwrap(),
        ])
    };

    let said = check();
    assert!(
        said.contains("needs Base.esm, which loads after it"),
        "{said}"
    );
    assert!(
        said.contains("needs Elsewhere.esm, which is not in the list"),
        "{said}"
    );
    // Read again from what the first check kept.
    let said = check();
    assert!(
        said.contains("needs Elsewhere.esm, which is not in the list"),
        "{said}"
    );

    // A new download of Patch, no longer needing Elsewhere, is read anew.
    make_zip(
        &downloads.join("Patch-2-1-0.zip"),
        &[(
            "Patch.esp",
            &plugin(&["Oblivion.esm", "Base.esm", "Base2.esm", "Base3.esm"]),
        )],
    );
    let said = check();
    assert!(!said.contains("Elsewhere.esm"), "{said}");
    assert!(
        said.contains("needs Base2.esm, which is not in the list"),
        "{said}"
    );
}

#[test]
fn a_master_from_another_mod_is_a_dependency() {
    use mudcrab::config::check::Kind;
    let dir = tempdir().unwrap();
    let (list, downloads) = masters_fixture(dir.path());
    let original = std::fs::read_to_string(&list).unwrap();
    let findings = |base: &str, patch: &str| -> Vec<(Kind, String)> {
        let text = original
            .replace("id = \"Base\"\n", &format!("id = \"Base\"\n{base}"))
            .replace("id = \"Patch\"\n", &format!("id = \"Patch\"\n{patch}"));
        std::fs::write(&list, text).unwrap();
        let compiled = mudcrab::config::pipeline::compile(&list, false).unwrap();
        let plan = mudcrab::config::query::build_plan(&compiled.modlist, &Default::default(), None)
            .unwrap();
        let report = mudcrab::config::check::check_all(
            &plan,
            &mudcrab::config::check::CheckSettings {
                search: Vec::new(),
                cache_dir: downloads.clone(),
                filter: Default::default(),
                masters_only: true,
                resources: None,
            },
        )
        .unwrap();
        report
            .findings
            .into_iter()
            .filter(|f| matches!(f.kind, Kind::Undeclared | Kind::OptionalMasters))
            .map(|f| (f.kind, f.item.text.to_plain()))
            .collect()
    };

    // Patch.esp's master Base.esm is Base's, which Patch does not list.
    let found = findings("", "");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, Kind::Undeclared);
    assert!(
        found[0].1.contains("Base.esm as a master, from Base"),
        "{found:?}"
    );
    assert!(findings("", "dependencies = [\"Base\"]\n").is_empty());

    // Base optional, and ticked in this plan: Patch still breaks when the
    // player unticks it.
    let optional = "optional = { default = true }\n";
    let found = findings(optional, "");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, Kind::OptionalMasters);
    assert!(found[0].1.contains("make Patch optional too"), "{found:?}");
    // Following it, it does not; it still does not list it.
    let found = findings(optional, "if = 'mod:\"Base\"'\n");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, Kind::Undeclared);
    // Optional too, and listing it: nothing to say.
    assert!(findings(optional, &format!("{optional}dependencies = [\"Base\"]\n")).is_empty());
}

#[test]
fn masters_are_written_as_dependencies() {
    let dir = tempdir().unwrap();
    let (list, downloads) = masters_fixture(dir.path());
    let text = std::fs::read_to_string(&list).unwrap().replace(
        "id = \"Patch\"\n",
        "id = \"Patch\"\ndependencies = [\"Absent\"]\n",
    );
    std::fs::write(&list, text).unwrap();
    let compiled = mudcrab::config::pipeline::compile(&list, false).unwrap();
    let plan =
        mudcrab::config::query::build_plan(&compiled.modlist, &Default::default(), None).unwrap();
    let report = mudcrab::config::check::check_all(
        &plan,
        &mudcrab::config::check::CheckSettings {
            search: Vec::new(),
            cache_dir: downloads,
            filter: Default::default(),
            masters_only: true,
            resources: None,
        },
    )
    .unwrap();
    let needs: Vec<_> = mudcrab::config::check::master_needs(&plan, &report.plugins)
        .into_iter()
        .filter(|need| !need.listed)
        .collect();
    assert_eq!(needs.len(), 1, "{needs:?}");
    assert_eq!(
        (needs[0].mod_id.as_str(), needs[0].needs.as_str()),
        ("Patch", "Base")
    );
    assert!(!needs[0].optional);

    // Written as `masters`, apart from the author's own; a plain list stays
    // the author's.
    let found = vec![("Patch".to_string(), vec!["Base".to_string()])];
    let planned = mudcrab::config::edit::plan_found_dependencies(&list, "masters", &found).unwrap();
    assert!(planned.skipped.is_empty(), "{:?}", planned.skipped);
    for edit in &planned.edits {
        edit.write().unwrap();
    }
    let written = std::fs::read_to_string(&list).unwrap();
    assert!(
        written.contains("dependencies = { masters = [\"Base\"], custom = [\"Absent\"] }"),
        "{written}"
    );
    mudcrab::config::pipeline::compile(&list, false).unwrap();

    // Found again with nothing: the group goes, the author's own stays.
    let found = vec![("Patch".to_string(), Vec::new())];
    let planned = mudcrab::config::edit::plan_found_dependencies(&list, "masters", &found).unwrap();
    for edit in &planned.edits {
        edit.write().unwrap();
    }
    let written = std::fs::read_to_string(&list).unwrap();
    assert!(written.contains("dependencies = [\"Absent\"]"), "{written}");
}

#[test]
fn the_authors_own_are_left_alone_and_loops_are_left_out() {
    let dir = tempdir().unwrap();
    let (list, _) = masters_fixture(dir.path());
    let text = std::fs::read_to_string(&list).unwrap().replace(
        "id = \"Patch\"\n",
        "id = \"Patch\"\ndependencies = [\"Base\"]\n",
    );
    std::fs::write(&list, text).unwrap();
    let found = vec![
        ("Patch".to_string(), vec!["Base".to_string()]),
        ("Base".to_string(), vec!["Patch".to_string()]),
    ];
    let planned =
        mudcrab::config::edit::plan_found_dependencies(&list, "resources", &found).unwrap();
    assert_eq!(planned.skipped.len(), 1, "{:?}", planned.skipped);
    assert!(planned.skipped[0].contains("loop"), "{:?}", planned.skipped);
    for edit in &planned.edits {
        edit.write().unwrap();
    }
    // Base, which Patch already lists itself, is not listed again; Base's
    // need of Patch would make a loop. Nothing to write.
    let written = std::fs::read_to_string(&list).unwrap();
    assert!(written.contains("dependencies = [\"Base\"]"), "{written}");
}

/// A plugin naming `models`, one static each, after a header naming
/// `masters`.
fn plugin_naming(masters: &[&str], models: &[&str]) -> Vec<u8> {
    let field = |kind: &[u8; 4], data: &[u8]| {
        let mut out = kind.to_vec();
        out.extend_from_slice(&(data.len() as u16).to_le_bytes());
        out.extend_from_slice(data);
        out
    };
    let mut records = Vec::new();
    for (i, model) in models.iter().enumerate() {
        let mut fields = field(b"EDID", format!("Thing{i}\0").as_bytes());
        fields.extend(field(b"MODL", format!("{model}\0").as_bytes()));
        records.extend_from_slice(b"STAT");
        records.extend_from_slice(&(fields.len() as u32).to_le_bytes());
        records.extend_from_slice(&0u32.to_le_bytes());
        records.extend_from_slice(&(0x0100_0800 + i as u32).to_le_bytes());
        records.extend_from_slice(&0u32.to_le_bytes());
        records.extend(fields);
    }
    let mut out = plugin(masters);
    out.extend_from_slice(b"GRUP");
    out.extend_from_slice(&(20 + records.len() as u32).to_le_bytes());
    out.extend_from_slice(b"STAT");
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend(records);
    out
}

#[test]
fn a_file_only_one_other_mod_provides_is_a_resource_dependency() {
    use mudcrab::config::check::Kind;
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    let game = root.join("game");
    std::fs::create_dir_all(game.join("Data/meshes")).unwrap();
    std::fs::write(game.join("Data/meshes/vanilla.nif"), b"nif").unwrap();
    make_zip(
        &downloads.join("Rocks-1-1-0.zip"),
        &[
            ("meshes/rocks/big.nif", b"nif"),
            ("meshes/shared.nif", b"nif"),
            ("meshes/vanilla.nif", b"nif"),
        ],
    );
    make_zip(
        &downloads.join("Other-2-1-0.zip"),
        &[("meshes/shared.nif", b"nif")],
    );
    make_zip(
        &downloads.join("Quarry-3-1-0.zip"),
        &[
            (
                "Quarry.esp",
                &plugin_naming(
                    &["Oblivion.esm"],
                    &[
                        // Only Rocks has it: Quarry needs Rocks.
                        "Rocks\\Big.nif",
                        // Rocks and Other: either would do.
                        "shared.nif",
                        // The game's own, which Rocks replaces.
                        "vanilla.nif",
                        // Its own.
                        "quarry/pit.nif",
                        // Nobody's: `assets`'s business.
                        "lost.nif",
                    ],
                ),
            ),
            ("meshes/quarry/pit.nif", b"nif"),
        ],
    );
    let list = root.join("list.toml");
    let entry = |id: &str, file: &str, n: u32| {
        format!(
            "[[mods]]\nid = \"{id}\"\n[[mods.archives]]\npath = \"nexus:oblivion/{n}/{n}0\"\nfile_name = \"{file}\"\n\n"
        )
    };
    std::fs::write(
        &list,
        format!(
            "name = \"L\"\nplugins = [\"Quarry.esp\"]\n\n{}{}{}",
            entry("Rocks", "Rocks-1-1-0.zip", 1),
            entry("Other", "Other-2-1-0.zip", 2),
            entry("Quarry", "Quarry-3-1-0.zip", 3).replace(
                "id = \"Quarry\"\n",
                "id = \"Quarry\"\nplugins = [\"Quarry.esp\"]\n"
            ),
        ),
    )
    .unwrap();
    let compiled = mudcrab::config::pipeline::compile(&list, false).unwrap();
    let plan =
        mudcrab::config::query::build_plan(&compiled.modlist, &Default::default(), None).unwrap();
    let report = mudcrab::config::check::check_all(
        &plan,
        &mudcrab::config::check::CheckSettings {
            search: Vec::new(),
            cache_dir: downloads,
            filter: Default::default(),
            masters_only: true,
            resources: Some(game),
        },
    )
    .unwrap();
    let needs = report.resources.expect("resources were asked for");
    assert_eq!(needs.len(), 1, "{needs:?}");
    assert_eq!(
        (needs[0].mod_id.as_str(), needs[0].needs.as_str()),
        ("Quarry", "Rocks")
    );
    assert_eq!(needs[0].named, "meshes/rocks/big.nif");
    let said: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.kind == Kind::UndeclaredResources)
        .map(|f| f.item.text.to_plain())
        .collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("which only Rocks provides"), "{said:?}");
    assert!(report.read.contains(&"Quarry".to_string()));
}

#[test]
fn each_mods_inis_bsas_and_shared_files_are_reported() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    make_zip(
        &downloads.join("Rocks-1-1-0.zip"),
        &[
            ("meshes/shared.nif", b"nif"),
            ("meshes/rocks.nif", b"nif"),
            ("Rocks.bsa", b"BSA"),
            ("readme.txt", b"hi"),
        ],
    );
    make_zip(
        &downloads.join("Other-2-1-0.zip"),
        &[
            ("meshes/shared.nif", b"nif"),
            ("ini/Other.ini", b"[Main]\nx=1\n"),
            ("readme.txt", b"hi"),
            ("meta.ini", b"[General]\nmodid=2\n"),
        ],
    );
    let list = root.join("list.toml");
    let entry = |id: &str, file: &str, n: u32| {
        format!(
            "[[mods]]\nid = \"{id}\"\n[[mods.archives]]\npath = \"nexus:oblivion/{n}/{n}0\"\nfile_name = \"{file}\"\n\n"
        )
    };
    std::fs::write(
        &list,
        format!(
            "name = \"L\"\n\n{}{}",
            entry("Rocks", "Rocks-1-1-0.zip", 1),
            entry("Other", "Other-2-1-0.zip", 2),
        ),
    )
    .unwrap();
    let compiled = mudcrab::config::pipeline::compile(&list, false).unwrap();
    let plan =
        mudcrab::config::query::build_plan(&compiled.modlist, &Default::default(), None).unwrap();
    let report = mudcrab::config::check::check_all(
        &plan,
        &mudcrab::config::check::CheckSettings {
            search: Vec::new(),
            cache_dir: downloads.clone(),
            filter: Default::default(),
            masters_only: true,
            resources: None,
        },
    )
    .unwrap();
    let [rocks, other] = &report.facts[..] else {
        panic!("{:?}", report.facts);
    };
    assert_eq!(rocks.bsas, ["rocks.bsa"]);
    assert!(rocks.inis.is_empty());
    assert_eq!(other.inis, ["ini/other.ini"]);
    // Other, later, replaces Rocks' one shared mesh; the readmes do not count.
    assert_eq!(rocks.losses, [("Other".to_string(), 1)]);
    assert!(rocks.wins.is_empty());
    assert_eq!(other.wins, [("Rocks".to_string(), 1)]);
    assert!(rocks.read && other.read);

    // Listings are kept between checks, but an archive that changes is
    // listed again.
    make_zip(
        &downloads.join("Other-2-1-0.zip"),
        &[
            ("meshes/shared.nif", b"nif"),
            ("ini/Other.ini", b"[Main]\nx=1\n"),
            ("ini/Extra.ini", b"[Main]\ny=2\n"),
        ],
    );
    let again = mudcrab::config::check::check_all(
        &plan,
        &mudcrab::config::check::CheckSettings {
            search: Vec::new(),
            cache_dir: downloads,
            filter: Default::default(),
            masters_only: true,
            resources: None,
        },
    )
    .unwrap();
    assert_eq!(again.facts[1].inis, ["ini/extra.ini", "ini/other.ini"]);
}

#[test]
fn an_archive_in_another_folder_is_read_where_it_is() {
    use mudcrab::config::check::Kind;
    let dir = tempdir().unwrap();
    let root = dir.path();
    let downloads = root.join("downloads");
    let elsewhere = root.join("elsewhere");
    std::fs::create_dir_all(&downloads).unwrap();
    make_zip(
        &elsewhere.join("Rocks-1-1-0.zip"),
        &[("ini/Rocks.ini", b"[Main]\nx=1\n")],
    );
    let list = root.join("list.toml");
    std::fs::write(
        &list,
        "name = \"L\"\n\n[[mods]]\nid = \"Rocks\"\n[[mods.archives]]\npath = \"nexus:oblivion/1/10\"\nfile_name = \"Rocks-1-1-0.zip\"\n",
    )
    .unwrap();
    let compiled = mudcrab::config::pipeline::compile(&list, false).unwrap();
    let plan =
        mudcrab::config::query::build_plan(&compiled.modlist, &Default::default(), None).unwrap();
    let check = |search: Vec<std::path::PathBuf>| {
        mudcrab::config::check::check_all(
            &plan,
            &mudcrab::config::check::CheckSettings {
                cache_dir: downloads.clone(),
                search,
                filter: Default::default(),
                masters_only: true,
                resources: None,
            },
        )
        .unwrap()
    };
    let missing = |report: &mudcrab::config::check::CheckReport| {
        report
            .findings
            .iter()
            .any(|f| f.kind == Kind::MissingArchive)
    };
    assert!(missing(&check(Vec::new())));
    let found = check(vec![elsewhere]);
    assert!(!missing(&found), "{:?}", found.findings);
    assert_eq!(found.facts[0].inis, ["ini/rocks.ini"]);
}
