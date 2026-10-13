pub mod add;
mod add_wizard;
mod assets;
mod bsa;
pub mod check;
mod choose;
pub mod compile;
pub mod conflicts;
mod dds;
pub mod diff;
pub mod download;
mod download_browser;
mod downloads;
mod edit;
pub mod export;
mod game_dir;
pub mod identify;
pub mod inspect;
pub mod install;
pub mod merge;
pub mod move_mod;
mod new_merge;
pub mod nexus_files;
pub mod prompt;
pub mod query;
pub mod setup_tools;
pub mod unhide_merges;
pub mod validate;

use crate::cli::{Cli, Command};
use std::path::PathBuf;

/// The game folder: `--game-dir`, else `GAME_DIR`, else the folder the MO2
/// instance being installed into was set up with -- so an MO2 install needs
/// neither. An install into a plain mods directory can do without one: what
/// needs it (game INI edits, LOOT) says so when it runs.
pub fn resolve_game_dir(
    explicit: Option<PathBuf>,
    mo2_instance: Option<&std::path::Path>,
) -> anyhow::Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        return Ok(Some(path));
    }
    if let Ok(value) = std::env::var("GAME_DIR") {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            anyhow::bail!("GAME_DIR is defined but empty");
        }
        return Ok(Some(PathBuf::from(trimmed)));
    }
    let Some(instance) = mo2_instance else {
        return Ok(None);
    };
    match crate::config::mo2::instance_game_path(instance) {
        Some(path) => Ok(Some(path)),
        None => anyhow::bail!(
            "no game folder: pass --game-dir or set GAME_DIR (the MO2 instance's \
             ModOrganizer.ini names none this can read)"
        ),
    }
}

/// The downloads folder: `--downloads`, else `downloads_dir` in tools.toml,
/// else an MO2 instance's own `downloads/`.
pub fn resolve_downloads_dir(
    explicit: Option<PathBuf>,
    mo2_instance: Option<&std::path::Path>,
) -> anyhow::Result<PathBuf> {
    if let Some(dir) = explicit {
        return Ok(dir);
    }
    let tools_path = crate::config::tools::ToolsConfig::default_path();
    if let Some(path) = tools_path
        && let Some(dir) = crate::config::tools::ToolsConfig::load(&path)?.downloads_dir
    {
        return Ok(dir);
    }
    if let Some(instance) = mo2_instance {
        return Ok(instance.join("downloads"));
    }
    anyhow::bail!("no downloads folder: pass --downloads <DIR>, or set downloads_dir in tools.toml")
}

pub async fn execute(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Add(args) => add::run(args).await,
        Command::Move(args) => move_mod::run(args),
        Command::Set(args) => edit::set(args),
        Command::Remove(args) => edit::remove(args),
        Command::Plugins(command) => edit::plugins(command),
        Command::Files(args) => edit::files(args),
        Command::Action(command) => edit::action(command),
        Command::Input(command) => edit::input(command),
        Command::Compile(args) => compile::run(args).await,
        Command::Conflicts(args) => conflicts::run(args).await,
        Command::Query(args) => query::run(args).await,
        Command::Download(args) => download::run(args).await,
        Command::Check(args) => check::run(args).await,
        Command::Inspect(args) => inspect::run(args).await,
        Command::Identify(args) => identify::run(args).await,
        Command::NexusFiles(args) => nexus_files::run(args).await,
        Command::Install(args) => install::run(args).await,
        Command::Diff(args) => diff::run(args).await,
        Command::Validate(args) => validate::run(args).await,
        Command::Export(args) => export::run(args).await,
        Command::SetupTools(args) => setup_tools::run(args).await,
        Command::GameDir(args) => game_dir::run(args),
        Command::Assets(args) => assets::run(args),
        Command::UnhideMerges(args) => unhide_merges::run(args).await,
        Command::Merge(args) => merge::run(args).await,
        Command::NewMerge(args) => new_merge::run(args).await,
        Command::Bsa(command) => bsa::run(command),
        Command::Dds(command) => dds::run(command),
        Command::Downloads(command) => downloads::run(command).await,
    }
}
