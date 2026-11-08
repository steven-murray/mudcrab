//! Author mode, reading: the load order with each plugin's masters, a mod's
//! archives as trees with what `install` takes from them, which mods share
//! files, and `check`'s findings -- the views an author edits from, once
//! phase 6 turns editing on. See docs/design/gui.md.
//!
//! Everything reads the list as planned with Author mode's own profile's
//! answers (`author_answers`, apart from Install mode's), planned again
//! whenever the file or those answers change.

use super::{State, locate, quote, shown};
use mudcrab::config;
use mudcrab::config::schema::{ArchiveLayout, PersonalizedPlan};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::SystemTime;

/// When the open list's file last changed.
pub(super) fn modified(state: &State) -> Result<Option<SystemTime>, String> {
    Ok(std::fs::metadata(state.list()?)
        .and_then(|m| m.modified())
        .ok())
}

/// Author mode's plans, made from its profile's answers: what that profile
/// installs, and every mod as it would install (for looking at the list).
#[derive(Clone)]
pub(super) struct AuthorPlans {
    /// When the list's file had last changed, and the answers: made again
    /// when either has changed.
    at: Option<SystemTime>,
    answers: BTreeMap<String, String>,
    install: Result<PersonalizedPlan, String>,
    pub(super) authoring: config::query::AuthoringPlan,
}

pub(super) fn author_plans(state: &State) -> Result<AuthorPlans, String> {
    let now = modified(state)?;
    let list = state.list()?;
    let answers = state
        .settings
        .lock()
        .map_err(|_| "a command panicked")?
        .list(&list)
        .author_answers();
    if let Some(plans) = state
        .session
        .lock()
        .map_err(|_| "a command panicked")?
        .author
        .as_ref()
        .filter(|p| p.at == now && p.answers == answers)
    {
        return Ok(plans.clone());
    }
    let compiled = config::pipeline::compile(&list, false).map_err(shown)?;
    let overrides: config::query::Overrides = answers.clone().into_iter().collect();
    let plans = AuthorPlans {
        at: now,
        install: config::query::build_plan(&compiled.modlist, &overrides, None).map_err(shown),
        authoring: config::query::build_authoring_plan(&compiled.modlist, &overrides)
            .map_err(shown)?,
        answers,
    };
    state
        .session
        .lock()
        .map_err(|_| "a command panicked")?
        .author = Some(plans.clone());
    Ok(plans)
}

/// What Author mode's profile installs: for its check, load order and
/// conflicts.
pub(super) fn current_plan(state: &State) -> Result<PersonalizedPlan, String> {
    author_plans(state)?.install
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

#[tauri::command(async)]
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
    /// Counting from 1 in the list's file, as `--archive` does.
    number: usize,
    label: String,
    layout: Option<ArchiveLayout>,
    /// The archive's `if`, when it has one.
    condition: Option<String>,
}

/// A mod: whether Author mode's profile installs it, and its archives as
/// the list declares them.
#[derive(Serialize)]
pub(super) struct ModView {
    id: String,
    /// Left out by the profile's answers: shown as it would install.
    left_out: bool,
    /// Why it cannot be planned with these answers, when it cannot.
    unplanned: Option<String>,
    /// The included file it is written in, relative to the list; absent for
    /// the list's own.
    origin: Option<String>,
    /// The fields the include that brings it overrides.
    overridden: Vec<String>,
    /// The shim it takes its definition from (`use`), relative to the list.
    used: Option<String>,
    archives: Vec<ArchiveChoice>,
}

