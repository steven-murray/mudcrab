//! Edits inside a mod, an input or the load order, made with `toml_edit`,
//! which keeps every byte it is not asked to change: comments, blank lines,
//! the order of keys, how arrays are laid out. Moving and removing whole mod
//! blocks stays with the line splicer in `config::add`, which knows which
//! comments belong to a block.
//!
//! Each function takes the file's text and returns the new text; validating
//! and writing it is [`super::plan`]'s job.

use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, Value};

/// A value given on the command line: TOML if it reads as a TOML value
/// (`true`, `3`, `["a", "b"]`, `"quoted"`), otherwise the text as a string,
/// so `layout=bain` needs no quotes.
pub fn parse_value(raw: &str) -> Value {
    raw.trim()
        .parse::<Value>()
        .map(|mut value| {
            value.decor_mut().clear();
            value
        })
        .unwrap_or_else(|_| Value::from(raw))
}

/// `key=value` pairs, split.
pub fn parse_pairs(pairs: &[String]) -> anyhow::Result<Vec<(String, Value)>> {
    pairs
        .iter()
        .map(|pair| {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("{pair}: expected key=value"))?;
            let key = key.trim();
            anyhow::ensure!(!key.is_empty(), "{pair}: no key before the =");
            Ok((key.to_string(), parse_value(value)))
        })
        .collect()
}

fn parse(text: &str) -> anyhow::Result<DocumentMut> {
    text.parse::<DocumentMut>()
        .map_err(|err| anyhow::anyhow!("the list is not valid TOML: {err}"))
}

fn mods(doc: &mut DocumentMut) -> anyhow::Result<&mut ArrayOfTables> {
    doc.get_mut("mods")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| anyhow::anyhow!("the list has no [[mods]]"))
}

fn mod_table<'a>(doc: &'a mut DocumentMut, id: &str) -> anyhow::Result<&'a mut Table> {
    mods(doc)?
        .iter_mut()
        .find(|table| table.get("id").and_then(Item::as_str) == Some(id))
        .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))
}

/// A mod's table, or with `archive` (counting from 1) one of its
/// `[[mods.archives]]`.
fn target<'a>(
    doc: &'a mut DocumentMut,
    id: &str,
    archive: Option<usize>,
) -> anyhow::Result<&'a mut Table> {
    let table = mod_table(doc, id)?;
    let Some(number) = archive else {
        return Ok(table);
    };
    let archives = table
        .get_mut("archives")
        .and_then(Item::as_array_of_tables_mut);
    let count = archives.as_ref().map_or(0, |a| a.len());
    archives
        .and_then(|archives| archives.get_mut(number.wrapping_sub(1)))
        .ok_or_else(|| {
            anyhow::anyhow!("mod '{id}' has {count} archive(s), so there is no archive {number}")
        })
}

/// Keys `set` leaves to the command made for them.
fn refused(key: &str) -> Option<&'static str> {
    match key {
        "id" => Some("an id is also the mod's folder and is named elsewhere; it cannot be set"),
        "section" => Some("use `mudcrab move --section`, which also moves the block"),
        "archives" => Some("set an archive's fields with --archive <N>"),
        "actions" => Some("use `mudcrab action`"),
        _ => None,
    }
}

/// Set (or with `unset`, remove) fields of a mod, or of one of its archives.
/// A key may be dotted (`merge.output`); a value replacing another keeps the
/// old one's comment.
pub fn set_fields(
    text: &str,
    id: &str,
    archive: Option<usize>,
    sets: &[(String, Value)],
    unsets: &[String],
) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    let table = target(&mut doc, id, archive)?;
    for (key, value) in sets {
        if archive.is_none()
            && let Some(why) = refused(key)
        {
            anyhow::bail!("{key}: {why}");
        }
        let (parents, last) = split_key(key)?;
        let mut here: &mut Table = table;
        for part in parents {
            let entry = here.entry(part).or_insert_with(|| {
                let mut table = Table::new();
                table.set_implicit(true);
                Item::Table(table)
            });
            here = entry
                .as_table_mut()
                .ok_or_else(|| anyhow::anyhow!("{key}: {part} is not a table"))?;
        }
        let mut value = value.clone();
        if let Some(old) = here.get(last).and_then(Item::as_value) {
            *value.decor_mut() = old.decor().clone();
        }
        here.insert(last, Item::Value(value));
    }
    for key in unsets {
        let (parents, last) = split_key(key)?;
        let mut here: &mut Table = table;
        for part in parents {
            here = here
                .get_mut(part)
                .and_then(Item::as_table_mut)
                .ok_or_else(|| anyhow::anyhow!("--unset {key}: there is no {part}"))?;
        }
        here.remove(last)
            .ok_or_else(|| anyhow::anyhow!("--unset {key}: it is not set"))?;
    }
    Ok(doc.to_string())
}

fn split_key(key: &str) -> anyhow::Result<(Vec<&str>, &str)> {
    let mut parts: Vec<&str> = key.split('.').map(str::trim).collect();
    anyhow::ensure!(
        parts.iter().all(|part| !part.is_empty()),
        "{key}: not a key"
    );
    let last = parts.pop().expect("split yields one part");
    Ok((parts, last))
}

/// Add strings to (or take them out of) a list field of a mod or one of its
/// archives -- `include`, `exclude`, `plugins` -- starting it if need be.
pub fn edit_strings(
    text: &str,
    id: &str,
    archive: Option<usize>,
    key: &str,
    add: &[String],
    remove: &[String],
) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    let table = target(&mut doc, id, archive)?;
    let item = table
        .entry(key)
        .or_insert(Item::Value(Value::Array(Array::new())));
    let array = item
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("{key} is not a list"))?;
    for wanted in remove {
        let at = position(array, wanted)
            .ok_or_else(|| anyhow::anyhow!("{key} does not have {wanted:?}"))?;
        remove_item(array, at);
    }
    for new in add {
        anyhow::ensure!(position(array, new).is_none(), "{key} already has {new:?}");
        insert_item(array, array.len(), Value::from(new.as_str()), false);
    }
    if array.is_empty() {
        table.remove(key);
    }
    Ok(doc.to_string())
}

fn position(array: &Array, wanted: &str) -> Option<usize> {
    array.iter().position(|value| {
        value
            .as_str()
            .is_some_and(|v| v.eq_ignore_ascii_case(wanted))
    })
}

/// Add an action to the end of a mod's actions.
pub fn add_action(
    text: &str,
    id: &str,
    action: &str,
    fields: &[(String, Value)],
    condition: Option<&str>,
) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    let table = mod_table(&mut doc, id)?;
    let mut new = Table::new();
    new.insert("action", toml_edit::value(action));
    if let Some(condition) = condition {
        new.insert("if", toml_edit::value(condition));
    }
    for (key, value) in fields {
        anyhow::ensure!(
            key != "action" && key != "if",
            "{key}: give it as the action's name or --if"
        );
        new.insert(key, Item::Value(value.clone()));
    }
    table
        .entry("actions")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| anyhow::anyhow!("mod '{id}' has `actions` written inline; write them as [[mods.actions]] tables to edit them"))?
        .push(new);
    Ok(doc.to_string())
}

