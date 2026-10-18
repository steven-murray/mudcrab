//! Author mode, adding a mod from an archive: the archives in the downloads
//! folders to choose from, the archive read as `inspect` reads it, its
//! installer walked a question at a time (the library's resumable walk, as
//! `add` drives it on a terminal), and the entry planned and written as `add`
//! writes it -- with the plugins it really installs, read from the planner
//! with the new entry in place, to declare and place in the load order.

use super::edits::{Edited, apply};
use super::{State, locate, quote, shown};
use mudcrab::config;
use mudcrab::config::add::{LoadPosition, NewMod, Placement, plan_add};
use mudcrab::config::inspect::{InspectReport, inspect_archive};
use mudcrab::config::installer::{self, Answer, ListPlugins, Walk};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

// --- The archives ---------------------------------------------------------------------

#[derive(Serialize)]
pub(super) struct ArchiveRow {
    #[serde(flatten)]
    listed: config::archive_store::Listed,
    /// The folder it is in, as chosen in Install.
    folder: String,
    /// Mods of the list that already use it, by name or source.
    used_by: Vec<String>,
}

/// Every archive in the downloads folder and the other folders to look in,
/// newest first, each with the mods that already use it.
#[tauri::command]
pub(super) fn archives(state: tauri::State<'_, State>) -> Result<Vec<ArchiveRow>, String> {
    let located = locate(&state)?;
    let dirs: Vec<PathBuf> = located
        .downloads
        .into_iter()
        .chain(located.search)
        .collect();
    let source = config::loader::load_modlist(&state.list()?).map_err(shown)?;
    // Each archive the list names, by lowercased file name and by source.
    let mut names: Vec<(String, &str)> = Vec::new();
    for entry in &source.mods {
        for archive in &entry.archives {
            let path = archive.path.as_deref().unwrap_or_default();
            let name = archive
                .file_name
                .clone()
                .unwrap_or_else(|| path.rsplit(['/', ':']).next().unwrap_or(path).to_string());
            names.push((name.to_lowercase(), entry.id.as_str()));
            if !path.is_empty() {
                names.push((path.to_lowercase(), entry.id.as_str()));
            }
        }
    }
    Ok(config::archive_store::list_archives(&dirs)
        .into_iter()
        .map(|listed| {
            let keys = [
                Some(listed.file_name.to_lowercase()),
                listed.source.as_ref().map(|s| s.to_lowercase()),
            ];
            let mut used_by: Vec<String> = names
                .iter()
                .filter(|(key, _)| keys.iter().flatten().any(|k| k == key))
                .map(|(_, id)| id.to_string())
                .collect();
            used_by.dedup();
            ArchiveRow {
                folder: listed
                    .path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                used_by,
                listed,
            }
        })
        .collect())
}

// --- Starting an add --------------------------------------------------------------------

/// An archive read for adding.
#[derive(Serialize)]
pub(super) struct Started {
    path: String,
    file_name: String,
    /// What `inspect` makes of its layout: kind (`fomod`, `bain`, `simple`,
    /// ...) and details.
    layout: config::inspect::LayoutGuess,
    layout_label: &'static str,
    /// Every plugin in the archive, wherever it is.
    plugins: Vec<String>,
    /// Where it came from, as the entry will say (`nexus:...`, `manual:...`).
    source: String,
    /// A name for the mod: Nexus's, else the file's without its Nexus ids.
    suggested_id: String,
    /// Whether choosing what to install means answering questions.
    has_installer: bool,
    /// The source came from asking Nexus by the archive's MD5.
    identified: bool,
    /// Why the source is a manual one, when it is: no API key to ask Nexus
    /// with, or Nexus not knowing the file.
    why_manual: Option<String>,
}

fn report_of(path: &Path) -> Result<InspectReport, String> {
    inspect_archive(path, false).map_err(shown)
}

