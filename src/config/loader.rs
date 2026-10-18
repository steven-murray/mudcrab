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
//! including itself, however indirectly, is refused. `include` can also name
//! a file in a git repository (`git+<url>?tag=<tag>#<file>`); see
//! [`crate::config::git`]. See docs/design/composability.md.

use crate::config::docs::{Docs, DocsFile};
use crate::config::git::{self, Lock};
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

/// One include, as the loader resolved it.
#[derive(Debug, Clone)]
pub struct Included {
    /// The `[[mods]]` entry's id.
    pub id: String,
    /// Its `include`, as written.
    pub source: String,
    /// The file read: on disk, or in the cache for a git include.
    pub file: PathBuf,
    /// The commit, for a git include.
    pub commit: Option<String>,
    /// The file that includes it.
    pub by: PathBuf,
    /// How many includes deep: 1 for the list's own.
    pub depth: usize,
}

/// Load the modlist at `path`, reading any file in `texts` from there rather
/// than from disk. Git includes resolve through the list's lock file, which
/// is written if any had to be resolved afresh.
pub fn load_with(path: &Path, with_docs: bool, texts: &Texts) -> anyhow::Result<SourceModlist> {
    let mut lock = Lock::read(path)?;
    let list = load_locked(path, with_docs, texts, &mut lock)?;
    lock.save()?;
    Ok(list)
}

/// [`load_with`], with git includes resolved through `lock`, which is left
/// for the caller to save (or not).
pub fn load_locked(
    path: &Path,
    with_docs: bool,
    texts: &Texts,
    lock: &mut Lock,
) -> anyhow::Result<SourceModlist> {
    let mut context = Context {
        with_docs,
        texts,
        lock,
        stack: Vec::new(),
    };
    resolve(&normalize(path), &mut context)
}

/// What resolving a list's includes carries along.
struct Context<'a> {
    with_docs: bool,
    texts: &'a Texts,
    lock: &'a mut Lock,
    /// The files being read, outermost first.
    stack: Vec<PathBuf>,
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
/// merged in.
fn resolve(path: &Path, context: &mut Context) -> anyhow::Result<SourceModlist> {
    if let Some(at) = context.stack.iter().position(|file| file == path) {
        let chain = context.stack[at..]
            .iter()
            .chain([&path.to_path_buf()])
            .map(|file| file.display().to_string())
            .collect::<Vec<_>>()
            .join(" includes ");
        anyhow::bail!("{} includes itself: {chain}", path.display());
    }
    let text = read(path, context.texts)?;
    let mut parsed = parse_file(&text, path)?;
    for entry in parsed.mods.iter().filter(|entry| entry.include.is_none()) {
        no_include_options(entry)?;
    }
    context.stack.push(path.to_path_buf());
    let spliced = splice(&mut parsed, &text, path, context);
    context.stack.pop();
    spliced?;

    // After the includes, so a list's docs file can document the mods its
    // included files bring.
    if context.with_docs
        && let Some(docs_file) = parsed.docs_file.clone()
    {
        let docs_path = normalize(&parsed.base_dir.clone().unwrap_or_default().join(&docs_file));
        let raw =
            read(&docs_path, context.texts).map_err(|err| anyhow::anyhow!("docs file: {err}"))?;
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
    context: &mut Context,
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
        let (child_path, commit) = locate(&base, &include, &entry.id, path, context, list)?;
        let child = resolve(&child_path, context).map_err(|err| {
            anyhow::anyhow!(
                "{}, included as '{}' by {}: {err:#}",
                shown(&child_path),
                entry.id,
                shown(path)
            )
        })?;
        list.includes.push(Included {
            id: entry.id.clone(),
            source: include.clone(),
            file: child_path.clone(),
            commit,
            by: path.to_path_buf(),
            depth: 1,
        });
        list.includes
            .extend(child.includes.iter().cloned().map(|mut nested| {
                nested.depth += 1;
                nested
            }));
        list.include_warnings
            .extend(child.include_warnings.iter().cloned());
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
            some(fragment)
        );
    }
    list.plugins = plugins;
    Ok(())
}

/// A file as messages show it: a git include's by its repository and commit.
fn shown(path: &Path) -> String {
    git::describe(path).unwrap_or_else(|| path.display().to_string())
}

/// The file `include` (in a file in `base`) names, and its commit for a git
/// include. A relative path inside a git repository stays inside it.
fn locate(
    base: &Path,
    include: &str,
    id: &str,
    by: &Path,
    context: &mut Context,
    list: &mut SourceModlist,
) -> anyhow::Result<(PathBuf, Option<String>)> {
    if let Some(source) = git::parse(include) {
        let source = source?;
        if let git::Reference::Branch(branch) = &source.reference {
            list.include_warnings.push(format!(
                "include '{id}' follows branch {branch}, which moves: once the component is                  released, name a tag (or a commit) so the list stays the same"
            ));
        }
        let resolved = git::resolve(&source, context.lock)
            .map_err(|err| anyhow::anyhow!("include '{id}' (in {}): {err:#}", shown(by)))?;
        return Ok((resolved.file, Some(resolved.commit)));
    }
    let path = normalize(&base.join(include));
    if let Some(checkout) = git::checkout_of(base) {
        anyhow::ensure!(
            path.starts_with(&checkout),
            "include '{id}' in {} names {include}, outside its repository",
            shown(by)
        );
        // The same commit's: no lock entry of its own.
        let commit = checkout
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        return Ok((path, commit));
    }
    Ok((path, None))
}

