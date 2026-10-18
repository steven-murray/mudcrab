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

use crate::config::loader::parse_modlist;
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
    /// The change from `before` (the file's text, as read) to `after`, which
    /// must load and validate.
    pub fn new(path: &Path, before: String, after: String) -> anyhow::Result<Self> {
        check(path, &after)?;
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
        crate::config::loader::check_docs(list, &after).map_err(|err| {
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
/// validate.
pub fn plan(
    path: &Path,
    change: impl FnOnce(&str, &SourceModlist) -> anyhow::Result<String>,
) -> anyhow::Result<Edit> {
    let before = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    let source = parse_modlist(&before, path)?;
    let after = change(&before, &source)?;
    Edit::new(path, before, after)
}

/// Plan setting (or, with `None` or empty text, removing) a mod's prose --
/// `title`, `summary`, `body`, `footer` -- where its docs already are: inline
/// in the list, or in the docs file the list names. A mod documented nowhere
/// yet gets its docs in the docs file when the list has one, inline when not.
pub fn plan_mod_docs(
    list: &Path,
    id: &str,
    fields: &[(String, Option<String>)],
) -> anyhow::Result<Edit> {
    let text = std::fs::read_to_string(list)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", list.display()))?;
    let source = parse_modlist(&text, list)?;
    let entry = source
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))?;
    match &source.docs_file {
        Some(file) if entry.docs.is_empty() => {
            let path = source.base_dir.clone().unwrap_or_default().join(file);
            let before = std::fs::read_to_string(&path)
                .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
            let after = doc::set_file_docs(&before, id, fields)?;
            Edit::new_docs(&path, list, before, after)
        }
        _ => {
            let after = doc::set_inline_docs(&text, id, fields)?;
            Edit::new(list, text, after)
        }
    }
}

/// Plan removing mod `id` from the list at `list`: its block (comments,
/// archives, actions) and its plugins from the load order, unless another
/// mod also has them -- and its notes from the docs file, where it has some
/// there, which would otherwise name a mod the list no longer has. The edits
/// to write, the list's first. Refused, naming what is in the way, while
/// anything else refers to it.
pub fn plan_remove(list: &Path, id: &str) -> anyhow::Result<Vec<Edit>> {
    let text = std::fs::read_to_string(list)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", list.display()))?;
    let source = parse_modlist(&text, list)?;
    let entry = source
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))?;
    let dropped = own_plugins(&source, entry);
    let after = crate::config::add::plan_removal(&text, &source.mods, id)?;
    let after = doc::drop_plugins(&after, &dropped)?;
    let mut edits = vec![Edit::new(list, text, after)?];
    if let (Some(file), Some(base)) = (&source.docs_file, &source.base_dir) {
        let path = base.join(file);
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
/// nearest mod before it that has any.
pub fn plan_enabled(
    list: &Path,
    id: &str,
    enabled: bool,
    position: Option<&crate::config::add::LoadPosition>,
) -> anyhow::Result<Edit> {
    use crate::config::add::{LoadPosition, last_plugin_before, place_plugins};
    plan(list, |text, source| {
        let at = source
            .mods
            .iter()
            .position(|entry| entry.id == id)
            .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))?;
        let entry = &source.mods[at];
        let plugins = own_plugins(source, entry);
        if !enabled {
            let text = doc::set_fields(
                text,
                id,
                None,
                &[("enabled".to_string(), toml_edit::Value::from(false))],
                &[],
            )?;
            return doc::drop_plugins(&text, &plugins);
        }
        let text = doc::set_fields(text, id, None, &[], &["enabled".to_string()])?;
        let missing: Vec<String> = plugins
            .into_iter()
            .filter(|p| !source.plugins.iter().any(|q| q.eq_ignore_ascii_case(p)))
            .collect();
        if missing.is_empty() {
            return Ok(text);
        }
        let position = match position {
            Some(position) => position.clone(),
            None => last_plugin_before(source, at)
                .map(LoadPosition::After)
                .unwrap_or(LoadPosition::Last),
        };
        place_plugins(&text, &missing, &position)
    })
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
    parse_modlist(text, path)
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
