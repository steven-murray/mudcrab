//! Author mode, editing: every change to the list is one of the editing
//! commands' (`plugins move`, `move`, `files`, `set`, `action`, `input`),
//! planned by the same library call, validated, and written only if the file
//! has not changed on disk since it was read. Each comes back with the
//! command that does the same and the diff it made.
//!
//! The session keeps every edit, with the file it wrote (the list, or its docs
//! file) and the text before and after, so it can be undone and redone --
//! writing the earlier or later text, and only while the file is still what
//! this window last wrote. An edit made outside ends the history there. See
//! docs/design/gui.md.

use super::{State, quote, shown};
use mudcrab::config::add::{Placement, plan_move};
use mudcrab::config::edit::{self, Edit, doc};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// One file's text before and after an edit.
#[derive(Clone)]
pub(super) struct FileChange {
    path: PathBuf,
    before: String,
    after: String,
}

/// One edit, as the session remembers it: the files it wrote, in the order
/// written (a removal writes the list, then its docs file).
#[derive(Clone)]
pub(super) struct Recorded {
    files: Vec<FileChange>,
    command: String,
    diff: String,
}

/// What this window has changed in the open list, in order, and what has been
/// undone (most recent last), for redo.
#[derive(Default)]
pub(super) struct History {
    done: Vec<Recorded>,
    undone: Vec<Recorded>,
    /// Each file as it was before its first edit, in the order first
    /// edited, for the session's diff.
    original: Vec<(PathBuf, String)>,
}

impl History {
    /// What this window last wrote to `path`, if it has.
    fn last_written(&self, path: &Path) -> Option<&str> {
        self.done
            .iter()
            .rev()
            .flat_map(|done| done.files.iter().rev())
            .find(|file| file.path == path)
            .map(|file| file.after.as_str())
    }
}

/// An edit as the page shows it.
#[derive(Serialize, Clone)]
pub(super) struct Change {
    command: String,
    diff: String,
}

impl From<&Recorded> for Change {
    fn from(recorded: &Recorded) -> Self {
        Self {
            command: recorded.command.clone(),
            diff: recorded.diff.clone(),
        }
    }
}

/// What is done and undone, after an edit, undo or redo.
#[derive(Serialize)]
pub(crate) struct Edited {
    /// What this step did.
    change: Change,
    /// Whether there is an edit to undo, or to redo.
    can_undo: bool,
    can_redo: bool,
}

fn edited(history: &History, change: Change) -> Edited {
    Edited {
        change,
        can_undo: !history.done.is_empty(),
        can_redo: !history.undone.is_empty(),
    }
}

/// Write a planned edit, remember it, and mark the plan out of date.
pub(super) fn apply(state: &State, command: String, edit: Edit) -> Result<Edited, String> {
    apply_all(state, command, vec![edit])
}

/// Write planned edits of several files as one step, in order: if one cannot
/// be written, those before it are put back.
pub(super) fn apply_all(
    state: &State,
    command: String,
    edits: Vec<Edit>,
) -> Result<Edited, String> {
    let edits: Vec<Edit> = edits.into_iter().filter(Edit::changes).collect();
    if edits.is_empty() {
        return Err("That changes nothing in the list.".to_string());
    }
    for (index, edit) in edits.iter().enumerate() {
        if let Err(err) = edit.write() {
            for written in edits[..index].iter().rev() {
                let _ = edit::write_atomic(&written.path, &written.before);
            }
            return Err(shown(err));
        }
    }
    let diff = edits.iter().map(Edit::diff).collect::<Vec<_>>().join("\n");
    let recorded = Recorded {
        files: edits
            .into_iter()
            .map(|edit| FileChange {
                path: edit.path,
                before: edit.before,
                after: edit.after,
            })
            .collect(),
        command,
        diff,
    };
    let mut session = state.session.lock().map_err(|_| "a command panicked")?;
    session.planned_at = None;
    let history = &mut session.history;
    // An edit made outside since the last one here ends the history: undoing
    // past it would undo someone else's change.
    if recorded.files.iter().any(|file| {
        history
            .last_written(&file.path)
            .is_some_and(|last| last != file.before)
    }) {
        *history = History::default();
    }
    for file in &recorded.files {
        if !history.original.iter().any(|(path, _)| path == &file.path) {
            history
                .original
                .push((file.path.clone(), file.before.clone()));
        }
    }
    history.undone.clear();
    let change = Change::from(&recorded);
    history.done.push(recorded);
    Ok(edited(history, change))
}

