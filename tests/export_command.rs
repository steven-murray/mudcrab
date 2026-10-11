//! `export`: the document every exporter reads, and where its prose comes from.

use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;
use tempfile::tempdir;

fn mudcrab() -> Command {
    let mut cmd = Command::cargo_bin("mudcrab").expect("binary builds");
    let state = std::env::temp_dir().join("mudcrab-test-state");
    cmd.env("XDG_STATE_HOME", &state)
        .env("LOCALAPPDATA", &state);
    cmd
}

const MODLIST: &str = r#"
name = "Docs"
plugins = ["Rats.esp"]
docs_file = "docs.toml"
ini.bSaveOnRest = 0
ini.SIntroSequence = ""

inputs.rats.type = "bool"
inputs.rats.query = "Install the rats?"
inputs.rats.default = true

[docs]
summary = "A tiny list."

[sections."1 - AUDIO"]
summary = "Sounds."
images = [{ path = "shots/audio.png", url = "https://example.com/audio.png", caption = "Ears" }]
mood = "loud"

[[mods]]
id = "Rats"
section = ["1 - AUDIO", "Creatures"]
if = "rats"
plugins = ["Rats.esp"]

[[mods.archives]]
path = "nexus:oblivion/52661/1000033714"
layout = "bain"
bain_subpackages = ["00 Core"]

[[mods.actions]]
action = "ini_set"
file = "ini/rats.ini"
key = "rats.swear"
value = 1

[[mods]]
id = "rootbuilder"
section = ["MO2 PLUGINS"]
type = "mo2-plugin"

[[mods.archives]]
path = "manual:rootbuilder.zip"
layout = "custom-data-folder"
data_folder = "rootbuilder"

[[mods]]
id = "Bells"
section = ["1 - AUDIO"]

[[mods.archives]]
path = "manual:bells.7z"
"#;

const DOCS_FILE: &str = r#"
[mods."Rats"]
summary = "Rats that swear."
body = "Optional, as the guide says."
"#;

fn write_list(dir: &Path, modlist: &str, docs: &str) -> std::path::PathBuf {
    std::fs::create_dir_all(dir.join("shots")).unwrap();
    std::fs::write(dir.join("shots/audio.png"), b"png").unwrap();
    std::fs::write(dir.join("docs.toml"), docs).unwrap();
    let path = dir.join("modlist.toml");
    std::fs::write(&path, modlist).unwrap();
    path
}

fn export(path: &Path, extra: &[&str]) -> Result<Value, String> {
    let out = mudcrab()
        .arg("export")
        .arg(path)
        .args(extra)
        .output()
        .expect("runs");
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(serde_json::from_slice(&out.stdout).expect("json on stdout"))
}

#[test]
fn json_is_a_tree_of_sections_with_prose_and_words() {
    let dir = tempdir().unwrap();
    let doc = export(&write_list(dir.path(), MODLIST, DOCS_FILE), &[]).unwrap();

    assert_eq!(doc["docs"]["summary"], "A tiny list.");
    assert_eq!(doc["inputs"][0]["query"], "Install the rats?");
    assert_eq!(doc["mod_count"], 2);

    let audio = &doc["entries"][0];
    assert_eq!(audio["kind"], "section");
    assert_eq!(audio["anchor"], "1-audio");
    assert_eq!(
        audio["docs"]["mood"], "loud",
        "unknown keys reach templates"
    );
    assert_eq!(audio["docs"]["images"][0]["caption"], "Ears");

    // Nested section first, then the mod that follows it in install order.
    let creatures = &audio["entries"][0];
    assert_eq!(creatures["name"], "1 - AUDIO - Creatures");
    assert_eq!(creatures["title"], "Creatures");
    assert_eq!(creatures["depth"], 2);
    assert_eq!(audio["entries"][1]["id"], "Bells");

    let rats = &creatures["entries"][0];
    assert_eq!(
        rats["docs"]["summary"], "Rats that swear.",
        "from the docs file"
    );
    assert_eq!(
        rats["condition"]["text"],
        "you answer yes to “Install the rats?”"
    );
    assert_eq!(
        rats["pages"][0]["mod_url"],
        "https://www.nexusmods.com/oblivion/mods/52661"
    );
    assert_eq!(rats["archives"][0]["instructions"][0]["kind"], "bain");
    assert_eq!(rats["steps"][0]["kind"], "ini");
    assert_eq!(
        doc["kinds"],
        // Tools and game settings first, then the mods in order.
        serde_json::json!(["manual", "mo2_plugin", "ini", "optional", "bain"])
    );
    assert_eq!(
        rats["archives"][0]["instructions"][0]["text"],
        "In the BAIN installer, select `00 Core`."
    );
    assert_eq!(
        rats["steps"][0]["text"],
        "In `ini/rats.ini`, set `rats.swear` to `1`."
    );
    assert_eq!(doc["load_order"][0]["mod"], "Rats");
}

