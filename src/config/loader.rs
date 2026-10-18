//! Reading a modlist: its file, the files it includes, and its docs file,
//! into one [`SourceModlist`] that everything after the loader reads as if it
//! had been written in one place.
//!
//! An include is a `[[mods]]` entry with `include = "<path>"` (relative to the
//! file it is in) instead of archives. The loader puts the included file's
//! mods where the entry is, each with the entry's `section` before its own,
//! and its plugins where the load order says `{ include = "<the entry's
//! id>" }`. Its `[ini]`, manual steps and post-install actions are added to
//! the list's; its `name`, `guide`, `mo2`, `export` and `about` describe a
//! whole list and are not read. Included files can include others; a file
//! including itself, however indirectly, is refused. See
//! docs/design/composability.md.

use crate::config::docs::{Docs, DocsFile};
use crate::config::schema::{ModEntry, PluginSlot, SourceModlist};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

/// Text to read in place of a file's, keyed by its path (as [`normalize`]
/// leaves it): an edit being checked before it is written.
pub type Texts = HashMap<PathBuf, String>;

/// Load a modlist, without its `docs_file`: only the guide needs the prose,
/// so a list shared without its docs file still compiles and installs.
pub fn load_modlist(path: &Path) -> anyhow::Result<SourceModlist> {
    load_with(path, false, &Texts::new())
}

/// Load a modlist with its `docs_file` folded in, for `export` and `validate`.
pub fn load_modlist_with_docs(path: &Path) -> anyhow::Result<SourceModlist> {
    load_with(path, true, &Texts::new())
}

/// Load the modlist at `path`, reading any file in `texts` from there rather
/// than from disk.
pub fn load_with(path: &Path, with_docs: bool, texts: &Texts) -> anyhow::Result<SourceModlist> {
    resolve(&normalize(path), with_docs, texts, &mut Vec::new())
}

/// `text` as the modlist at `path` (without its docs file), with the files it
/// includes read from disk: for checking an edit before it is written.
pub fn parse_modlist(text: &str, path: &Path) -> anyhow::Result<SourceModlist> {
    load_with(
        path,
        false,
        &Texts::from([(normalize(path), text.to_string())]),
    )
}

/// `text` as the one file at `path`, its includes left as they are: what an
/// edit of that file reads, as it changes that file's lines alone. Its
/// `plugins` leave out where included plugins go.
pub fn parse_file(text: &str, path: &Path) -> anyhow::Result<SourceModlist> {
    let mut parsed = toml::from_str::<SourceModlist>(text)
        .map_err(|err| anyhow::anyhow!("failed to parse TOML {}: {err}", path.display()))?;
    parsed.base_dir = Some(path.parent().map(Path::to_path_buf).unwrap_or_default());
    Ok(parsed)
}

/// `path` with `.` and `..` taken out where they can be, so one file has one
/// name however it was reached.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn read(path: &Path, texts: &Texts) -> anyhow::Result<String> {
    match texts.get(path) {
        Some(text) => Ok(text.clone()),
        None => std::fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display())),
    }
}

/// The file at `path` with what it includes put in place, then its docs file
/// merged in. `stack` is the files being read, outermost first.
fn resolve(
    path: &Path,
    with_docs: bool,
    texts: &Texts,
    stack: &mut Vec<PathBuf>,
) -> anyhow::Result<SourceModlist> {
    if let Some(at) = stack.iter().position(|file| file == path) {
        let chain = stack[at..]
            .iter()
            .chain([&path.to_path_buf()])
            .map(|file| file.display().to_string())
            .collect::<Vec<_>>()
            .join(" includes ");
        anyhow::bail!("{} includes itself: {chain}", path.display());
    }
    let text = read(path, texts)?;
    let mut parsed = parse_file(&text, path)?;
    stack.push(path.to_path_buf());
    let spliced = splice(&mut parsed, &text, path, with_docs, texts, stack);
    stack.pop();
    spliced?;

    // After the includes, so a list's docs file can document the mods its
    // included files bring.
    if with_docs && let Some(docs_file) = parsed.docs_file.clone() {
        let docs_path = normalize(&parsed.base_dir.clone().unwrap_or_default().join(&docs_file));
        let raw = read(&docs_path, texts).map_err(|err| anyhow::anyhow!("docs file: {err}"))?;
        let docs: DocsFile = toml::from_str(&raw).map_err(|err| {
            anyhow::anyhow!("failed to parse docs file {}: {err}", docs_path.display())
        })?;
        merge_docs(&mut parsed, docs).map_err(|err| anyhow::anyhow!("{docs_file}: {err}"))?;
    }
    Ok(parsed)
}