/// The first few of `names`, and how many more.
fn some(names: &[String]) -> String {
    const SHOWN: usize = 3;
    let mut text = names[..names.len().min(SHOWN)].join(", ");
    if names.len() > SHOWN {
        text.push_str(&format!(", and {} more", names.len() - SHOWN));
    }
    text
}

/// An include entry names a file and where its mods go, and nothing else:
/// anything a mod has would belong to no mod.
fn only_an_include(entry: &ModEntry) -> anyhow::Result<()> {
    let extra = [
        ("oracle_name", entry.oracle_name.is_some()),
        ("dependencies", !entry.dependencies.is_empty()),
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
        "'{}' includes a file, so it has only `id`, `section`, `include`, `if` and the include \
         options (`prefix`, `only`, `except`, `override`); it also has {}",
        entry.id,
        set.join(", ")
    );
    Ok(())
}

/// A mod (not an include) with an include's options: they would do nothing.
fn no_include_options(entry: &ModEntry) -> anyhow::Result<()> {
    let set: Vec<&str> = [
        ("prefix", entry.prefix.is_some()),
        ("only", !entry.only.is_empty()),
        ("except", !entry.except.is_empty()),
        ("override", !entry.overrides.is_empty()),
    ]
    .iter()
    .filter(|(_, set)| *set)
    .map(|(name, _)| *name)
    .collect();
    anyhow::ensure!(
        set.is_empty(),
        "mod '{}' has {}, which only an include has",
        entry.id,
        set.join(", ")
    );
    Ok(())
}