/// A mod id from the archive: the mod's name in its `.meta`, else the file
/// name without its extension and Nexus's `-<mod id>-<version>` tail.
fn suggested_id(listed: Option<&config::archive_store::Listed>, file_name: &str) -> String {
    if let Some(name) = listed.and_then(|l| l.mod_name.clone()) {
        return name.trim().to_string();
    }
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);
    // Nexus names files `<name>-<mod id>-<version parts>`.
    let parts: Vec<&str> = stem.split('-').collect();
    let cut = parts
        .iter()
        .position(|part| part.len() >= 3 && part.chars().all(|c| c.is_ascii_digit()))
        .filter(|&at| at > 0)
        .unwrap_or(parts.len());
    parts[..cut].join("-").trim().to_string()
}

#[tauri::command]
pub(super) async fn add_start(path: String) -> Result<Started, String> {
    let path = PathBuf::from(path);
    let read = path.clone();
    let mut started = tokio::task::spawn_blocking(move || {
        let path = read;
        let report = report_of(&path)?;
        let listed = config::archive_store::list_archives(&[path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()])
        .into_iter()
        .find(|l| l.path == path);
        let has_installer = match &report.layout {
            config::inspect::LayoutGuess::Fomod { .. } => true,
            config::inspect::LayoutGuess::Bain { subpackages, .. } => subpackages.len() > 1,
            _ => false,
        };
        Ok::<_, String>(Started {
            path: path.display().to_string(),
            suggested_id: suggested_id(listed.as_ref(), &report.file_name),
            source: installer::source_of(&path, &report),
            file_name: report.file_name.clone(),
            layout_label: report.layout.label(),
            plugins: report.plugins.clone(),
            has_installer,
            identified: false,
            why_manual: None,
            layout: report.layout,
        })
    })
    .await
    .map_err(|err| err.to_string())??;
    // An archive whose `.meta` does not say where it came from: ask Nexus, by
    // its MD5, as `identify` does. Best effort; without a key or an answer it
    // stays a manual archive, which the author can change.
    if started.source.starts_with("manual:") {
        let page = config::identify::nexus_mod_in_name(&started.file_name)
            .map(|id| format!(" Its name says Nexus mod {id}, so the entry links that mod's page."))
            .unwrap_or_default();
        match config::tools::ToolsConfig::nexus_api_key() {
            None => {
                started.why_manual = Some(format!(
                    "Nexus was not asked where it came from: there is no Nexus API key (set one \
                     in Folders).{page}"
                ))
            }
            Some(key) => {
                let base = std::env::var("NEXUS_API_BASE").ok();
                let hint = config::identify::mod_id_from_file_name(&started.file_name);
                match config::identify::identify(
                    &client()?,
                    &path,
                    "oblivion",
                    &key,
                    base.as_deref(),
                    hint,
                )
                .await
                {
                    Ok(found) => {
                        started.source =
                            format!("nexus:oblivion/{}/{}", found.mod_id, found.file_id);
                        if let Some(name) = found.mod_name {
                            started.suggested_id = name.trim().to_string();
                        }
                        started.identified = true;
                    }
                    Err(_) => {
                        started.why_manual = Some(format!(
                            "Nexus does not know this file by its checksum (renamed, repacked, \
                             or not from Nexus).{page}"
                        ))
                    }
                }
            }
        }
    }
    Ok(started)
}

/// The installer walked with `answers` so far: the next question, or what was
/// chosen.
#[tauri::command]
pub(super) async fn add_walk(
    state: tauri::State<'_, State>,
    path: String,
    answers: Vec<Answer>,
) -> Result<Walk, String> {
    let list = ListPlugins::of(&config::loader::load_modlist(&state.list()?).map_err(shown)?);
    let path = PathBuf::from(path);
    tokio::task::spawn_blocking(move || {
        let report = report_of(&path)?;
        installer::walk(&path, &report, &list, &Default::default(), &answers, false).map_err(shown)
    })
    .await
    .map_err(|err| err.to_string())?
}

