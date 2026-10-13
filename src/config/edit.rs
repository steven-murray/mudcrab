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

    /// Whether the edit changes anything.
    pub fn changes(&self) -> bool {
        self.before != self.after
    }

    /// The change as a unified diff, with three lines of context.
    pub fn diff(&self) -> String {
        let name = self.path.display().to_string();
        similar::TextDiff::from_lines(&self.before, &self.after)
            .unified_diff()
            .context_radius(3)
            .header(&format!("a/{name}"), &format!("b/{name}"))
            .to_string()
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