/// The included mods as the include entry asks for them: `only`, `except`
/// and `skip` take some out, `override` changes fields, `prefix` renames
/// them (and every mention of them among themselves), and `if` puts a
/// condition on each. Also the plugins only the mods taken out declare.
fn shape(entry: &ModEntry, mods: Vec<ModEntry>) -> anyhow::Result<(Vec<ModEntry>, Vec<String>)> {
    let ids: HashSet<&str> = mods.iter().map(|m| m.id.as_str()).collect();
    let named = entry
        .only
        .iter()
        .map(|id| ("only", id))
        .chain(entry.except.iter().map(|id| ("except", id)))
        .chain(entry.overrides.keys().map(|id| ("override", id)));
    for (option, id) in named {
        // An option that outlived its mod would quietly do nothing.
        anyhow::ensure!(
            ids.contains(id.as_str()),
            "{option} names '{id}', which is not one of the included file's mods"
        );
    }
    let skipped = |id: &str| {
        entry.overrides.get(id).and_then(|o| o.get("skip")) == Some(&toml::Value::Boolean(true))
    };
    let (kept, dropped): (Vec<ModEntry>, Vec<ModEntry>) = mods.into_iter().partition(|m| {
        (entry.only.is_empty() || entry.only.contains(&m.id))
            && !entry.except.contains(&m.id)
            && !skipped(&m.id)
    });
    let mut kept = kept
        .into_iter()
        .map(|m| match entry.overrides.get(&m.id) {
            Some(fields) => apply_override(m, fields),
            None => Ok(m),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let dropped_plugins = dropped
        .iter()
        .flat_map(|m| m.plugins.iter())
        .filter(|plugin| {
            !kept
                .iter()
                .any(|m| m.plugins.iter().any(|p| p.eq_ignore_ascii_case(plugin)))
        })
        .cloned()
        .collect();

    if let Some(prefix) = &entry.prefix {
        let own: HashSet<String> = kept.iter().map(|m| m.id.clone()).collect();
        let rename = |id: &str| own.contains(id).then(|| format!("{prefix}{id}"));
        for m in &mut kept {
            rename_in(m, &rename)?;
        }
    }
    if let Some(condition) = &entry.condition {
        for m in &mut kept {
            m.condition = Some(match &m.condition {
                Some(own) => crate::config::condition::and(condition, own)
                    .map_err(|err| anyhow::anyhow!("mod '{}': {err:#}", m.id))?,
                None => condition.clone(),
            });
        }
    }
    Ok((kept, dropped_plugins))
}

/// Mod `m` renamed by `rename`, with every mod it names that `rename`
/// renames: its dependencies, conflict partners, merge sources and the mods
/// its conditions ask about.
fn rename_in(m: &mut ModEntry, rename: &dyn Fn(&str) -> Option<String>) -> anyhow::Result<()> {
    use crate::config::condition::Condition;
    use crate::config::schema::{BainSubpackage, ModAction};
    let renamed = |id: &mut String| {
        if let Some(new) = rename(id) {
            *id = new;
        }
    };
    let condition = |text: &mut String| -> anyhow::Result<()> {
        let mut parsed = Condition::parse(text)?;
        if parsed.mods().iter().any(|id| rename(id).is_some()) {
            parsed.rename_mods(rename);
            *text = parsed.to_text();
        }
        Ok(())
    };
    if let Some(new) = rename(&m.id) {
        m.written_id
            .get_or_insert_with(|| std::mem::replace(&mut m.id, new.clone()));
        m.id = new;
    }
    m.dependencies.iter_mut().for_each(renamed);
    if let Some(text) = &mut m.condition {
        condition(text)?;
    }
    for action in &mut m.actions {
        if let Some(text) = &mut action.condition {
            condition(text)?;
        }
        match &mut action.action {
            ModAction::FilePrune(prune) => prune.conflicts_with.iter_mut().for_each(renamed),
            ModAction::FileHide(hide) => hide.conflicts_with.iter_mut().for_each(renamed),
            _ => {}
        }
    }
    for archive in &mut m.archives {
        if let Some(text) = &mut archive.condition {
            condition(text)?;
        }
        for selection in &mut archive.fomod_selections {
            if let Some(text) = &mut selection.condition {
                condition(text)?;
            }
        }
        for package in &mut archive.bain_subpackages {
            if let BainSubpackage::Conditional {
                condition: text, ..
            } = package
            {
                condition(text)?;
            }
        }
    }
    if let Some(merge) = &mut m.merge {
        for source in &mut merge.sources {
            renamed(&mut source.mod_id);
        }
    }
    Ok(())
}

/// Mod `m` with an override's `fields` in place of its own. A list field
/// given as `{ append = [...] }` gains those entries instead.
fn apply_override(m: ModEntry, fields: &toml::Table) -> anyhow::Result<ModEntry> {
    let id = m.id.clone();
    let fail = |err: String| anyhow::anyhow!("override of '{id}': {err}");
    let mut table = match toml::Value::try_from(&m).map_err(|err| fail(err.to_string()))? {
        toml::Value::Table(table) => table,
        _ => unreachable!("a mod is a table"),
    };
    for (key, value) in fields {
        match key.as_str() {
            "skip" => continue,
            "id" | "include" => return Err(fail(format!("`{key}` cannot be overridden"))),
            _ => {}
        }
        let append = match value {
            toml::Value::Table(inner) if inner.len() == 1 => inner.get("append"),
            _ => None,
        };
        match append {
            Some(toml::Value::Array(more)) => match table
                .entry(key.clone())
                .or_insert_with(|| toml::Value::Array(Vec::new()))
            {
                toml::Value::Array(have) => have.extend(more.iter().cloned()),
                _ => return Err(fail(format!("`{key}` is not a list to append to"))),
            },
            Some(_) => return Err(fail(format!("`{key}.append` takes a list"))),
            None => {
                table.insert(key.clone(), value.clone());
            }
        }
    }
    let mut changed: ModEntry = toml::Value::Table(table)
        .try_into()
        .map_err(|err: toml::de::Error| fail(err.message().to_string()))?;
    // What the loader keeps about the mod is not in its fields.
    changed.origin = m.origin;
    changed.inherited_sections = m.inherited_sections;
    changed.written_id = m.written_id;
    changed.overridden = m.overridden;
    changed
        .overridden
        .extend(fields.keys().filter(|key| key.as_str() != "skip").cloned());
    Ok(changed)
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
    // Images are named relative to the file that names them: from a file
    // elsewhere (a git include's, in the cache), by its whole path.
    let folder = if git::parse(include).is_some() || git::checkout_of(child_path).is_some() {
        child_path
            .parent()
            .map(|folder| folder.display().to_string())
    } else {
        include
            .rsplit_once('/')
            .map(|(folder, _)| folder.to_string())
    };
    let rebase = |docs: &mut Docs| {
        if let Some(folder) = &folder {
            for image in &mut docs.images {
                if let Some(path) = &mut image.path
                    && !Path::new(path.as_str()).is_absolute()
                {
                    *path = format!("{folder}/{path}");
                }
            }
        }
    };

    let (child_mods, dropped) = shape(entry, child.mods)?;
    for mut spliced in child_mods {
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
    let kept = |plugin: &String| !dropped.iter().any(|d| d.eq_ignore_ascii_case(plugin));
    list.inactive_plugins
        .extend(child.inactive_plugins.into_iter().filter(kept));
    list.manual_steps.extend(child.manual_steps);
    list.post_install_actions.extend(child.post_install_actions);
    Ok(child.plugins.into_iter().filter(kept).collect())
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
