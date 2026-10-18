//! Author mode's edits, through the commands the page calls, on Tauri's mock
//! runtime: each writes the list as its CLI command would and says which
//! command that is; undo and redo step through them; an edit made outside
//! ends the history.

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

const LIST: &str = r#"name = "Edits"

plugins = [
    "Oblivion.esm",
    "A.esp",
    "B.esp",
]

[[mods]]
id = "A"
section = ["One"]
plugins = ["A.esp"]

[[mods.archives]]
path = "manual:A.zip"

[[mods]]
id = "B"
section = ["One"]
plugins = ["B.esp"]

[[mods.archives]]
path = "manual:B.zip"

[[mods]]
id = "C"
section = ["Two"]

[[mods.archives]]
path = "manual:C.zip"
"#;

#[test]
fn edits_are_the_commands_and_can_be_undone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
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
    let read = || std::fs::read_to_string(&list).unwrap();

    let app = mudcrab_gui::app(mock_builder(), None, false)
        .build(tauri::generate_context!())
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    call(&webview, "open_list", json!({ "path": list })).unwrap();

    // The load order: B before A.
    let moved = call(
        &webview,
        "move_plugin",
        json!({ "plugin": "B.esp", "before": "A.esp" }),
    )
    .unwrap();
    assert!(
        moved["change"]["command"]
            .as_str()
            .unwrap()
            .ends_with(" B.esp --before A.esp"),
        "{moved}"
    );
    assert!(read().contains("\"B.esp\",\n    \"A.esp\","));
    assert_eq!(moved["can_undo"], true);

    // A mod to another section, and before another mod.
    let moved = call(
        &webview,
        "move_mod",
        json!({ "id": "C", "section": ["One"], "beforeMod": "A" }),
    )
    .unwrap();
    assert!(
        moved["change"]["command"]
            .as_str()
            .unwrap()
            .contains("--mod C --section One --before-mod A"),
        "{moved}"
    );
    let text = read();
    assert!(text.find("id = \"C\"").unwrap() < text.find("id = \"A\"").unwrap());

    // A filter, an action and a question.
    call(
        &webview,
        "edit_filter",
        json!({ "id": "A", "number": 1, "key": "exclude", "add": ["docs/**"], "drop": [] }),
    )
    .unwrap();
    assert!(read().contains("exclude = [\"docs/**\"]"));
    let hidden = call(
        &webview,
        "add_action",
        json!({ "id": "A", "action": "file_hide", "fields": ["conflicts_with=[\"B\"]"], "condition": null }),
    )
    .unwrap();
    assert!(
        hidden["change"]["command"]
            .as_str()
            .unwrap()
            .contains("action add"),
        "{hidden}"
    );
    assert_eq!(
        call(&webview, "actions", json!({ "id": "A" })).unwrap()[0],
        "action = \"file_hide\", conflicts_with = [\"B\"]"
    );
    call(
        &webview,
        "add_input",
        json!({ "name": "big", "kind": "bool", "query": "Big?", "choices": [], "default": "false" }),
    )
    .unwrap();
    assert!(read().contains("big"));

    // A change the list's rules refuse is not written.
    let before = read();
    let refused = call(
        &webview,
        "move_plugin",
        json!({ "plugin": "Nope.esp", "before": null }),
    );
    assert!(refused.is_err());
    assert_eq!(read(), before);

    // The session's changes, then undone back to the start, and redone.
    let changes = call(&webview, "changes", json!({})).unwrap();
    assert_eq!(changes["edits"].as_array().unwrap().len(), 5, "{changes}");
    assert!(changes["diff"].as_str().unwrap().contains("Big?"));
    // An undo's diff is what it did: the edit backwards.
    let undone = call(&webview, "undo", json!({})).unwrap();
    assert!(
        undone["change"]["diff"]
            .as_str()
            .unwrap()
            .lines()
            .any(|line| line.starts_with('-') && line.contains("Big?")),
        "{undone}"
    );
    for _ in 0..4 {
        call(&webview, "undo", json!({})).unwrap();
    }
    assert_eq!(read(), LIST);
    assert!(call(&webview, "undo", json!({})).is_err());
    let redone = call(&webview, "redo", json!({})).unwrap();
    assert_eq!(redone["can_redo"], true);
    assert!(read().contains("\"B.esp\",\n    \"A.esp\","));

    // An edit made outside ends the history: undo will not write over it.
    std::fs::write(&list, read().replace("Edits", "Edited")).unwrap();
    assert!(call(&webview, "undo", json!({})).is_err());
    assert!(read().contains("Edited"));
    let changes = call(&webview, "changes", json!({})).unwrap();
    assert_eq!(changes["edits"], json!([]));
}
