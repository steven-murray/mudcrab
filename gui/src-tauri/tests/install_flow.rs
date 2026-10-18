//! Install mode end to end, through the commands the page calls, on Tauri's
//! mock runtime: open a list, say where things go, plan with an answer,
//! download (the archives are already there), check, install into an MO2
//! instance, and read the steps left by hand.

use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};

fn call(
    webview: &WebviewWindow<tauri::test::MockRuntime>,
    cmd: &str,
    args: Value,
) -> Result<Value, Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            // Where the app's page is served from, on each platform.
            url: if cfg!(windows) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize::<Value>().unwrap())
}

fn zip(path: &std::path::Path, files: &[&str]) {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
    for name in files {
        zip.start_file(*name, options).unwrap();
        zip.write_all(b"x").unwrap();
    }
    zip.finish().unwrap();
}

const LIST: &str = r#"name = "Flow"
inputs.extra.type = "bool"
inputs.extra.query = "Install the extra?"
inputs.extra.default = false

plugins = ["Oblivion.esm"]

[[mods]]
id = "Base"
section = ["Core"]

[[mods.archives]]
path = "manual:Base.zip"

[[mods]]
id = "Extra"
section = ["Core"]
if = "extra"

[[mods.archives]]
path = "manual:Extra.zip"

[[manual-steps]]
title = "look at it"
steps = "1. Look.\n2. Smile."
"#;