/// Plan with `change` on the open list, as the command would.
fn plan(
    state: &State,
    change: impl FnOnce(&str, &mudcrab::config::schema::SourceModlist) -> anyhow::Result<String>,
) -> Result<(std::path::PathBuf, Edit), String> {
    let list = state.list()?;
    let planned = edit::plan(&list, change).map_err(shown)?;
    Ok((list, planned))
}

fn list_arg(list: &Path) -> String {
    quote(list.display().to_string())
}

// --- Undo, redo, and the session's changes -----------------------------------------

/// Write each file's `from` text back to its `to` (undo: after to before;
/// redo: before to after), but only if every one is still what this window
/// last wrote. Otherwise the history ends there.
fn rewrite(state: &State, recorded: &Recorded, undo: bool) -> Result<(), String> {
    fn side(file: &FileChange, undo: bool) -> (&str, &str) {
        if undo {
            (&file.after, &file.before)
        } else {
            (&file.before, &file.after)
        }
    }
    for file in &recorded.files {
        let now = std::fs::read_to_string(&file.path).map_err(|err| err.to_string())?;
        if now != side(file, undo).0 {
            state
                .session
                .lock()
                .map_err(|_| "a command panicked")?
                .history = History::default();
            return Err(format!(
                "{} has changed outside this window since the last edit here, so there is \
                 nothing to undo or redo safely; the history starts again from here.",
                file.path.display()
            ));
        }
    }
    let files: Vec<&FileChange> = if undo {
        recorded.files.iter().rev().collect()
    } else {
        recorded.files.iter().collect()
    };
    for file in files {
        edit::write_atomic(&file.path, side(file, undo).1).map_err(shown)?;
    }
    state
        .session
        .lock()
        .map_err(|_| "a command panicked")?
        .planned_at = None;
    Ok(())
}

#[tauri::command]
pub(super) fn undo(state: tauri::State<'_, State>) -> Result<Edited, String> {
    let last = {
        let session = state.session.lock().map_err(|_| "a command panicked")?;
        session
            .history
            .done
            .last()
            .cloned()
            .ok_or("nothing to undo")?
    };
    rewrite(&state, &last, true)?;
    // What the undo did: the edit, backwards.
    let diff = last
        .files
        .iter()
        .map(|file| edit::diff(&file.path, &file.after, &file.before))
        .collect::<Vec<_>>()
        .join("\n");
    let mut session = state.session.lock().map_err(|_| "a command panicked")?;
    let history = &mut session.history;
    let recorded = history.done.pop().expect("there was one");
    history.undone.push(recorded);
    Ok(edited(
        history,
        Change {
            command: format!("undo: {}", last.command),
            diff,
        },
    ))
}

#[tauri::command]
pub(super) fn redo(state: tauri::State<'_, State>) -> Result<Edited, String> {
    let next = {
        let session = state.session.lock().map_err(|_| "a command panicked")?;
        session
            .history
            .undone
            .last()
            .cloned()
            .ok_or("nothing to redo")?
    };
    rewrite(&state, &next, false)?;
    let mut session = state.session.lock().map_err(|_| "a command panicked")?;
    let history = &mut session.history;
    let recorded = history.undone.pop().expect("there was one");
    let change = Change::from(&recorded);
    history.done.push(recorded);
    Ok(edited(history, change))
}

