//! Adding a mod from an archive, through the commands the page calls, on
//! Tauri's mock runtime: the archives listed, one read, its installer walked
//! a question at a time, the entry planned with the plugins it installs, and
//! added as `add` would.

use serde_json::{Value, json};
use std::io::Write;
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

fn zip(path: &std::path::Path, files: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
    for (name, bytes) in files {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

const LIST: &str = r#"name = "Adding"

plugins = [
    "Oblivion.esm",
    "Old.esp",
]

[[mods]]
id = "Old"
section = ["Core"]
plugins = ["Old.esp"]

[[mods.archives]]
path = "manual:Old.zip"
"#;

const FOMOD: &str = r#"<config>
  <moduleName>Choice</moduleName>
  <installSteps order="Explicit">
    <installStep name="Main">
      <optionalFileGroups order="Explicit">
        <group name="Size" type="SelectExactlyOne">
          <plugins order="Explicit">
            <plugin name="Big"><files><folder source="big" destination="" /></files>
              <typeDescriptor><type name="Optional" /></typeDescriptor></plugin>
            <plugin name="Small"><files><folder source="small" destination="" /></files>
              <typeDescriptor><type name="Optional" /></typeDescriptor></plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
  </installSteps>
</config>"#;

#[test]
fn a_mod_is_added_from_an_archive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // SAFETY: set before the app (or anything else) starts.
    unsafe {
        std::env::set_var("MUDCRAB_TOOLS_CONFIG", root.join("config/tools.toml"));
        std::env::set_var("XDG_DATA_HOME", root.join("data"));
        std::env::set_var("HOME", root);
        std::env::set_var("LOCALAPPDATA", root.join("data"));
        std::env::set_var("APPDATA", root.join("config"));
        // Never the real Nexus: an unknown archive stays a manual one.
        std::env::remove_var("NEXUS_API_KEY");
    }
    let list = root.join("list.toml");
    std::fs::write(&list, LIST).unwrap();
    let downloads = root.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    zip(&downloads.join("Old.zip"), &[("Old.esp", b"TES4")]);
    zip(
        &downloads.join("New Thing-12345-1-0.zip"),
        &[("New.esp", b"TES4"), ("textures/new.dds", b"x")],
    );
    zip(
        &downloads.join("Choice.zip"),
        &[
            ("fomod/ModuleConfig.xml", FOMOD.as_bytes()),
            ("big/Big.esp", b"TES4"),
            ("small/Small.esp", b"TES4"),
        ],
    );

    let app = mudcrab_gui::app(mock_builder(), None, false)
        .build(tauri::generate_context!())
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    call(&webview, "open_list", json!({ "path": list })).unwrap();
    call(
        &webview,
        "set_location",
        json!({ "kind": "downloads", "path": downloads }),
    )
    .unwrap();

    // The archives, and which the list already uses.
    let archives = call(&webview, "archives", json!({})).unwrap();
    let used = |name: &str| {
        archives
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["file_name"] == name)
            .map(|a| a["used_by"].clone())
            .unwrap()
    };
    assert_eq!(used("Old.zip"), json!(["Old"]));
    assert_eq!(used("New Thing-12345-1-0.zip"), json!([]));

    // A plain archive: nothing to ask.
    let new = downloads.join("New Thing-12345-1-0.zip");
    let started = call(&webview, "add_start", json!({ "path": new })).unwrap();
    assert_eq!(started["suggested_id"], "New Thing", "{started}");
    assert_eq!(started["has_installer"], false);
    assert_eq!(started["source"], "manual:New Thing-12345-1-0.zip");
    let walked = call(&webview, "add_walk", json!({ "path": new, "answers": [] })).unwrap();
    assert_eq!(walked["kind"], "done", "{walked}");

    let request = |plugins: Value, after: Value| {
        json!({ "request": {
            "path": new, "id": "New Thing", "section": ["Core"], "before_mod": null,
            "condition": null, "source": "manual:New Thing-12345-1-0.zip", "answers": [],
            "plugins": plugins, "load_after": after,
        }})
    };
    let plan = call(&webview, "add_plan", request(json!([]), Value::Null)).unwrap();
    assert_eq!(plan["installs_plugins"], json!(["New.esp"]), "{plan}");
    assert_eq!(plan["files"], 2);
    assert_eq!(plan["suggested_after"], "Old.esp");
    // Declaring a plugin says where it loads.
    let added = call(
        &webview,
        "add_mod",
        request(json!(["New.esp"]), json!("Old.esp")),
    )
    .unwrap();
    let command = added["change"]["command"].as_str().unwrap();
    assert!(
        command.contains(
            "--id 'New Thing' --section Core --manual --plugin New.esp --load-after Old.esp"
        ),
        "{command}"
    );
    let text = std::fs::read_to_string(&list).unwrap();
    assert!(text.contains("id = \"New Thing\""), "{text}");
    assert!(text.contains("\"Old.esp\",\n    \"New.esp\","), "{text}");
    // It can be undone like any edit.
    call(&webview, "undo", json!({})).unwrap();
    assert_eq!(std::fs::read_to_string(&list).unwrap(), LIST);

    // A FOMOD: its question, then the plugin its answer installs.
    let choice = downloads.join("Choice.zip");
    let started = call(&webview, "add_start", json!({ "path": choice })).unwrap();
    assert_eq!(started["has_installer"], true);
    let walked = call(
        &webview,
        "add_walk",
        json!({ "path": choice, "answers": [] }),
    )
    .unwrap();
    assert_eq!(
        walked["asked"]["question"]["title"], "Main: Size",
        "{walked}"
    );
    let answers = json!([{ "pick": [1] }]);
    let walked = call(
        &webview,
        "add_walk",
        json!({ "path": choice, "answers": answers }),
    )
    .unwrap();
    assert_eq!(walked["kind"], "done");
    let plan = call(
        &webview,
        "add_plan",
        json!({ "request": {
            "path": choice, "id": "Choice", "section": ["Core"], "before_mod": "Old",
            "condition": null, "source": null, "answers": answers,
            "plugins": [], "load_after": null,
        }}),
    )
    .unwrap();
    assert_eq!(plan["installs_plugins"], json!(["Small.esp"]), "{plan}");
    assert!(plan["block"].as_str().unwrap().contains("\"Small\""));
    assert_eq!(
        plan["suggested_after"],
        Value::Null,
        "nothing before Old has plugins"
    );

    // Added, then its options chosen again: the list's choice is the default.
    call(
        &webview,
        "add_mod",
        json!({ "request": {
            "path": choice, "id": "Choice", "section": ["Core"], "before_mod": null,
            "condition": null, "source": "manual:Choice.zip", "answers": answers,
            "plugins": ["Small.esp"], "load_after": null,
        }}),
    )
    .unwrap();
    let again = call(
        &webview,
        "choose_walk",
        json!({ "id": "Choice", "number": 1, "answers": [] }),
    )
    .unwrap();
    assert_eq!(again["asked"]["question"]["default"], json!([1]), "{again}");
    let chosen = call(
        &webview,
        "choose_write",
        json!({ "id": "Choice", "number": 1, "answers": [{ "pick": [0] }] }),
    )
    .unwrap();
    let command = chosen["change"]["command"].as_str().unwrap();
    assert!(
        command.starts_with("mudcrab choose ") && command.ends_with(" --mod Choice"),
        "{command}"
    );
    let text = std::fs::read_to_string(&list).unwrap();
    assert!(text.contains("options = [\"Big\"]"), "{text}");
}
