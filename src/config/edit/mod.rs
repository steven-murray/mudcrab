//! Changing a modlist file: the one way `add`, `move`, `set` and the other
//! editing commands -- and a GUI -- write to one.
//!
//! An edit is planned as text: the file is read, the change is spliced into
//! its lines (see `config::add` for why nothing is re-emitted), and the result
//! is parsed and validated in memory. Only an edit that leaves a valid list is
//! ever written, and it is written whole, through a temporary file, and only
//! if the file has not changed on disk since it was read. An [`Edit`] holds
//! the text before and after, so a caller can show the diff, write it, or
//! both.

pub mod doc;

use crate::config::loader::{Texts, load_modlist, load_with, normalize, parse_file};
use crate::config::schema::SourceModlist;
use std::path::{Path, PathBuf};

/// A planned change to one modlist file. Nothing is written until
/// [`Edit::write`].
#[derive(Debug, Clone)]
pub struct Edit {
    pub path: PathBuf,
    pub before: String,
    pub after: String,
}

impl Edit {
    /// The change from `before` (the file's text, as read) to `after` in the
    /// modlist at `path`, which must load and validate.
    pub fn new(path: &Path, before: String, after: String) -> anyhow::Result<Self> {
        Self::new_in(path, path, before, after)
    }

    /// The change from `before` to `after` in `path`: the modlist at `list`,
    /// or a file it includes. The list, with the change, must load and
    /// validate.
    pub fn new_in(list: &Path, path: &Path, before: String, after: String) -> anyhow::Result<Self> {
        check_in(list, path, &after)?;
        Ok(Self {
            path: path.to_path_buf(),
            before,
            after,
        })
    }

    /// The change from `before` to `after` in the docs file at `path`, which
    /// must still merge into the modlist at `list`.
    pub fn new_docs(
        path: &Path,
        list: &Path,
        before: String,
        after: String,
    ) -> anyhow::Result<Self> {
        let texts = Texts::from([(normalize(path), after.clone())]);
        load_with(list, true, &texts).map_err(|err| {
            anyhow::anyhow!(
                "the edit would leave {} invalid, so nothing was written: {err:#}",
                path.display()
            )
        })?;
        Ok(Self {
            path: path.to_path_buf(),
            before,
            after,
        })
    }

    /// Whether the edit changes anything.
    pub fn changes(&self) -> bool {
        self.before != self.after
    }

    /// The change as a unified diff, with three lines of context.
    pub fn diff(&self) -> String {
        diff(&self.path, &self.before, &self.after)
    }

    /// Write the new text, unless the file changed on disk since it was read.
    pub fn write(&self) -> anyhow::Result<()> {
        let now = std::fs::read_to_string(&self.path)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", self.path.display()))?;
        anyhow::ensure!(
            now == self.before,
            "{} changed on disk while it was being edited; nothing was written",
            self.path.display()
        );
        write_atomic(&self.path, &self.after)
    }
}

/// The change from `before` to `after` in the file at `path`, as a unified
/// diff with three lines of context.
pub fn diff(path: &Path, before: &str, after: &str) -> String {
    let name = path.display().to_string();
    similar::TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(3)
        .header(&name, &name)
        .to_string()
}

/// Plan an edit of the modlist at `path`: `change` gets its text and the list
/// parsed from it, and returns the new text, which must still load and
/// validate. The list read is the file alone: what its includes bring is not
/// in its text, so it is not in what `change` sees either.
pub fn plan(
    path: &Path,
    change: impl FnOnce(&str, &SourceModlist) -> anyhow::Result<String>,
) -> anyhow::Result<Edit> {
    plan_in(path, path, change)
}

/// [`plan`], for `file`: the list at `list`, or a file it includes.
pub fn plan_in(
    list: &Path,
    file: &Path,
    change: impl FnOnce(&str, &SourceModlist) -> anyhow::Result<String>,
) -> anyhow::Result<Edit> {
    let before = read(file)?;
    let source = parse_file(&before, file)?;
    let after = change(&before, &source)?;
    Edit::new_in(list, file, before, after)
}

/// [`plan`], in the file mod `id` is written in: the list, or the file it
/// includes the mod from.
pub fn plan_mod(
    list: &Path,
    id: &str,
    change: impl FnOnce(&str, &SourceModlist) -> anyhow::Result<String>,
) -> anyhow::Result<Edit> {
    plan_in(list, &file_of_mod(list, id)?, change)
}

