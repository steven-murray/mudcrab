//! What both spikes show and do, so the comparison is only of the front ends.
//!
//! Two views, as `docs/design/gui.md` asks of the spike:
//!
//! - **The load order**: every plugin, with the mods that declare it; a drag
//!   moves one, through `config::edit::doc::move_plugin` -- the code
//!   `mudcrab plugins move` runs.
//! - **A mod's files**: every entry in one of its archives as a tree, ticked
//!   where `install` takes it (`config::check::archive_contents`, the planner
//!   `check` and `files` use). Unticking adds an `exclude` glob through
//!   `config::edit::doc::edit_strings` -- the code `mudcrab files --exclude`
//!   runs -- and the tree is planned again.
//!
//! Every edit is validated and written as the CLI writes it, and comes back
//! with the CLI command that does the same and the diff it made.

use mudcrab::config::edit::{self, doc};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

/// The list being looked at, and where its archives are.
#[derive(Debug, Clone)]
pub struct Session {
    pub list: PathBuf,
    pub downloads: PathBuf,
}

/// One plugin in the load order.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginRow {
    pub name: String,
    /// The mods that declare it (or make it, for a merge).
    pub mods: Vec<String>,
    pub inactive: bool,
}

/// What an edit did: the command that does the same from the terminal, and
/// the change to the list.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Edited {
    pub command: String,
    pub diff: String,
    pub millis: u128,
}

/// One row of a flattened file tree.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileRow {
    /// The entry's path in the archive; for a folder, the folder's.
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub folder: bool,
    /// For a file, whether `install` takes it; for a folder, whether it takes
    /// all, some or none of what is inside.
    pub state: Ticked,
    /// Where a taken file goes in the mod's folder.
    pub destination: Option<String>,
    /// Files inside a folder, and how many of them are taken.
    pub files: usize,
    pub taken: usize,
    /// Whether the user can tick it back: an exclude this list wrote for
    /// exactly this path can be dropped; anything else is the layout's doing.
    pub excluded_here: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Ticked {
    All,
    Some,
    None,
}

/// A mod's archive, as a tree.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Tree {
    pub label: String,
    pub rows: Vec<FileRow>,
    pub entries: usize,
    pub taken: usize,
    pub millis: u128,
}