#[tauri::command(async)]
pub(super) fn mod_view(state: tauri::State<'_, State>, id: String) -> Result<ModView, String> {
    let authoring = author_plans(&state)?.authoring;
    let source = config::loader::load_modlist(&state.list()?).map_err(shown)?;
    let entry = source
        .mods
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| format!("the list has no mod with id '{id}'"))?;
    let base = source.base_dir.clone().unwrap_or_default();
    Ok(ModView {
        left_out: authoring.left_out.contains(&id),
        unplanned: (!authoring.plan.mods.iter().any(|m| m.id == id)).then(|| {
            authoring
                .notes
                .iter()
                .find(|note| note.contains(&format!("mod {id}:")))
                .cloned()
                .unwrap_or_else(|| "it cannot be planned with these answers".to_string())
        }),
        origin: entry
            .origin
            .as_deref()
            .map(|path| config::compiler::origin_name(&base, path)),
        overridden: entry.overridden.clone(),
        used: entry
            .used
            .as_deref()
            .map(|path| config::compiler::origin_name(&base, path)),
        archives: entry
            .archives
            .iter()
            .enumerate()
            .map(|(index, archive)| ArchiveChoice {
                number: index + 1,
                // Its file name, or the last part of where it comes from
                // (`manual:Name.zip`, a URL).
                label: archive.file_name.clone().unwrap_or_else(|| {
                    let path = archive.path.as_deref().unwrap_or_default();
                    path.rsplit(['/', ':']).next().unwrap_or(path).to_string()
                }),
                layout: archive.layout,
                condition: archive.condition.clone(),
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
    /// Where a taken file (or a folder of them) goes in the mod's folder:
    /// what a `file_move` names it as.
    destination: Option<String>,
    /// The path the archive's `include` and `exclude` see it as.
    filtered_as: Option<String>,
    /// The `exclude` that leaves exactly this out (for a folder, everything
    /// in it), where the layout offers it: what unticking it writes. `None`
    /// for what the layout does not offer, and for the data folder itself.
    exclude_glob: Option<String>,
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
    /// Packages chosen always; those chosen only under a condition are in
    /// `bain_conditional`, and changed in the file itself.
    bain_subpackages: Vec<String>,
    bain_conditional: Vec<String>,
    rows: Vec<FileRow>,
    /// The files this archive puts in the mod's folder, where they end up
    /// once the mod's `file_move`s have run.
    installed: Vec<InstalledFile>,
    /// How many of the files the layout offers each `include` and `exclude`
    /// matches: none, and it does nothing.
    matches: Vec<FilterMatches>,
    entries: usize,
    taken: usize,
    /// The command that shows the same from a terminal.
    command: String,
}

#[derive(Serialize)]
pub(super) struct FilterMatches {
    key: &'static str,
    glob: String,
    files: usize,
}

/// How many offered files each of the archive's filters matches, by the
/// path the filters see each as.
fn filter_matches(
    contents: &config::check::ArchiveContents,
    include: &[String],
    exclude: &[String],
) -> Vec<FilterMatches> {
    let count = |glob: &String, keeps: bool| {
        let filters = if keeps {
            mudcrab::archive::ArchiveFilters::new(std::slice::from_ref(glob), &[])
        } else {
            mudcrab::archive::ArchiveFilters::new(&[], std::slice::from_ref(glob))
        };
        filters.map_or(0, |filters| {
            contents
                .available
                .iter()
                .filter(|t| filters.should_extract(&t.filtered_as) == keeps)
                .count()
        })
    };
    include
        .iter()
        .map(|glob| FilterMatches {
            key: "include",
            glob: glob.clone(),
            files: count(glob, true),
        })
        .chain(exclude.iter().map(|glob| FilterMatches {
            key: "exclude",
            glob: glob.clone(),
            files: count(glob, false),
        }))
        .collect()
}

/// Mod `id`'s `number`th archive (from 1, as the list declares them) as a
/// tree, with what `install` takes from it: the planner `check` and `files`
/// use, with the answers given in Install.
#[tauri::command]
pub(super) async fn mod_files(
    state: tauri::State<'_, State>,
    id: String,
    number: usize,
) -> Result<Tree, String> {
    let downloads = locate(&state)?
        .downloads
        .ok_or("say where the downloads folder is first (in Settings)")?;
    let list = state.list()?;
    let answers: config::query::Overrides = state
        .settings
        .lock()
        .map_err(|_| "a command panicked")?
        .list(&list)
        .author_answers()
        .into_iter()
        .collect();
    let text = std::fs::read_to_string(&list).map_err(|err| err.to_string())?;
    let path = list.clone();
    let mod_id = id.clone();
    let (contents, archive, actions) = tokio::task::spawn_blocking(move || {
        let planned = config::pipeline::planned_archive(&text, &path, &mod_id, number, &answers)?;
        let contents = config::check::archive_contents(
            &planned.entry,
            &planned.archive,
            &downloads,
            &planned.active_plugins,
        )?;
        anyhow::Ok((contents, planned.archive, planned.entry.actions))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(shown)?;
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
            .filter(|p| p.condition().is_none())
            .map(|p| p.name().to_string())
            .collect(),
        bain_conditional: archive
            .bain_subpackages
            .iter()
            .filter(|p| p.condition().is_some())
            .map(|p| p.name().to_string())
            .collect(),
        rows: rows(&contents, archive.layout == Some(ArchiveLayout::Bain)),
        installed: installed(&contents, &actions),
        matches: filter_matches(&contents, &archive.include, &archive.exclude),
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

/// A file in the mod's folder, or one the archive's filters leave out of it.
#[derive(Serialize)]
pub(super) struct InstalledFile {
    /// Where it ends up (or would, were it taken).
    path: String,
    /// Where the archive's layout puts it, before any `file_move`.
    laid_out: String,
    /// Named by a `file_hide`'s `paths`: kept, but hidden from the game.
    hidden: bool,
    /// As the archive names it.
    source: String,
    /// Whether `install` takes it: false for one an `include` or `exclude`
    /// leaves out.
    taken: bool,
    /// The path the archive's `include` and `exclude` match it as.
    filtered_as: String,
}

/// The files `contents` puts in the mod's folder, and those its filters
/// leave out, with the mod's `file_move` actions applied in order, as
/// install runs them; a moved folder takes everything in it.
fn installed(
    contents: &config::check::ArchiveContents,
    actions: &[config::schema::ModAction],
) -> Vec<InstalledFile> {
    use config::schema::ModAction;
    // Inside `folder`; `get` keeps a name with accents from being cut
    // mid-character.
    let under = |path: &str, folder: &str| {
        path.get(..folder.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(folder))
            && path.as_bytes().get(folder.len()) == Some(&b'/')
    };
    let taken: BTreeSet<&str> = contents.taken.iter().map(|t| t.source.as_str()).collect();
    let file = |t: &config::check::Taken, taken: bool| InstalledFile {
        path: t.destination.clone(),
        laid_out: t.destination.clone(),
        hidden: false,
        source: t.source.clone(),
        taken,
        filtered_as: t.filtered_as.clone(),
    };
    let mut out: Vec<InstalledFile> = contents
        .taken
        .iter()
        .map(|t| file(t, true))
        .chain(
            contents
                .available
                .iter()
                .filter(|t| !taken.contains(t.source.as_str()))
                .map(|t| file(t, false)),
        )
        .collect();
    for action in actions {
        match action {
            ModAction::FileMove(m) => {
                let from = m.from.replace('\\', "/");
                let to = m.to.replace('\\', "/");
                for file in &mut out {
                    if file.path.eq_ignore_ascii_case(&from) {
                        file.path = to.clone();
                    } else if under(&file.path, &from) {
                        file.path = format!("{to}{}", &file.path[from.len()..]);
                    }
                }
            }
            ModAction::FileHide(h) => {
                for named in &h.paths {
                    let named = named.replace('\\', "/");
                    for file in &mut out {
                        if file.path.eq_ignore_ascii_case(&named) || under(&file.path, &named) {
                            file.hidden = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out.sort_by_key(|file| file.path.to_lowercase());
    out
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
    // What each folder is to the filters: the part of an offered file's
    // filtered path above the part inside the folder.
    let mut folder_as: BTreeMap<&str, String> = BTreeMap::new();
    for offered in &contents.available {
        let source = offered.source.as_str();
        let mut at = source.len();
        while let Some(slash) = source[..at].rfind('/') {
            let folder = &source[..slash];
            let tail = &source[slash + 1..];
            if let Some(Some(head)) =
                (!folder_as.contains_key(folder)).then(|| offered.filtered_as.strip_suffix(tail))
            {
                folder_as.insert(folder, head.trim_end_matches('/').to_string());
            }
            at = slash;
        }
    }
    // Where each folder lands in the mod's folder, read the same way from a
    // taken file's destination: what a `file_move` names it as.
    let mut folder_to: BTreeMap<&str, String> = BTreeMap::new();
    for taken in &contents.taken {
        let source = taken.source.as_str();
        let mut at = source.len();
        while let Some(slash) = source[..at].rfind('/') {
            let folder = &source[..slash];
            let tail = &source[slash + 1..];
            if let Some(Some(head)) =
                (!folder_to.contains_key(folder)).then(|| taken.destination.strip_suffix(tail))
            {
                folder_to.insert(folder, head.trim_end_matches('/').to_string());
            }
            at = slash;
        }
    }
    let mut files: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for entry in contents.entries.iter().filter(|e| !e.ends_with('/')) {
        files.entry(parent(entry)).or_default().push(entry);
    }
    let mut out = Vec::new();
    Walk {
        folder_to: &folder_to,
        folder_as: &folder_as,
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
    folder_to: &'a BTreeMap<&'a str, String>,
    folder_as: &'a BTreeMap<&'a str, String>,
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
                // The mod's own top folder has no name of its own to move.
                destination: self
                    .folder_to
                    .get(folder)
                    .filter(|path| !path.is_empty())
                    .cloned(),
                filtered_as: self.folder_as.get(folder).cloned(),
                exclude_glob: self
                    .folder_as
                    .get(folder)
                    .filter(|path| !path.is_empty())
                    .map(|path| format!("{}/**", globset::escape(path))),
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
                exclude_glob: self
                    .offered
                    .get(entry.as_str())
                    .map(|t| globset::escape(&t.filtered_as)),
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
        .ok_or("choose an MO2 instance with the list installed first (in Settings)")?;
    Ok(config::install::InstallSettings {
        cache_dir: located.downloads.unwrap_or_default(),
        mods_dir: config::mo2::layout::mods_dir(&instance),
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

/// An INI as the INI editor shows it: its lines, and the mod's INI actions
/// on it by condition.
#[derive(Serialize)]
pub(super) struct IniView {
    lines: Vec<config::ini_edit::IniLine>,
    groups: Vec<config::ini_edit::IniGroup>,
    /// Where the text was read from.
    from: String,
}

/// Mod `id`'s INI `file` (`source` in its archive `number`), or the game's
/// `Oblivion.ini` (its default, as the game ships it) for `scope = "game"`.
#[tauri::command]
pub(super) async fn ini_view(
    state: tauri::State<'_, State>,
    id: String,
    file: String,
    scope: config::schema::IniScope,
    number: Option<usize>,
    source: Option<String>,
) -> Result<IniView, String> {
    use config::schema::IniScope;
    let list = state.list()?;
    let located = locate(&state)?;
    let modlist = config::loader::load_modlist(&list).map_err(shown)?;
    let entry = modlist
        .mods
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| format!("the list has no mod with id '{id}'"))?;
    let groups = config::ini_edit::groups(entry, &file, scope);
    let (text, from) = match scope {
        IniScope::Game => {
            let game = located
                .game
                .ok_or("say where the game folder is first (in Settings)")?;
            let path = std::fs::read_dir(&game)
                .map_err(|err| format!("failed to read {}: {err}", game.display()))?
                .flatten()
                .map(|e| e.path())
                .find(|p| {
                    p.file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("Oblivion_default.ini"))
                })
                .ok_or_else(|| format!("{} has no Oblivion_default.ini", game.display()))?;
            let bytes = std::fs::read(&path).map_err(|err| err.to_string())?;
            let text = match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(err) => err.into_bytes().iter().map(|&b| b as char).collect(),
            };
            (text, path.display().to_string())
        }
        IniScope::Mod => {
            let number = number.ok_or("which archive the INI is in")?;
            let source = source.ok_or("where the INI is in its archive")?;
            let downloads = located
                .downloads
                .ok_or("say where the downloads folder is first (in Settings)")?;
            let answers: config::query::Overrides = state
                .settings
                .lock()
                .map_err(|_| "a command panicked")?
                .list(&list)
                .author_answers()
                .into_iter()
                .collect();
            let search = located.search.clone();
            let text = std::fs::read_to_string(&list).map_err(|err| err.to_string())?;
            let mod_id = id.clone();
            let entry_name = source.clone();
            tokio::task::spawn_blocking(move || {
                let planned =
                    config::pipeline::planned_archive(&text, &list, &mod_id, number, &answers)?;
                let reference = config::archive_store::ArchiveRef::for_archive(&planned.archive);
                let archive = config::archive_store::find(&downloads, &search, &reference)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "'{mod_id}'s archive {number} is not in {}; download it first",
                            downloads.display()
                        )
                    })?;
                let text = config::ini_edit::archive_text(&archive, &entry_name)?;
                anyhow::Ok((text, format!("{} in {}", entry_name, archive.display())))
            })
            .await
            .map_err(|err| err.to_string())?
            .map_err(shown)?
        }
    };
    Ok(IniView {
        lines: config::ini_edit::read_lines(&text),
        groups,
        from,
    })
}

/// Author mode's profile: its name, the profiles to choose from (both
/// modes'), and its answers.
#[derive(Serialize)]
pub(super) struct AuthorProfile {
    name: String,
    profiles: Vec<String>,
    answers: BTreeMap<String, String>,
}

fn author_profile_of(state: &State) -> Result<AuthorProfile, String> {
    let list = state.list()?;
    let instance = locate(state)?.instance;
    let settings = state.settings.lock().map_err(|_| "a command panicked")?;
    let chosen = settings.list(&list);
    let mut profiles = chosen.profile_names();
    if let Some(instance) = instance {
        profiles.extend(super::instance_profiles(&instance));
        profiles.sort();
        profiles.dedup();
    }
    Ok(AuthorProfile {
        name: chosen.author_profile(),
        profiles,
        answers: chosen.author_answers(),
    })
}

/// Author mode's profile (`author_profile`).
#[tauri::command(async)]
pub(super) fn author_profile(state: tauri::State<'_, State>) -> Result<AuthorProfile, String> {
    author_profile_of(&state)
}

/// Plan Author mode with profile `name`: one already made, or a new one,
/// starting from the answers it had.
#[tauri::command(async)]
pub(super) fn set_author_profile(
    state: tauri::State<'_, State>,
    name: String,
) -> Result<AuthorProfile, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("a profile needs a name".to_string());
    }
    let list = state.list()?;
    {
        let mut settings = state.settings.lock().map_err(|_| "a command panicked")?;
        let chosen = settings.list_mut(&list);
        let answers = chosen.author_answers();
        chosen.author_profile = Some(name.clone());
        chosen.profile_answers.entry(name).or_insert(answers);
        let _ = settings.save();
    }
    author_profile_of(&state)
}

/// Author mode's profile's answers, kept.
#[tauri::command(async)]
pub(super) fn set_author_answers(
    state: tauri::State<'_, State>,
    answers: BTreeMap<String, String>,
) -> Result<AuthorProfile, String> {
    let list = state.list()?;
    {
        let mut settings = state.settings.lock().map_err(|_| "a command panicked")?;
        settings.list_mut(&list).set_author_answers(answers);
        let _ = settings.save();
    }
    author_profile_of(&state)
}