/// The file mod `id` is written in: the list at `list`, or a file it
/// includes.
pub fn file_of_mod(list: &Path, id: &str) -> anyhow::Result<PathBuf> {
    let source = load_modlist(list)?;
    let entry = source
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))?;
    Ok(entry.origin.clone().unwrap_or_else(|| list.to_path_buf()))
}

/// Where a mod is written: the file, and the sections every mod in that file
/// is under because of how it is included, which its own `section` leaves
/// out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub file: PathBuf,
    pub sections: Vec<String>,
}

impl Place {
    /// The list's own file.
    pub fn list(list: &Path) -> Self {
        Self {
            file: list.to_path_buf(),
            sections: Vec::new(),
        }
    }

    /// `section`, as the list shows it, as this file writes it.
    pub fn local(&self, section: &[String]) -> anyhow::Result<Vec<String>> {
        section
            .strip_prefix(self.sections.as_slice())
            .map(<[String]>::to_vec)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "every mod in {} is under section {}, as that is where it is included",
                    self.file.display(),
                    self.sections.join(" - ")
                )
            })
    }
}

/// Where mod `id` is written.
pub fn place_of_mod(list: &Path, id: &str) -> anyhow::Result<Place> {
    let source = load_modlist(list)?;
    let entry = source
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))?;
    Ok(match &entry.origin {
        Some(file) => Place {
            file: file.clone(),
            sections: entry.section[..entry.inherited_sections].to_vec(),
        },
        None => Place::list(list),
    })
}

/// Where an edit placing a mod just before mod `anchor` is made: in
/// `anchor`'s file, or, with none, in the list's.
pub fn place_beside(list: &Path, anchor: Option<&str>) -> anyhow::Result<Place> {
    match anchor {
        Some(anchor) => place_of_mod(list, anchor),
        None => Ok(Place::list(list)),
    }
}

/// Plan moving mod `id` to `section` (as the list shows it; `None` keeps
/// its own), before mod `placement.before_mod` or at the section's end, in
/// the file it is written in, which a mod cannot leave by moving.
pub fn plan_mod_move(
    list: &Path,
    id: &str,
    section: Option<&[String]>,
    placement: &crate::config::add::Placement,
) -> anyhow::Result<(Edit, crate::config::add::Insertion)> {
    let place = place_of_mod(list, id)?;
    if let Some(anchor) = &placement.before_mod {
        let theirs = place_of_mod(list, anchor)?;
        anyhow::ensure!(
            theirs.file == place.file,
            "'{id}' is in {} and '{anchor}' in {}; a mod moves only within the file it is              written in",
            place.file.display(),
            theirs.file.display()
        );
    }
    let section = section.map(|section| place.local(section)).transpose()?;
    let before_section = placement
        .before_section
        .as_ref()
        .map(|section| place.local(section))
        .transpose()?;
    let placement = crate::config::add::Placement {
        before_mod: placement.before_mod.clone(),
        before_section,
    };
    let mut planned = None;
    let edit = plan_in(list, &place.file, |text, source| {
        let current = source
            .mods
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))?;
        let section = section.unwrap_or_else(|| current.section.clone());
        let plan = crate::config::add::plan_move(text, &source.mods, id, &section, &placement)?;
        let text = plan.new_text.clone();
        planned = Some(plan);
        Ok(text)
    })?;
    Ok((edit, planned.expect("a planned edit runs its change")))
}

/// The list's file, then each file it includes.
pub fn list_files(list: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let source = load_modlist(list)?;
    let mut files = vec![list.to_path_buf()];
    for file in source.included {
        if !files.contains(&file) {
            files.push(file);
        }
    }
    Ok(files)
}

/// The file whose `plugins` place `plugin` in the load order: the list's
/// own, or an included file's.
pub fn file_of_plugin(list: &Path, plugin: &str) -> anyhow::Result<PathBuf> {
    let files = list_files(list)?;
    for file in files {
        let own = parse_file(&read(&file)?, &file)?;
        if own.plugins.iter().any(|p| p.eq_ignore_ascii_case(plugin)) {
            return Ok(file);
        }
    }
    anyhow::bail!("{plugin} is not in the load order")
}