// --- Planning and writing the entry ---------------------------------------------------

/// What the page asks to add.
#[derive(Deserialize, Clone)]
pub(super) struct AddRequest {
    path: String,
    id: String,
    section: Vec<String>,
    before_mod: Option<String>,
    condition: Option<String>,
    /// Where the archive comes from, when not what its `.meta` says.
    source: Option<String>,
    answers: Vec<Answer>,
    /// The plugins to declare, and where the ones new to the load order go.
    plugins: Vec<String>,
    load_after: Option<String>,
}

/// The entry as it would be written.
#[derive(Serialize)]
pub(super) struct AddPlan {
    block: String,
    diff: String,
    command: String,
    /// The plugins the archive installs at the top of the mod, as planned
    /// with the installer's answers: the ones to declare.
    installs_plugins: Vec<String>,
    /// How many files it installs.
    files: usize,
    /// Where new plugins would go by default: after the last plugin of the
    /// mod before this one that has any.
    suggested_after: Option<String>,
    notes: Vec<String>,
}

/// The new mod, its block and notes, from the archive and the answers.
fn new_mod(request: &AddRequest) -> Result<(NewMod, Vec<String>, InspectReport), String> {
    let path = PathBuf::from(&request.path);
    let report = report_of(&path)?;
    // The walk is run unasked here; it has to be finished already.
    let list = ListPlugins::default();
    let installed = match installer::walk(
        &path,
        &report,
        &list,
        &Default::default(),
        &request.answers,
        false,
    )
    .map_err(shown)?
    {
        Walk::Done { installed } => installed,
        Walk::Ask { .. } => return Err("answer the installer's questions first".to_string()),
    };
    let source = request
        .source
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| installer::source_of(&path, &report));
    let block = installer::archive_block(&path, &report, &source, &installed).map_err(shown)?;
    config::install::safe_mod_dir_name(&request.id).map_err(shown)?;
    Ok((
        NewMod {
            id: request.id.trim().to_string(),
            section: request.section.clone(),
            condition: request.condition.clone().filter(|c| !c.trim().is_empty()),
            plugins: request.plugins.clone(),
            archive_block: Some(block),
            ..NewMod::default()
        },
        installed.notes,
        report,
    ))
}

/// Plan `new_mod` into the list: the new text, or why not.
fn planned(
    list: &Path,
    new_mod: &NewMod,
    request: &AddRequest,
) -> anyhow::Result<config::edit::Edit> {
    let placement = Placement {
        before_mod: request.before_mod.clone().filter(|b| !b.is_empty()),
        before_section: None,
    };
    let position = if new_mod.plugins.is_empty() {
        None
    } else {
        Some(match &request.load_after {
            Some(after) if !after.is_empty() => LoadPosition::After(after.clone()),
            _ => LoadPosition::Last,
        })
    };
    // Before a mod from an included file, the new one goes in that file.
    let place = config::edit::place_beside(list, placement.before_mod.as_deref())?;
    let mut local = new_mod.clone();
    local.section = place.local(&new_mod.section)?;
    config::edit::plan_in(list, &place.file, |text, source| {
        Ok(
            plan_add(text, source, &local, &placement, position.as_ref())?
                .insertion
                .new_text,
        )
    })
}