#[test]
fn a_plan_narrows_the_document_to_one_build() {
    let dir = tempdir().unwrap();
    let list = write_list(dir.path(), MODLIST, DOCS_FILE);
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    mudcrab()
        .args(["compile"])
        .arg(&list)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();
    mudcrab()
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&plan)
        .args(["--headless", "--set", "rats=false"])
        .assert()
        .success();

    let doc = export(&list, &["--plan", plan.to_str().unwrap()]).unwrap();
    assert_eq!(doc["mod_count"], 1);
    assert_eq!(doc["entries"][0]["entries"][0]["id"], "Bells");
    assert_eq!(doc["inputs"][0]["answer"], false);
    assert_eq!(doc["load_order"], serde_json::json!([]));
}

#[test]
fn docs_that_would_vanish_are_refused() {
    let dir = tempdir().unwrap();
    let cases = [
        (
            MODLIST.replace("[sections.\"1 - AUDIO\"]", "[sections.\"1 - AUDI\"]"),
            DOCS_FILE.to_string(),
            "no mod is in",
        ),
        (
            MODLIST.replace("shots/audio.png", "shots/missing.png"),
            DOCS_FILE.to_string(),
            "does not exist",
        ),
        (
            MODLIST.to_string(),
            DOCS_FILE.replace("[mods.\"Rats\"]", "[mods.\"Ratz\"]"),
            "does not have",
        ),
        (
            MODLIST.replace("if = \"rats\"", "if = \"rats\"\ndocs.summary = \"twice\""),
            DOCS_FILE.to_string(),
            "both the modlist and the docs file",
        ),
    ];
    for (modlist, docs, expected) in cases {
        let err = export(&write_list(dir.path(), &modlist, &docs), &[]).unwrap_err();
        assert!(err.contains(expected), "{expected}: {err}");
    }
}

fn export_text(path: &Path, extra: &[&str]) -> Result<(String, String), String> {
    let out = mudcrab()
        .arg("export")
        .arg(path)
        .args(extra)
        .output()
        .expect("runs");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if !out.status.success() {
        return Err(stderr);
    }
    Ok((String::from_utf8(out.stdout).expect("utf-8"), stderr))
}

#[test]
fn nexus_writes_bbcode_with_links_conditions_and_steps() {
    let dir = tempdir().unwrap();
    let list = write_list(dir.path(), MODLIST, DOCS_FILE);
    let (bbcode, _) = export_text(&list, &["--to", "nexus"]).unwrap();
    assert!(
        bbcode.contains("[size=5][b]1 - AUDIO[/b][/size]"),
        "{bbcode}"
    );
    assert!(
        bbcode.contains(
            "1. [b]Rats[/b] ❓ [url=https://www.nexusmods.com/oblivion/mods/52661]🌐[/url] \
             [url=https://www.nexusmods.com/oblivion/mods/52661?tab=files&file_id=1000033714]⬇️[/url]\n\
             [i]❓ Optional: Install the rats?[/i]"
        ),
        "{bbcode}"
    );
    assert!(
        bbcode.contains("[img]https://example.com/audio.png[/img]"),
        "{bbcode}"
    );
    assert!(
        bbcode.contains("[*]📋 In the BAIN installer, select [font=Courier New]00 Core[/font]."),
        "{bbcode}"
    );
    assert!(bbcode.contains("Optional, as the guide says."), "{bbcode}");
    assert!(
        bbcode.contains("[spoiler]\nRats.esp\n[/spoiler]"),
        "{bbcode}"
    );

    let (lean, _) = export_text(
        &list,
        &[
            "--to",
            "nexus",
            "--opt",
            "load_order=false",
            "--opt",
            "steps=no",
        ],
    )
    .unwrap();
    assert!(
        !lean.contains("[spoiler]") && !lean.contains("BAIN"),
        "{lean}"
    );

    let err = export_text(&list, &["--to", "nexus", "--opt", "load_ordr=false"]).unwrap_err();
    assert!(
        err.contains("no such option") && err.contains("load_order"),
        "{err}"
    );
}

#[test]
fn an_image_without_a_url_is_left_out_of_nexus_and_said_so() {
    let dir = tempdir().unwrap();
    let list = write_list(
        dir.path(),
        &MODLIST.replace(", url = \"https://example.com/audio.png\"", ""),
        DOCS_FILE,
    );
    let (bbcode, stderr) = export_text(&list, &["--to", "nexus"]).unwrap();
    assert!(!bbcode.contains("[img]"), "{bbcode}");
    assert!(
        stderr.contains("section 1 - AUDIO: image shots/audio.png has no url"),
        "{stderr}"
    );
}