/// The load order as the file writes it, includes and all.
fn plugin_slots(text: &str) -> anyhow::Result<Vec<PluginSlot>> {
    #[derive(serde::Deserialize)]
    struct Slots {
        #[serde(default)]
        plugins: Vec<PluginSlot>,
    }
    Ok(toml::from_str::<Slots>(text)?.plugins)
}

/// Put each included file's mods and plugins where `list` says.
fn splice(
    list: &mut SourceModlist,
    text: &str,
    path: &Path,
    with_docs: bool,
    texts: &Texts,
    stack: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let slots = plugin_slots(text)?;
    let placing = slots
        .iter()
        .any(|slot| matches!(slot, PluginSlot::Include { .. }));
    if !placing && list.mods.iter().all(|entry| entry.include.is_none()) {
        return Ok(());
    }

    let base = list.base_dir.clone().unwrap_or_default();
    let mut mods = Vec::with_capacity(list.mods.len());
    // Each include's plugins, by its id, in its own order.
    let mut fragments: IndexMap<String, Vec<String>> = IndexMap::new();
    for entry in std::mem::take(&mut list.mods) {
        let Some(include) = entry.include.clone() else {
            mods.push(entry);
            continue;
        };
        only_an_include(&entry)?;
        anyhow::ensure!(
            !fragments.contains_key(&entry.id),
            "two includes are called '{}'; each needs its own id",
            entry.id
        );
        let child_path = normalize(&base.join(&include));
        let child = resolve(&child_path, with_docs, texts, stack).map_err(|err| {
            anyhow::anyhow!(
                "{}, included as '{}' by {}: {err:#}",
                child_path.display(),
                entry.id,
                path.display()
            )
        })?;
        list.included.push(child_path.clone());
        list.included.extend(child.included.iter().cloned());
        let fragment = absorb(list, &mut mods, &entry, &include, &child_path, child)
            .map_err(|err| anyhow::anyhow!("include '{}' ({include}): {err:#}", entry.id))?;
        fragments.insert(entry.id, fragment);
    }
    list.mods = mods;

    let mut plugins = Vec::new();
    let mut placed = HashSet::new();
    for slot in slots {
        match slot {
            PluginSlot::Plugin(name) => plugins.push(name),
            PluginSlot::Include { include: id } => {
                let fragment = fragments.get(&id).ok_or_else(|| {
                    anyhow::anyhow!(
                        "plugins has {{ include = {id:?} }}, but no [[mods]] entry with that id \
                         includes a file"
                    )
                })?;
                anyhow::ensure!(
                    placed.insert(id.clone()),
                    "plugins places include '{id}' twice"
                );
                plugins.extend(fragment.iter().cloned());
            }
        }
    }
    // As a mod's plugin missing from the load order is an error, so is an
    // include's: the order cannot be guessed.
    for (id, fragment) in &fragments {
        anyhow::ensure!(
            fragment.is_empty() || placed.contains(id),
            "include '{id}' has plugins ({}), and `plugins` does not say where they go: add \
             {{ include = \"{id}\" }} to it where they belong in the load order",
            fragment.join(", ")
        );
    }
    list.plugins = plugins;
    Ok(())
}

/// An include entry names a file and where its mods go, and nothing else:
/// anything a mod has would belong to no mod.
fn only_an_include(entry: &ModEntry) -> anyhow::Result<()> {
    let extra = [
        ("oracle_name", entry.oracle_name.is_some()),
        ("dependencies", !entry.dependencies.is_empty()),
        ("if", entry.condition.is_some()),
        ("archives", !entry.archives.is_empty()),
        ("type", entry.mod_type.is_some()),
        ("enabled", !entry.enabled),
        ("merge", entry.merge.is_some()),
        ("plugins", !entry.plugins.is_empty()),
        ("files", !entry.files.is_empty()),
        ("docs", !entry.docs.is_empty()),
        ("actions", !entry.actions.is_empty()),
    ];
    let set: Vec<&str> = extra
        .iter()
        .filter(|(_, set)| *set)
        .map(|(name, _)| *name)
        .collect();
    anyhow::ensure!(
        set.is_empty(),
        "'{}' includes a file, so it has only `id`, `section` and `include`; it also has {}",
        entry.id,
        set.join(", ")
    );
    Ok(())
}