fn quote(text: &str) -> String {
    if text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
    {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

impl Session {
    pub fn new(list: impl Into<PathBuf>, downloads: Option<PathBuf>) -> anyhow::Result<Self> {
        let list = list.into();
        let downloads = match downloads {
            Some(dir) => dir,
            None => mudcrab::config::tools::ToolsConfig::default_path()
                .map(|path| mudcrab::config::tools::ToolsConfig::load(&path))
                .transpose()?
                .and_then(|tools| tools.downloads_dir)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "no downloads folder: pass --downloads, or set downloads_dir in tools.toml"
                    )
                })?,
        };
        Ok(Self { list, downloads })
    }

    fn text(&self) -> anyhow::Result<String> {
        std::fs::read_to_string(&self.list)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", self.list.display()))
    }

    /// The load order, a row a plugin.
    pub fn load_order(&self) -> anyhow::Result<Vec<PluginRow>> {
        let source = mudcrab::config::loader::load_modlist(&self.list)?;
        Ok(source
            .plugins
            .iter()
            .map(|plugin| PluginRow {
                name: plugin.clone(),
                mods: source
                    .mods
                    .iter()
                    .filter(|entry| {
                        entry.plugins.iter().any(|p| p.eq_ignore_ascii_case(plugin))
                            || entry
                                .merge
                                .as_ref()
                                .is_some_and(|merge| merge.output.eq_ignore_ascii_case(plugin))
                    })
                    .map(|entry| entry.id.clone())
                    .collect(),
                inactive: source
                    .inactive_plugins
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(plugin)),
            })
            .collect())
    }

    /// Move `plugin` to just before `before`, or to the end.
    pub fn move_plugin(&self, plugin: &str, before: Option<&str>) -> anyhow::Result<Edited> {
        let started = Instant::now();
        let to = match before {
            Some(name) => doc::PluginPosition::Before(name.to_string()),
            None => doc::PluginPosition::Last,
        };
        let planned = edit::plan(&self.list, |text, _| doc::move_plugin(text, plugin, &to))?;
        planned.write()?;
        let place = match before {
            Some(name) => format!("--before {}", quote(name)),
            None => "--last".to_string(),
        };
        Ok(Edited {
            command: format!(
                "mudcrab plugins move {} {} {place}",
                quote(&self.list.display().to_string()),
                quote(plugin)
            ),
            diff: planned.diff(),
            millis: started.elapsed().as_millis(),
        })
    }

    /// Move the plugin at index `from` so that it ends up at index `to`, as a
    /// drag-and-drop leaves it.
    pub fn move_plugin_to(&self, from: usize, to: usize) -> anyhow::Result<Edited> {
        let source = mudcrab::config::loader::load_modlist(&self.list)?;
        let names = &source.plugins;
        let plugin = names
            .get(from)
            .ok_or_else(|| anyhow::anyhow!("there is no plugin {from}"))?;
        // Landing at `to`: before what is there now, or, moving down, before
        // the one after it (the end, past the last).
        let before = if to > from {
            names.get(to + 1)
        } else {
            names.get(to)
        };
        self.move_plugin(plugin, before.map(String::as_str))
    }

    /// The ids of the mods with archives, in list order.
    pub fn mods(&self) -> anyhow::Result<Vec<String>> {
        let source = mudcrab::config::loader::load_modlist(&self.list)?;
        Ok(source
            .mods
            .iter()
            .filter(|entry| !entry.archives.is_empty())
            .map(|entry| entry.id.clone())
            .collect())
    }

    /// Mod `id`'s first archive as a tree, folders before files, with folders
    /// in `open` shown open (all of them, with `None`).
    pub fn tree(&self, id: &str, open: Option<&BTreeSet<String>>) -> anyhow::Result<Tree> {
        let started = Instant::now();
        let text = self.text()?;
        let planned = mudcrab::config::pipeline::planned_archive(
            &text,
            &self.list,
            id,
            1,
            &Default::default(),
        )?;
        let contents = mudcrab::config::check::archive_contents(
            &planned.entry,
            &planned.archive,
            &self.downloads,
            &planned.active_plugins,
        )?
        .ok_or_else(|| {
            anyhow::anyhow!("what '{id}' installs is only known at install (an archive inside an archive, or similar)")
        })?;
        let taken: BTreeMap<&str, &str> = contents
            .taken
            .iter()
            .map(|(from, to)| (from.as_str(), to.as_str()))
            .collect();
        let excluded: BTreeSet<&str> = planned.archive.exclude.iter().map(String::as_str).collect();

        // Folder -> (files inside, taken inside), for every folder on a path.
        let mut folders: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for entry in &contents.entries {
            let mut at = 0;
            while let Some(slash) = entry[at..].find('/') {
                let folder = &entry[..at + slash];
                let counts = folders.entry(folder.to_string()).or_default();
                counts.0 += 1;
                counts.1 += usize::from(taken.contains_key(entry.as_str()));
                at += slash + 1;
            }
        }

        let mut rows = Vec::new();
        self.walk(
            "",
            0,
            &contents.entries,
            &folders,
            &taken,
            &excluded,
            open,
            &mut rows,
        );
        Ok(Tree {
            label: planned
                .archive
                .file_name
                .clone()
                .or(planned.archive.path.clone())
                .unwrap_or_default(),
            rows,
            entries: contents.entries.len(),
            taken: contents.taken.len(),
            millis: started.elapsed().as_millis(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        &self,
        under: &str,
        depth: usize,
        entries: &[String],
        folders: &BTreeMap<String, (usize, usize)>,
        taken: &BTreeMap<&str, &str>,
        excluded: &BTreeSet<&str>,
        open: Option<&BTreeSet<String>>,
        rows: &mut Vec<FileRow>,
    ) {
        let prefix = if under.is_empty() {
            String::new()
        } else {
            format!("{under}/")
        };
        let child_folders: BTreeSet<&str> = folders
            .keys()
            .filter_map(|folder| {
                let rest = folder.strip_prefix(&prefix)?;
                (!rest.contains('/')).then_some(folder.as_str())
            })
            .collect();
        for folder in child_folders {
            let (files, inside) = folders[folder];
            rows.push(FileRow {
                path: folder.to_string(),
                name: folder[prefix.len()..].to_string(),
                depth,
                folder: true,
                state: match inside {
                    0 => Ticked::None,
                    n if n == files => Ticked::All,
                    _ => Ticked::Some,
                },
                destination: None,
                files,
                taken: inside,
                excluded_here: excluded.contains(folder_glob(folder).as_str()),
            });
            if open.is_none_or(|open| open.contains(folder)) {
                self.walk(
                    folder,
                    depth + 1,
                    entries,
                    folders,
                    taken,
                    excluded,
                    open,
                    rows,
                );
            }
        }
        for entry in entries {
            let Some(name) = entry.strip_prefix(&prefix) else {
                continue;
            };
            if name.contains('/') {
                continue;
            }
            let destination = taken.get(entry.as_str()).map(|to| to.to_string());
            rows.push(FileRow {
                path: entry.clone(),
                name: name.to_string(),
                depth,
                folder: false,
                state: if destination.is_some() {
                    Ticked::All
                } else {
                    Ticked::None
                },
                destination,
                files: 1,
                taken: usize::from(taken.contains_key(entry.as_str())),
                excluded_here: excluded.contains(globset::escape(entry).as_str()),
            });
        }
    }

    /// Untick (`keep` false) or tick back a file or folder of mod `id`'s
    /// first archive.
    pub fn toggle(&self, id: &str, path: &str, folder: bool, keep: bool) -> anyhow::Result<Edited> {
        let started = Instant::now();
        let glob = if folder {
            folder_glob(path)
        } else {
            globset::escape(path)
        };
        let (add, drop) = if keep {
            (vec![], vec![glob.clone()])
        } else {
            (vec![glob.clone()], vec![])
        };
        let planned = edit::plan(&self.list, |text, _| {
            doc::edit_strings(text, id, Some(1), "exclude", &add, &drop).map_err(|err| {
                if keep {
                    anyhow::anyhow!(
                        "{path} is left out by the archive's layout or by a broader filter, not \
                         by an exclude of its own ({err})"
                    )
                } else {
                    err
                }
            })
        })?;
        planned.write()?;
        Ok(Edited {
            command: format!(
                "mudcrab files {} --mod {} --{} {}",
                quote(&self.list.display().to_string()),
                quote(id),
                if keep { "drop-exclude" } else { "exclude" },
                quote(&glob)
            ),
            diff: planned.diff(),
            millis: started.elapsed().as_millis(),
        })
    }
}

fn folder_glob(folder: &str) -> String {
    format!("{}/**", globset::escape(folder))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const LIST: &str = r#"name = "Spike"
plugins = [
    "Oblivion.esm",
    "A.esp",
    "B.esp",
    "C.esp",
]

[[mods]]
id = "A"
plugins = ["A.esp", "B.esp", "C.esp"]

[[mods.archives]]
path = "manual:A.zip"
"#;

    fn setup() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("list.toml");
        std::fs::write(&list, LIST).unwrap();
        let downloads = dir.path().join("downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(downloads.join("A.zip")).unwrap());
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        for name in [
            "A.esp",
            "B.esp",
            "C.esp",
            "textures/a/x.dds",
            "textures/a/y.dds",
            "textures/b [1]/z.dds",
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(b"x").unwrap();
        }
        zip.finish().unwrap();
        let session = Session::new(list, Some(downloads)).unwrap();
        (dir, session)
    }

    fn order(session: &Session) -> Vec<String> {
        session
            .load_order()
            .unwrap()
            .into_iter()
            .map(|row| row.name)
            .collect()
    }

    #[test]
    fn a_drop_puts_the_plugin_at_the_index_it_was_dropped_on() {
        let (_dir, session) = setup();
        session.move_plugin_to(1, 3).unwrap();
        assert_eq!(order(&session), ["Oblivion.esm", "B.esp", "C.esp", "A.esp"]);
        session.move_plugin_to(3, 1).unwrap();
        assert_eq!(order(&session), ["Oblivion.esm", "A.esp", "B.esp", "C.esp"]);
        let edited = session.move_plugin_to(2, 1).unwrap();
        assert_eq!(order(&session), ["Oblivion.esm", "B.esp", "A.esp", "C.esp"]);
        assert!(
            edited.command.ends_with("B.esp --before A.esp"),
            "{}",
            edited.command
        );
    }

    #[test]
    fn the_tree_ticks_what_is_taken_and_unticking_writes_an_exclude() {
        let (_dir, session) = setup();
        let tree = session.tree("A", None).unwrap();
        assert_eq!((tree.entries, tree.taken), (6, 6));
        let names: Vec<(&str, usize)> = tree
            .rows
            .iter()
            .map(|r| (r.name.as_str(), r.depth))
            .collect();
        assert_eq!(
            names,
            [
                ("textures", 0),
                ("a", 1),
                ("x.dds", 2),
                ("y.dds", 2),
                ("b [1]", 1),
                ("z.dds", 2),
                ("A.esp", 0),
                ("B.esp", 0),
                ("C.esp", 0)
            ]
        );

        // A folder whose name is not a valid glob as it stands.
        let edited = session.toggle("A", "textures/b [1]", true, false).unwrap();
        assert!(
            edited
                .command
                .ends_with("--exclude 'textures/b [[]1[]]/**'"),
            "{}",
            edited.command
        );
        let tree = session.tree("A", None).unwrap();
        assert_eq!(tree.taken, 5);
        let textures = &tree.rows[0];
        assert_eq!(
            (textures.state, textures.taken, textures.files),
            (Ticked::Some, 2, 3)
        );
        assert!(tree.rows[4].excluded_here);

        session.toggle("A", "textures/b [1]", true, true).unwrap();
        assert_eq!(session.tree("A", None).unwrap().taken, 6);
        // Ticking back what no exclude of its own left out is refused.
        session
            .toggle("A", "textures/a/x.dds", false, false)
            .unwrap();
        assert!(session.toggle("A", "textures/a", true, true).is_err());
    }

    #[test]
    fn a_name_with_brackets_is_quoted_for_the_shell_and_escaped_as_a_glob() {
        assert_eq!(quote("Harvest [Flora].esp"), "'Harvest [Flora].esp'");
        assert_eq!(
            globset::escape("Harvest [Flora].esp"),
            "Harvest [[]Flora[]].esp"
        );
        assert_eq!(quote("Oblivion.esm"), "Oblivion.esm");
    }
}