#[test]
fn a_folder_can_override_one_template_of_a_builtin() {
    let dir = tempdir().unwrap();
    let list = write_list(dir.path(), MODLIST, DOCS_FILE);
    let custom = dir.path().join("exporters").join("mine");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::write(
        custom.join("exporter.toml"),
        "name = \"mine\"\nbase = \"nexus\"\n\n[options.stars]\ndefault = 3\n",
    )
    .unwrap();
    // Replaces how a mod is written; the built-in guide and section templates
    // find this one, because includes go through the same layers.
    std::fs::write(
        custom.join("mod.j2"),
        "MOD {{ mod.title }} {{ '*' * options.stars }}\n",
    )
    .unwrap();

    let (text, _) = export_text(&list, &["--to", "mine", "--opt", "stars=2"]).unwrap();
    assert!(
        text.contains("MOD Rats **") && text.contains("MOD Bells **"),
        "{text}"
    );
    assert!(
        text.contains("[size=5][b]1 - AUDIO[/b][/size]"),
        "the rest is still nexus: {text}"
    );

    // Options default from the modlist's [export.<name>].
    let with_defaults = format!("{MODLIST}\n[export.mine]\nstars = 1\n");
    let list = write_list(dir.path(), &with_defaults, DOCS_FILE);
    let (text, _) = export_text(&list, &["--to", "mine"]).unwrap();
    assert!(text.contains("MOD Rats *\n"), "{text}");
}

#[test]
fn a_template_can_extend_a_builtin_and_eject_copies_one_out() {
    let dir = tempdir().unwrap();
    let list = write_list(dir.path(), MODLIST, DOCS_FILE);
    let ejected = dir.path().join("my-nexus");
    let out = mudcrab()
        .args(["export", "--to", "nexus", "--eject"])
        .arg(&ejected)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(ejected.join("mod.j2").is_file() && ejected.join("exporter.toml").is_file());

    // An ejected copy runs as it stands...
    let (original, _) = export_text(&list, &["--to", "nexus"]).unwrap();
    let (copy, _) = export_text(&list, &["--to", ejected.to_str().unwrap()]).unwrap();
    assert_eq!(original, copy);

    // ...and ejecting over an existing folder is refused.
    let again = mudcrab()
        .args(["export", "--to", "nexus", "--eject"])
        .arg(&ejected)
        .output()
        .unwrap();
    assert!(!again.status.success());

    // A template that wraps the built-in guide rather than replacing it.
    let wrap = dir.path().join("wrap");
    std::fs::create_dir_all(&wrap).unwrap();
    std::fs::write(
        wrap.join("exporter.toml"),
        "name = \"wrap\"\nbase = \"nexus\"\n\n[[pages]]\ntemplate = \"outer.j2\"\n",
    )
    .unwrap();
    std::fs::write(
        wrap.join("outer.j2"),
        "HEADER\n{% include \"builtin/nexus/guide.j2\" %}FOOTER\n",
    )
    .unwrap();
    let (text, _) = export_text(&list, &["--to", wrap.to_str().unwrap()]).unwrap();
    assert!(
        text.starts_with("HEADER\n") && text.ends_with("FOOTER\n") && text.contains("[url="),
        "{text}"
    );
}

#[test]
fn markdown_is_one_file_with_headings_by_depth() {
    let dir = tempdir().unwrap();
    let list = write_list(dir.path(), MODLIST, DOCS_FILE);
    let (md, _) = export_text(&list, &["--to", "markdown"]).unwrap();
    assert!(md.starts_with("# Docs\n"), "{md}");
    assert!(md.contains("- [1 - AUDIO](#1-audio)"), "{md}");
    assert!(md.contains("\n## 1 - AUDIO\n"), "{md}");
    assert!(md.contains("\n### Creatures\n"), "{md}");
    assert!(
        md.contains(
            "\n#### 1. Rats ❓ [🌐](https://www.nexusmods.com/oblivion/mods/52661 \"Nexus mod page\") \
             [⬇️](https://www.nexusmods.com/oblivion/mods/52661?tab=files&file_id=1000033714 \
             \"nexus:oblivion/52661/1000033714\")\n"
        ),
        "{md}"
    );
    assert!(
        md.contains("![Ears](https://example.com/audio.png)"),
        "{md}"
    );
    assert!(
        md.contains("- ⚙️ In `ini/rats.ini`, set `rats.swear` to `1`."),
        "{md}"
    );
}

