//! mudcrab's GUI, install mode: open a list, read it, answer its questions,
//! say where things go, and install -- the CLI's pipeline (`compile`, `query`,
//! `download`, `check`, `install`) run step by step from one window, each
//! step saying what it writes and which command does the same.
//!
//! Everything here calls mudcrab's library, as the CLI does; nothing is done
//! that a command cannot do. See docs/design/gui.md. The commands are here,
//! generic over Tauri's runtime, so tests drive them on its mock runtime;
//! `main.rs` only starts the window.

mod adding;
mod author;
mod edits;
pub mod settings;
pub mod sink;

use mudcrab::config;
use mudcrab::config::by_hand::{Found, Watcher};
use mudcrab::config::download::Unfetched;
use mudcrab::config::schema::PersonalizedPlan;
use serde::Serialize;
use settings::Settings;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Manager, Runtime};

/// A command's error, as the page shows it, with where the full log is.
fn shown(err: anyhow::Error) -> String {
    match mudcrab::ui::log_path() {
        Some(log) => format!("{err:#}\n\nFull log: {}", log.display()),
        None => format!("{err:#}"),
    }
}

/// The list open now, and where its run is.
#[derive(Default)]
struct Session {
    list: Option<PathBuf>,
    /// Where `compile` and `query` write, for this list.
    work: Option<PathBuf>,
    /// Whether the list cleans plugins, and so needs xEdit set up.
    cleans: bool,
    plan: Option<PersonalizedPlan>,
    /// When the list's file had last changed when `plan` was made, so author
    /// mode can tell it is out of date.
    planned_at: Option<std::time::SystemTime>,
    /// Each plugin's masters, as the last `check` read them.
    masters: Option<Vec<config::check::PluginMasters>>,
    /// The edits made here, for undo, redo and the session's changes.
    history: edits::History,
    /// Archives still to come by hand, and what watches for them.
    pending: Vec<Unfetched>,
    watcher: Option<Watcher>,
    /// An archive being fetched by hand to add as a mod, and what watches
    /// for it.
    fetching: Option<(Unfetched, Watcher)>,
}

pub struct State {
    settings: Mutex<Settings>,
    session: Mutex<Session>,
}

impl State {
    fn list(&self) -> Result<PathBuf, String> {
        self.session
            .lock()
            .map_err(|_| "a command panicked".to_string())?
            .list
            .clone()
            .ok_or_else(|| "no list is open".to_string())
    }
}

/// Quote a path or value for a POSIX shell, if it needs it: in single
/// quotes, where nothing is special (no `$`, backticks or backslashes), a
/// single quote itself written as `'\''`. The commands shown are copied into
/// terminals, so they must replay exactly what was done.
fn quote(text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    if !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c))
    {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_quoted_value_is_taken_literally_by_a_shell() {
        use super::quote;
        assert_eq!(quote("plain/path.toml"), "plain/path.toml");
        assert_eq!(quote("New Thing"), "'New Thing'");
        assert_eq!(quote("$(rm -rf ~) `x` \\n"), "'$(rm -rf ~) `x` \\n'");
        assert_eq!(quote("it's"), r"'it'\''s'");
        assert_eq!(quote(""), "''");
    }
}

// --- Opening a list -----------------------------------------------------------

#[derive(Serialize)]
struct ListView {
    path: String,
    /// The list's `about`, for whoever installs it.
    about: Option<String>,
    document: mudcrab::export::Document,
    /// The answers last given here, by input.
    answers: BTreeMap<String, String>,
}

