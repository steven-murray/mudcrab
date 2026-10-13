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

use mudcrab::config::check::ArchiveContents;
use mudcrab::config::edit::{self, doc};
use mudcrab::config::schema::ArchiveLayout;
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
    /// all, some or none of what the layout offers inside it.
    pub state: Ticked,
    /// Whether the layout offers it at all. A file it does not is left out by
    /// the layout (a BAIN package or FOMOD option not chosen), not by a
    /// filter, and only changing that choice brings it in.
    pub offered: bool,
    /// A BAIN package: ticking it chooses the package.
    pub package: bool,
    /// Whether ticking or unticking it can do anything: what the layout
    /// offers, and in a BAIN archive anything inside a package (ticking it
    /// chooses the package, keeping only that).
    pub tickable: bool,
    /// Where a taken file goes in the mod's folder.
    pub destination: Option<String>,
    /// Inside a folder: the files the layout offers, and how many are taken.
    pub files: usize,
    pub taken: usize,
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

    /// Mod `id`'s first archive, planned as `install` would.
    fn contents(
        &self,
        id: &str,
    ) -> anyhow::Result<(mudcrab::config::pipeline::PlannedArchive, ArchiveContents)> {
        self.contents_of(&self.text()?, id)
    }

    /// As [`Session::contents`], for the list as `text` has it.
    fn contents_of(
        &self,
        text: &str,
        id: &str,
    ) -> anyhow::Result<(mudcrab::config::pipeline::PlannedArchive, ArchiveContents)> {
        let text = text.to_string();
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
            anyhow::anyhow!(
                "what '{id}' installs is only known at install (an archive inside an archive, \
                 or similar)"
            )
        })?;
        Ok((planned, contents))
    }

    /// Mod `id`'s first archive as a tree, folders before files, with folders
    /// in `open` shown open (all of them, with `None`).
    pub fn tree(&self, id: &str, open: Option<&BTreeSet<String>>) -> anyhow::Result<Tree> {
        let started = Instant::now();
        let (planned, contents) = self.contents(id)?;
        let taken: BTreeMap<&str, &str> = contents
            .taken
            .iter()
            .map(|t| (t.source.as_str(), t.destination.as_str()))
            .collect();
        let offered: BTreeSet<&str> = contents
            .available
            .iter()
            .map(|t| t.source.as_str())
            .collect();
        let bain = planned.archive.layout == Some(ArchiveLayout::Bain);

        // Folder -> (files offered inside, taken inside), for every folder on
        // a path.
        let mut folders: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for entry in &contents.entries {
            let mut at = 0;
            while let Some(slash) = entry[at..].find('/') {
                let counts = folders.entry(entry[..at + slash].to_string()).or_default();
                counts.0 += usize::from(offered.contains(entry.as_str()));
                counts.1 += usize::from(taken.contains_key(entry.as_str()));
                at += slash + 1;
            }
        }

        let mut rows = Vec::new();
        let cx = Walk {
            entries: &contents.entries,
            folders: &folders,
            taken: &taken,
            offered: &offered,
            bain,
            open,
        };
        cx.walk("", 0, &mut rows);
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

    /// Untick (`keep` false) or tick back a file or folder of mod `id`'s
    /// first archive: an `exclude` for what the filters see it as, or for a
    /// BAIN package, a change to `bain_subpackages`.
    pub fn toggle(&self, id: &str, path: &str, folder: bool, keep: bool) -> anyhow::Result<Edited> {
        let started = Instant::now();
        let (planned, contents) = self.contents(id)?;
        let archive = &planned.archive;

        // Ticking something inside a BAIN package not yet chosen: choose the
        // package, keeping only that.
        if keep && archive.layout == Some(ArchiveLayout::Bain) {
            let package = path.split('/').next().unwrap_or_default();
            let chosen = bain_names(archive)
                .iter()
                .any(|name| name.eq_ignore_ascii_case(package));
            if !chosen && package != path {
                return self.choose_part_of_package(id, package, path, folder, &contents, started);
            }
        }

        let (key, value) =
            if folder && !path.contains('/') && archive.layout == Some(ArchiveLayout::Bain) {
                ("bain_subpackages", path.to_string())
            } else {
                let filtered_as = if folder {
                    let under = format!("{path}/");
                    let inside = contents
                    .available
                    .iter()
                    .find(|t| t.source.replace('\\', "/").starts_with(&under))
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "the layout takes nothing from {path}, so there is nothing to filter"
                        )
                    })?;
                    let tail = &inside.source.replace('\\', "/")[under.len()..];
                    let folder_as = inside
                        .filtered_as
                        .strip_suffix(tail)
                        .map(|rest| rest.trim_end_matches('/'))
                        .unwrap_or_default();
                    anyhow::ensure!(
                        !folder_as.is_empty(),
                        "{path} is the mod's data folder itself; untick what is in it instead"
                    );
                    folder_glob(folder_as)
                } else {
                    let offered = contents
                    .available
                    .iter()
                    .find(|t| t.source == path)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "{path} is left out by the archive's layout (a package or option not \
                             chosen), not by a filter"
                        )
                    })?;
                    globset::escape(&offered.filtered_as)
                };
                ("exclude", filtered_as)
            };

        // Ticking a package adds it; for a filter, ticking takes the exclude out.
        let adding = (key == "bain_subpackages") == keep;
        if !adding {
            let listed = if key == "exclude" {
                &archive.exclude
            } else {
                &bain_names(archive)
            };
            anyhow::ensure!(
                listed.iter().any(|v| v.eq_ignore_ascii_case(&value)),
                "{path} is left out by {} rather than by an exclude of its own, so it cannot be \
                 ticked back here",
                if archive.include.is_empty() {
                    format!("the excludes {:?}", archive.exclude)
                } else {
                    format!(
                        "the includes {:?} or excludes {:?}",
                        archive.include, archive.exclude
                    )
                }
            );
        }
        let (add, drop) = if adding {
            (vec![value.clone()], vec![])
        } else {
            (vec![], vec![value.clone()])
        };
        let planned_edit = edit::plan(&self.list, |text, _| {
            doc::edit_strings(text, id, Some(1), key, &add, &drop)
        })?;
        planned_edit.write()?;
        let command = if key == "exclude" {
            format!(
                "mudcrab files {} --mod {} --{} {}",
                quote(&self.list.display().to_string()),
                quote(id),
                if adding { "exclude" } else { "drop-exclude" },
                quote(&value)
            )
        } else {
            let mut packages = bain_names(archive);
            if adding {
                packages.push(value.clone());
            } else {
                packages.retain(|p| !p.eq_ignore_ascii_case(&value));
            }
            format!(
                "mudcrab set {} --mod {} --archive 1 {}",
                quote(&self.list.display().to_string()),
                quote(id),
                quote(&format!(
                    "bain_subpackages=[{}]",
                    packages
                        .iter()
                        .map(|p| format!("{p:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            )
        };
        Ok(Edited {
            command,
            diff: planned_edit.diff(),
            millis: started.elapsed().as_millis(),
        })
    }
}

impl Session {
    /// Choose BAIN package `package` of mod `id`, keeping only `path` (a file,
    /// or with `folder` everything in it): the package is added, and the rest
    /// of it excluded -- a whole folder at a time where `path` is not inside
    /// it. Refused when an exclude would also leave out a file another chosen
    /// package installs, since filters apply to every package alike.
    fn choose_part_of_package(
        &self,
        id: &str,
        package: &str,
        path: &str,
        folder: bool,
        now: &ArchiveContents,
        started: Instant,
    ) -> anyhow::Result<Edited> {
        let text = self.text()?;
        let with_package = doc::edit_strings(
            &text,
            id,
            Some(1),
            "bain_subpackages",
            &[package.to_string()],
            &[],
        )?;
        let (_, then) = self.contents_of(&with_package, id)?;
        let under = format!("{package}/");
        let wanted = |source: &str| {
            let source = source.replace('\\', "/");
            if folder {
                source.starts_with(&format!("{path}/"))
            } else {
                source == path
            }
        };
        let inside: Vec<&mudcrab::config::check::Taken> = then
            .taken
            .iter()
            .filter(|t| t.source.replace('\\', "/").starts_with(&under))
            .collect();
        let keep: Vec<&str> = inside
            .iter()
            .filter(|t| wanted(&t.source))
            .map(|t| t.filtered_as.as_str())
            .collect();
        anyhow::ensure!(
            !keep.is_empty(),
            "{path} is not something package {package} installs"
        );
        // The folders on the way to what is kept, as the filters see them.
        let kept_dirs: BTreeSet<String> = keep
            .iter()
            .flat_map(|f| {
                let parts: Vec<&str> = f.split('/').collect();
                (1..parts.len())
                    .map(move |n| parts[..n].join("/"))
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut excludes: BTreeSet<String> = BTreeSet::new();
        for other in inside.iter().filter(|t| !wanted(&t.source)) {
            let parts: Vec<&str> = other.filtered_as.split('/').collect();
            // The outermost folder of it that leads nowhere kept, or the file.
            let glob = (1..parts.len())
                .map(|n| parts[..n].join("/"))
                .find(|dir| !kept_dirs.contains(dir))
                .map(|dir| folder_glob(&dir))
                .unwrap_or_else(|| globset::escape(&other.filtered_as));
            excludes.insert(glob);
        }
        // What the excludes would also take from the packages already chosen.
        let globs = mudcrab::archive::ArchiveFilters::new(
            &[],
            &excludes.iter().cloned().collect::<Vec<_>>(),
        )?;
        let hit: Vec<&str> = now
            .taken
            .iter()
            .filter(|t| !globs.should_extract(&t.filtered_as))
            .map(|t| t.source.as_str())
            .collect();
        anyhow::ensure!(
            hit.is_empty(),
            "choosing only {path} from {package} means excluding paths that another chosen \
             package installs too ({}), since excludes apply to every package; tick the whole \
             package instead",
            hit.iter().take(3).copied().collect::<Vec<_>>().join(", ")
        );

        let excludes: Vec<String> = excludes.into_iter().collect();
        let edit = edit::plan(&self.list, |text, _| {
            let text = doc::edit_strings(
                text,
                id,
                Some(1),
                "bain_subpackages",
                &[package.to_string()],
                &[],
            )?;
            doc::edit_strings(&text, id, Some(1), "exclude", &excludes, &[])
        })?;
        edit.write()?;
        let list = quote(&self.list.display().to_string());
        let mut command = format!(
            "mudcrab set {list} --mod {} --archive 1 'bain_subpackages=[...{package:?}]' && \
             mudcrab files {list} --mod {}",
            quote(id),
            quote(id)
        );
        for glob in &excludes {
            command.push_str(&format!(" --exclude {}", quote(glob)));
        }
        Ok(Edited {
            command,
            diff: edit.diff(),
            millis: started.elapsed().as_millis(),
        })
    }
}

fn bain_names(archive: &mudcrab::config::schema::CompiledArchive) -> Vec<String> {
    archive
        .bain_subpackages
        .iter()
        .map(|p| p.name().to_string())
        .collect()
}

/// What a tree is walked over.
struct Walk<'a> {
    entries: &'a [String],
    folders: &'a BTreeMap<String, (usize, usize)>,
    taken: &'a BTreeMap<&'a str, &'a str>,
    offered: &'a BTreeSet<&'a str>,
    bain: bool,
    open: Option<&'a BTreeSet<String>>,
}

impl Walk<'_> {
    fn walk(&self, under: &str, depth: usize, rows: &mut Vec<FileRow>) {
        let prefix = if under.is_empty() {
            String::new()
        } else {
            format!("{under}/")
        };
        let child_folders: BTreeSet<&str> = self
            .folders
            .keys()
            .filter_map(|folder| {
                let rest = folder.strip_prefix(&prefix)?;
                (!rest.contains('/')).then_some(folder.as_str())
            })
            .collect();
        for folder in child_folders {
            let (files, inside) = self.folders[folder];
            let package = self.bain && depth == 0;
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
                offered: files > 0 || package,
                package,
                tickable: files > 0 || (self.bain && !under.is_empty()) || package,
                destination: None,
                files,
                taken: inside,
            });
            if self.open.is_none_or(|open| open.contains(folder)) {
                self.walk(folder, depth + 1, rows);
            }
        }
        for entry in self.entries {
            let Some(name) = entry.strip_prefix(&prefix) else {
                continue;
            };
            if name.contains('/') {
                continue;
            }
            let destination = self.taken.get(entry.as_str()).map(|to| to.to_string());
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
                offered: self.offered.contains(entry.as_str()),
                package: false,
                tickable: self.offered.contains(entry.as_str()) || (self.bain && depth > 0),
                destination,
                files: 1,
                taken: usize::from(self.taken.contains_key(entry.as_str())),
            });
        }
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

        session.toggle("A", "textures/b [1]", true, true).unwrap();
        assert_eq!(session.tree("A", None).unwrap().taken, 6);
        // Ticking back what no exclude of its own left out is refused.
        session.toggle("A", "textures/a", true, false).unwrap();
        assert!(
            session
                .toggle("A", "textures/a/x.dds", false, true)
                .is_err()
        );
    }

    const BAIN: &str = r#"name = "Spike"