#[test]
fn github_pages_writes_a_site_with_its_images() {
    let dir = tempdir().unwrap();
    let list = write_list(dir.path(), MODLIST, DOCS_FILE);
    let site = dir.path().join("site");

    let err = export_text(&list, &["--to", "github-pages"]).unwrap_err();
    assert!(err.contains("-o"), "a site needs a directory: {err}");

    export_text(
        &list,
        &["--to", "github-pages", "-o", site.to_str().unwrap()],
    )
    .unwrap();
    let read =
        |name: &str| std::fs::read_to_string(site.join(name)).unwrap_or_else(|_| panic!("{name}"));

    assert!(read("_config.yml").contains("title: \"Docs\""));
    let index = read("index.md");
    assert!(
        index.starts_with("---\nlayout: default\ntitle: \"Docs\"\n---\n{% raw %}\n"),
        "{index}"
    );
    assert!(
        index.contains("1. [1 - AUDIO](1-audio.md) — Sounds."),
        "{index}"
    );
    assert!(
        index.trim_end().ends_with("{% endraw %}"),
        "Jekyll must not read the content as Liquid"
    );

    let audio = read("1-audio.md");
    assert!(
        audio.contains("\n# 1 - AUDIO\n") && audio.contains("\n## Creatures\n"),
        "{audio}"
    );
    // A site holds files, so the local copy is used, not the url.
    assert!(audio.contains("![Ears](images/shots-audio.png)"), "{audio}");
    assert_eq!(
        std::fs::read(site.join("images/shots-audio.png")).unwrap(),
        b"png"
    );
    assert!(read("load-order.md").contains("1 plugins, in order."));
}

#[test]
fn tools_and_game_settings_come_before_the_mods() {
    let dir = tempdir().unwrap();
    let list = write_list(dir.path(), MODLIST, DOCS_FILE);
    let doc = export(&list, &[]).unwrap();
    assert_eq!(doc["tools"][0]["id"], "rootbuilder");
    assert_eq!(
        doc["tools"][0]["archives"][0]["instructions"][0]["kind"],
        "mo2_plugin"
    );
    // An MO2 plugin is not a mod in the list, so its section does not appear.
    assert!(
        doc["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["name"] != "MO2 PLUGINS")
    );
    assert_eq!(doc["game_settings"].as_array().unwrap().len(), 2);

    let (md, _) = export_text(&list, &["--to", "markdown"]).unwrap();
    let tools = md.find("## Tools").expect("tools section");
    let settings = md.find("## Game settings").expect("settings section");
    let audio = md.find("\n## 1 - AUDIO").expect("audio section");
    assert!(tools < settings && settings < audio, "{md}");
    assert!(
        md.contains("- 🧰 Extract `rootbuilder` into Mod Organizer 2's `plugins/` folder"),
        "{md}"
    );
    assert!(md.contains("set `bSaveOnRest` to `0`."), "{md}");
    assert!(
        md.contains("set `SIntroSequence` to nothing (leave it empty)."),
        "{md}"
    );
    assert!(
        md.contains("This whole install can be done automatically"),
        "{md}"
    );
    assert!(
        !md.contains("## Choices"),
        "a manual guide asks each question where it applies: {md}"
    );
}

#[test]
fn a_guides_closing_text_is_a_page_of_its_own_when_there_is_one() {
    let dir = tempdir().unwrap();
    let site = dir.path().join("site");
    let list = write_list(dir.path(), MODLIST, DOCS_FILE);
    export_text(&list, &["--to", "github-pages", "-o", site.to_str().unwrap()]).unwrap();
    assert!(!site.join("closing.md").exists(), "no footer, no page");

    let with_footer = MODLIST.replace(
        "[docs]\nsummary = \"A tiny list.\"",
        "[docs]\nsummary = \"A tiny list.\"\nfooter = \"Thanks for playing.\"\nfooter_title = \"Epilogue\"",
    );
    assert_ne!(with_footer, MODLIST);
    let site = dir.path().join("site2");
    let list = write_list(dir.path(), &with_footer, DOCS_FILE);
    export_text(&list, &["--to", "github-pages", "-o", site.to_str().unwrap()]).unwrap();
    let closing = std::fs::read_to_string(site.join("closing.md")).unwrap();
    assert!(closing.contains("# Epilogue\n\nThanks for playing."), "{closing}");
    let index = std::fs::read_to_string(site.join("index.md")).unwrap();
    assert!(index.contains("- [Epilogue](closing.md)") && !index.contains("Thanks for playing"), "{index}");
    let contents = index.find("## Contents").unwrap();
    assert!(contents < index.find("## Key").unwrap_or(usize::MAX), "contents come first: {index}");
}
