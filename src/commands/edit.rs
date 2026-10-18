//! The editing commands besides `add` and `move`: `set`, `remove`, `plugins`,
//! `files`, `action` and `input`. Each plans its change with
//! `config::edit`, which validates it before anything is written, and with
//! `--dry-run` prints the change as a diff instead.

use crate::cli::{
    ActionAddArgs, ActionCommand, ActionListArgs, ActionRemoveArgs, FilesArgs, InputAddArgs,
    InputCommand, InputListArgs, InputRemoveArgs, InputSetArgs, PluginsCommand, PluginsListArgs,
    PluginsMoveArgs, RemoveArgs, SetArgs,
};
use crate::config::edit::{self, Edit, doc};
use crate::config::schema::{InputType, InputValue, SourceModlist};
use crate::ui::{self, Tone};
use std::path::Path;

/// Write `edit` and say `done`, or with `dry_run` print its diff.
fn finish(edit: Edit, dry_run: bool, done: String) -> anyhow::Result<()> {
    if !edit.changes() {
        ui::status(Tone::Note, "Nothing to change");
        return Ok(());
    }
    if dry_run {
        ui::diff(&edit.diff());
        ui::say(ui::dim("(--dry-run: nothing written)"));
        return Ok(());
    }
    edit.write()?;
    ui::status(Tone::Good, done);
    Ok(())
}

/// [`finish`], for an edit of several files, written in order.
fn finish_all(edits: Vec<Edit>, dry_run: bool, done: String) -> anyhow::Result<()> {
    if edits.is_empty() {
        ui::status(Tone::Note, "Nothing to change");
        return Ok(());
    }
    if dry_run {
        for edit in &edits {
            ui::diff(&edit.diff());
        }
        ui::say(ui::dim("(--dry-run: nothing written)"));
        return Ok(());
    }
    for edit in &edits {
        edit.write()?;
    }
    ui::status(Tone::Good, done);
    Ok(())
}

fn find_mod<'a>(
    source: &'a SourceModlist,
    id: &str,
) -> anyhow::Result<&'a crate::config::schema::ModEntry> {
    source
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))
}

pub fn set(args: SetArgs) -> anyhow::Result<()> {
    anyhow::ensure!(
        !args.sets.is_empty() || !args.unsets.is_empty(),
        "nothing to set: give key=value, or --unset <key>"
    );
    let sets = doc::parse_pairs(&args.sets)?;
    let edit = edit::plan_mod(&args.input, &args.mod_id, |text, _| {
        doc::set_fields(text, &args.mod_id, args.archive, &sets, &args.unsets)
    })?;
    let what = match args.archive {
        Some(n) => format!("{}'s archive {n}", ui::name(&args.mod_id)),
        None => ui::name(&args.mod_id),
    };
    let keys: Vec<String> = sets
        .iter()
        .map(|(key, value)| format!("{key} = {}", value.to_string().trim()))
        .chain(args.unsets.iter().map(|key| format!("{key} removed")))
        .collect();
    finish(
        edit,
        args.dry_run,
        format!("Set on {what}: {}", keys.join(", ")),
    )
}

pub fn new_list(args: crate::cli::NewArgs) -> anyhow::Result<()> {
    edit::create_list(&args.output, &args.name)?;
    ui::status(
        Tone::Good,
        format!(
            "Started {} at {}; add its mods with `mudcrab add`",
            ui::name(args.name.trim()),
            ui::dim(args.output.display())
        ),
    );
    Ok(())
}

pub fn choose(args: crate::cli::ChooseArgs) -> anyhow::Result<()> {
    use std::io::IsTerminal;
    let overrides = crate::config::query::parse_overrides(&args.set)?;
    let downloads = super::resolve_downloads_dir(args.downloads.clone(), None)?;
    let existing = crate::config::installer::existing(
        &args.input,
        &args.mod_id,
        args.archive,
        &overrides,
        &downloads,
        &[],
    )?;
    let interactive = !args.defaults && std::io::stdin().is_terminal();
    let installed = super::add::walk_installer(
        &existing.archive,
        &args.mod_id,
        &existing.report,
        &existing.list,
        &existing.prior,
        interactive,
    )?;
    for note in &installed.notes {
        ui::status_err(Tone::Note, format!("'{}': {note}", args.mod_id));
    }
    for action in &installed.actions {
        ui::status_err(
            Tone::Note,
            format!(
                "the wizard also calls for this action, not written (add it with `mudcrab \
                 action add`):\n{action}"
            ),
        );
    }
    let edit =
        crate::config::installer::plan_choice(&args.input, &args.mod_id, args.archive, &installed)?;
    finish(
        edit,
        args.dry_run,
        format!(
            "Wrote {}'s installer choices for archive {}",
            ui::name(&args.mod_id),
            args.archive
        ),
    )
}