#[test]
fn a_list_installs_from_the_commands_the_page_calls() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // Everything the app keeps goes under the test's folder.
    // SAFETY: set before the app (or anything else) starts.
    unsafe {
        std::env::set_var("MUDCRAB_TOOLS_CONFIG", root.join("config/tools.toml"));
        std::env::set_var("XDG_DATA_HOME", root.join("data"));
        std::env::set_var("HOME", root);
        std::env::set_var("LOCALAPPDATA", root.join("data"));
        std::env::set_var("APPDATA", root.join("config"));
        std::env::remove_var("NEXUS_API_KEY");
    }
    let list = root.join("list.toml");
    std::fs::write(&list, LIST).unwrap();
    let downloads = root.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    // Base's archive is only in another folder, which the list is told to
    // look in.
    let elsewhere = root.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    zip(&elsewhere.join("Base.zip"), &["textures/base.dds"]);
    zip(&downloads.join("Extra.zip"), &["textures/extra.dds"]);
    let instance = root.join("instance");
    let game = root.join("game");
    std::fs::create_dir_all(&instance).unwrap();
    std::fs::write(instance.join("ModOrganizer.exe"), b"").unwrap();
    std::fs::create_dir_all(game.join("Data")).unwrap();
    std::fs::write(game.join("Oblivion.ini"), "[General]\n").unwrap();

    // The app's own context, so its commands are allowed as in the real app
    // (a mock context grants nothing).
    let app = mudcrab_gui::app(mock_builder(), None, false)
        .build(tauri::generate_context!())
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let opened = call(&webview, "open_list", json!({ "path": list })).unwrap();
    assert_eq!(opened["document"]["name"], "Flow");
    assert_eq!(opened["document"]["mod_count"], 2);
    assert_eq!(opened["document"]["inputs"][0]["id"], "extra");

    // Nothing to install into yet.
    assert!(call(&webview, "step_install", json!({})).is_err());
    // A folder that is not an MO2 instance is refused.
    let refused = call(
        &webview,
        "set_location",
        json!({ "kind": "instance", "path": game }),
    );
    assert!(
        refused
            .unwrap_err()
            .as_str()
            .unwrap()
            .contains("ModOrganizer.exe")
    );
    for (kind, path) in [
        ("instance", &instance),
        ("downloads", &downloads),
        ("game", &game),
    ] {
        call(
            &webview,
            "set_location",
            json!({ "kind": kind, "path": path }),
        )
        .unwrap();
    }
    call(
        &webview,
        "add_search_folder",
        json!({ "path": root.join("gone") }),
    )
    .unwrap();
    call(
        &webview,
        "remove_search_folder",
        json!({ "path": root.join("gone") }),
    )
    .unwrap();
    call(&webview, "add_search_folder", json!({ "path": elsewhere })).unwrap();
    let locations = call(&webview, "locations", json!({})).unwrap();
    assert_eq!(
        locations["search"].as_array().unwrap().len(),
        1,
        "{locations}"
    );
    assert_eq!(locations["search"][0]["exists"], true);
    assert_eq!(locations["tools"]["from"], "MUDCRAB_TOOLS_CONFIG");
    assert_eq!(
        locations["tools"]["problems"],
        json!([]),
        "the list cleans nothing"
    );
    let other = root.join("other-tools.toml");
    std::fs::write(&other, "downloads_dir = '/nowhere'\n").unwrap();
    let chosen = call(&webview, "set_tools_config", json!({ "path": other })).unwrap();
    assert_eq!(chosen["tools"]["from"], "chosen");
    assert_eq!(chosen["tools"]["exists"], true);
    let usual = call(&webview, "set_tools_config", json!({ "path": null })).unwrap();
    assert_eq!(usual["tools"]["from"], "MUDCRAB_TOOLS_CONFIG");
    // The Nexus API key, kept in the tools.toml in use, and taken out again.
    let keyed = call(&webview, "set_api_key", json!({ "key": "abc" })).unwrap();
    assert_eq!(keyed["tools"]["nexus_key"], "tools.toml", "{keyed}");
    assert!(
        std::fs::read_to_string(root.join("config/tools.toml"))
            .unwrap()
            .contains("nexus_api_key = \"abc\"")
    );
    let unkeyed = call(&webview, "set_api_key", json!({ "key": null })).unwrap();
    assert_eq!(unkeyed["tools"]["nexus_key"], Value::Null);
    assert_eq!(locations["instance"]["from"], "chosen");
    assert_eq!(locations["downloads"]["exists"], true);

    let planned = call(
        &webview,
        "step_plan",
        json!({ "answers": { "extra": "true" } }),
    )
    .unwrap();
    assert!(
        planned["summary"]
            .as_str()
            .unwrap()
            .starts_with("2 of 2 mod(s)"),
        "{planned}"
    );
    assert!(
        planned["command"]
            .as_str()
            .unwrap()
            .contains("--set extra=true"),
        "{planned}"
    );

    let downloaded = call(&webview, "step_download", json!({})).unwrap();
    assert_eq!(downloaded["by_hand"], json!([]), "{downloaded}");
    assert!(
        downloads.join("Base.zip").is_file(),
        "linked in from the other folder"
    );
    assert!(
        downloaded["done"]["command"]
            .as_str()
            .unwrap()
            .contains(" --archive-search-path "),
        "{downloaded}"
    );

    let checked = call(&webview, "step_check", json!({})).unwrap();
    assert_eq!(checked["problems"], 0, "{checked}");

    let installed = call(&webview, "step_install", json!({})).unwrap();
    assert!(
        installed["command"]
            .as_str()
            .unwrap()
            .starts_with("mudcrab install")
    );
    assert!(instance.join("mods/Extra/textures/extra.dds").is_file());
    assert!(instance.join("ModOrganizer.ini").is_file());

    let steps = call(&webview, "manual_steps", json!({})).unwrap();
    assert_eq!(steps[0]["title"], "look at it");
    assert_eq!(
        steps[0]["done"],
        Value::Null,
        "no plugin to tell it is done by"
    );
    assert_eq!(steps[0]["steps"], json!(["1. Look.", "2. Smile."]));

    // Author mode reads the same plan. Check has read the masters.
    let order = call(&webview, "load_order", json!({})).unwrap();
    assert_eq!(order["masters_read"], true, "{order}");
    assert_eq!(order["plugins"][0]["name"], "Oblivion.esm");
    let base = call(&webview, "mod_view", json!({ "id": "Base" })).unwrap();
    assert_eq!(base["archives"][0]["label"], "Base.zip", "{base}");
    let tree = call(&webview, "mod_files", json!({ "id": "Base", "number": 1 })).unwrap();
    assert_eq!(tree["taken"], 1, "{tree}");
    let rows: Vec<&str> = tree["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    assert_eq!(rows, ["textures", "textures/base.dds"]);
    assert_eq!(tree["rows"][1]["destination"], "textures/base.dds");
    // A folder too, so it can be moved as one (`file_move`).
    assert_eq!(tree["rows"][0]["destination"], "textures");
    // Moved within the mod, the file is shown where it ends up.
    call(
        &webview,
        "add_action",
        json!({ "id": "Base", "action": "file_move",
                "fields": ["from=\"textures\"", "to=\"optional/textures\""], "condition": null }),
    )
    .unwrap();
    let tree = call(&webview, "mod_files", json!({ "id": "Base", "number": 1 })).unwrap();
    assert_eq!(
        tree["installed"],
        json!([{ "path": "optional/textures/base.dds", "laid_out": "textures/base.dds", "hidden": false }]),
        "{tree}"
    );
    call(&webview, "undo", json!({})).unwrap();
    // Both mods install a texture of their own name, so share nothing.
    let conflicts = call(&webview, "conflicts", json!({})).unwrap();
    assert_eq!(conflicts["sweep"]["installed"], 2, "{conflicts}");
    assert_eq!(conflicts["sweep"]["mods"], json!([]));
    let between = call(
        &webview,
        "conflicts_between",
        json!({ "id": "Base", "other": "Extra" }),
    )
    .unwrap();
    assert_eq!(between["shared"]["overlap"], json!([]), "{between}");
    assert!(
        between["command"]
            .as_str()
            .unwrap()
            .contains("--mod Base --with Extra")
    );
    // A list changed on disk is planned again, with the answers given.
    let text = std::fs::read_to_string(&list).unwrap();
    std::fs::write(&list, text.replace("name = \"Flow\"", "name = \"Flowing\"")).unwrap();
    let filetime = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(&list)
        .unwrap()
        .set_modified(filetime)
        .unwrap();
    let base = call(&webview, "mod_view", json!({ "id": "Extra" })).unwrap();
    assert_eq!(
        base["left_out"], false,
        "the remembered answer still brings it in"
    );

    // The answers are remembered for next time.
    let again = call(&webview, "open_list", json!({ "path": list })).unwrap();
    assert_eq!(again["answers"]["extra"], "true");
    // Opening a page is only ever a web page.
    assert!(
        call(
            &webview,
            "open_page",
            json!({ "url": "file:///etc/passwd" })
        )
        .is_err()
    );
}