#[tauri::command]
fn recent(state: tauri::State<'_, State>) -> Vec<String> {
    state
        .settings
        .lock()
        .map(|s| {
            s.recent
                .iter()
                .filter(|p| p.is_file())
                .map(|p| p.display().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command]
fn open_list<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, State>,
    path: String,
) -> Result<ListView, String> {
    let path = PathBuf::from(path);
    let (modlist, document) = read_list(&path)?;

    let work = app
        .path()
        .app_local_data_dir()
        .map_err(|err| err.to_string())?
        .join("lists")
        .join(work_name(&path));
    let answers = {
        let mut settings = state.settings.lock().map_err(|_| "a command panicked")?;
        settings.opened(&path);
        let _ = settings.save();
        settings.list(&path).answers
    };
    let cleans = modlist.mods.iter().any(|m| {
        m.actions
            .iter()
            .any(|a| matches!(a.action, config::schema::ModAction::Qac(_)))
    });
    *state.session.lock().map_err(|_| "a command panicked")? = Session {
        list: Some(path.clone()),
        work: Some(work),
        cleans,
        ..Session::default()
    };
    Ok(ListView {
        about: modlist.about.clone(),
        path: path.display().to_string(),
        document,
        answers,
    })
}

/// The list at `path` and the document the page shows of it. The prose is
/// in the docs file when the list has one; a list shared without it still
/// opens, with less to read.
fn read_list(
    path: &Path,
) -> Result<(config::schema::SourceModlist, mudcrab::export::Document), String> {
    let modlist = config::loader::load_modlist_with_docs(path)
        .or_else(|_| config::loader::load_modlist(path))
        .map_err(shown)?;
    config::validator::validate(&modlist).map_err(shown)?;
    let document = mudcrab::export::build_document(&modlist, None, None).map_err(shown)?;
    Ok((modlist, document))
}

/// The open list read again, after an edit, keeping the session (its plan,
/// history and checks).
#[tauri::command]
fn reload_list(state: tauri::State<'_, State>) -> Result<ListView, String> {
    let path = state.list()?;
    let (modlist, document) = read_list(&path)?;
    let answers = state
        .settings
        .lock()
        .map_err(|_| "a command panicked")?
        .list(&path)
        .answers;
    Ok(ListView {
        about: modlist.about.clone(),
        path: path.display().to_string(),
        document,
        answers,
    })
}

/// A folder name for a list's working files: its name, and a hash of its
/// path so two lists of the same name do not share one.
fn work_name(list: &Path) -> String {
    let stem: String = list
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // FNV-1a: stable across runs and platforms, unlike the std hasher.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in list.display().to_string().bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("{stem}-{:08x}", hash as u32)
}

// --- Where things go --------------------------------------------------------------

#[derive(Serialize)]
struct Location {
    path: Option<String>,
    /// Where the value came from: "chosen", "tools.toml", "the MO2 instance",
    /// "found".
    from: Option<String>,
    exists: bool,
}

impl Location {
    fn of(found: Option<(PathBuf, &str)>) -> Self {
        match found {
            Some((path, from)) => Self {
                exists: path.exists(),
                path: Some(path.display().to_string()),
                from: Some(from.to_string()),
            },
            None => Self {
                path: None,
                from: None,
                exists: false,
            },
        }
    }
}

/// The tools.toml in use, and what in it stops this list installing.
#[derive(Serialize)]
struct Tools {
    path: Option<String>,
    /// "chosen" here, "MUDCRAB_TOOLS_CONFIG", or "usual".
    from: String,
    exists: bool,
    problems: Vec<String>,
}

#[derive(Serialize)]
struct Locations {
    tools: Tools,
    instance: Location,
    downloads: Location,
    game: Location,
    profile: String,
    /// Other folders to look in for archives, before downloading.
    search: Vec<Location>,
}

/// Where things go, as shown, and as paths.
struct Located {
    search: Vec<PathBuf>,
    shown: Locations,
    instance: Option<PathBuf>,
    downloads: Option<PathBuf>,
    game: Option<PathBuf>,
}

/// The downloads folder, MO2 instance and game folder for the open list:
/// what was chosen here, else what tools.toml and the instance say, else
/// what can be found, as `setup-tools` and `game-dir` find them.
fn locate(state: &State) -> Result<Located, String> {
    let list = state.list()?;
    let chosen = state
        .settings
        .lock()
        .map_err(|_| "a command panicked")?
        .list(&list);
    let tools = config::tools::ToolsConfig::default_path()
        .and_then(|path| config::tools::ToolsConfig::load(&path).ok())
        .unwrap_or_default();

    let instance = chosen.instance.clone().map(|p| (p, "chosen"));
    let downloads = chosen
        .downloads
        .clone()
        .map(|p| (p, "chosen"))
        .or_else(|| tools.downloads_dir.clone().map(|p| (p, "tools.toml")))
        .or_else(|| {
            let instance = chosen.instance.as_ref()?;
            config::mo2::instance_download_dir(instance)
                .or_else(|| Some(instance.join("downloads")))
                .map(|p| (p, "the MO2 instance"))
        });
    let game = chosen
        .game
        .clone()
        .map(|p| (p, "chosen"))
        .or_else(|| {
            chosen
                .instance
                .as_ref()
                .and_then(|i| config::mo2::instance_game_path(i))
                .map(|p| (p, "the MO2 instance"))
        })
        .or_else(|| {
            use config::tools::discover;
            discover::game_dirs(&discover::steam_install_roots(), &discover::search_roots())
                .into_iter()
                .next()
                .map(|candidate| (candidate.path, "found"))
        });
    let cleans = state
        .session
        .lock()
        .map_err(|_| "a command panicked")?
        .cleans;
    let tools_path = config::tools::ToolsConfig::default_path();
    let tools_info = Tools {
        from: match config::tools::ToolsConfig::chosen_path() {
            Some((_, "--tools-config")) => "chosen".to_string(),
            Some((_, from)) => from.to_string(),
            None => "usual".to_string(),
        },
        exists: tools_path.as_ref().is_some_and(|p| p.is_file()),
        path: tools_path.as_ref().map(|p| p.display().to_string()),
        problems: if cleans {
            tools.xedit_problems()
        } else {
            Vec::new()
        },
    };
    Ok(Located {
        search: chosen.search.clone(),
        instance: instance.as_ref().map(|(p, _)| p.clone()),
        downloads: downloads.as_ref().map(|(p, _)| p.clone()),
        game: game.as_ref().map(|(p, _)| p.clone()),
        shown: Locations {
            tools: tools_info,
            instance: Location::of(instance),
            downloads: Location::of(downloads),
            game: Location::of(game),
            profile: chosen.profile.unwrap_or_else(|| "Default".to_string()),
            search: chosen
                .search
                .iter()
                .map(|p| Location::of(Some((p.clone(), "chosen"))))
                .collect(),
        },
    })
}

#[tauri::command]
fn locations(state: tauri::State<'_, State>) -> Result<Locations, String> {
    locate(&state).map(|located| located.shown)
}

#[tauri::command]
fn set_location(
    state: tauri::State<'_, State>,
    kind: String,
    path: Option<String>,
) -> Result<Locations, String> {
    let list = state.list()?;
    {
        let mut settings = state.settings.lock().map_err(|_| "a command panicked")?;
        let chosen = settings.list_mut(&list);
        let path = path.map(PathBuf::from);
        if kind == "instance"
            && let Some(problem) = path.as_deref().and_then(config::mo2::not_an_instance)
        {
            return Err(problem);
        }
        match kind.as_str() {
            "instance" => chosen.instance = path,
            "downloads" => chosen.downloads = path,
            "game" => chosen.game = path,
            "profile" => chosen.profile = path.map(|p| p.display().to_string()),
            other => return Err(format!("no location called {other}")),
        }
        settings.save().map_err(shown)?;
    }
    locations(state)
}

/// Use this tools.toml (or with `None`, the usual one), from now on.
#[tauri::command]
fn set_tools_config(
    state: tauri::State<'_, State>,
    path: Option<String>,
) -> Result<Locations, String> {
    let path = path.map(PathBuf::from);
    if let Some(path) = &path {
        config::tools::ToolsConfig::load(path).map_err(shown)?;
    }
    config::tools::ToolsConfig::set_path_override(path.clone());
    {
        let mut settings = state.settings.lock().map_err(|_| "a command panicked")?;
        settings.tools_config = path;
        settings.save().map_err(shown)?;
    }
    locations(state)
}

/// Look in another folder for archives, before downloading them.
#[tauri::command]
fn add_search_folder(state: tauri::State<'_, State>, path: String) -> Result<Locations, String> {
    let list = state.list()?;
    {
        let mut settings = state.settings.lock().map_err(|_| "a command panicked")?;
        let search = &mut settings.list_mut(&list).search;
        let path = PathBuf::from(path);
        if !search.contains(&path) {
            search.push(path);
        }
        settings.save().map_err(shown)?;
    }
    locations(state)
}

/// Stop looking in a folder.
#[tauri::command]
fn remove_search_folder(state: tauri::State<'_, State>, path: String) -> Result<Locations, String> {
    let list = state.list()?;
    {
        let mut settings = state.settings.lock().map_err(|_| "a command panicked")?;
        let path = PathBuf::from(path);
        settings.list_mut(&list).search.retain(|p| p != &path);
        settings.save().map_err(shown)?;
    }
    locations(state)
}

// --- Installing, a step at a time ---------------------------------------------------

/// What a step did: a line to show, the files it wrote, and the command that
/// does the same from a terminal.
#[derive(Serialize)]
struct Done {
    summary: String,
    writes: Vec<String>,
    command: String,
}

/// Compile the list and plan it with these answers: `compile`, then `query`.
#[tauri::command]
fn step_plan(
    state: tauri::State<'_, State>,
    answers: BTreeMap<String, String>,
) -> Result<Done, String> {
    let list = state.list()?;
    {
        let mut settings = state.settings.lock().map_err(|_| "a command panicked")?;
        settings.list_mut(&list).answers = answers.clone();
        let _ = settings.save();
    }
    plan_with(&state, answers)
}

/// `compile` and `query` with these answers, keeping the plan for the steps
/// after.
fn plan_with(state: &State, answers: BTreeMap<String, String>) -> Result<Done, String> {
    let list = state.list()?;
    let work = state
        .session
        .lock()
        .map_err(|_| "a command panicked")?
        .work
        .clone()
        .ok_or("no list is open")?;
    let planned_at = std::fs::metadata(&list).and_then(|m| m.modified()).ok();
    let compiled_path = work.join("compiled.json");
    let plan_path = work.join("plan.json");
    let compiled = config::pipeline::compile(&list, false).map_err(shown)?;
    config::pipeline::write_json(&compiled_path, &compiled.modlist).map_err(shown)?;
    let overrides: HashMap<String, String> = answers.clone().into_iter().collect();
    let plan = config::query::build_plan(&compiled.modlist, &overrides, None).map_err(shown)?;
    config::pipeline::write_json(&plan_path, &plan).map_err(shown)?;

    let sets: String = answers
        .iter()
        .map(|(key, value)| format!(" --set {}", quote(format!("{key}={value}"))))
        .collect();
    let done = Done {
        summary: format!(
            "{} of {} mod(s) selected, {} plugin(s)",
            plan.selected_mods.len(),
            plan.mod_order.len(),
            plan.plugins.len()
        ),
        writes: vec![
            compiled_path.display().to_string(),
            plan_path.display().to_string(),
        ],
        command: format!(
            "mudcrab compile {} --output {}\nmudcrab query {} --headless{sets} --output {}",
            quote(list.display().to_string()),
            quote(compiled_path.display().to_string()),
            quote(compiled_path.display().to_string()),
            quote(plan_path.display().to_string()),
        ),
    };
    let mut session = state.session.lock().map_err(|_| "a command panicked")?;
    session.plan = Some(plan);
    session.planned_at = planned_at;
    Ok(done)
}

/// An archive to fetch by hand, as the page shows it.
#[derive(Serialize)]
struct ByHand {
    mod_id: String,
    label: String,
    page: Option<String>,
    note: Option<String>,
    why: String,
}

impl ByHand {
    fn of(item: &Unfetched) -> Self {
        Self {
            mod_id: item.mod_id.clone(),
            label: item.label.clone(),
            page: config::by_hand::page_of(item),
            note: item.note.clone(),
            why: item.error.clone(),
        }
    }
}

#[derive(Serialize)]
struct Downloaded {
    done: Done,
    by_hand: Vec<ByHand>,
    /// Where browser downloads are watched for, if anywhere.
    browser_folder: Option<String>,
}

fn plan_of(state: &State) -> Result<PersonalizedPlan, String> {
    state
        .session
        .lock()
        .map_err(|_| "a command panicked")?
        .plan
        .clone()
        .ok_or_else(|| "plan the list first".to_string())
}

fn plan_path(state: &State) -> Result<PathBuf, String> {
    Ok(state
        .session
        .lock()
        .map_err(|_| "a command panicked")?
        .work
        .clone()
        .ok_or("no list is open")?
        .join("plan.json"))
}

/// Fetch what can be fetched (`download`), and list what has to come by hand.
#[tauri::command]
async fn step_download(state: tauri::State<'_, State>) -> Result<Downloaded, String> {
    let plan = plan_of(&state)?;
    let Located {
        downloads, search, ..
    } = locate(&state)?;
    let downloads = downloads.ok_or("say where the downloads folder is first")?;
    // An archive found in one of these is linked into the downloads folder,
    // so `check` and `install` find everything in one place.
    let search_args: String = search
        .iter()
        .map(|dir| {
            format!(
                " --archive-search-path {}",
                quote(dir.display().to_string())
            )
        })
        .collect();
    std::fs::create_dir_all(&downloads).map_err(|err| format!("{}: {err}", downloads.display()))?;
    let settings = config::download::DownloadSettings {
        cache_dir: downloads.clone(),
        retry: 3,
        nexus_api_key: config::tools::ToolsConfig::nexus_api_key(),
        nexus_api_base: std::env::var("NEXUS_API_BASE").ok(),
        filter: Default::default(),
        archive_search_paths: search,
        parallel: 4,
    };
    // The download's future cannot be shown to be `Send` (a rustc limit with
    // the stream of closures inside), which a command's future must be; on a
    // runtime of its own, in a blocking thread, it need not be.
    let unfetched = tokio::task::spawn_blocking(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(config::download::download_report(&plan, &settings))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(shown)?;
    let watcher = Watcher::new(&downloads, None);
    let downloaded = Downloaded {
        done: Done {
            summary: if unfetched.is_empty() {
                "every archive is in the downloads folder".to_string()
            } else {
                format!("{} archive(s) to download by hand", unfetched.len())
            },
            writes: vec![downloads.display().to_string()],
            command: format!(
                "mudcrab download {} --downloads {}{search_args}",
                quote(plan_path(&state)?.display().to_string()),
                quote(downloads.display().to_string())
            ),
        },
        by_hand: unfetched.iter().map(ByHand::of).collect(),
        browser_folder: watcher.browser_folder().map(|p| p.display().to_string()),
    };
    let mut session = state.session.lock().map_err(|_| "a command panicked")?;
    session.pending = unfetched;
    session.watcher = Some(watcher);
    Ok(downloaded)
}

/// Look once for the archives being fetched by hand: what has arrived is
/// taken in (moved from the browser's folder, with its `.meta`), and what is
/// still missing comes back.
#[tauri::command]
fn look_for_downloads(state: tauri::State<'_, State>) -> Result<Vec<ByHand>, String> {
    let mut session = state.session.lock().map_err(|_| "a command panicked")?;
    let Session {
        pending, watcher, ..
    } = &mut *session;
    let watcher = watcher.as_mut().ok_or("download first")?;
    let mut still = Vec::new();
    for item in pending.drain(..) {
        match watcher.look(&item) {
            Some(Found::Here(_)) => {}
            Some(Found::InBrowserFolder(file)) => {
                if let Err(err) =
                    watcher.take_in(&file, &item, config::by_hand::BrowserDownloads::Move)
                {
                    report_warning(&format!("{err:#}"));
                    still.push(item);
                }
            }
            None => still.push(item),
        }
    }
    *pending = still;
    Ok(pending.iter().map(ByHand::of).collect())
}

fn report_warning(message: &str) {
    mudcrab::events::status(mudcrab::events::Tone::Warning, message);
}

/// Open a page in the browser.
#[tauri::command]
fn open_page(url: String) -> Result<(), String> {
    // Only web pages: a list names these, and a list is someone else's file.
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("not a web page: {url}"));
    }
    if config::by_hand::open_url(&url) {
        Ok(())
    } else {
        Err(format!("could not open {url}; open it yourself"))
    }
}

#[derive(Serialize)]
struct Checked {
    done: Done,
    problems: usize,
    warnings: usize,
    findings: Vec<config::check::Finding>,
    /// The findings under their headings, as the terminal shows them.
    groups: Vec<FindingGroup>,
}

#[derive(Serialize)]
struct FindingGroup {
    heading: &'static str,
    tone: mudcrab::events::Tone,
    items: Vec<mudcrab::events::Item>,
}

/// Findings grouped by kind, in the order `check` reports them.
fn grouped(findings: &[config::check::Finding]) -> Vec<FindingGroup> {
    let mut by_kind: BTreeMap<config::check::Kind, Vec<mudcrab::events::Item>> = BTreeMap::new();
    for finding in findings {
        by_kind
            .entry(finding.kind)
            .or_default()
            .push(finding.item.clone());
    }
    by_kind
        .into_iter()
        .map(|(kind, items)| FindingGroup {
            heading: kind.heading(),
            tone: kind.tone(),
            items,
        })
        .collect()
}

/// Everything that can be known before installing (`check`).
#[tauri::command]
async fn step_check(state: tauri::State<'_, State>) -> Result<Checked, String> {
    let plan = author::current_plan(&state)?;
    let downloads = locate(&state)?.downloads;
    let downloads = downloads.ok_or("say where the downloads folder is first")?;
    let command = format!(
        "mudcrab check {} --downloads {}",
        quote(plan_path(&state)?.display().to_string()),
        quote(downloads.display().to_string())
    );
    let settings = config::check::CheckSettings {
        cache_dir: downloads,
        filter: Default::default(),
    };
    let report = tokio::task::spawn_blocking(move || config::check::check_all(&plan, &settings))
        .await
        .map_err(|err| err.to_string())?
        .map_err(shown)?;
    state
        .session
        .lock()
        .map_err(|_| "a command panicked")?
        .masters = Some(report.plugins);
    Ok(Checked {
        done: Done {
            summary: format!(
                "{} archive(s) in {} mod(s): {} problem(s), {} warning(s)",
                report.archives_checked, report.mods_checked, report.problems, report.warnings
            ),
            writes: Vec::new(),
            command,
        },
        problems: report.problems,
        warnings: report.warnings,
        groups: grouped(&report.findings),
        findings: report.findings,
    })
}

/// Install settings for the open list, as `install --mo2-instance-dir` makes
/// them.
fn install_settings(state: &State) -> Result<config::install::InstallSettings, String> {
    let Located {
        shown: places,
        instance,
        downloads,
        game,
        ..
    } = locate(state)?;
    let instance = instance.ok_or("choose the MO2 instance to install into first")?;
    let downloads = downloads.ok_or("say where the downloads folder is first")?;
    let tools = config::tools::ToolsConfig::default_path()
        .map(|path| config::tools::ToolsConfig::load(&path))
        .transpose()
        .map_err(shown)?
        .unwrap_or_default();
    Ok(config::install::InstallSettings {
        cache_dir: downloads,
        mods_dir: instance.join("mods"),
        mo2_instance_dir: Some(instance),
        profile_name: places.profile,
        game_dir: game,
        execute_actions: true,
        dry_run: false,
        tools,
        filter: Default::default(),
        force_merges: false,
        force: false,
        redo_actions: Vec::new(),
    })
}

/// Install into the MO2 instance (`install`).
#[tauri::command]
async fn step_install(state: tauri::State<'_, State>) -> Result<Done, String> {
    let plan = plan_of(&state)?;
    let settings = install_settings(&state)?;
    let instance = settings.mo2_instance_dir.clone().ok_or("no instance")?;
    let pids = config::mo2::running::running_in(&instance);
    if !pids.is_empty() {
        return Err(format!(
            "Mod Organizer 2 has {} open. Close it first: MO2 writes its own copy of the profile \
             back when it exits, which would undo this install.",
            instance.display()
        ));
    }
    let command = format!(
        "mudcrab install {} --mo2-instance-dir {} --downloads {}{}",
        quote(plan_path(&state)?.display().to_string()),
        quote(instance.display().to_string()),
        quote(settings.cache_dir.display().to_string()),
        settings
            .game_dir
            .as_ref()
            .map(|dir| format!(" --game-dir {}", quote(dir.display().to_string())))
            .unwrap_or_default()
    );
    let profile = instance.join("profiles").join(&settings.profile_name);
    tokio::task::spawn_blocking(move || config::install::install_all(&plan, &settings))
        .await
        .map_err(|err| err.to_string())?
        .map_err(shown)?;
    Ok(Done {
        summary: "installed".to_string(),
        writes: vec![
            instance.join("mods").display().to_string(),
            profile.display().to_string(),
        ],
        command,
    })
}

#[derive(Serialize)]
struct StepByHand {
    title: String,
    steps: Vec<String>,
    /// Known done (its plugin exists), known not, or unknowable.
    done: Option<bool>,
}

/// The steps left to do by hand, and which are done.
#[tauri::command]
fn manual_steps(state: tauri::State<'_, State>) -> Result<Vec<StepByHand>, String> {
    let plan = plan_of(&state)?;
    let settings = install_settings(&state)?;
    Ok(config::install::manual_steps(&plan, &settings)
        .into_iter()
        .map(|(step, done)| StepByHand {
            done: step.done_when_plugin.is_some().then_some(done),
            steps: step
                .steps
                .lines()
                .map(str::trim_end)
                .filter(|l| !l.trim().is_empty())
                .map(str::to_string)
                .collect(),
            title: step.title,
        })
        .collect())
}

/// The list given on the command line, to open at start.
pub struct Opening(pub Option<String>);

#[tauri::command]
fn opening(start: tauri::State<'_, Opening>) -> Option<String> {
    start.0.clone()
}

/// The app: its state, the file pickers (where there is a real window to
/// show them from), mudcrab's events sent to the page, and the commands.
/// `start` is a list to open at once.
pub fn app<R: Runtime>(
    builder: tauri::Builder<R>,
    start: Option<String>,
    pickers: bool,
) -> tauri::Builder<R> {
    let builder = if pickers {
        builder.plugin(tauri_plugin_dialog::init())
    } else {
        builder
    };
    builder
        .manage(State {
            settings: Mutex::new(Settings::load()),
            session: Mutex::new(Session::default()),
        })
        .manage(Opening(start))
        .setup(|app| {
            // The tools.toml chosen here last time, unless the command line
            // or environment names one.
            let chosen = app
                .state::<State>()
                .settings
                .lock()
                .ok()
                .and_then(|s| s.tools_config.clone());
            if config::tools::ToolsConfig::chosen_path().is_none()
                && let Some(path) = chosen
            {
                config::tools::ToolsConfig::set_path_override(path);
            }
            mudcrab::events::set_sink(std::sync::Arc::new(sink::GuiSink::new(
                app.handle().clone(),
            )));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            opening,
            recent,
            open_list,
            locations,
            set_location,
            set_tools_config,
            add_search_folder,
            remove_search_folder,
            step_plan,
            step_download,
            look_for_downloads,
            open_page,
            step_check,
            step_install,
            manual_steps,
            author::load_order,
            author::mod_view,
            author::mod_files,
            author::conflicts,
            author::conflicts_between,
            reload_list,
            edits::undo,
            edits::redo,
            edits::changes,
            edits::move_plugin,
            edits::move_mod,
            edits::edit_filter,
            edits::set_fields,
            edits::add_action,
            edits::remove_action,
            edits::actions,
            edits::add_input,
            edits::set_input,
            edits::remove_input,
            edits::set_docs,
            adding::archives,
            adding::add_start,
            adding::add_walk,
            adding::add_plan,
            adding::add_mod,
            adding::nexus_link,
            adding::nexus_fetch,
            adding::nexus_look,
        ])
}