/// Plan moving `plugin` in the load order, in the file that places it: the
/// list's `plugins`, or an included file's. Next to another plugin, both
/// must be in the same file's.
pub fn plan_plugin_move(
    list: &Path,
    plugin: &str,
    to: &doc::PluginPosition,
) -> anyhow::Result<Edit> {
    let file = file_of_plugin(list, plugin)?;
    if let doc::PluginPosition::Before(anchor) | doc::PluginPosition::After(anchor) = to {
        let theirs = file_of_plugin(list, anchor)?;
        anyhow::ensure!(
            theirs == file,
            "{plugin} is placed by {} and {anchor} by {}; a plugin moves only within the file              that places it",
            file.display(),
            theirs.display()
        );
    }
    plan_in(list, &file, |text, _| doc::move_plugin(text, plugin, to))
}

fn read(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))
}

/// The docs file of the modlist file at `file`, if it names one.
fn docs_file_of(file: &Path, source: &SourceModlist) -> Option<PathBuf> {
    let name = source.docs_file.as_ref()?;
    Some(normalize(
        &file.parent().unwrap_or(Path::new("")).join(name),
    ))
}

/// Whether the docs file at `path` documents mod `id`.
fn documents(path: &Path, id: &str) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<crate::config::docs::DocsFile>(&text).ok())
        .is_some_and(|docs| docs.mods.contains_key(id))
}

/// Plan setting (or, with `None` or empty text, removing) a mod's prose --
/// `title`, `summary`, `body`, `footer` -- where its docs already are: inline
/// in the file it is written in, in that file's docs file, or in the list's.
/// A mod documented nowhere yet gets its docs in its own file's docs file
/// when that has one, inline when not.
pub fn plan_mod_docs(
    list: &Path,
    id: &str,
    fields: &[(String, Option<String>)],
) -> anyhow::Result<Edit> {
    let file = file_of_mod(list, id)?;
    let text = read(&file)?;
    let source = parse_file(&text, &file)?;
    let entry = source
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))?;
    let own = docs_file_of(&file, &source);
    let docs_path = if !entry.docs.is_empty() {
        None
    } else if let Some(path) = own.as_ref().filter(|path| documents(path, id)) {
        Some(path.clone())
    } else if let Some(path) = (file != list)
        .then(|| docs_file_of(list, &parse_file(&read(list).ok()?, list).ok()?))
        .flatten()
        .filter(|path| documents(path, id))
    {
        Some(path)
    } else {
        own
    };
    match docs_path {
        Some(path) => {
            let before = read(&path)?;
            let after = doc::set_file_docs(&before, id, fields)?;
            Edit::new_docs(&path, list, before, after)
        }
        None => {
            let after = doc::set_inline_docs(&text, id, fields)?;
            Edit::new_in(list, &file, text, after)
        }
    }
}

/// Several files changed as one edit: each read once and changed in turn,
/// and the list checked with every change made, as one change can be invalid
/// without another (a mod's block in one file, its plugin in another's load
/// order).
struct Work<'a> {
    list: &'a Path,
    /// Each file, its text as read, and as changed so far.
    files: Vec<(PathBuf, String, String)>,
}

impl<'a> Work<'a> {
    fn new(list: &'a Path) -> Self {
        Self {
            list,
            files: Vec::new(),
        }
    }

    /// The text of `path` as changed so far.
    fn text(&mut self, path: &Path) -> anyhow::Result<&mut String> {
        let at = match self.files.iter().position(|(file, _, _)| file == path) {
            Some(at) => at,
            None => {
                let text = read(path)?;
                self.files.push((path.to_path_buf(), text.clone(), text));
                self.files.len() - 1
            }
        };
        Ok(&mut self.files[at].2)
    }

    /// Change `path`'s text with `change`, which gets it parsed as well.
    fn change(
        &mut self,
        path: &Path,
        change: impl FnOnce(&str, &SourceModlist) -> anyhow::Result<String>,
    ) -> anyhow::Result<()> {
        let text = self.text(path)?;
        let parsed = parse_file(text, path)?;
        *text = change(text, &parsed)?;
        Ok(())
    }

    /// The edits, once the list loads and validates with all of them.
    fn finish(self) -> anyhow::Result<Vec<Edit>> {
        let texts: Texts = self
            .files
            .iter()
            .map(|(path, _, after)| (normalize(path), after.clone()))
            .collect();
        load_with(self.list, false, &texts)
            .and_then(|list| crate::config::validator::validate(&list).map(|_| ()))
            .map_err(|err| {
                anyhow::anyhow!(
                    "the edit would leave {} invalid, so nothing was written: {err:#}",
                    self.list.display()
                )
            })?;
        Ok(self
            .files
            .into_iter()
            .filter(|(_, before, after)| before != after)
            .map(|(path, before, after)| Edit {
                path,
                before,
                after,
            })
            .collect())
    }
}