pub fn docs(args: crate::cli::DocsArgs) -> anyhow::Result<()> {
    let body = match &args.body_file {
        Some(path) if path.as_os_str() == "-" => {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
            Some(text)
        }
        Some(path) => Some(
            std::fs::read_to_string(path)
                .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?,
        ),
        None => args.body.clone(),
    };
    let mut fields: Vec<(String, Option<String>)> = [
        ("title", args.title.clone()),
        ("summary", args.summary.clone()),
        ("body", body),
        ("footer", args.footer.clone()),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|v| (key.to_string(), Some(v))))
    .collect();
    fields.extend(args.unset.iter().map(|key| (key.clone(), None)));
    anyhow::ensure!(
        !fields.is_empty(),
        "say what to write: --title, --summary, --body, --body-file, --footer or --unset"
    );
    let edit = edit::plan_mod_docs(&args.input, &args.mod_id, &fields)?;
    let file = edit.path.display().to_string();
    finish(
        edit,
        args.dry_run,
        format!(
            "Wrote {}'s notes in {}",
            ui::name(&args.mod_id),
            ui::dim(file)
        ),
    )
}

pub fn remove(args: RemoveArgs) -> anyhow::Result<()> {
    let edits = edit::plan_remove(&args.input, &args.mod_id)?;
    if args.dry_run {
        for edit in &edits {
            ui::diff(&edit.diff());
        }
        ui::say(ui::dim("(--dry-run: nothing written)"));
        return Ok(());
    }
    for edit in &edits {
        edit.write()?;
    }
    let files: Vec<String> = edits
        .iter()
        .map(|edit| edit.path.display().to_string())
        .collect();
    ui::status(
        Tone::Good,
        format!(
            "Removed {}, its plugins from the load order and its notes ({})",
            ui::name(&args.mod_id),
            ui::dim(files.join(", "))
        ),
    );
    Ok(())
}

pub fn enabled(args: crate::cli::EnableArgs, enabled: bool) -> anyhow::Result<()> {
    use crate::config::add::LoadPosition;
    let position = match (&args.load_after, args.load_last) {
        (Some(after), _) => Some(LoadPosition::After(after.clone())),
        (None, true) => Some(LoadPosition::Last),
        (None, false) => None,
    };
    let edits = edit::plan_enabled(&args.input, &args.mod_id, enabled, position.as_ref())?;
    finish_all(
        edits,
        args.dry_run,
        if enabled {
            format!(
                "{} is ticked in MO2, its plugins in the load order",
                ui::name(&args.mod_id)
            )
        } else {
            format!(
                "{} is left unticked in MO2, its plugins out of the load order",
                ui::name(&args.mod_id)
            )
        },
    )
}

pub fn plugins(command: PluginsCommand) -> anyhow::Result<()> {
    match command {
        PluginsCommand::List(args) => plugins_list(args),
        PluginsCommand::Move(args) => plugins_move(args),
    }
}

fn plugins_list(args: PluginsListArgs) -> anyhow::Result<()> {
    let source = crate::config::loader::load_modlist(&args.input)?;
    for plugin in &source.plugins {
        let from: Vec<&str> = source
            .mods
            .iter()
            .filter(|entry| {
                entry.plugins.iter().any(|p| p.eq_ignore_ascii_case(plugin))
                    || entry
                        .merge
                        .as_ref()
                        .is_some_and(|merge| merge.output.eq_ignore_ascii_case(plugin))
            })
            .map(|entry| entry.id.as_str())
            .collect();
        let from = if from.is_empty() {
            if crate::config::check::GAME_PLUGINS.contains(&plugin.to_ascii_lowercase().as_str()) {
                "(the game)".to_string()
            } else {
                "-".to_string()
            }
        } else {
            from.join(", ")
        };
        let inactive = source
            .inactive_plugins
            .iter()
            .any(|p| p.eq_ignore_ascii_case(plugin));
        ui::data(format!(
            "{plugin}\t{from}{}",
            if inactive { "\tinactive" } else { "" }
        ));
    }
    Ok(())
}