/// Each of a mod's actions on one line, as written: `action = "file_hide",
/// paths = ["a"]`.
pub fn action_lines(text: &str, id: &str) -> anyhow::Result<Vec<String>> {
    let mut doc = parse(text)?;
    let table = mod_table(&mut doc, id)?;
    let Some(actions) = table.get("actions") else {
        return Ok(Vec::new());
    };
    let line = |table: &dyn toml_edit::TableLike| {
        table
            .iter()
            .map(|(key, item)| format!("{key} = {}", item.to_string().trim()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    Ok(match actions {
        Item::ArrayOfTables(tables) => tables.iter().map(|t| line(t)).collect(),
        Item::Value(Value::Array(array)) => array
            .iter()
            .filter_map(Value::as_inline_table)
            .map(|t| line(t))
            .collect(),
        _ => anyhow::bail!("mod '{id}': `actions` is not a list"),
    })
}

/// Remove a mod's `number`th action, counting from 1.
pub fn remove_action(text: &str, id: &str, number: usize) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    let table = mod_table(&mut doc, id)?;
    let actions = table
        .get_mut("actions")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| anyhow::anyhow!("mod '{id}' has no actions"))?;
    anyhow::ensure!(
        (1..=actions.len()).contains(&number),
        "mod '{id}' has {} action(s), so there is no action {number}",
        actions.len()
    );
    actions.remove(number - 1);
    if actions.is_empty() {
        table.remove("actions");
    }
    Ok(doc.to_string())
}

/// Write `layer` as mod `id`'s INI actions on its file under its condition --
/// its settings, those it comments out and the lines it appends -- in place
/// of the ones there were. The settings are one `ini_set` (one per format),
/// its `values` a sub-table, a section's keys in a table of their own. One
/// kept keeps its place and comments; a new one goes at the end of the
/// mod's actions.
pub fn set_ini_layer(
    text: &str,
    id: &str,
    layer: &crate::config::ini_edit::IniLayer,
) -> anyhow::Result<String> {
    use crate::config::ini_edit::same_file;
    use crate::config::schema::{IniScope, IniSetFormat};
    // Moved from another condition: that one's actions on the file go first.
    if let Some(from) = layer
        .moved_from
        .as_ref()
        .filter(|from| from.condition != layer.condition)
    {
        let emptied = crate::config::ini_edit::IniLayer {
            file: layer.file.clone(),
            scope: layer.scope,
            condition: from.condition.clone(),
            ..Default::default()
        };
        let text = set_ini_layer(text, id, &emptied)?;
        let rest = crate::config::ini_edit::IniLayer {
            moved_from: None,
            ..layer.clone()
        };
        return set_ini_layer(&text, id, &rest);
    }
    let mut doc = parse(text)?;
    let table = mod_table(&mut doc, id)?;
    let actions = table
        .entry("actions")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| anyhow::anyhow!("mod '{id}' has `actions` written inline; write them as [[mods.actions]] tables to edit them"))?;
    let get = |t: &Table, key: &str| t.get(key).and_then(Item::as_str).map(str::to_string);
    let set_format = |t: &mut Table, format: IniSetFormat| match format {
        IniSetFormat::SetTo => {
            t.insert("format", toml_edit::value("set-to"));
        }
        IniSetFormat::Standard => {
            t.remove("format");
        }
    };
    // The INI action a table is, when it is one of the layer's.
    let kind_of = |t: &Table| -> Option<String> {
        let kind = get(t, "action")?;
        let in_layer = get(t, "file").is_some_and(|file| same_file(&file, &layer.file))
            && get(t, "if") == layer.condition;
        let scope = match get(t, "scope").as_deref() {
            Some("game") => IniScope::Game,
            _ => IniScope::Mod,
        };
        let fits = match kind.as_str() {
            "ini_set" => scope == layer.scope,
            "ini_comment" | "ini_append_block" => layer.scope == IniScope::Mod,
            _ => false,
        };
        (in_layer && fits).then_some(kind)
    };

    // The settings by format, each written as one `ini_set`'s `values`.
    let mut groups: Vec<(IniSetFormat, Vec<&crate::config::ini_edit::IniSetting>)> = Vec::new();
    for setting in &layer.sets {
        match groups.iter_mut().find(|(f, _)| *f == setting.format) {
            Some((_, group)) => group.push(setting),
            None => groups.push((setting.format, vec![setting])),
        }
    }
    let values_of = |group: &[&crate::config::ini_edit::IniSetting]| {
        let mut top = Table::new();
        // No `[...values]` header of its own when it only holds sections.
        top.set_implicit(true);
        for setting in group {
            let value = toml_edit::value(setting.value.as_str());
            match &setting.section {
                None => {
                    top.insert(&setting.key, value);
                }
                Some(section) => {
                    if let Some(keys) = top
                        .entry(section)
                        .or_insert(Item::Table(Table::new()))
                        .as_table_mut()
                    {
                        keys.insert(&setting.key, value);
                    }
                }
            }
        }
        Item::Table(top)
    };
    let mut placed: Vec<IniSetFormat> = Vec::new();
    // The keys to comment out by format, each one `ini_comment`'s `keys`.
    let mut commented: Vec<(IniSetFormat, Vec<&str>)> = Vec::new();
    for comment in &layer.comments {
        match commented.iter_mut().find(|(f, _)| *f == comment.format) {
            Some((_, keys)) => keys.push(&comment.key),
            None => commented.push((comment.format, vec![&comment.key])),
        }
    }
    let keys_of = |keys: &[&str]| Item::Value(Value::Array(keys.iter().copied().collect()));
    let mut placed_comments: Vec<IniSetFormat> = Vec::new();
    let mut block = layer.block.as_deref().filter(|b| !b.trim().is_empty());
    let mut gone = Vec::new();
    for (index, t) in actions.iter_mut().enumerate() {
        let Some(kind) = kind_of(t) else { continue };
        let kept = match kind.as_str() {
            "ini_set" => {
                // The first of a format's keeps its place, holding them all.
                let format = match get(t, "format").as_deref() {
                    Some("set-to") => IniSetFormat::SetTo,
                    _ => IniSetFormat::Standard,
                };
                match groups.iter().find(|(f, _)| *f == format) {
                    Some((_, group)) if !placed.contains(&format) => {
                        for old in ["key", "value", "section", "values"] {
                            t.remove(old);
                        }
                        t.insert("values", values_of(group));
                        set_format(t, format);
                        placed.push(format);
                        true
                    }
                    _ => false,
                }
            }
            "ini_comment" => {
                let format = match get(t, "format").as_deref() {
                    Some("set-to") => IniSetFormat::SetTo,
                    _ => IniSetFormat::Standard,
                };
                match commented.iter().find(|(f, _)| *f == format) {
                    Some((_, keys)) if !placed_comments.contains(&format) => {
                        t.remove("key");
                        t.insert("keys", keys_of(keys));
                        set_format(t, format);
                        placed_comments.push(format);
                        true
                    }
                    _ => false,
                }
            }
            _ => match block.take() {
                Some(lines) => {
                    t.insert("block", toml_edit::value(lines));
                    true
                }
                None => false,
            },
        };
        if !kept {
            gone.push(index);
        }
    }
    for index in gone.into_iter().rev() {
        actions.remove(index);
    }

    let new = |action: &str| {
        let mut t = Table::new();
        t.insert("action", toml_edit::value(action));
        if let Some(condition) = &layer.condition {
            t.insert("if", toml_edit::value(condition.as_str()));
        }
        t.insert("file", toml_edit::value(layer.file.as_str()));
        t
    };
    for (format, group) in groups.iter().filter(|(f, _)| !placed.contains(f)) {
        let mut t = new("ini_set");
        set_format(&mut t, *format);
        if layer.scope == IniScope::Game {
            t.insert("scope", toml_edit::value("game"));
        }
        t.insert("values", values_of(group));
        actions.push(t);
    }
    for (format, keys) in commented
        .iter()
        .filter(|(f, _)| !placed_comments.contains(f))
    {
        let mut t = new("ini_comment");
        t.insert("keys", keys_of(keys));
        set_format(&mut t, *format);
        actions.push(t);
    }
    if let Some(lines) = block {
        let mut t = new("ini_append_block");
        t.insert("block", toml_edit::value(lines));
        actions.push(t);
    }
    if actions.is_empty() {
        table.remove("actions");
    }
    Ok(doc.to_string())
}