plugins = ["Oblivion.esm", "Core.esp"]

[[mods]]
id = "P"
plugins = ["Core.esp"]

[[mods.archives]]
path = "manual:P.zip"
layout = "bain"
bain_subpackages = ["00 Core"]
"#;

    #[test]
    fn in_a_bain_archive_filters_see_paths_inside_the_package_and_packages_tick() {
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("list.toml");
        std::fs::write(&list, BAIN).unwrap();
        let downloads = dir.path().join("downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(downloads.join("P.zip")).unwrap());
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        for name in [
            "00 Core/Core.esp",
            "00 Core/Docs/readme.txt",
            "01 Extra/textures/e.dds",
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(b"x").unwrap();
        }
        zip.finish().unwrap();
        let session = Session::new(&list, Some(downloads)).unwrap();

        let tree = session.tree("P", None).unwrap();
        assert_eq!(tree.taken, 2);
        let extra = tree
            .rows
            .iter()
            .find(|r| r.path == "01 Extra/textures/e.dds")
            .unwrap();
        assert!(!extra.offered);

        // A folder inside the package is excluded as the filters see it.
        let edited = session.toggle("P", "00 Core/Docs", true, false).unwrap();
        assert!(
            edited.command.ends_with("--exclude 'Docs/**'"),
            "{}",
            edited.command
        );
        assert_eq!(session.tree("P", None).unwrap().taken, 1);
        session
            .toggle("P", "00 Core/Docs/readme.txt", false, true)
            .unwrap_err();
        session.toggle("P", "00 Core/Docs", true, true).unwrap();

        // A package is chosen and unchosen.
        let edited = session.toggle("P", "01 Extra", true, true).unwrap();
        assert!(
            edited
                .command
                .contains(r#"bain_subpackages=["00 Core", "01 Extra"]"#),
            "{}",
            edited.command
        );
        assert_eq!(session.tree("P", None).unwrap().taken, 3);
        session.toggle("P", "01 Extra", true, false).unwrap();
        assert_eq!(std::fs::read_to_string(&list).unwrap(), BAIN);
    }

    #[test]
    fn ticking_a_file_in_an_unchosen_package_chooses_the_package_with_only_that() {
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("list.toml");
        std::fs::write(&list, BAIN).unwrap();
        let downloads = dir.path().join("downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(downloads.join("P.zip")).unwrap());
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        for name in [
            "00 Core/Core.esp",
            "00 Core/Docs/readme.txt",
            "01 Extra/textures/a/keep.dds",
            "01 Extra/textures/a/other.dds",
            "01 Extra/textures/b/x.dds",
            "01 Extra/meshes/m.nif",
            "02 Clash/Docs/readme.txt",
            "02 Clash/c.esp",
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(b"x").unwrap();
        }
        zip.finish().unwrap();
        let session = Session::new(&list, Some(downloads)).unwrap();

        let edited = session
            .toggle("P", "01 Extra/textures/a/keep.dds", false, true)
            .unwrap();
        let text = std::fs::read_to_string(&list).unwrap();
        assert!(
            text.contains(r#"bain_subpackages = ["00 Core", "01 Extra"]"#),
            "{text}"
        );
        assert!(
            text.contains(r#"exclude = ["meshes/**", "textures/a/other.dds", "textures/b/**"]"#),
            "{text}\n{}",
            edited.command
        );
        let tree = session.tree("P", None).unwrap();
        assert_eq!(tree.taken, 3);

        // Keeping only c.esp would exclude Docs/readme.txt, which 00 Core installs.
        let err = session
            .toggle("P", "02 Clash/c.esp", false, true)
            .unwrap_err();
        assert!(err.to_string().contains("00 Core/Docs/readme.txt"), "{err}");
    }
}
