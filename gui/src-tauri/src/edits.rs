//! Author mode, editing: every change to the list is one of the editing
//! commands' (`plugins move`, `move`, `files`, `set`, `action`, `input`),
//! planned by the same library call, validated, and written only if the file
//! has not changed on disk since it was read. Each comes back with the
//! command that does the same and the diff it made.
//!
//! The session keeps every edit, with the text before and after, so it can be
//! undone and redone -- writing the earlier or later text, and only while the
//! file is still what this window last wrote. An edit made outside ends the
//! history there. See docs/design/gui.md.

use super::{State, quote, shown};
use mudcrab::config::add::{Placement, plan_move};
use mudcrab::config::edit::{self, Edit, doc};
use serde::Serialize;
use std::path::Path;

/// One edit, as the session remembers it.
#[derive(Clone)]
pub(super) struct Recorded {
    command: String,
    diff: String,
    before: String,
    after: String,
}

/// What this window has changed in the open list, in order, and what has been
/// undone (most recent last), for redo.
#[derive(Default)]
pub(super) struct History {
    done: Vec<Recorded>,
    undone: Vec<Recorded>,
    /// The file as it was before the first edit, for the session's diff.
    original: Option<String>,
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
pub(super) struct Edited {
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
fn apply(state: &State, command: String, edit: Edit) -> Result<Edited, String> {
    if !edit.changes() {
        return Err("That changes nothing in the list.".to_string());
    }
    edit.write().map_err(shown)?;
    let recorded = Recorded {
        command,
        diff: edit.diff(),
        before: edit.before,
        after: edit.after,
    };
    let mut session = state.session.lock().map_err(|_| "a command panicked")?;
    session.planned_at = None;
    let history = &mut session.history;
    // An edit made outside since the last one here ends the history: undoing
    // past it would undo someone else's change.
    if history
        .done
        .last()
        .is_some_and(|last| last.after != recorded.before)
    {
        *history = History::default();
    }
    history
        .original
        .get_or_insert_with(|| recorded.before.clone());
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

/// Write `text` over the list, if it is still `expected`.
fn rewrite(state: &State, expected: &str, text: &str) -> Result<(), String> {
    let list = state.list()?;
    let now = std::fs::read_to_string(&list).map_err(|err| err.to_string())?;
    if now != expected {
        state
            .session
            .lock()
            .map_err(|_| "a command panicked")?
            .history = History::default();
        return Err(format!(
            "{} has changed outside this window since the last edit here, so there is nothing \
             to undo or redo safely; the history starts again from here.",
            list.display()
        ));
    }
    edit::write_atomic(&list, text).map_err(shown)?;
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
    rewrite(&state, &last.after, &last.before)?;
    let mut session = state.session.lock().map_err(|_| "a command panicked")?;
    let history = &mut session.history;
    let recorded = history.done.pop().expect("there was one");
    history.undone.push(recorded);
    Ok(edited(
        history,
        Change {
            command: format!("undo: {}", last.command),
            diff: last.diff,
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
    rewrite(&state, &next.before, &next.after)?;
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
    let list = state.list()?;
    let session = state.session.lock().map_err(|_| "a command panicked")?;
    let history = &session.history;
    let diff = match (&history.original, history.done.last()) {
        (Some(original), Some(last)) => edit::diff(&list, original, &last.after),
        _ => String::new(),
    };
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