/// Plan removing mod `id` from the list at `list`: its block (comments,
/// archives, actions) and its plugins from the load order, unless another
/// mod also has them -- and its notes from the docs file, where it has some
/// there, which would otherwise name a mod the list no longer has. The edits
/// to write, the file the mod is written in first. Refused, naming what is in
/// the way, while anything else refers to it.
pub fn plan_remove(list: &Path, id: &str) -> anyhow::Result<Vec<Edit>> {
    let source = load_modlist(list)?;
    let entry = source
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))?;
    let file = entry.origin.clone().unwrap_or_else(|| list.to_path_buf());
    let mut work = Work::new(list);
    work.change(&file, |text, own| {
        crate::config::add::plan_removal(text, &own.mods, id)
    })?;
    drop_from_load_order(&mut work, &own_plugins(&source, entry))?;
    let mut edits = work.finish()?;

    let own = parse_file(&read(&file)?, &file)?;
    let mut docs_files: Vec<PathBuf> = docs_file_of(&file, &own).into_iter().collect();
    if file != list
        && let Some(path) = parse_file(&read(list)?, list)
            .ok()
            .and_then(|root| docs_file_of(list, &root))
    {
        docs_files.push(path);
    }
    for path in docs_files {
        if let Ok(before) = std::fs::read_to_string(&path) {
            let after = doc::remove_file_docs(&before, id)?;
            if after != before {
                // Checked as a docs file only: the list it merges into is the
                // one written just before it.
                toml::from_str::<crate::config::docs::DocsFile>(&after)
                    .map_err(|err| anyhow::anyhow!("not a valid docs file: {err}"))?;
                edits.push(Edit {
                    path,
                    before,
                    after,
                });
            }
        }
    }
    Ok(edits)
}

/// Take `plugins` out of the load order, from whichever file places each.
fn drop_from_load_order(work: &mut Work, plugins: &[String]) -> anyhow::Result<()> {
    for plugin in plugins {
        if let Ok(file) = file_of_plugin(work.list, plugin) {
            let text = work.text(&file)?;
            *text = doc::drop_plugins(text, std::slice::from_ref(plugin))?;
        }
    }
    Ok(())
}

/// The plugins a mod declares that no other mod does: the ones that leave
/// the load order with it.
fn own_plugins(source: &SourceModlist, entry: &crate::config::schema::ModEntry) -> Vec<String> {
    entry
        .plugins
        .iter()
        .filter(|plugin| {
            !source.mods.iter().any(|other| {
                other.id != entry.id && other.plugins.iter().any(|p| p.eq_ignore_ascii_case(plugin))
            })
        })
        .cloned()
        .collect()
}

/// Plan ticking mod `id` in MO2, or leaving it unticked (`enabled = false`).
/// An unticked mod's plugins cannot be in the load order -- MO2 does not see
/// them -- so disabling takes them out, and enabling puts back the ones
/// missing: at `position`, or by default after the last plugin of the
/// nearest mod before it that has any. The edits to write: the file the mod
/// is written in, and the files whose load order changes.
pub fn plan_enabled(
    list: &Path,
    id: &str,
    enabled: bool,
    position: Option<&crate::config::add::LoadPosition>,
) -> anyhow::Result<Vec<Edit>> {
    use crate::config::add::{LoadPosition, last_plugin_before, place_plugins};
    let source = load_modlist(list)?;
    let at = source
        .mods
        .iter()
        .position(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))?;
    let entry = &source.mods[at];
    let file = entry.origin.clone().unwrap_or_else(|| list.to_path_buf());
    let plugins = own_plugins(&source, entry);
    let mut work = Work::new(list);
    if !enabled {
        work.change(&file, |text, _| {
            doc::set_fields(
                text,
                id,
                None,
                &[("enabled".to_string(), toml_edit::Value::from(false))],
                &[],
            )
        })?;
        drop_from_load_order(&mut work, &plugins)?;
        return work.finish();
    }
    work.change(&file, |text, _| {
        doc::set_fields(text, id, None, &[], &["enabled".to_string()])
    })?;
    let missing: Vec<String> = plugins
        .into_iter()
        .filter(|p| !source.plugins.iter().any(|q| q.eq_ignore_ascii_case(p)))
        .collect();
    if !missing.is_empty() {
        // Next to a plugin, in the file that places it; at the end, in the
        // mod's own file if that places plugins, or else the list's.
        let at_end = || -> anyhow::Result<PathBuf> {
            let own = parse_file(&read(&file)?, &file)?;
            Ok(if own.plugins.is_empty() {
                list.to_path_buf()
            } else {
                file.clone()
            })
        };
        let position = match position {
            Some(position) => position.clone(),
            None => last_plugin_before(&source, at)
                .map(LoadPosition::After)
                .unwrap_or(LoadPosition::Last),
        };
        let target = match &position {
            LoadPosition::After(anchor) => file_of_plugin(list, anchor)?,
            LoadPosition::Last => at_end()?,
        };
        let text = work.text(&target)?;
        *text = place_plugins(text, &missing, &position)?;
    }
    work.finish()
}

