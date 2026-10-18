//! Author mode, reading: the load order with each plugin's masters, a mod's
//! archives as trees with what `install` takes from them, which mods share
//! files, and `check`'s findings -- the views an author edits from, once
//! phase 6 turns editing on. See docs/design/gui.md.
//!
//! Everything reads the list as planned with the answers last given for it
//! (install mode's), planned again whenever the file has changed on disk.

use super::{State, locate, plan_of, quote, shown};
use mudcrab::config;
use mudcrab::config::schema::{ArchiveLayout, PersonalizedPlan};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::SystemTime;

/// When the open list's file last changed.
fn modified(state: &State) -> Result<Option<SystemTime>, String> {
    Ok(std::fs::metadata(state.list()?)
        .and_then(|m| m.modified())
        .ok())
}

/// The plan, made again with the remembered answers when the list has
/// changed since it was made (or it never was).
pub(super) fn current_plan(state: &State) -> Result<PersonalizedPlan, String> {
    let now = modified(state)?;
    let fresh = {
        let session = state.session.lock().map_err(|_| "a command panicked")?;
        session.plan.is_some() && session.planned_at.is_some() && session.planned_at == now
    };
    if !fresh {
        let list = state.list()?;
        let answers = state
            .settings
            .lock()
            .map_err(|_| "a command panicked")?
            .list(&list)
            .answers;
        super::plan_with(state, answers)?;
    }
    plan_of(state)
}

// --- The load order -----------------------------------------------------------------

/// One plugin, in load order.
#[derive(Serialize)]
pub(super) struct PluginRow {
    name: String,
    /// The mods that declare it, or make it (a merge).
    mods: Vec<String>,
    inactive: bool,
    /// The masters its header names, once `check` has read it; `None` before
    /// then, or for a plugin made at install.
    masters: Option<Vec<String>>,
}

#[derive(Serialize)]
pub(super) struct LoadOrder {
    plugins: Vec<PluginRow>,
    /// The game's own plugins, present whether the list names them or not.
    game: Vec<String>,
    /// Whether `check` has read the masters yet.
    masters_read: bool,
    /// Plugins merged away, lowercased, and the merge each went into.
    merged: BTreeMap<String, String>,
}

#[tauri::command]
pub(super) fn load_order(state: tauri::State<'_, State>) -> Result<LoadOrder, String> {
    let plan = current_plan(&state)?;
    let masters = state
        .session
        .lock()
        .map_err(|_| "a command panicked")?
        .masters
        .clone();
    let by_plugin: HashMap<String, Vec<String>> = masters
        .iter()
        .flatten()
        .map(|p| (p.plugin.to_ascii_lowercase(), p.masters.clone()))
        .collect();
    let inactive: BTreeSet<String> = plan
        .inactive_plugins
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    let plugins = plan
        .plugins
        .iter()
        .map(|name| {
            let key = name.to_ascii_lowercase();
            PluginRow {
                mods: plan
                    .mods
                    .iter()
                    .filter(|entry| {
                        entry.plugins.iter().any(|p| p.eq_ignore_ascii_case(name))
                            || entry
                                .merge
                                .as_ref()
                                .is_some_and(|m| m.output.eq_ignore_ascii_case(name))
                    })
                    .map(|entry| entry.id.clone())
                    .collect(),
                inactive: inactive.contains(&key),
                masters: by_plugin.get(&key).cloned(),
                name: name.clone(),
            }
        })
        .collect();
    Ok(LoadOrder {
        plugins,
        game: config::check::GAME_PLUGINS
            .iter()
            .map(|p| p.to_string())
            .collect(),
        masters_read: masters.is_some(),
        merged: plan
            .merges()
            .flat_map(|(_, merge)| {
                merge
                    .sources
                    .iter()
                    .map(|source| (source.plugin.to_ascii_lowercase(), merge.output.clone()))
            })
            .collect(),
    })
}

// --- A mod's files ------------------------------------------------------------------

/// One of a mod's archives, to choose between.
#[derive(Serialize)]
pub(super) struct ArchiveChoice {
    /// Counting from 1, as `--archive` does.
    number: usize,
    label: String,
    layout: Option<ArchiveLayout>,
}

/// A mod as planned: what it is, and its archives.
#[derive(Serialize)]
pub(super) struct ModView {
    id: String,
    /// Left out by the answers given, so not planned.
    left_out: bool,
    archives: Vec<ArchiveChoice>,
}

#[tauri::command]
pub(super) fn mod_view(state: tauri::State<'_, State>, id: String) -> Result<ModView, String> {
    let plan = current_plan(&state)?;
    let Some(entry) = plan.mods.iter().find(|m| m.id == id) else {
        return Ok(ModView {
            id,
            left_out: true,
            archives: Vec::new(),
        });
    };
    Ok(ModView {
        left_out: false,
        archives: entry
            .archives
            .iter()
            .enumerate()
            .map(|(index, archive)| ArchiveChoice {
                number: index + 1,
                label: archive
                    .file_name
                    .clone()
                    .or(archive.path.clone())
                    .unwrap_or_default(),
                layout: archive.layout,
            })
            .collect(),
        id,
    })
}