/// The `inputs` table, made if need be: as dotted keys at the top, the way a
/// list keeps its questions above its mods.
fn inputs(doc: &mut DocumentMut) -> anyhow::Result<&mut Table> {
    doc.entry("inputs")
        .or_insert_with(|| {
            let mut table = Table::new();
            table.set_dotted(true);
            Item::Table(table)
        })
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("`inputs` is not a table"))
}

/// Declare a new input with `fields` (`type`, `query`, `choices`, `default`).
pub fn add_input(text: &str, name: &str, fields: &[(String, Value)]) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    let inputs = inputs(&mut doc)?;
    anyhow::ensure!(!inputs.contains_key(name), "input {name} already exists");
    let mut table = Table::new();
    // Written the way the list's other inputs are.
    let dotted = inputs.is_dotted()
        || inputs
            .iter()
            .any(|(_, item)| item.as_table().is_some_and(Table::is_dotted));
    table.set_dotted(dotted);
    for (key, value) in fields {
        table.insert(key, Item::Value(value.clone()));
    }
    inputs.insert(name, Item::Table(table));
    Ok(doc.to_string())
}

/// Change (or remove) an input's fields.
pub fn set_input(
    text: &str,
    name: &str,
    sets: &[(String, Value)],
    unsets: &[String],
) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    let input = input_table(&mut doc, name)?;
    for (key, value) in sets {
        let mut value = value.clone();
        if let Some(old) = input.get(key).and_then(Item::as_value) {
            *value.decor_mut() = old.decor().clone();
        }
        input.insert(key, Item::Value(value));
    }
    for key in unsets {
        input
            .remove(key)
            .ok_or_else(|| anyhow::anyhow!("--unset {key}: input {name} does not set it"))?;
    }
    Ok(doc.to_string())
}

/// Remove an input; anything still using it fails validation, by name.
pub fn remove_input(text: &str, name: &str) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    input_table(&mut doc, name)?;
    let inputs = inputs(&mut doc)?;
    inputs.remove(name);
    if inputs.is_empty() {
        doc.remove("inputs");
    }
    Ok(doc.to_string())
}

fn input_table<'a>(
    doc: &'a mut DocumentMut,
    name: &str,
) -> anyhow::Result<&'a mut dyn toml_edit::TableLike> {
    doc.get_mut("inputs")
        .and_then(|inputs| inputs.get_mut(name))
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| anyhow::anyhow!("the list has no input {name}"))
}

/// Where a plugin goes in the load order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginPosition {
    After(String),
    Before(String),
    First,
    Last,
}

/// Move `plugin` within the top-level `plugins` load order. Comments in the
/// array stay where they are: a heading above the plugin stays above
/// whatever follows it, and the plugin's own end-of-line comment goes with
/// it.
pub fn move_plugin(text: &str, plugin: &str, to: &PluginPosition) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    let array = doc
        .get_mut("plugins")
        .and_then(Item::as_array_mut)
        .ok_or_else(|| anyhow::anyhow!("the list has no `plugins` load order"))?;
    let from = position(array, plugin)
        .ok_or_else(|| anyhow::anyhow!("{plugin} is not in the load order"))?;
    let anchor = |array: &Array, name: &str| {
        anyhow::ensure!(
            !name.eq_ignore_ascii_case(plugin),
            "cannot place {plugin} relative to itself"
        );
        position(array, name).ok_or_else(|| anyhow::anyhow!("{name} is not in the load order"))
    };
    // Checked before anything moves.
    if let PluginPosition::After(name) | PluginPosition::Before(name) = to {
        anchor(array, name)?;
    }
    let (value, comment) = remove_item(array, from);
    let (at, under_heading) = match to {
        PluginPosition::After(name) => (anchor(array, name)? + 1, false),
        PluginPosition::Before(name) => (anchor(array, name)?, true),
        PluginPosition::First => (0, true),
        PluginPosition::Last => (array.len(), false),
    };
    let mut value = value;
    value.decor_mut().clear();
    insert_item(array, at, value, under_heading);
    if let Some(comment) = comment {
        attach_comment(array, at, &comment);
    }
    Ok(doc.to_string())
}

/// Leave `plugin` unticked in the load order (in `inactive-plugins`), or tick
/// it again. `inactive-plugins` keeps the load order's order.
pub fn set_plugin_active(text: &str, plugin: &str, active: bool) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    let order: Vec<String> = doc
        .get("plugins")
        .and_then(Item::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_str().map(str::to_ascii_lowercase))
                .collect()
        })
        .unwrap_or_default();
    let place = |name: &str| order.iter().position(|p| *p == name.to_ascii_lowercase());
    let Some(at_in_order) = place(plugin) else {
        anyhow::bail!("{plugin} is not in the load order");
    };
    if doc.get("inactive-plugins").is_none() {
        if active {
            return Ok(doc.to_string());
        }
        doc.insert("inactive-plugins", Item::Value(Value::Array(Array::new())));
    }
    let array = doc
        .get_mut("inactive-plugins")
        .and_then(Item::as_array_mut)
        .ok_or_else(|| anyhow::anyhow!("`inactive-plugins` is not a list"))?;
    match (position(array, plugin), active) {
        (Some(at), true) => {
            remove_item(array, at);
            if array.is_empty() {
                doc.remove("inactive-plugins");
            }
        }
        (None, false) => {
            let at = array
                .iter()
                .position(|value| {
                    value
                        .as_str()
                        .and_then(place)
                        .is_some_and(|other| other > at_in_order)
                })
                .unwrap_or(array.len());
            insert_item(array, at, Value::from(plugin), false);
        }
        _ => {}
    }
    Ok(doc.to_string())
}

/// Take plugins out of the load order and `inactive-plugins`, wherever they
/// are listed.
pub fn drop_plugins(text: &str, plugins: &[String]) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    for key in ["plugins", "inactive-plugins"] {
        let Some(array) = doc.get_mut(key).and_then(Item::as_array_mut) else {
            continue;
        };
        for plugin in plugins {
            if let Some(at) = position(array, plugin) {
                remove_item(array, at);
            }
        }
    }
    Ok(doc.to_string())
}

// How a multi-line array keeps its comments. Each value's prefix holds what
// sits between the comma before it and the value: first the rest of the
// previous value's line (spaces, perhaps an end-of-line comment), then whole
// lines (comments, blanks), then the value's own indentation. The text after
// the last value is the array's trailing text, read the same way.

/// Without a trailing comma, the space before `]` is the last value's
/// suffix; it is moved to the array's trailing text, where it is with one, so
/// it stays put when values move.
fn normalize(array: &mut Array) {
    if array.trailing_comma() {
        return;
    }
    let Some(last) = array.iter_mut().last() else {
        return;
    };
    let suffix = last
        .decor()
        .suffix()
        .and_then(|raw| raw.as_str())
        .unwrap_or("")
        .to_string();
    if suffix.is_empty() {
        return;
    }
    last.decor_mut().set_suffix("");
    let trailing = format!("{suffix}{}", trailing(array));
    array.set_trailing(trailing);
}