/// The command that adds the same from a terminal. The installer's answers
/// are asked there, as here.
fn command(list: &Path, request: &AddRequest, new_mod: &NewMod) -> String {
    let mut out = format!(
        "mudcrab add {} --archive {} --id {}",
        quote(list.display().to_string()),
        quote(&request.path),
        quote(&new_mod.id)
    );
    for level in &new_mod.section {
        out.push_str(&format!(" --section {}", quote(level)));
    }
    if let Some(before) = request.before_mod.as_ref().filter(|b| !b.is_empty()) {
        out.push_str(&format!(" --before-mod {}", quote(before)));
    }
    if let Some(condition) = &new_mod.condition {
        out.push_str(&format!(" --if {}", quote(condition)));
    }
    if let Some(source) = request.source.as_ref().filter(|s| !s.trim().is_empty()) {
        match source.strip_prefix("nexus:oblivion/") {
            Some(ids) => out.push_str(&format!(" --nexus {ids}")),
            None if source.starts_with("manual:") => out.push_str(" --manual"),
            None => {}
        }
    }
    for plugin in &new_mod.plugins {
        out.push_str(&format!(" --plugin {}", quote(plugin)));
    }
    if !new_mod.plugins.is_empty() {
        match request.load_after.as_ref().filter(|a| !a.is_empty()) {
            Some(after) => out.push_str(&format!(" --load-after {}", quote(after))),
            None => out.push_str(" --load-last"),
        }
    }
    out
}

/// The plugins the new mod installs at its top, and how many files: its
/// archive planned as `install` would, with the entry in place.
fn installs(
    list: &Path,
    text: &str,
    request: &AddRequest,
    id: &str,
) -> anyhow::Result<(Vec<String>, usize)> {
    let answers = request
        .condition
        .iter()
        .flat_map(|condition| {
            // An optional mod is planned as though its question were answered
            // yes; otherwise there is nothing to plan.
            condition
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|word| !word.is_empty())
                .map(|word| (word.to_string(), "true".to_string()))
                .collect::<Vec<_>>()
        })
        .collect();
    let planned = config::pipeline::planned_archive(text, list, id, 1, &answers)?;
    let path = PathBuf::from(&request.path);
    let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let contents = config::check::archive_contents(
        &planned.entry,
        &planned.archive,
        &folder,
        &planned.active_plugins,
    )?
    .ok_or_else(|| anyhow::anyhow!("only known at install"))?;
    let plugins = contents
        .taken
        .iter()
        .map(|t| t.destination.as_str())
        .filter(|d| {
            let lower = d.to_ascii_lowercase();
            !d.contains('/') && (lower.ends_with(".esp") || lower.ends_with(".esm"))
        })
        .map(str::to_string)
        .collect();
    Ok((plugins, contents.taken.len()))
}

/// The last plugin, in load order, of the nearest mod before where the new
/// one goes that has any in the load order.
fn suggested_after(
    source: &config::schema::SourceModlist,
    section: &[String],
    before_mod: Option<&str>,
) -> Option<String> {
    let mods = &source.mods;
    // Where the new mod goes: before `before_mod`, or after the last mod of
    // its section, or at the end.
    let at = match before_mod {
        Some(before) => mods.iter().position(|m| m.id == before)?,
        None => mods
            .iter()
            .rposition(|m| m.section == section)
            .map_or(mods.len(), |last| last + 1),
    };
    let order: Vec<String> = source
        .plugins
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    mods[..at].iter().rev().find_map(|entry| {
        entry
            .plugins
            .iter()
            .filter_map(|p| order.iter().position(|o| o == &p.to_ascii_lowercase()))
            .max()
            .map(|index| source.plugins[index].clone())
    })
}