/// One row of a file tree, flattened: folders before files, each folder's
/// contents after it, one level deeper.
#[derive(Serialize)]
pub(super) struct FileRow {
    /// The entry's path in the archive; for a folder, the folder's.
    path: String,
    name: String,
    depth: usize,
    folder: bool,
    /// For a file, whether `install` takes it; for a folder, whether it takes
    /// all, some or none of what the layout offers inside it.
    state: Ticked,
    /// Whether the layout offers it at all. One it does not is left out by
    /// the layout (a BAIN package or FOMOD option not chosen), not a filter.
    offered: bool,
    /// A BAIN package: a top-level folder of a BAIN archive.
    package: bool,
    /// Where a taken file goes in the mod's folder.
    destination: Option<String>,
    /// The path the archive's `include` and `exclude` see it as.
    filtered_as: Option<String>,
    /// Inside a folder: the files the layout offers, and how many are taken.
    files: usize,
    taken: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Ticked {
    All,
    Some,
    None,
}

/// A mod's archive as a tree.
#[derive(Serialize)]
pub(super) struct Tree {
    label: String,
    layout: Option<ArchiveLayout>,
    include: Vec<String>,
    exclude: Vec<String>,
    bain_subpackages: Vec<String>,
    rows: Vec<FileRow>,
    entries: usize,
    taken: usize,
    /// The command that shows the same from a terminal.
    command: String,
}

/// Mod `id`'s `number`th archive (from 1) as a tree, with what `install`
/// takes from it: the planner `check` and `files` use.
#[tauri::command]
pub(super) async fn mod_files(
    state: tauri::State<'_, State>,
    id: String,
    number: usize,
) -> Result<Tree, String> {
    let plan = current_plan(&state)?;
    let downloads = locate(&state)?
        .downloads
        .ok_or("say where the downloads folder is first (in Install)")?;
    let list = state.list()?;
    let entry = plan
        .mods
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| format!("the answers given leave '{id}' out"))?
        .clone();
    let archive = entry
        .archives
        .get(number.wrapping_sub(1))
        .ok_or_else(|| format!("'{id}' has no archive {number}"))?
        .clone();
    let active = plan
        .plugins
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    let contents = tokio::task::spawn_blocking(move || {
        config::check::archive_contents(&entry, &archive, &downloads, &active)
            .map(|contents| (contents, archive))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(shown)?;
    let (contents, archive) = contents;
    let contents = contents.ok_or(
        "what this archive installs is only known at install (an archive inside an archive, an \
         MO2 plugin, or a build from files)",
    )?;
    Ok(Tree {
        label: archive
            .file_name
            .clone()
            .or(archive.path.clone())
            .unwrap_or_default(),
        layout: archive.layout,
        include: archive.include.clone(),
        exclude: archive.exclude.clone(),
        bain_subpackages: archive
            .bain_subpackages
            .iter()
            .map(|p| p.name().to_string())
            .collect(),
        rows: rows(&contents, archive.layout == Some(ArchiveLayout::Bain)),
        entries: contents.entries.len(),
        taken: contents.taken.len(),
        command: format!(
            "mudcrab files {} --mod {}{}",
            quote(list.display().to_string()),
            quote(&id),
            if number > 1 {
                format!(" --archive {number}")
            } else {
                String::new()
            }
        ),
    })
}

/// The folder a path is in; `""` at the top.
fn parent(path: &str) -> &str {
    path.rfind('/').map_or("", |at| &path[..at])
}

/// The archive's entries as tree rows.
fn rows(contents: &config::check::ArchiveContents, bain: bool) -> Vec<FileRow> {
    let taken: BTreeMap<&str, &config::check::Taken> = contents
        .taken
        .iter()
        .map(|t| (t.source.as_str(), t))
        .collect();
    let offered: BTreeMap<&str, &config::check::Taken> = contents
        .available
        .iter()
        .map(|t| (t.source.as_str(), t))
        .collect();
    // Folder -> (files offered inside, taken inside), for every folder on a
    // path.
    let mut folders: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for entry in &contents.entries {
        let mut at = 0;
        while let Some(slash) = entry[at..].find('/') {
            let counts = folders.entry(entry[..at + slash].to_string()).or_default();
            counts.0 += usize::from(offered.contains_key(entry.as_str()));
            counts.1 += usize::from(taken.contains_key(entry.as_str()));
            at += slash + 1;
        }
    }
    // Each folder's own folders and files, so the walk visits each once
    // (an archive of textures can hold tens of thousands).
    let mut subfolders: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for folder in folders.keys() {
        subfolders.entry(parent(folder)).or_default().insert(folder);
    }
    let mut files: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for entry in contents.entries.iter().filter(|e| !e.ends_with('/')) {
        files.entry(parent(entry)).or_default().push(entry);
    }
    let mut out = Vec::new();
    Walk {
        subfolders: &subfolders,
        files: &files,
        folders: &folders,
        taken: &taken,
        offered: &offered,
        bain,
    }
    .walk("", 0, &mut out);
    out
}

struct Walk<'a> {
    subfolders: &'a BTreeMap<&'a str, BTreeSet<&'a str>>,
    files: &'a BTreeMap<&'a str, Vec<&'a String>>,
    folders: &'a BTreeMap<String, (usize, usize)>,
    taken: &'a BTreeMap<&'a str, &'a config::check::Taken>,
    offered: &'a BTreeMap<&'a str, &'a config::check::Taken>,
    bain: bool,
}

impl Walk<'_> {
    fn walk(&self, under: &str, depth: usize, rows: &mut Vec<FileRow>) {
        let name = |path: &str| path.rsplit('/').next().unwrap_or(path).to_string();
        for &folder in self.subfolders.get(under).into_iter().flatten() {
            let (files, inside) = self.folders[folder];
            let package = self.bain && depth == 0;
            rows.push(FileRow {
                path: folder.to_string(),
                name: name(folder),
                depth,
                folder: true,
                state: match inside {
                    0 => Ticked::None,
                    n if n == files => Ticked::All,
                    _ => Ticked::Some,
                },
                offered: files > 0 || package,
                package,
                destination: None,
                filtered_as: None,
                files,
                taken: inside,
            });
            self.walk(folder, depth + 1, rows);
        }
        for &entry in self.files.get(under).into_iter().flatten() {
            let taken = self.taken.get(entry.as_str());
            rows.push(FileRow {
                path: entry.clone(),
                name: name(entry),
                depth,
                folder: false,
                state: if taken.is_some() {
                    Ticked::All
                } else {
                    Ticked::None
                },
                offered: self.offered.contains_key(entry.as_str()),
                package: false,
                destination: taken.map(|t| t.destination.clone()),
                filtered_as: self
                    .offered
                    .get(entry.as_str())
                    .map(|t| t.filtered_as.clone()),
                files: 1,
                taken: usize::from(taken.is_some()),
            });
        }
    }
}