fn multi_line(array: &Array) -> bool {
    array.iter().any(|value| prefix(value).contains('\n'))
}

fn prefix(value: &Value) -> &str {
    value
        .decor()
        .prefix()
        .and_then(|raw| raw.as_str())
        .unwrap_or("")
}

/// The text after the array's last value, up to `]`.
fn trailing(array: &Array) -> &str {
    array.trailing().as_str().unwrap_or("")
}

/// The first line of `text` (the previous value's line), and the rest from
/// its newline on.
fn split_first_line(text: &str) -> (&str, &str) {
    match text.find('\n') {
        Some(at) => text.split_at(at),
        None => (text, ""),
    }
}

/// Remove the value at `at`, leaving the comment lines above it in place for
/// what now follows. Returns the value and its end-of-line comment, if any.
fn remove_item(array: &mut Array, at: usize) -> (Value, Option<String>) {
    normalize(array);
    let multi = multi_line(array);
    let removed = array.remove(at);
    if !multi {
        // The new first value sits against `[` as the old one did.
        if at == 0
            && let Some(first) = array.get_mut(0)
        {
            first.decor_mut().set_prefix(prefix(&removed).to_string());
        }
        return (removed, None);
    }
    let own = prefix(&removed).to_string();
    // The lines above the removed value, without its indentation.
    let above = match own.rfind('\n') {
        Some(at) => own[..at].to_string(),
        None => own.clone(),
    };
    let next = if at < array.len() {
        prefix(array.get(at).expect("in range")).to_string()
    } else {
        trailing(array).to_string()
    };
    let (its_comment, rest) = split_first_line(&next);
    let comment = its_comment.trim();
    let comment = (!comment.is_empty()).then(|| its_comment.to_string());
    let joined = format!("{above}{rest}");
    if at < array.len() {
        array
            .get_mut(at)
            .expect("in range")
            .decor_mut()
            .set_prefix(joined);
    } else {
        array.set_trailing(joined);
    }
    (removed, comment)
}

/// Insert `value` at `at`, laid out as the array's other values are. With
/// `under_heading`, it goes directly above what is at `at`, below any comment
/// lines above that; otherwise above them.
fn insert_item(array: &mut Array, at: usize, mut value: Value, under_heading: bool) {
    normalize(array);
    if !multi_line(array) {
        if array.is_empty() {
            value.decor_mut().clear();
        } else {
            value.decor_mut().set_prefix(if at == 0 { "" } else { " " });
            if at == 0 {
                array
                    .get_mut(0)
                    .expect("not empty")
                    .decor_mut()
                    .set_prefix(" ");
            }
        }
        value.decor_mut().set_suffix("");
        array.insert_formatted(at, value);
        return;
    }
    let indent = array
        .iter()
        .map(prefix)
        .find(|p| p.contains('\n'))
        .and_then(|p| p.rsplit('\n').next())
        .unwrap_or("    ")
        .to_string();
    // What is now at `at` keeps the lines above it; the previous value's
    // end-of-line comment stays on that value's line, now before the new one.
    let next = if at < array.len() {
        prefix(array.get(at).expect("in range")).to_string()
    } else {
        trailing(array).to_string()
    };
    let (first_line, rest) = split_first_line(&next);
    let (mine, rest) = if under_heading && at < array.len() && !rest.is_empty() {
        (format!("{first_line}{rest}"), format!("\n{indent}"))
    } else {
        (format!("{first_line}\n{indent}"), rest.to_string())
    };
    value.decor_mut().set_prefix(mine);
    value.decor_mut().set_suffix("");
    if at < array.len() {
        let rest = if rest.is_empty() {
            format!("\n{indent}")
        } else {
            rest.to_string()
        };
        array
            .get_mut(at)
            .expect("in range")
            .decor_mut()
            .set_prefix(rest);
    } else {
        array.set_trailing(rest);
    }
    array.insert_formatted(at, value);
}

/// Put `comment` (a value's end-of-line comment, with its leading spaces) back
/// after the value at `at`.
fn attach_comment(array: &mut Array, at: usize, comment: &str) {
    if at + 1 < array.len() {
        let next = array.get_mut(at + 1).expect("in range");
        let old = prefix(next).to_string();
        let (_, rest) = split_first_line(&old);
        next.decor_mut().set_prefix(format!("{comment}{rest}"));
    } else {
        let old = trailing(array).to_string();
        let (_, rest) = split_first_line(&old);
        array.set_trailing(format!("{comment}{rest}"));
    }
}

/// The prose fields a mod's docs take a string for.
pub const DOC_FIELDS: &[&str] = &["title", "summary", "body", "footer"];

/// Prose as a TOML value: several lines as a `'''` literal block, as a person
/// writes them, so the file stays readable; one line as a plain string.
fn prose(text: &str) -> Value {
    if text.contains('\n') && !text.contains("'''") {
        let parsed = format!("v = '''\n{text}'''")
            .parse::<DocumentMut>()
            .ok()
            .and_then(|doc| doc.get("v").and_then(Item::as_value).cloned());
        if let Some(value) = parsed {
            return value;
        }
    }
    Value::from(text)
}

/// Set (or with `None`, remove) prose fields of `table`.
fn set_prose(table: &mut Table, fields: &[(String, Option<String>)]) -> anyhow::Result<()> {
    for (key, value) in fields {
        anyhow::ensure!(
            DOC_FIELDS.contains(&key.as_str()),
            "{key} is not a docs field; these are {}",
            DOC_FIELDS.join(", ")
        );
        match value.as_deref().filter(|v| !v.trim().is_empty()) {
            Some(text) => {
                table.insert(key, Item::Value(prose(text)));
            }
            None => {
                table.remove(key);
            }
        }
    }
    Ok(())
}

/// A mod's docs written inline in the list, as `docs.<field>` keys in its
/// `[[mods]]` table.
pub fn set_inline_docs(
    text: &str,
    id: &str,
    fields: &[(String, Option<String>)],
) -> anyhow::Result<String> {
    let mut doc = parse(text)?;
    let table = mod_table(&mut doc, id)?;
    let docs = table.entry("docs").or_insert_with(|| {
        let mut dotted = Table::new();
        dotted.set_dotted(true);
        Item::Table(dotted)
    });
    let docs = docs
        .as_table_like_mut()
        .ok_or_else(|| anyhow::anyhow!("'{id}' has a docs that is not a table"))?;
    let mut owned = Table::new();
    for (key, item) in docs.iter() {
        owned.insert(key, item.clone());
    }
    set_prose(&mut owned, fields)?;
    // Written back key by key, so the table keeps its own form (dotted keys,
    // or a [mods.docs] table).
    let keys: Vec<String> = docs.iter().map(|(key, _)| key.to_string()).collect();
    for key in keys {
        if !owned.contains_key(&key) {
            docs.remove(&key);
        }
    }
    for (key, item) in owned.iter() {
        docs.insert(key, item.clone());
    }
    if docs.is_empty() {
        table.remove("docs");
    }
    Ok(doc.to_string())
}