#[tauri::command]
pub(super) async fn add_plan(
    state: tauri::State<'_, State>,
    request: AddRequest,
) -> Result<AddPlan, String> {
    let list = state.list()?;
    tokio::task::spawn_blocking(move || {
        let (new_mod, mut notes, report) = new_mod(&request)?;
        let source = config::loader::load_modlist(&list).map_err(shown)?;
        // What it installs, read with the entry in place and no plugins
        // declared yet -- the declaration is what this decides.
        let bare = NewMod {
            plugins: Vec::new(),
            ..new_mod.clone()
        };
        let bare_edit = planned(&list, &bare, &request).map_err(shown)?;
        let (installs_plugins, files) =
            match installs(&list, &bare_edit.after, &request, &new_mod.id) {
                Ok(found) => found,
                Err(err) => {
                    notes.push(format!(
                        "what it installs could not be read ({err:#}); its plugins are listed \
                         from the archive"
                    ));
                    let names = report
                        .plugins
                        .iter()
                        .map(|p| p.rsplit('/').next().unwrap_or(p).to_string())
                        .collect::<HashSet<_>>()
                        .into_iter()
                        .collect();
                    (names, 0)
                }
            };
        let edit = if new_mod.plugins.is_empty() {
            bare_edit
        } else {
            planned(&list, &new_mod, &request).map_err(shown)?
        };
        Ok(AddPlan {
            block: new_mod.archive_block.clone().unwrap_or_default(),
            diff: edit.diff(),
            command: command(&list, &request, &new_mod),
            installs_plugins,
            files,
            suggested_after: suggested_after(
                &source,
                &request.section,
                request.before_mod.as_deref().filter(|b| !b.is_empty()),
            ),
            notes,
        })
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Add the mod (`add`), into the history like any edit.
#[tauri::command]
pub(super) fn add_mod(
    state: tauri::State<'_, State>,
    request: AddRequest,
) -> Result<Edited, String> {
    let list = state.list()?;
    let (new_mod, _, _) = new_mod(&request)?;
    let edit = planned(&list, &new_mod, &request).map_err(shown)?;
    apply(&state, command(&list, &request, &new_mod), edit)
}

// --- An existing archive's installer, walked again ---------------------------------------

/// Mod `id`'s `number`th archive, found for walking its installer again, with
/// the answers given in Install deciding which archives it has.
fn existing(state: &State, id: &str, number: usize) -> Result<installer::Existing, String> {
    let list = state.list()?;
    let located = locate(state)?;
    let downloads = located
        .downloads
        .ok_or("say where the downloads folder is first (in Install)")?;
    let answers: config::query::Overrides = state
        .settings
        .lock()
        .map_err(|_| "a command panicked")?
        .list(&list)
        .answers
        .into_iter()
        .collect();
    installer::existing(&list, id, number, &answers, &downloads, &located.search).map_err(shown)
}

/// The installer of mod `id`'s archive walked with `answers` so far, what the
/// list chooses now offered as each default (`choose`).
#[tauri::command]
pub(super) async fn choose_walk(
    state: tauri::State<'_, State>,
    id: String,
    number: usize,
    answers: Vec<Answer>,
) -> Result<Walk, String> {
    let found = existing(&state, &id, number)?;
    tokio::task::spawn_blocking(move || {
        installer::walk(
            &found.archive,
            &found.report,
            &found.list,
            &found.prior,
            &answers,
            false,
        )
        .map_err(shown)
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Write what the walk chose (`choose`), into the history like any edit.
#[tauri::command]
pub(super) fn choose_write(
    state: tauri::State<'_, State>,
    id: String,
    number: usize,
    answers: Vec<Answer>,
) -> Result<Edited, String> {
    let list = state.list()?;
    let found = existing(&state, &id, number)?;
    let installed = match installer::walk(
        &found.archive,
        &found.report,
        &found.list,
        &found.prior,
        &answers,
        false,
    )
    .map_err(shown)?
    {
        Walk::Done { installed } => installed,
        Walk::Ask { .. } => return Err("answer the installer's questions first".to_string()),
    };
    let edit = installer::plan_choice(&list, &id, number, &installed).map_err(shown)?;
    let mut command = format!(
        "mudcrab choose {} --mod {}",
        quote(list.display().to_string()),
        quote(&id)
    );
    if number != 1 {
        command.push_str(&format!(" --archive {number}"));
    }
    apply(&state, command, edit)
}

// --- From a Nexus link -------------------------------------------------------------------

/// A mod on Nexus, and its files to choose from.
#[derive(Serialize)]
pub(super) struct NexusMod {
    mod_id: u64,
    /// The file the link named, if it named one.
    file_id: Option<u64>,
    files: Vec<NexusFileRow>,
}

#[derive(Serialize)]
pub(super) struct NexusFileRow {
    #[serde(flatten)]
    file: config::identify::NexusFile,
    /// Where it already is, if it is in a downloads folder.
    here: Option<String>,
}

/// The mod and file a Nexus link names: `.../oblivion/mods/<mod>`, with
/// `file_id=<file>` when it names a file; or a bare mod id.
fn parse_link(link: &str) -> Option<(u64, Option<u64>)> {
    let link = link.trim();
    if let Ok(mod_id) = link.parse() {
        return Some((mod_id, None));
    }
    let after = link.split("/mods/").nth(1)?;
    let mod_id = after
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    let file_id = link
        .split(['?', '&'])
        .find_map(|part| part.strip_prefix("file_id="))
        .and_then(|id| id.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok());
    Some((mod_id, file_id))
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("mudcrab")
        .build()
        .map_err(|err| err.to_string())
}

/// The files of the mod a Nexus link names (`nexus-files`), current ones
/// first.
#[tauri::command]
pub(super) async fn nexus_link(
    state: tauri::State<'_, State>,
    link: String,
) -> Result<NexusMod, String> {
    let (mod_id, file_id) = parse_link(&link)
        .ok_or("that is not a link to a Nexus mod (…nexusmods.com/oblivion/mods/<number>)")?;
    let key = config::tools::ToolsConfig::nexus_api_key().ok_or(
        "listing a mod's files needs a Nexus API key (NEXUS_API_KEY, or nexus_api_key in \
         tools.toml)",
    )?;
    let located = locate(&state)?;
    let dirs: Vec<PathBuf> = located
        .downloads
        .into_iter()
        .chain(located.search)
        .collect();
    let base = std::env::var("NEXUS_API_BASE").ok();
    let mut files =
        config::identify::nexus_files(&client()?, base.as_deref(), &key, "oblivion", mod_id)
            .await
            .map_err(shown)?;
    let rank = |category: Option<&str>| match category {
        Some("MAIN") => 0,
        Some("UPDATE") => 1,
        Some("OPTIONAL") => 2,
        Some("MISCELLANEOUS") => 3,
        Some("OLD_VERSION") => 5,
        Some("ARCHIVED" | "DELETED") => 6,
        _ => 4,
    };
    files.sort_by(|a, b| {
        rank(a.category_name.as_deref())
            .cmp(&rank(b.category_name.as_deref()))
            .then(b.file_id.cmp(&a.file_id))
    });
    let listed = config::archive_store::list_archives(&dirs);
    Ok(NexusMod {
        mod_id,
        file_id,
        files: files
            .into_iter()
            .map(|file| NexusFileRow {
                here: listed
                    .iter()
                    .find(|l| {
                        l.source.as_deref()
                            == Some(&format!("nexus:oblivion/{mod_id}/{}", file.file_id))
                            || file
                                .file_name
                                .as_deref()
                                .is_some_and(|name| name.eq_ignore_ascii_case(&l.file_name))
                    })
                    .map(|l| l.path.display().to_string()),
                file,
            })
            .collect(),
    })
}

/// A fetch: the archive, when it is here; otherwise the page to get it from
/// by hand, and why.
#[derive(Serialize)]
pub(super) struct Fetched {
    path: Option<String>,
    page: Option<String>,
    why: Option<String>,
}

/// Fetch one Nexus file into the downloads folder, as `download` fetches a
/// list's: with a Premium key, straight away; otherwise its page is given
/// to download from, and `nexus_look` watches for it.
#[tauri::command]
pub(super) async fn nexus_fetch(
    state: tauri::State<'_, State>,
    mod_id: u64,
    file_id: u64,
    file_name: Option<String>,
) -> Result<Fetched, String> {
    let downloads = locate(&state)?
        .downloads
        .ok_or("say where the downloads folder is first (in Install)")?;
    std::fs::create_dir_all(&downloads).map_err(|err| format!("{}: {err}", downloads.display()))?;
    // A plan of one archive, so the download is `download`'s own.
    let plan: config::schema::PersonalizedPlan = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "name": "fetch", "responses": {},
        "mod_order": ["fetch"], "selected_mods": ["fetch"], "plugins": [],
        "mods": [{"id": "fetch", "archives": [{
            "path": format!("nexus:oblivion/{mod_id}/{file_id}"),
            "download_handler": "nexus",
            "file_name": file_name,
        }]}]
    }))
    .map_err(|err| err.to_string())?;
    let settings = config::download::DownloadSettings {
        cache_dir: downloads.clone(),
        retry: 3,
        nexus_api_key: config::tools::ToolsConfig::nexus_api_key(),
        nexus_api_base: std::env::var("NEXUS_API_BASE").ok(),
        filter: Default::default(),
        archive_search_paths: Vec::new(),
        parallel: 1,
    };
    let fetch_plan = plan.clone();
    let unfetched = tokio::task::spawn_blocking(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(config::download::download_report(&fetch_plan, &settings))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(shown)?;
    let Some(item) = unfetched.into_iter().next() else {
        let archive = &plan.mods[0].archives[0];
        let reference = config::archive_store::ArchiveRef::for_archive(archive);
        let path = config::archive_store::find(&downloads, &[], &reference)
            .ok_or("downloaded, but not found in the downloads folder")?;
        return Ok(Fetched {
            path: Some(path.display().to_string()),
            page: None,
            why: None,
        });
    };
    let fetched = Fetched {
        path: None,
        page: config::by_hand::page_of(&item),
        why: Some(item.error.clone()),
    };
    let watcher = config::by_hand::Watcher::new(&downloads, None);
    state
        .session
        .lock()
        .map_err(|_| "a command panicked")?
        .fetching = Some((item, watcher));
    Ok(fetched)
}

/// Look once for the archive being fetched by hand: its path once it has
/// arrived (taken in from the browser's folder, with its `.meta`).
#[tauri::command]
pub(super) fn nexus_look(state: tauri::State<'_, State>) -> Result<Option<String>, String> {
    let mut session = state.session.lock().map_err(|_| "a command panicked")?;
    let Some((item, watcher)) = session.fetching.as_mut() else {
        return Err("nothing is being fetched".to_string());
    };
    let path = match watcher.look(item) {
        Some(config::by_hand::Found::Here(path)) => path,
        Some(config::by_hand::Found::InBrowserFolder(file)) => watcher
            .take_in(&file, item, config::by_hand::BrowserDownloads::Move)
            .map_err(shown)?,
        None => return Ok(None),
    };
    session.fetching = None;
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::{parse_link, suggested_id};

    #[test]
    fn a_nexus_link_names_a_mod_and_maybe_a_file() {
        assert_eq!(
            parse_link("https://www.nexusmods.com/oblivion/mods/16704?tab=files&file_id=50293"),
            Some((16704, Some(50293)))
        );
        assert_eq!(
            parse_link("https://www.nexusmods.com/games/oblivion/mods/21862"),
            Some((21862, None))
        );
        assert_eq!(parse_link(" 123 "), Some((123, None)));
        assert_eq!(parse_link("https://example.com/"), None);
    }

    #[test]
    fn an_id_comes_from_the_file_name_without_nexus_ids() {
        assert_eq!(
            suggested_id(None, "Formid Finder 451-16704.7z"),
            "Formid Finder 451"
        );
        assert_eq!(suggested_id(None, "SMO-51073-1-6-2-1640189859.zip"), "SMO");
        assert_eq!(suggested_id(None, "Plain Name.zip"), "Plain Name");
        assert_eq!(suggested_id(None, "A-B Mod-1-2.7z"), "A-B Mod-1-2");
    }
}