/// Everything changed in the list since it was opened here.
#[derive(Serialize)]
pub(super) struct Changes {
    edits: Vec<Change>,
    /// The session's changes as one diff.
    diff: String,
    can_undo: bool,
    can_redo: bool,
}

#[tauri::command]
pub(super) fn changes(state: tauri::State<'_, State>) -> Result<Changes, String> {
    let session = state.session.lock().map_err(|_| "a command panicked")?;
    let history = &session.history;
    // Each file from before its first edit to after its last one still done.
    let diff = history
        .original
        .iter()
        .filter_map(|(path, original)| {
            Some(edit::diff(path, original, history.last_written(path)?))
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Changes {
        edits: history.done.iter().map(Change::from).collect(),
        diff,
        can_undo: !history.done.is_empty(),
        can_redo: !history.undone.is_empty(),
    })
}

// --- The edits ------------------------------------------------------------------------

/// Move a plugin to just before another, or to the end (`plugins move`).
#[tauri::command]
pub(super) fn move_plugin(
    state: tauri::State<'_, State>,
    plugin: String,
    before: Option<String>,
) -> Result<Edited, String> {
    let to = match &before {
        Some(name) => doc::PluginPosition::Before(name.clone()),
        None => doc::PluginPosition::Last,
    };
    let (list, planned) = plan(&state, |text, _| doc::move_plugin(text, &plugin, &to))?;
    let place = match &before {
        Some(name) => format!("--before {}", quote(name)),
        None => "--last".to_string(),
    };
    let command = format!(
        "mudcrab plugins move {} {} {place}",
        list_arg(&list),
        quote(&plugin)
    );
    apply(&state, command, planned)
}

/// Move a mod to a section, before another mod or at the section's end
/// (`move`).
#[tauri::command]
pub(super) fn move_mod(
    state: tauri::State<'_, State>,
    id: String,
    section: Vec<String>,
    before_mod: Option<String>,
) -> Result<Edited, String> {
    let placement = Placement {
        before_mod: before_mod.clone(),
        before_section: None,
    };
    let (list, planned) = plan(&state, |text, source| {
        Ok(plan_move(text, &source.mods, &id, &section, &placement)?.new_text)
    })?;
    let mut command = format!("mudcrab move {} --mod {}", list_arg(&list), quote(&id));
    for level in &section {
        command.push_str(&format!(" --section {}", quote(level)));
    }
    if let Some(before) = &before_mod {
        command.push_str(&format!(" --before-mod {}", quote(before)));
    }
    apply(&state, command, planned)
}

/// Add strings to or take them out of an archive's `include`, `exclude` or
/// `bain_subpackages` (`files`).
#[tauri::command]
pub(super) fn edit_filter(
    state: tauri::State<'_, State>,
    id: String,
    number: usize,
    key: String,
    add: Vec<String>,
    drop: Vec<String>,
) -> Result<Edited, String> {
    let (flag, drop_flag) = match key.as_str() {
        "include" => ("--include", "--drop-include"),
        "exclude" => ("--exclude", "--drop-exclude"),
        "bain_subpackages" => ("--package", "--drop-package"),
        other => return Err(format!("{other} is not a filter")),
    };
    let (list, planned) = plan(&state, |text, _| {
        doc::edit_strings(text, &id, Some(number), &key, &add, &drop)
    })?;
    let mut command = format!("mudcrab files {} --mod {}", list_arg(&list), quote(&id));
    if number != 1 {
        command.push_str(&format!(" --archive {number}"));
    }
    for value in &add {
        command.push_str(&format!(" {flag} {}", quote(value)));
    }
    for value in &drop {
        command.push_str(&format!(" {drop_flag} {}", quote(value)));
    }
    apply(&state, command, planned)
}

/// Set or remove fields of a mod or one of its archives (`set`). `sets` are
/// `key=value`, values in TOML (a bare word is a string).
#[tauri::command]
pub(super) fn set_fields(
    state: tauri::State<'_, State>,
    id: String,
    archive: Option<usize>,
    sets: Vec<String>,
    unsets: Vec<String>,
) -> Result<Edited, String> {
    let pairs = doc::parse_pairs(&sets).map_err(shown)?;
    let (list, planned) = plan(&state, |text, _| {
        doc::set_fields(text, &id, archive, &pairs, &unsets)
    })?;
    let mut command = format!("mudcrab set {} --mod {}", list_arg(&list), quote(&id));
    if let Some(number) = archive {
        command.push_str(&format!(" --archive {number}"));
    }
    for set in &sets {
        command.push_str(&format!(" {}", quote(set)));
    }
    for key in &unsets {
        command.push_str(&format!(" --unset {}", quote(key)));
    }
    apply(&state, command, planned)
}

/// Add an action to the end of a mod's actions (`action add`).
#[tauri::command]
pub(super) fn add_action(
    state: tauri::State<'_, State>,
    id: String,
    action: String,
    fields: Vec<String>,
    condition: Option<String>,
) -> Result<Edited, String> {
    let pairs = doc::parse_pairs(&fields).map_err(shown)?;
    let (list, planned) = plan(&state, |text, source| {
        anyhow::ensure!(
            source.mods.iter().any(|m| m.id == id),
            "the list has no mod with id '{id}'"
        );
        doc::add_action(text, &id, &action, &pairs, condition.as_deref())
    })?;
    let mut command = format!(
        "mudcrab action add {} --mod {} {action}",
        list_arg(&list),
        quote(&id)
    );
    for field in &fields {
        command.push_str(&format!(" {}", quote(field)));
    }
    if let Some(condition) = &condition {
        command.push_str(&format!(" --if {}", quote(condition)));
    }
    apply(&state, command, planned)
}

/// Remove a mod's action by its number, from 1 (`action remove`).
#[tauri::command]
pub(super) fn remove_action(
    state: tauri::State<'_, State>,
    id: String,
    number: usize,
) -> Result<Edited, String> {
    let (list, planned) = plan(&state, |text, _| doc::remove_action(text, &id, number))?;
    let command = format!(
        "mudcrab action remove {} --mod {} {number}",
        list_arg(&list),
        quote(&id)
    );
    apply(&state, command, planned)
}

/// A mod's actions, numbered as `action remove` takes them.
#[tauri::command]
pub(super) fn actions(state: tauri::State<'_, State>, id: String) -> Result<Vec<String>, String> {
    let list = state.list()?;
    let text = std::fs::read_to_string(&list).map_err(|err| err.to_string())?;
    doc::action_lines(&text, &id).map_err(shown)
}

/// Declare a question (`input add`).
#[tauri::command]
pub(super) fn add_input(
    state: tauri::State<'_, State>,
    name: String,
    kind: String,
    query: String,
    choices: Vec<String>,
    default: Option<String>,
) -> Result<Edited, String> {
    let mut fields = vec![
        ("type".to_string(), toml_edit::Value::from(kind.as_str())),
        ("query".to_string(), toml_edit::Value::from(query.as_str())),
    ];
    if !choices.is_empty() {
        fields.push((
            "choices".to_string(),
            toml_edit::Value::Array(choices.iter().map(String::as_str).collect()),
        ));
    }
    if let Some(default) = &default {
        let value = if kind == "bool" {
            toml_edit::Value::from(
                default
                    .parse::<bool>()
                    .map_err(|_| "a yes-or-no default is true or false".to_string())?,
            )
        } else {
            toml_edit::Value::from(default.as_str())
        };
        fields.push(("default".to_string(), value));
    }
    let (list, planned) = plan(&state, |text, _| doc::add_input(text, &name, &fields))?;
    let mut command = format!(
        "mudcrab input add {} {} --type {kind} --query {}",
        list_arg(&list),
        quote(&name),
        quote(&query)
    );
    for choice in &choices {
        command.push_str(&format!(" --choice {}", quote(choice)));
    }
    if let Some(default) = &default {
        command.push_str(&format!(" --default {}", quote(default)));
    }
    apply(&state, command, planned)
}

/// Change a question's fields (`input set`).
#[tauri::command]
pub(super) fn set_input(
    state: tauri::State<'_, State>,
    name: String,
    sets: Vec<String>,
    unsets: Vec<String>,
) -> Result<Edited, String> {
    let pairs = doc::parse_pairs(&sets).map_err(shown)?;
    let (list, planned) = plan(&state, |text, _| {
        doc::set_input(text, &name, &pairs, &unsets)
    })?;
    let mut command = format!("mudcrab input set {} {}", list_arg(&list), quote(&name));
    for set in &sets {
        command.push_str(&format!(" {}", quote(set)));
    }
    for key in &unsets {
        command.push_str(&format!(" --unset {}", quote(key)));
    }
    apply(&state, command, planned)
}

/// Remove a question (`input remove`); refused while a condition asks it.
#[tauri::command]
pub(super) fn remove_input(state: tauri::State<'_, State>, name: String) -> Result<Edited, String> {
    let (list, planned) = plan(&state, |text, _| doc::remove_input(text, &name))?;
    let command = format!("mudcrab input remove {} {}", list_arg(&list), quote(&name));
    apply(&state, command, planned)
}

/// Write a mod's notes (`docs`): each field set to its text, or removed when
/// `None` or empty -- in the list or its docs file, wherever they already are.
#[tauri::command]
pub(super) fn set_docs(
    state: tauri::State<'_, State>,
    id: String,
    fields: Vec<(String, Option<String>)>,
) -> Result<Edited, String> {
    let list = state.list()?;
    let planned = edit::plan_mod_docs(&list, &id, &fields).map_err(shown)?;
    let mut command = format!("mudcrab docs {} --mod {}", list_arg(&list), quote(&id));
    for (key, value) in &fields {
        match value.as_deref().filter(|v| !v.trim().is_empty()) {
            Some(text) => command.push_str(&format!(" --{key} {}", quote(text))),
            None => command.push_str(&format!(" --unset {key}")),
        }
    }
    apply(&state, command, planned)
}

/// A mod's plain fields, or one of its archives', as `set` changes them.
#[tauri::command]
pub(super) fn fields(
    state: tauri::State<'_, State>,
    id: String,
    archive: Option<usize>,
) -> Result<Vec<(String, serde_json::Value)>, String> {
    let text = std::fs::read_to_string(state.list()?).map_err(|err| err.to_string())?;
    doc::field_values(&text, &id, archive).map_err(shown)
}

/// Remove a mod (`remove`): its block, its plugins from the load order, and
/// its notes from the docs file -- one step, undone as one.
#[tauri::command]
pub(super) fn remove_mod(state: tauri::State<'_, State>, id: String) -> Result<Edited, String> {
    let list = state.list()?;
    let edits = edit::plan_remove(&list, &id).map_err(shown)?;
    let command = format!("mudcrab remove {} --mod {}", list_arg(&list), quote(&id));
    apply_all(&state, command, edits)
}

/// Tick a mod in MO2 or leave it unticked (`enable`, `disable`), its plugins
/// put back in the load order or taken out.
#[tauri::command]
pub(super) fn set_enabled(
    state: tauri::State<'_, State>,
    id: String,
    enabled: bool,
) -> Result<Edited, String> {
    let list = state.list()?;
    let edit = edit::plan_enabled(&list, &id, enabled, None).map_err(shown)?;
    let command = format!(
        "mudcrab {} {} --mod {}",
        if enabled { "enable" } else { "disable" },
        list_arg(&list),
        quote(&id)
    );
    apply(&state, command, edit)
}