/// A mod's docs in the list's docs file: its `[mods."<id>"]` table, started
/// if need be.
pub fn set_file_docs(
    text: &str,
    id: &str,
    fields: &[(String, Option<String>)],
) -> anyhow::Result<String> {
    let mut doc = text
        .parse::<DocumentMut>()
        .map_err(|err| anyhow::anyhow!("the docs file is not valid TOML: {err}"))?;
    let mods = doc.entry("mods").or_insert_with(|| {
        let mut implicit = Table::new();
        implicit.set_implicit(true);
        Item::Table(implicit)
    });
    let mods = mods
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("the docs file's `mods` is not a table"))?;
    let table = mods
        .entry(id)
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("the docs file's [mods.\"{id}\"] is not a table"))?;
    set_prose(table, fields)?;
    if table.is_empty() {
        mods.remove(id);
    }
    Ok(doc.to_string())
}

/// The plain fields of a mod, or with `archive` one of its archives: each
/// key whose value is a string, number, boolean or list of those, as JSON,
/// in the file's order. Tables and lists of tables (`archives`, `actions`,
/// `merge`, `fomod_selections`...) have commands of their own and are left
/// out. What `set` changes, for a form to show.
pub fn field_values(
    text: &str,
    id: &str,
    archive: Option<usize>,
) -> anyhow::Result<Vec<(String, serde_json::Value)>> {
    fn json(value: &Value) -> Option<serde_json::Value> {
        Some(match value {
            Value::String(s) => serde_json::Value::from(s.value().as_str()),
            Value::Integer(n) => serde_json::Value::from(*n.value()),
            Value::Float(f) => serde_json::Value::from(*f.value()),
            Value::Boolean(b) => serde_json::Value::from(*b.value()),
            Value::Array(items) => serde_json::Value::Array(
                items
                    .iter()
                    .map(|item| match item {
                        Value::InlineTable(_) | Value::Array(_) => None,
                        other => json(other),
                    })
                    .collect::<Option<Vec<_>>>()?,
            ),
            _ => return None,
        })
    }
    let mut doc = parse(text)?;
    let table = target(&mut doc, id, archive)?;
    Ok(table
        .iter()
        .filter_map(|(key, item)| Some((key.to_string(), json(item.as_value()?)?)))
        .collect())
}

/// Write one set of an installer's choices into a mod's archive (from 1):
/// `set`'s, or the base set's for `None`, the others' kept, as
/// [`crate::config::installer_sets`] lays them out. A BAIN wizard's choices
/// go only into the base set: its plugins left out are the archive's.
pub fn set_installer_set(
    text: &str,
    id: &str,
    archive: usize,
    chosen: &crate::config::inspect::Chosen,
    set: Option<&str>,
) -> anyhow::Result<String> {
    use crate::config::inspect::Chosen;
    use crate::config::installer_sets;
    use crate::config::schema::{BainSubpackage, FomodSelection};
    anyhow::ensure!(
        set.is_none() || !matches!(chosen, Chosen::Wizard { .. }),
        "a BAIN wizard's choices cannot depend on a question; choose its packages directly"
    );
    let mut doc = parse(text)?;
    let table = target(&mut doc, id, Some(archive))?;
    match chosen {
        Chosen::Fomod(selections) => {
            let old: Vec<Table> = table
                .get("fomod_selections")
                .and_then(Item::as_array_of_tables)
                .map(|tables| tables.iter().cloned().collect())
                .unwrap_or_default();
            let rows: Vec<FomodSelection> = old
                .iter()
                .map(|t| toml::from_str(&t.to_string()))
                .collect::<Result<_, _>>()
                .map_err(|err| anyhow::anyhow!("mod '{id}': a FOMOD selection: {err}"))?;
            let plain: Vec<FomodSelection> = selections
                .iter()
                .filter(|s| s.condition.is_none())
                .cloned()
                .collect();
            let written = installer_sets::write_fomod(&rows, set, &plain);
            let mut tables = ArrayOfTables::new();
            for selection in &written {
                let mut new = Table::new();
                // The row's old table, for its comments.
                if let Some(was) = old.iter().find(|t| {
                    t.get("step").and_then(Item::as_str) == Some(selection.step.as_str())
                        && t.get("group").and_then(Item::as_str) == Some(selection.group.as_str())
                        && t.get("if").and_then(Item::as_str) == selection.condition.as_deref()
                }) {
                    *new.decor_mut() = was.decor().clone();
                }
                if let Some(condition) = &selection.condition {
                    new.insert("if", toml_edit::value(condition.as_str()));
                }
                new.insert("step", toml_edit::value(selection.step.as_str()));
                new.insert("group", toml_edit::value(selection.group.as_str()));
                new.insert(
                    "options",
                    Item::Value(Value::Array(
                        selection.options.iter().map(String::as_str).collect(),
                    )),
                );
                tables.push(new);
            }
            if tables.is_empty() {
                table.remove("fomod_selections");
            } else {
                table.insert("fomod_selections", Item::ArrayOfTables(tables));
            }
            Ok(doc.to_string())
        }
        Chosen::Bain(names) => {
            let entries: Vec<BainSubpackage> = match table.get("bain_subpackages") {
                Some(item) => toml::from_str::<
                    std::collections::BTreeMap<String, Vec<BainSubpackage>>,
                >(&format!("x = {}", item.to_string().trim()))
                .map_err(|err| anyhow::anyhow!("mod '{id}': bain_subpackages: {err}"))?
                .remove("x")
                .unwrap_or_default(),
                None => Vec::new(),
            };
            let order: Vec<String> = entries
                .iter()
                .map(|e| e.name().to_string())
                .chain(names.iter().cloned())
                .collect();
            let written = installer_sets::write_bain(&entries, set, names, &order)?;
            let mut array = Array::new();
            for entry in &written {
                match entry {
                    BainSubpackage::Name(name) => array.push(name.as_str()),
                    BainSubpackage::Conditional { name, condition } => {
                        let mut t = toml_edit::InlineTable::new();
                        t.insert("name", name.as_str().into());
                        t.insert("if", condition.as_str().into());
                        array.push(Value::InlineTable(t));
                    }
                }
            }
            table.insert("bain_subpackages", Item::Value(Value::Array(array)));
            Ok(doc.to_string())
        }
        Chosen::Wizard { .. } => set_installer(text, id, archive, chosen, false),
    }
}

