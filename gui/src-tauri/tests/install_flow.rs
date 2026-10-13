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
    }
    let list = root.join("list.toml");
    std::fs::write(&list, LIST).unwrap();
    let downloads = root.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    zip(&downloads.join("Base.zip"), &["textures/base.dds"]);
    zip(&downloads.join("Extra.zip"), &["textures/extra.dds"]);
    let instance = root.join("instance");
    let game = root.join("game");
    std::fs::create_dir_all(&instance).unwrap();
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
    let locations = call(&webview, "locations", json!({})).unwrap();
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