fn plugins_move(args: PluginsMoveArgs) -> anyhow::Result<()> {
    let to = match (&args.after, &args.before, args.first) {
        (Some(after), _, _) => doc::PluginPosition::After(after.clone()),
        (_, Some(before), _) => doc::PluginPosition::Before(before.clone()),
        (_, _, true) => doc::PluginPosition::First,
        _ => doc::PluginPosition::Last,
    };
    let edit = edit::plan_plugin_move(&args.input, &args.plugin, &to)?;
    let place = match &to {
        doc::PluginPosition::After(name) => format!("after {}", ui::name(name)),
        doc::PluginPosition::Before(name) => format!("before {}", ui::name(name)),
        doc::PluginPosition::First => "first".to_string(),
        doc::PluginPosition::Last => "last".to_string(),
    };
    finish(
        edit,
        args.dry_run,
        format!("Moved {} {place} in the load order", ui::name(&args.plugin)),
    )
}

pub fn files(args: FilesArgs) -> anyhow::Result<()> {
    let overrides = crate::config::query::parse_overrides(&args.set)?;
    let downloads = super::resolve_downloads_dir(args.downloads.clone(), None)?;
    let editing = !(args.include.is_empty()
        && args.exclude.is_empty()
        && args.drop_include.is_empty()
        && args.drop_exclude.is_empty()
        && args.package.is_empty()
        && args.drop_package.is_empty());

    let before = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;
    let taken = |text: &str| {
        taken(
            text,
            &args.input,
            &args.mod_id,
            args.archive,
            &overrides,
            &downloads,
        )
    };
    if !editing {
        let Some((label, files)) = taken(&before)? else {
            ui::status_err(
                Tone::Note,
                format!(
                    "What {} installs from it is only known at install: an archive inside an \
                     archive, an MO2 plugin, or a build",
                    ui::name(&args.mod_id)
                ),
            );
            return Ok(());
        };
        for file in &files {
            ui::data(file);
        }
        ui::status_err(
            Tone::Good,
            format!("{}: {} file(s) from {label}", args.mod_id, files.len()),
        );
        return Ok(());
    }

    let edit = edit::plan_mod(&args.input, &args.mod_id, |text, _| {
        let archive = Some(args.archive);
        let text = doc::edit_strings(
            text,
            &args.mod_id,
            archive,
            "include",
            &args.include,
            &args.drop_include,
        )?;
        let text = doc::edit_strings(
            &text,
            &args.mod_id,
            archive,
            "exclude",
            &args.exclude,
            &args.drop_exclude,
        )?;
        doc::edit_strings(
            &text,
            &args.mod_id,
            archive,
            "bain_subpackages",
            &args.package,
            &args.drop_package,
        )
    })?;
    // What the change does to the files taken, where the archive is here to
    // say.
    match (taken(&edit.before), taken(&edit.after)) {
        (Ok(Some((label, was))), Ok(Some((_, now)))) => {
            let gone: Vec<&String> = was.iter().filter(|f| !now.contains(f)).collect();
            let added: Vec<&String> = now.iter().filter(|f| !was.contains(f)).collect();
            for file in &added {
                ui::say(format!("  {} {file}", console::style("+").green()));
            }
            for file in &gone {
                ui::say(format!("  {} {file}", console::style("-").red()));
            }
            ui::status(
                Tone::Note,
                format!(
                    "{label}: {} file(s) taken, was {} ({} more, {} fewer)",
                    now.len(),
                    was.len(),
                    added.len(),
                    gone.len()
                ),
            );
        }
        (Err(err), _) | (_, Err(err)) => ui::status(
            Tone::Note,
            format!("What the archive installs cannot be shown: {err:#}"),
        ),
        // Only known at install.
        _ => {}
    }
    finish(
        edit,
        args.dry_run,
        format!(
            "Filters and packages of {}'s archive {} changed",
            ui::name(&args.mod_id),
            args.archive
        ),
    )
}

/// The files mod `id` takes from its `number`th archive, as `text` has it,
/// with the archive's label.
fn taken(
    text: &str,
    path: &Path,
    id: &str,
    number: usize,
    overrides: &crate::config::query::Overrides,
    downloads: &Path,
) -> anyhow::Result<Option<(String, Vec<String>)>> {
    let crate::config::pipeline::PlannedArchive {
        entry,
        archive,
        active_plugins: active,
    } = crate::config::pipeline::planned_archive(text, path, id, number, overrides)?;
    let (entry, archive) = (&entry, &archive);
    let label = archive
        .file_name
        .clone()
        .or_else(|| archive.path.clone())
        .unwrap_or_else(|| format!("archive {number}"));
    Ok(
        crate::config::check::planned_files(entry, archive, downloads, &active)?
            .map(|files| (label, files)),
    )
}