/// The text of a new, empty modlist called `name`: valid as it is, with the
/// game's master first in the load order, ready for `add`.
pub fn new_list_text(name: &str) -> String {
    let quoted = toml_edit::Value::from(name).to_string();
    // The name on one line, for the comment heading the file.
    let title = name.replace(['\n', '\r'], " ");
    format!(
        "# {title}: a modlist for mudcrab. What goes in it:\n\
         # https://steven-murray.github.io/mudcrab/modlist-format.html\n\
         name = {}\n\n\
         # The load order, top to bottom: the game's own master first, then each\n\
         # mod's plugins as it is added.\n\
         plugins = [\n    \"Oblivion.esm\",\n]\n",
        quoted.trim()
    )
}

/// Write a new, empty modlist called `name` at `path`, which must not exist.
pub fn create_list(path: &Path, name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!name.trim().is_empty(), "a list needs a name");
    let text = new_list_text(name.trim());
    check(path, &text)?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::AlreadyExists => anyhow::anyhow!(
                "{} already exists; a new list is never written over another file",
                path.display()
            ),
            _ => anyhow::anyhow!("failed to create {}: {err}", path.display()),
        })?;
    file.write_all(text.as_bytes())
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))
}

/// `text` loads and validates as the modlist at `path`.
pub fn check(path: &Path, text: &str) -> anyhow::Result<()> {
    check_in(path, path, text)
}

/// The modlist at `list` loads and validates with `text` as the file at
/// `path`: the list itself, or a file it includes.
pub fn check_in(list: &Path, path: &Path, text: &str) -> anyhow::Result<()> {
    let texts = Texts::from([(normalize(path), text.to_string())]);
    load_with(list, false, &texts)
        .and_then(|list| crate::config::validator::validate(&list).map(|_| ()))
        .map_err(|err| {
            anyhow::anyhow!(
                "the edit would leave {} invalid, so nothing was written: {err:#}",
                path.display()
            )
        })
}

/// Write via a temporary file next to the target, then rename, so an
/// interrupted write cannot leave half a modlist behind.
pub fn write_atomic(path: &Path, content: &str) -> anyhow::Result<()> {
    let mut temp = path.as_os_str().to_os_string();
    temp.push(format!(".mudcrab-edit-{}.tmp", std::process::id()));
    let temp = PathBuf::from(temp);

    std::fs::write(&temp, content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", temp.display()))?;

    std::fs::rename(&temp, path).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        anyhow::anyhow!("failed to replace {}: {err}", path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = "name = \"T\"\nplugins = [\n    \"A.esp\",\n]\n\n[[mods]]\nid = \"A\"\nplugins = [\"A.esp\"]\n";

    #[test]
    fn an_edit_that_breaks_the_list_is_refused_before_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("list.toml");
        std::fs::write(&path, LIST).unwrap();
        let err = plan(&path, |text, _| {
            Ok(text.replace("plugins = [\"A.esp\"]", "plugins = [\"B.esp\"]"))
        })
        .unwrap_err();
        assert!(err.to_string().contains("nothing was written"), "{err:#}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), LIST);
    }

    #[test]
    fn an_edit_shows_its_diff_and_will_not_overwrite_a_newer_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("list.toml");
        std::fs::write(&path, LIST).unwrap();
        let edit = plan(&path, |text, _| {
            Ok(text.replace("name = \"T\"", "name = \"U\""))
        })
        .unwrap();
        assert!(edit.changes());
        assert!(
            edit.diff().contains("-name = \"T\"\n+name = \"U\""),
            "{}",
            edit.diff()
        );

        std::fs::write(&path, LIST.replace('T', "V")).unwrap();
        assert!(edit.write().is_err());
        std::fs::write(&path, LIST).unwrap();
        edit.write().unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("name = \"U\"")
        );
    }
}