/// Add what `child` (included by `entry` as `include`) brings to `list`, its
/// mods to `mods`, and return its plugins, for the load order to place.
fn absorb(
    list: &mut SourceModlist,
    mods: &mut Vec<ModEntry>,
    entry: &ModEntry,
    include: &str,
    child_path: &Path,
    child: SourceModlist,
) -> anyhow::Result<Vec<String>> {
    anyhow::ensure!(
        child.inputs.is_empty(),
        "it has questions ([inputs]), which an included file cannot have yet: ask them in the \
         including list, whose questions an included file's conditions can use"
    );
    anyhow::ensure!(
        child.docs.is_empty(),
        "it has the guide's [docs], which are for a whole list; an included file documents its \
         mods and [sections]"
    );
    // Images are named relative to the file that names them.
    let folder = include.rsplit_once('/').map(|(folder, _)| folder);
    let rebase = |docs: &mut Docs| {
        if let Some(folder) = folder {
            for image in &mut docs.images {
                if let Some(path) = &mut image.path {
                    *path = format!("{folder}/{path}");
                }
            }
        }
    };

    for mut spliced in child.mods {
        spliced.section = entry
            .section
            .iter()
            .cloned()
            .chain(spliced.section)
            .collect();
        spliced.inherited_sections += entry.section.len();
        spliced
            .origin
            .get_or_insert_with(|| child_path.to_path_buf());
        rebase(&mut spliced.docs);
        mods.push(spliced);
    }
    // MO2 names a nested separator by its path, joined with " - ".
    for (name, mut docs) in child.sections {
        let name = entry
            .section
            .iter()
            .map(String::as_str)
            .chain([name.as_str()])
            .collect::<Vec<_>>()
            .join(" - ");
        anyhow::ensure!(
            !list.sections.contains_key(&name),
            "section {name:?} is documented both here and in the including list"
        );
        rebase(&mut docs);
        list.sections.insert(name, docs);
    }
    merge_ini(&mut list.ini, child.ini)?;
    list.inactive_plugins.extend(child.inactive_plugins);
    list.manual_steps.extend(child.manual_steps);
    list.post_install_actions.extend(child.post_install_actions);
    Ok(child.plugins)
}

/// Add an included file's `[ini]` to the list's. A key both set, to different
/// values, is refused: which should win is not for the loader to guess.
fn merge_ini(into: &mut toml::Table, from: toml::Table) -> anyhow::Result<()> {
    for (key, value) in from {
        match (into.get_mut(&key), value) {
            (None, value) => {
                into.insert(key, value);
            }
            (Some(toml::Value::Table(have)), toml::Value::Table(more)) => {
                for (inner, value) in more {
                    match have.get(&inner) {
                        Some(had) if *had != value => anyhow::bail!(
                            "it sets [ini.{key}] {inner} = {value}, and the including list sets \
                             it to {had}"
                        ),
                        _ => {
                            have.insert(inner, value);
                        }
                    }
                }
            }
            (Some(had), value) => anyhow::ensure!(
                *had == value,
                "it sets ini {key} = {value}, and the including list sets it to {had}"
            ),
        }
    }
    Ok(())
}

/// Fold a docs file into the modlist. Anything documented in both is refused:
/// silently preferring one copy would leave the other to be edited in vain.
fn merge_docs(modlist: &mut SourceModlist, file: DocsFile) -> anyhow::Result<()> {
    if let Some(about) = file.about {
        anyhow::ensure!(
            modlist.about.is_none(),
            "`about` is in both the modlist and the docs file"
        );
        modlist.about = Some(about);
    }
    if let Some(docs) = file.docs {
        anyhow::ensure!(
            modlist.docs.is_empty(),
            "the guide's [docs] are in both the modlist and the docs file"
        );
        modlist.docs = docs;
    }
    for (name, docs) in file.sections {
        anyhow::ensure!(
            !modlist.sections.contains_key(&name),
            "section {name:?} is documented in both the modlist and the docs file"
        );
        modlist.sections.insert(name, docs);
    }
    for (id, docs) in file.mods {
        let entry = modlist
            .mods
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or_else(|| {
                anyhow::anyhow!("documents mod {id:?}, which the modlist does not have")
            })?;
        anyhow::ensure!(
            entry.docs.is_empty(),
            "mod {id:?} is documented in both the modlist and the docs file"
        );
        entry.docs = docs;
    }
    Ok(())
}