pub fn action(command: ActionCommand) -> anyhow::Result<()> {
    match command {
        ActionCommand::List(args) => action_list(args),
        ActionCommand::Add(args) => action_add(args),
        ActionCommand::Remove(args) => action_remove(args),
    }
}

fn action_list(args: ActionListArgs) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;
    let lines = doc::action_lines(&text, &args.mod_id)?;
    if lines.is_empty() {
        ui::status_err(
            Tone::Note,
            format!("{} has no actions", ui::name(&args.mod_id)),
        );
    }
    for (number, line) in lines.iter().enumerate() {
        ui::data(format!("{}. {line}", number + 1));
    }
    Ok(())
}

fn action_add(args: ActionAddArgs) -> anyhow::Result<()> {
    let fields = doc::parse_pairs(&args.fields)?;
    let edit = edit::plan_mod(&args.input, &args.mod_id, |text, source| {
        find_mod(source, &args.mod_id)?;
        doc::add_action(
            text,
            &args.mod_id,
            &args.action,
            &fields,
            args.condition.as_deref(),
        )
    })?;
    finish(
        edit,
        args.dry_run,
        format!("Added {} to {}", args.action, ui::name(&args.mod_id)),
    )
}

fn action_remove(args: ActionRemoveArgs) -> anyhow::Result<()> {
    let edit = edit::plan_mod(&args.input, &args.mod_id, |text, _| {
        doc::remove_action(text, &args.mod_id, args.number)
    })?;
    finish(
        edit,
        args.dry_run,
        format!(
            "Removed action {} from {}",
            args.number,
            ui::name(&args.mod_id)
        ),
    )
}

pub fn input(command: InputCommand) -> anyhow::Result<()> {
    match command {
        InputCommand::List(args) => input_list(args),
        InputCommand::Add(args) => input_add(args),
        InputCommand::Set(args) => input_set(args),
        InputCommand::Remove(args) => input_remove(args),
    }
}

fn input_list(args: InputListArgs) -> anyhow::Result<()> {
    let source = crate::config::loader::load_modlist(&args.input)?;
    for (name, spec) in &source.inputs {
        let kind = match spec.input_type {
            InputType::Bool => "bool".to_string(),
            InputType::Choice => format!("choice [{}]", spec.choices.join(", ")),
            InputType::Text => "text".to_string(),
        };
        let default = match &spec.default {
            Some(InputValue::Bool(b)) => b.to_string(),
            Some(InputValue::Text(t)) => t.clone(),
            None => "-".to_string(),
        };
        ui::data(format!("{name}\t{kind}\t{default}\t{}", spec.query));
    }
    Ok(())
}

fn input_add(args: InputAddArgs) -> anyhow::Result<()> {
    let mut fields = vec![
        (
            "type".to_string(),
            toml_edit::Value::from(args.input_type.as_str()),
        ),
        (
            "query".to_string(),
            toml_edit::Value::from(args.query.as_str()),
        ),
    ];
    if !args.choices.is_empty() {
        fields.push((
            "choices".to_string(),
            toml_edit::Value::Array(args.choices.iter().map(String::as_str).collect()),
        ));
    }
    if let Some(default) = &args.default {
        let value = if args.input_type == "bool" {
            let flag: bool = default
                .parse()
                .map_err(|_| anyhow::anyhow!("--default {default}: a bool is true or false"))?;
            toml_edit::Value::from(flag)
        } else {
            toml_edit::Value::from(default.as_str())
        };
        fields.push(("default".to_string(), value));
    }
    let edit = edit::plan(&args.input, |text, _| {
        doc::add_input(text, &args.name, &fields)
    })?;
    finish(
        edit,
        args.dry_run,
        format!("Declared input {}", ui::name(&args.name)),
    )
}

fn input_set(args: InputSetArgs) -> anyhow::Result<()> {
    anyhow::ensure!(
        !args.sets.is_empty() || !args.unsets.is_empty(),
        "nothing to set: give key=value, or --unset <key>"
    );
    let sets = doc::parse_pairs(&args.sets)?;
    let edit = edit::plan(&args.input, |text, _| {
        doc::set_input(text, &args.name, &sets, &args.unsets)
    })?;
    finish(
        edit,
        args.dry_run,
        format!("Changed input {}", ui::name(&args.name)),
    )
}

fn input_remove(args: InputRemoveArgs) -> anyhow::Result<()> {
    let edit = edit::plan(&args.input, |text, _| doc::remove_input(text, &args.name))?;
    finish(
        edit,
        args.dry_run,
        format!("Removed input {}", ui::name(&args.name)),
    )
}