// --- Conflicts ----------------------------------------------------------------------

/// Install settings for reading an installed instance's mods, as
/// `conflicts --mods-dir` makes them.
fn reading_settings(state: &State) -> Result<config::install::InstallSettings, String> {
    let located = locate(state)?;
    let instance = located
        .instance
        .ok_or("choose an MO2 instance with the list installed first (in Install)")?;
    Ok(config::install::InstallSettings {
        cache_dir: located.downloads.unwrap_or_default(),
        mods_dir: instance.join("mods"),
        mo2_instance_dir: None,
        profile_name: String::new(),
        game_dir: None,
        execute_actions: false,
        dry_run: true,
        tools: Default::default(),
        filter: Default::default(),
        force_merges: false,
        force: false,
        redo_actions: Vec::new(),
    })
}

#[derive(Serialize)]
pub(super) struct Conflicts {
    sweep: config::conflicts::Sweep,
    command: String,
}

/// Which installed mods share files, and who wins (`conflicts`).
#[tauri::command]
pub(super) async fn conflicts(state: tauri::State<'_, State>) -> Result<Conflicts, String> {
    let plan = current_plan(&state)?;
    let settings = reading_settings(&state)?;
    let command = format!(
        "mudcrab conflicts --plan {} --mods-dir {}",
        quote(super::plan_path(&state)?.display().to_string()),
        quote(settings.mods_dir.display().to_string())
    );
    let sweep = tokio::task::spawn_blocking(move || {
        config::conflicts::sweep(&plan, &settings, &Default::default())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(shown)?;
    Ok(Conflicts { sweep, command })
}

#[derive(Serialize)]
pub(super) struct Between {
    shared: config::conflicts::Shared,
    command: String,
}

/// The files mod `id` shares with mod `other` (`conflicts --mod --with`).
#[tauri::command]
pub(super) async fn conflicts_between(
    state: tauri::State<'_, State>,
    id: String,
    other: String,
) -> Result<Between, String> {
    let plan = current_plan(&state)?;
    let settings = reading_settings(&state)?;
    let command = format!(
        "mudcrab conflicts --plan {} --mods-dir {} --mod {} --with {}",
        quote(super::plan_path(&state)?.display().to_string()),
        quote(settings.mods_dir.display().to_string()),
        quote(&id),
        quote(&other)
    );
    let shared = tokio::task::spawn_blocking(move || {
        config::conflicts::shared(&plan, &settings, &id, &[other], None)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(shown)?;
    Ok(Between { shared, command })
}