/// Write an installer's choices into a mod's archive (from 1): a FOMOD's as
/// its `fomod_selections`, one table per group asked, in order (a group's
/// table keeps its comments); BAIN packages as `bain_subpackages`; a
/// wizard's packages likewise, with the plugins it left out added to
/// `exclude`. Choices that depend on the list's questions (an `if` on a
/// selection or package) are kept by a walk that showed them (`rows`: the
/// GUI's, which writes FOMOD selections with their `if`); otherwise an
/// archive with any is refused rather than have them dropped.
pub fn set_installer(
    text: &str,
    id: &str,
    archive: usize,
    chosen: &crate::config::inspect::Chosen,
    rows: bool,
) -> anyhow::Result<String> {
    use crate::config::inspect::Chosen;
    let mut doc = parse(text)?;
    let table = target(&mut doc, id, Some(archive))?;
    let conditional = |item: Option<&Item>| -> bool {
        match item {
            Some(Item::ArrayOfTables(tables)) => tables.iter().any(|t| t.contains_key("if")),
            Some(Item::Value(Value::Array(items))) => items
                .iter()
                .any(|v| v.as_inline_table().is_some_and(|t| t.contains_key("if"))),
            _ => false,
        }
    };
    let packages =
        |names: &[String]| Item::Value(Value::Array(names.iter().map(String::as_str).collect()));
    match chosen {
        Chosen::Fomod(selections) => {
            anyhow::ensure!(
                rows || !conditional(table.get("fomod_selections")),
                "some of '{id}'s FOMOD selections depend on the list's questions (an `if`); \
                 change them in the list's file"
            );
            let old: Vec<Table> = table
                .get("fomod_selections")
                .and_then(Item::as_array_of_tables)
                .map(|tables| tables.iter().cloned().collect())
                .unwrap_or_default();
            let mut tables = ArrayOfTables::new();
            for selection in selections {
                let mut new = Table::new();
                // The group's old table, for its comments.
                if let Some(was) = old.iter().find(|t| {
                    t.get("step").and_then(Item::as_str) == Some(selection.step.as_str())
                        && t.get("group").and_then(Item::as_str) == Some(selection.group.as_str())
                        && t.get("if").and_then(Item::as_str) == selection.condition.as_deref()
                }) {
                    *new.decor_mut() = was.decor().clone();
                }
                if let Some(condition) = &selection.condition {
                    new.insert("if", toml_edit::value(condition.as_str()));
                }
                new.insert("step", toml_edit::value(selection.step.as_str()));
                new.insert("group", toml_edit::value(selection.group.as_str()));
                new.insert(
                    "options",
                    Item::Value(Value::Array(
                        selection.options.iter().map(String::as_str).collect(),
                    )),
                );
                tables.push(new);
            }
            if tables.is_empty() {
                table.remove("fomod_selections");
            } else {
                table.insert("fomod_selections", Item::ArrayOfTables(tables));
            }
        }
        Chosen::Bain(names)
        | Chosen::Wizard {
            subpackages: names, ..
        } => {
            anyhow::ensure!(
                !conditional(table.get("bain_subpackages")),
                "some of '{id}'s BAIN packages depend on the list's questions (an `if`); change \
                 them in the list's file"
            );
            table.insert("bain_subpackages", packages(names));
            if let Chosen::Wizard { exclude, .. } = chosen {
                let item = table
                    .entry("exclude")
                    .or_insert(Item::Value(Value::Array(Array::new())));
                let array = item
                    .as_array_mut()
                    .ok_or_else(|| anyhow::anyhow!("exclude is not a list"))?;
                for plugin in exclude {
                    if position(array, plugin).is_none() {
                        insert_item(array, array.len(), Value::from(plugin.as_str()), false);
                    }
                }
                if array.is_empty() {
                    table.remove("exclude");
                }
            }
        }
    }
    Ok(doc.to_string())
}

