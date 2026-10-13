//! The spike in Slint: `spike-core` behind the window in `ui/app.slint`.
//!
//! `spike-slint <list.toml> [--downloads <dir>] [--mod <id>]`, `--mod` opening
//! that mod's files. It edits the list: run it
//! on a copy.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use spike_core::{Edited, FileRow, Session, Ticked};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

slint::include_modules!();

/// What the window shows of the files view, kept between events.
#[derive(Default)]
struct Files {
    mod_id: Option<String>,
    open: BTreeSet<String>,
    rows: Vec<FileRow>,
}

fn show_status(app: &App, text: impl Into<SharedString>, bad: bool) {
    app.set_status(text.into());
    app.set_status_bad(bad);
}

fn show_edit(app: &App, edited: &Edited) {
    app.set_command(edited.command.clone().into());
    let lines: Vec<DiffLine> = edited
        .diff
        .lines()
        .map(|line| DiffLine {
            text: line.into(),
            kind: if line.starts_with("@@") {
                3
            } else if line.starts_with('+') && !line.starts_with("+++") {
                1
            } else if line.starts_with('-') && !line.starts_with("---") {
                2
            } else {
                0
            },
        })
        .collect();
    app.set_diff(ModelRc::new(VecModel::from(lines)));
}

fn load_order(app: &App, session: &Session) -> Vec<String> {
    let started = std::time::Instant::now();
    match session.load_order() {
        Ok(rows) => {
            let names = rows.iter().map(|row| row.name.clone()).collect();
            let items: Vec<PluginItem> = rows
                .into_iter()
                .map(|row| PluginItem {
                    name: row.name.into(),
                    mods: if row.mods.is_empty() {
                        "—".into()
                    } else {
                        row.mods.join(", ").into()
                    },
                    inactive: row.inactive,
                })
                .collect();
            app.set_plugins(ModelRc::new(VecModel::from(items)));
            show_status(
                app,
                format!("Load order read in {} ms.", started.elapsed().as_millis()),
                false,
            );
            names
        }
        Err(err) => {
            show_status(app, format!("{err:#}"), true);
            Vec::new()
        }
    }
}

/// Plan the tree on a worker thread, then show it.
fn load_tree(app: &App, session: &Session, files: &Arc<Mutex<Files>>) {
    let (Some(id), open) = ({
        let files = files.lock().expect("not poisoned");
        (files.mod_id.clone(), files.open.clone())
    }) else {
        return;
    };
    show_status(app, format!("Planning {id}…"), false);
    let weak = app.as_weak();
    let session = session.clone();
    let files = files.clone();
    std::thread::spawn(move || {
        let result = session.tree(&id, Some(&open));
        let _ = slint::invoke_from_event_loop(move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            match result {
                Ok(tree) => {
                    let items: Vec<FileItem> = tree
                        .rows
                        .iter()
                        .map(|row| FileItem {
                            name: row.name.clone().into(),
                            depth: row.depth as i32,
                            folder: row.folder,
                            state: match row.state {
                                Ticked::None => 0,
                                Ticked::Some => 1,
                                Ticked::All => 2,
                            },
                            open: open.contains(&row.path),
                            detail: if row.folder {
                                format!("{} of {}", row.taken, row.files).into()
                            } else {
                                row.destination
                                    .as_ref()
                                    .map(|to| format!("→ {to}"))
                                    .unwrap_or_default()
                                    .into()
                            },
                        })
                        .collect();
                    app.set_files(ModelRc::new(VecModel::from(items)));
                    app.set_tree_label(
                        format!(
                            "{}: {} of {} files taken · planned in {} ms",
                            tree.label, tree.taken, tree.entries, tree.millis
                        )
                        .into(),
                    );
                    show_status(&app, format!("Planned {id} in {} ms.", tree.millis), false);
                    files.lock().expect("not poisoned").rows = tree.rows;
                }
                Err(err) => {
                    app.set_files(ModelRc::new(VecModel::from(Vec::<FileItem>::new())));
                    show_status(&app, format!("{err:#}"), true);
                }
            }
        });
    });
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut list, mut downloads, mut start_mod) = (None, None, None::<String>);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--downloads" => downloads = args.next().map(Into::into),
            "--mod" => start_mod = args.next(),
            _ => list = Some(arg),
        }
    }
    let Some(list) = list else {
        eprintln!(
            "usage: spike-slint <list.toml> [--downloads <dir>] [--mod <id>]  (it edits the list: use a copy)"
        );
        std::process::exit(2);
    };
    let session = Session::new(list, downloads)?;
    let app = App::new()?;
    app.set_list_path(session.list.display().to_string().into());

    let names = Arc::new(Mutex::new(load_order(&app, &session)));
    let files = Arc::new(Mutex::new(Files::default()));

    app.on_move_plugin({
        let (weak, session, names) = (app.as_weak(), session.clone(), names.clone());
        move |from, to| {
            let Some(app) = weak.upgrade() else { return };
            let plugin = names
                .lock()
                .expect("not poisoned")
                .get(from as usize)
                .cloned()
                .unwrap_or_default();
            match session.move_plugin_to(from as usize, to as usize) {
                Ok(edited) => {
                    show_edit(&app, &edited);
                    *names.lock().expect("not poisoned") = load_order(&app, &session);
                    show_status(
                        &app,
                        format!(
                            "Moved {plugin} in {} ms (validated and written).",
                            edited.millis
                        ),
                        false,
                    );
                }
                Err(err) => show_status(&app, format!("{err:#}"), true),
            }
        }
    });

    app.on_choose_mod({
        let (weak, session, files) = (app.as_weak(), session.clone(), files.clone());
        move |id| {
            let Some(app) = weak.upgrade() else { return };
            {
                let mut files = files.lock().expect("not poisoned");
                files.mod_id = Some(id.trim().to_string());
                files.open.clear();
            }
            load_tree(&app, &session, &files);
        }
    });

    app.on_twisty({
        let (weak, session, files) = (app.as_weak(), session.clone(), files.clone());
        move |index| {
            let Some(app) = weak.upgrade() else { return };
            {
                let mut files = files.lock().expect("not poisoned");
                let Some(row) = files.rows.get(index as usize).cloned() else {
                    return;
                };
                if !files.open.remove(&row.path) {
                    files.open.insert(row.path);
                }
            }
            load_tree(&app, &session, &files);
        }
    });

    app.on_toggle_file({
        let (weak, session, files) = (app.as_weak(), session.clone(), files.clone());
        move |index| {
            let Some(app) = weak.upgrade() else { return };
            let (id, row) = {
                let files = files.lock().expect("not poisoned");
                let (Some(id), Some(row)) = (
                    files.mod_id.clone(),
                    files.rows.get(index as usize).cloned(),
                ) else {
                    return;
                };
                (id, row)
            };
            let keep = row.state != Ticked::All;
            match session.toggle(&id, &row.path, row.folder, keep) {
                Ok(edited) => {
                    show_edit(&app, &edited);
                    load_tree(&app, &session, &files);
                }
                Err(err) => show_status(&app, format!("{err:#}"), true),
            }
        }
    });

    if let Some(id) = start_mod {
        app.set_view(1);
        app.invoke_choose_mod(id.into());
    }

    app.run()?;
    Ok(())
}
