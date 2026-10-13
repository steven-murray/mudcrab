//! The spike in Tauri: `spike-core` behind a few commands, drawn by the web
//! page in `ui/`.
//!
//! `spike-tauri <list.toml> [--downloads <dir>] [--mod <id>]`, `--mod` opening
//! that mod's files. It edits the list: run it
//! on a copy.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use spike_core::{Edited, PluginRow, Session, Tree};
use std::collections::BTreeSet;

/// A command's error, as the page shows it.
fn shown<T>(result: anyhow::Result<T>) -> Result<T, String> {
    result.map_err(|err| format!("{err:#}"))
}

#[tauri::command]
fn list_path(session: tauri::State<'_, Session>) -> String {
    session.list.display().to_string()
}

/// The mod `--mod` named, to open on.
struct StartMod(Option<String>);

#[tauri::command]
fn start_mod(start: tauri::State<'_, StartMod>) -> Option<String> {
    start.0.clone()
}

#[tauri::command]
fn load_order(session: tauri::State<'_, Session>) -> Result<Vec<PluginRow>, String> {
    shown(session.load_order())
}

#[tauri::command]
fn move_plugin(
    session: tauri::State<'_, Session>,
    from: usize,
    to: usize,
) -> Result<Edited, String> {
    shown(session.move_plugin_to(from, to))
}

#[tauri::command]
fn mods(session: tauri::State<'_, Session>) -> Result<Vec<String>, String> {
    shown(session.mods())
}

// Planning reads the archive, so it runs off the UI thread (an async command).
#[tauri::command]
async fn tree(
    session: tauri::State<'_, Session>,
    id: String,
    open: Option<Vec<String>>,
) -> Result<Tree, String> {
    let open: Option<BTreeSet<String>> = open.map(|open| open.into_iter().collect());
    shown(session.tree(&id, open.as_ref()))
}

#[tauri::command]
async fn toggle(
    session: tauri::State<'_, Session>,
    id: String,
    path: String,
    folder: bool,
    keep: bool,
) -> Result<Edited, String> {
    shown(session.toggle(&id, &path, folder, keep))
}

fn main() {
    // WebKitGTK's DMA-BUF renderer dies on NVIDIA under Wayland ("Error 71
    // (Protocol error) dispatching to Wayland display"); without it, it draws
    // fine. Tauri's own docs give this workaround; a release would set it only
    // where needed.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: nothing else is running yet to read the environment.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
    let mut args = std::env::args().skip(1);
    let (mut list, mut downloads, mut opening) = (None, None, None::<String>);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--downloads" => downloads = args.next().map(Into::into),
            "--mod" => opening = args.next(),
            _ => list = Some(arg),
        }
    }
    let Some(list) = list else {
        eprintln!(
            "usage: spike-tauri <list.toml> [--downloads <dir>] [--mod <id>]  (it edits the list: use a copy)"
        );
        std::process::exit(2);
    };
    let session = match Session::new(list, downloads) {
        Ok(session) => session,
        Err(err) => {
            eprintln!("{err:#}");
            std::process::exit(1);
        }
    };
    tauri::Builder::default()
        .manage(session)
        .manage(StartMod(opening))
        .invoke_handler(tauri::generate_handler![
            list_path,
            start_mod,
            load_order,
            move_plugin,
            mods,
            tree,
            toggle
        ])
        .run(tauri::generate_context!())
        .expect("the window opens");
}