/// The docs file without mod `id`'s notes.
pub fn remove_file_docs(text: &str, id: &str) -> anyhow::Result<String> {
    let mut doc = text
        .parse::<DocumentMut>()
        .map_err(|err| anyhow::anyhow!("the docs file is not valid TOML: {err}"))?;
    if let Some(mods) = doc.get_mut("mods").and_then(Item::as_table_like_mut) {
        mods.remove(id);
    }
    // The blank line before the next table stays with it; at the top of the
    // file it would only be noise.
    let out = doc.to_string();
    Ok(if text.starts_with('\n') {
        out
    } else {
        out.trim_start_matches('\n').to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOAD_ORDER: &str = r#"name = "T"
plugins = [
    "Oblivion.esm",
    # Part 1
    "A.esp", # a note on A
    "B.esp",
    # Part 2
    "C.esp",
    "D.esp",
]
"#;

    #[test]
    fn values_from_the_command_line() {
        assert_eq!(parse_value("bain").as_str(), Some("bain"));
        assert_eq!(parse_value("true").as_bool(), Some(true));
        assert_eq!(parse_value("\"true\"").as_str(), Some("true"));
        assert_eq!(
            parse_value("[\"a\", \"b\"]").as_array().map(Array::len),
            Some(2)
        );
        assert_eq!(parse_value("Data Files").as_str(), Some("Data Files"));
    }

    #[test]
    fn moving_a_plugin_keeps_the_headings_where_they_were() {
        let out = move_plugin(LOAD_ORDER, "A.esp", &PluginPosition::After("C.esp".into())).unwrap();
        assert_eq!(
            out,
            r#"name = "T"
plugins = [
    "Oblivion.esm",
    # Part 1
    "B.esp",
    # Part 2
    "C.esp",
    "A.esp", # a note on A
    "D.esp",
]
"#
        );
    }

    #[test]
    fn moving_to_either_end() {
        let last = move_plugin(LOAD_ORDER, "B.esp", &PluginPosition::Last).unwrap();
        assert!(
            last.ends_with("    \"D.esp\",\n    \"B.esp\",\n]\n"),
            "{last}"
        );
        let first = move_plugin(LOAD_ORDER, "D.esp", &PluginPosition::First).unwrap();
        assert!(
            first.starts_with("name = \"T\"\nplugins = [\n    \"D.esp\",\n    \"Oblivion.esm\",\n"),
            "{first}"
        );
        let back = move_plugin(&last, "B.esp", &PluginPosition::Before("C.esp".into())).unwrap();
        assert_eq!(
            back,
            LOAD_ORDER.replace(
                "    \"B.esp\",\n    # Part 2\n",
                "    # Part 2\n    \"B.esp\",\n"
            )
        );
    }

    #[test]
    fn moving_the_last_plugin_of_an_array_without_a_trailing_comma() {
        let text = "plugins = [\n    \"A.esp\",\n    \"B.esp\"\n]\n";
        let out = move_plugin(text, "B.esp", &PluginPosition::First).unwrap();
        assert_eq!(out, "plugins = [\n    \"B.esp\",\n    \"A.esp\"\n]\n");
    }

    #[test]
    fn dropping_plugins_from_one_line_and_many_line_arrays() {
        let text = format!("{LOAD_ORDER}inactive-plugins = [\"B.esp\", \"D.esp\"]\n");
        let out = drop_plugins(&text, &["B.esp".to_string(), "A.esp".to_string()]).unwrap();
        assert!(
            out.contains("    # Part 1\n    # Part 2\n    \"C.esp\""),
            "{out}"
        );
        assert!(!out.contains("a note on A"), "{out}");
        assert!(out.contains("inactive-plugins = [\"D.esp\"]"), "{out}");
    }

    const LIST: &str = r#"name = "T"
inputs.a.type = "bool"
inputs.a.query = "A?"

# about A
[[mods]]
id = "A"
layout = "bain" # chosen by hand

[[mods.archives]]
path = "x"
exclude = [
    "a",
]

# about B
[[mods]]
id = "B"
"#;

    #[test]
    fn setting_fields_keeps_comments() {
        let sets = parse_pairs(&["layout=fomod".into(), "if=a".into()]).unwrap();
        let out = set_fields(LIST, "A", None, &sets, &[]).unwrap();
        assert!(
            out.contains("layout = \"fomod\" # chosen by hand\nif = \"a\"\n"),
            "{out}"
        );
        let out = set_fields(&out, "A", None, &[], &["if".into()]).unwrap();
        assert!(!out.contains("if ="), "{out}");
        let archive = parse_pairs(&["data_folder=Data".into()]).unwrap();
        let out = set_fields(&out, "A", Some(1), &archive, &[]).unwrap();
        assert!(
            out.contains("path = \"x\"\nexclude = [\n    \"a\",\n]\ndata_folder = \"Data\""),
            "{out}"
        );
        assert!(set_fields(LIST, "A", Some(2), &archive, &[]).is_err());
        assert!(
            set_fields(
                LIST,
                "A",
                None,
                &parse_pairs(&["section=[]".into()]).unwrap(),
                &[]
            )
            .is_err()
        );
    }

    #[test]
    fn filters_and_actions() {
        let out = edit_strings(LIST, "A", Some(1), "exclude", &["b".into()], &[]).unwrap();
        assert!(
            out.contains("exclude = [\n    \"a\",\n    \"b\",\n]"),
            "{out}"
        );
        let out = edit_strings(
            &out,
            "A",
            Some(1),
            "exclude",
            &[],
            &["a".into(), "b".into()],
        )
        .unwrap();
        assert!(!out.contains("exclude"), "{out}");
        let out = edit_strings(LIST, "B", None, "plugins", &["B.esp".into()], &[]).unwrap();
        assert!(
            out.ends_with("id = \"B\"\nplugins = [\"B.esp\"]\n"),
            "{out}"
        );

        let fields = parse_pairs(&["paths=[\"x/y\"]".into()]).unwrap();
        let out = add_action(LIST, "A", "file_hide", &fields, Some("a")).unwrap();
        assert!(
            out.contains("\n[[mods.actions]]\naction = \"file_hide\"\nif = \"a\"\npaths = [\"x/y\"]\n\n# about B"),
            "{out}"
        );
        let out = remove_action(&out, "A", 1).unwrap();
        assert_eq!(out, LIST);
    }

    #[test]
    fn inputs_follow_the_lists_style() {
        let fields = parse_pairs(&[
            "type=bool".into(),
            "query=B?".into(),
            "default=false".into(),
        ])
        .unwrap();
        let out = add_input(LIST, "b", &fields).unwrap();
        assert!(
            out.starts_with("name = \"T\"\ninputs.a.type = \"bool\"\ninputs.a.query = \"A?\"\ninputs.b.type = \"bool\"\ninputs.b.query = \"B?\"\ninputs.b.default = false\n"),
            "{out}"
        );
        let out = set_input(
            &out,
            "b",
            &parse_pairs(&["default=true".into()]).unwrap(),
            &[],
        )
        .unwrap();
        assert!(out.contains("inputs.b.default = true"), "{out}");
        let out = remove_input(&out, "b").unwrap();
        assert_eq!(out, LIST);

        let bare = "name = \"T\"\n\n[[mods]]\nid = \"A\"\n";
        let out = add_input(bare, "x", &fields).unwrap();
        assert!(
            out.starts_with("name = \"T\"\ninputs.x.type = \"bool\""),
            "{out}"
        );
    }

    #[test]
    fn a_mods_plain_fields_are_read_for_a_form() {
        let text = "name = \"L\"\n\n[[mods]]\nid = \"A\"\nif = \"big\"\nenabled = false\ndependencies = [\"B\"]\n\n[[mods.archives]]\npath = \"manual:A.zip\"\nlayout = \"bain\"\nbain_subpackages = [{ name = \"00 Core\", if = \"x\" }]\n\n[[mods.actions]]\naction = \"file_hide\"\n";
        let fields = super::field_values(text, "A", None).unwrap();
        let keys: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["id", "if", "enabled", "dependencies"]);
        assert_eq!(fields[2].1, serde_json::json!(false));
        let archive = super::field_values(text, "A", Some(1)).unwrap();
        assert_eq!(
            archive,
            [
                ("path".to_string(), serde_json::json!("manual:A.zip")),
                ("layout".to_string(), serde_json::json!("bain"))
            ]
        );
    }

    #[test]
    fn an_installers_choices_replace_the_old_ones_keeping_comments() {
        use crate::config::inspect::Chosen;
        use crate::config::schema::FomodSelection;
        let text = "name = \"L\"\n\n[[mods]]\nid = \"A\"\n\n[[mods.archives]]\npath = \"x\"\n\n# Lite, for low-end machines.\n[[mods.archives.fomod_selections]]\nstep = \"S\"\ngroup = \"G\"\noptions = [\"Lite\"]\n\n[[mods.actions]]\naction = \"file_hide\"\n";
        let chosen = Chosen::Fomod(vec![
            FomodSelection {
                condition: None,
                step: "S".to_string(),
                group: "G".to_string(),
                options: vec!["Full".to_string()],
            },
            FomodSelection {
                condition: None,
                step: "S".to_string(),
                group: "H".to_string(),
                options: vec![],
            },
        ]);
        let out = super::set_installer(text, "A", 1, &chosen, false).unwrap();
        assert!(
            out.contains("# Lite, for low-end machines.\n[[mods.archives.fomod_selections]]\nstep = \"S\"\ngroup = \"G\"\noptions = [\"Full\"]"),
            "{out}"
        );
        assert!(out.contains("group = \"H\"\noptions = []"), "{out}");
        assert!(out.find("group = \"H\"").unwrap() < out.find("[[mods.actions]]").unwrap());

        let bain = "name = \"L\"\n\n[[mods]]\nid = \"A\"\n\n[[mods.archives]]\npath = \"x\"\nbain_subpackages = [\"00 Core\"]\nexclude = [\"docs/**\"]\n";
        let out = super::set_installer(
            bain,
            "A",
            1,
            &Chosen::Wizard {
                subpackages: vec!["00 Core".to_string(), "10 Extra".to_string()],
                exclude: vec!["Extra.esp".to_string()],
            },
            false,
        )
        .unwrap();
        assert!(
            out.contains("bain_subpackages = [\"00 Core\", \"10 Extra\"]"),
            "{out}"
        );
        assert!(
            out.contains("exclude = [\"docs/**\", \"Extra.esp\"]"),
            "{out}"
        );

        let conditional = bain.replace("[\"00 Core\"]", "[{ name = \"00 Core\", if = \"x\" }]");
        assert!(super::set_installer(&conditional, "A", 1, &Chosen::Bain(vec![]), false).is_err());
    }
    #[test]
    fn fomod_rows_that_depend_on_the_questions_are_written_with_their_if() {
        use crate::config::inspect::Chosen;
        use crate::config::schema::FomodSelection;
        let text = "name = \"L\"\n\n[[mods]]\nid = \"A\"\n\n[[mods.archives]]\npath = \"x\"\n\n# Small for a slow machine.\n[[mods.archives.fomod_selections]]\nif = \"potato\"\nstep = \"S\"\ngroup = \"G\"\noptions = [\"Lite\"]\n";
        let row = |condition: Option<&str>, option: &str| FomodSelection {
            condition: condition.map(str::to_string),
            step: "S".to_string(),
            group: "G".to_string(),
            options: vec![option.to_string()],
        };
        let chosen = Chosen::Fomod(vec![
            row(Some("potato"), "Lite"),
            row(Some("!potato"), "Full"),
        ]);
        // A walk that did not show the rows would drop them: refused.
        assert!(super::set_installer(text, "A", 1, &chosen, false).is_err());
        let out = super::set_installer(text, "A", 1, &chosen, true).unwrap();
        assert!(
            out.contains("# Small for a slow machine.\n[[mods.archives.fomod_selections]]\nif = \"potato\"\nstep = \"S\"\ngroup = \"G\"\noptions = [\"Lite\"]"),
            "{out}"
        );
        assert!(
            out.contains("[[mods.archives.fomod_selections]]\nif = \"!potato\"\nstep = \"S\"\ngroup = \"G\"\noptions = [\"Full\"]"),
            "{out}"
        );
    }

    #[test]
    fn an_ini_layer_replaces_only_its_own_actions() {
        use crate::config::ini_edit::{IniKey, IniLayer, IniSetting};
        use crate::config::schema::IniSetFormat;
        let text = "name = \"L\"\n\n[[mods]]\nid = \"A\"\n\n[[mods.actions]]\naction = \"file_hide\"\npaths = [\"a\"]\n\n# Smaller shadows for a slow machine.\n[[mods.actions]]\naction = \"ini_set\"\nif = \"potato\"\nfile = \"ini/a.ini\"\nsection = \"Display\"\nkey = \"Shadows\"\nvalue = \"1024\"\n\n[[mods.actions]]\naction = \"ini_set\"\nif = \"potato\"\nfile = \"ini/a.ini\"\nkey = \"Gone\"\nvalue = \"1\"\n\n[[mods.actions]]\naction = \"ini_set\"\nfile = \"ini/a.ini\"\nkey = \"Other\"\nvalue = \"2\"\n";
        let layer = IniLayer {
            file: "INI\\A.ini".to_string(),
            condition: Some("potato".to_string()),
            sets: vec![
                IniSetting {
                    section: Some("Display".to_string()),
                    key: "Shadows".to_string(),
                    value: "512".to_string(),
                    format: IniSetFormat::Standard,
                },
                IniSetting {
                    section: None,
                    key: "dc.x".to_string(),
                    value: "70".to_string(),
                    format: IniSetFormat::SetTo,
                },
            ],
            comments: vec![IniKey {
                key: "Old".to_string(),
                format: IniSetFormat::Standard,
            }],
            block: Some("Extra=1\n".to_string()),
            ..IniLayer::default()
        };
        let out = super::set_ini_layer(text, "A", &layer).unwrap();
        // The settings are one ini_set per format: the first kept in place,
        // comment and all, holding them by section; the one not given is
        // gone; another condition's, and other actions, are left alone.
        assert!(out.contains("# Smaller shadows for a slow machine.\n[[mods.actions]]\naction = \"ini_set\"\nif = \"potato\"\nfile = \"ini/a.ini\"\n\n[mods.actions.values.Display]\nShadows = \"512\""), "{out}");
        assert!(!out.contains("Gone"), "{out}");
        assert!(out.contains("key = \"Other\"\nvalue = \"2\""), "{out}");
        assert!(out.contains("action = \"file_hide\""), "{out}");
        assert!(
            out.contains("format = \"set-to\"\n\n[mods.actions.values]\n\"dc.x\" = \"70\""),
            "{out}"
        );
        // And it reads back as the settings written.
        let list: crate::config::schema::SourceModlist = toml::from_str(&out).unwrap();
        let sets: Vec<String> = list.mods[0]
            .actions
            .iter()
            .filter_map(|a| match &a.action {
                crate::config::schema::ModAction::IniSet(set) => Some(set),
                _ => None,
            })
            .flat_map(|set| {
                set.settings()
                    .into_iter()
                    .map(|s| format!("{:?} {}={}", s.section, s.key, s.value))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(
            sets,
            [
                "Some(\"Display\") Shadows=512",
                "None Other=2",
                "None dc.x=70"
            ]
        );
        assert!(
            out.contains(
                "action = \"ini_comment\"\nif = \"potato\"\nfile = 'INI\\A.ini'\nkeys = [\"Old\"]"
            ),
            "{out}"
        );
        assert!(out.contains("action = \"ini_append_block\""), "{out}");

        // An empty layer takes its actions out, and only them.
        let empty = IniLayer {
            file: "ini/a.ini".to_string(),
            condition: Some("potato".to_string()),
            ..IniLayer::default()
        };
        let out = super::set_ini_layer(&out, "A", &empty).unwrap();
        assert!(!out.contains("potato"), "{out}");
        assert!(out.contains("key = \"Other\""), "{out}");
    }

    #[test]
    fn an_ini_layer_moved_to_another_condition_leaves_the_old_one() {
        use crate::config::ini_edit::{IniLayer, IniSetting, MovedFrom};
        use crate::config::schema::IniSetFormat;
        let text = "name = \"L\"\n\n[[mods]]\nid = \"A\"\n\n[[mods.actions]]\naction = \"ini_set\"\nfile = \"a.ini\"\nkey = \"Shadows\"\nvalue = \"1024\"\n\n[[mods.actions]]\naction = \"ini_set\"\nif = \"res\"\nfile = \"a.ini\"\nkey = \"Other\"\nvalue = \"1\"\n";
        let layer = IniLayer {
            file: "a.ini".to_string(),
            condition: Some("potato".to_string()),
            sets: vec![IniSetting {
                section: None,
                key: "Shadows".to_string(),
                value: "512".to_string(),
                format: IniSetFormat::Standard,
            }],
            moved_from: Some(MovedFrom { condition: None }),
            ..IniLayer::default()
        };
        let out = super::set_ini_layer(text, "A", &layer).unwrap();
        assert!(!out.contains("1024"), "{out}");
        assert!(out.contains("if = \"potato\""), "{out}");
        assert!(out.contains("Shadows = \"512\""), "{out}");
        assert!(
            out.contains("key = \"Other\""),
            "another condition is left: {out}"
        );
    }

    #[test]
    fn an_installer_set_is_written_beside_the_others() {
        use crate::config::inspect::Chosen;
        use crate::config::schema::FomodSelection;
        let fomod = "name = \"L\"\n\n[[mods]]\nid = \"A\"\n\n[[mods.archives]]\npath = \"x\"\n\n# The usual size.\n[[mods.archives.fomod_selections]]\nstep = \"S\"\ngroup = \"Size\"\noptions = [\"1K\"]\n";
        let row = |group: &str, option: &str| FomodSelection {
            condition: None,
            step: "S".to_string(),
            group: group.to_string(),
            options: vec![option.to_string()],
        };
        let chosen = Chosen::Fomod(vec![row("Size", "2K")]);
        let out = super::set_installer_set(fomod, "A", 1, &chosen, Some("res == 2k")).unwrap();
        assert!(out.contains("# The usual size.\n[[mods.archives.fomod_selections]]\nstep = \"S\"\ngroup = \"Size\"\noptions = [\"1K\"]"), "{out}");
        assert!(out.contains("[[mods.archives.fomod_selections]]\nif = \"res == 2k\"\nstep = \"S\"\ngroup = \"Size\"\noptions = [\"2K\"]"), "{out}");

        let bain = "name = \"L\"\n\n[[mods]]\nid = \"A\"\n\n[[mods.archives]]\npath = \"x\"\nbain_subpackages = [\"00 Core\", \"01 1K\"]\n";
        let chosen = Chosen::Bain(vec!["00 Core".to_string(), "02 2K".to_string()]);
        let out = super::set_installer_set(bain, "A", 1, &chosen, Some("res == 2k")).unwrap();
        assert!(
            out.contains("bain_subpackages = [\"00 Core\", { name = \"01 1K\", if = \"res != 2k\" }, { name = \"02 2K\", if = \"res == 2k\" }]"),
            "{out}"
        );
        // A wizard's choices go only in the base set.
        let wizard = Chosen::Wizard {
            subpackages: vec![],
            exclude: vec![],
        };
        assert!(super::set_installer_set(bain, "A", 1, &wizard, Some("x")).is_err());
    }
}
